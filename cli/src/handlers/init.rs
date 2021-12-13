use crate::config::{
    AnchorPackage, BootstrapMode, BuildConfig, Config, ConfigOverride, Manifest, ProgramDeployment,
    ProgramWorkspace, Test, WithPath,
};
use crate::handlers::new::new_program;
use crate::handlers::template;
use anchor_client::Cluster;
use anchor_lang::idl::{IdlAccount, IdlInstruction};
use anchor_lang::{AccountDeserialize, AnchorDeserialize, AnchorSerialize};
use anchor_syn::idl::Idl;
use anyhow::{anyhow, Context, Result};
use clap::Clap;
use flate2::read::GzDecoder;
use flate2::read::ZlibDecoder;
use flate2::write::{GzEncoder, ZlibEncoder};
use flate2::Compression;
use heck::SnakeCase;
use rand::rngs::OsRng;
use reqwest::blocking::multipart::{Form, Part};
use reqwest::blocking::Client;
use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};
use solana_client::rpc_client::RpcClient;
use solana_client::rpc_config::RpcSendTransactionConfig;
use solana_program::instruction::{AccountMeta, Instruction};
use solana_sdk::account_utils::StateMut;
use solana_sdk::bpf_loader;
use solana_sdk::bpf_loader_deprecated;
use solana_sdk::bpf_loader_upgradeable::{self, UpgradeableLoaderState};
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Keypair;
use solana_sdk::signature::Signer;
use solana_sdk::sysvar;
use solana_sdk::transaction::Transaction;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::prelude::*;
use std::path::{Path, PathBuf};
use std::process::{Child, Stdio};
use std::string::ToString;
use tar::Archive;
pub fn init(cfg_override: &ConfigOverride, name: String, javascript: bool) -> Result<()> {
    if Config::discover(cfg_override)?.is_some() {
        return Err(anyhow!("Workspace already initialized"));
    }

    // The list is taken from https://doc.rust-lang.org/reference/keywords.html.
    let key_words = [
        "as", "break", "const", "continue", "crate", "else", "enum", "extern", "false", "fn",
        "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref",
        "return", "self", "Self", "static", "struct", "super", "trait", "true", "type", "unsafe",
        "use", "where", "while", "async", "await", "dyn", "abstract", "become", "box", "do",
        "final", "macro", "override", "priv", "typeof", "unsized", "virtual", "yield", "try",
        "unique",
    ];

    if key_words.contains(&name[..].into()) {
        return Err(anyhow!(
            "{} is a reserved word in rust, name your project something else!",
            name
        ));
    } else if name.chars().next().unwrap().is_numeric() {
        return Err(anyhow!(
            "Cannot start project name with numbers, name your project something else!"
        ));
    }

    fs::create_dir(name.clone())?;
    std::env::set_current_dir(&name)?;
    fs::create_dir("app")?;

    let mut cfg = Config::default();
    cfg.scripts.insert(
        "test".to_owned(),
        if javascript {
            "yarn run mocha -t 1000000 tests/"
        } else {
            "yarn run ts-mocha -p ./tsconfig.json -t 1000000 tests/**/*.ts"
        }
        .to_owned(),
    );
    let mut localnet = BTreeMap::new();
    localnet.insert(
        name.to_snake_case(),
        ProgramDeployment {
            address: template::default_program_id(),
            path: None,
            idl: None,
        },
    );
    cfg.programs.insert(Cluster::Localnet, localnet);
    let toml = cfg.to_string();
    let mut file = File::create("Anchor.toml")?;
    file.write_all(toml.as_bytes())?;

    // Build virtual manifest.
    let mut virt_manifest = File::create("Cargo.toml")?;
    virt_manifest.write_all(template::virtual_manifest().as_bytes())?;

    // Initialize .gitignore file
    let mut virt_manifest = File::create(".gitignore")?;
    virt_manifest.write_all(template::git_ignore().as_bytes())?;

    // Build the program.
    fs::create_dir("programs")?;

    new_program(&name)?;

    // Build the test suite.
    fs::create_dir("tests")?;
    // Build the migrations directory.
    fs::create_dir("migrations")?;

    if javascript {
        // Build javascript config
        let mut package_json = File::create("package.json")?;
        package_json.write_all(template::package_json().as_bytes())?;

        let mut mocha = File::create(&format!("tests/{}.js", name))?;
        mocha.write_all(template::mocha(&name).as_bytes())?;

        let mut deploy = File::create("migrations/deploy.js")?;
        deploy.write_all(template::deploy_script().as_bytes())?;
    } else {
        // Build typescript config
        let mut ts_config = File::create("tsconfig.json")?;
        ts_config.write_all(template::ts_config().as_bytes())?;

        let mut ts_package_json = File::create("package.json")?;
        ts_package_json.write_all(template::ts_package_json().as_bytes())?;

        let mut deploy = File::create("migrations/deploy.ts")?;
        deploy.write_all(template::ts_deploy_script().as_bytes())?;

        let mut mocha = File::create(&format!("tests/{}.ts", name))?;
        mocha.write_all(template::ts_mocha(&name).as_bytes())?;
    }

    // Install node modules.
    let yarn_result = std::process::Command::new("yarn")
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .output()
        .map_err(|e| anyhow::format_err!("yarn install failed: {}", e.to_string()))?;
    if !yarn_result.status.success() {
        println!("Failed yarn install will attempt to npm install");
        std::process::Command::new("npm")
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .output()
            .map_err(|e| anyhow::format_err!("npm install failed: {}", e.to_string()))?;
        println!("Failed to install node dependencies")
    }

    println!("{} initialized", name);

    Ok(())
}
