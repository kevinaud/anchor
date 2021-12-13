use crate::config::{
    AnchorPackage, BootstrapMode, BuildConfig, Config, ConfigOverride, Manifest, ProgramDeployment,
    ProgramWorkspace, Test, WithPath,
};
use crate::handlers::shared::cluster_url;
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

use super::shared::with_workspace;

pub fn migrate(cfg_override: &ConfigOverride) -> Result<()> {
    with_workspace(cfg_override, |cfg| {
        println!("Running migration deploy script");

        let url = cluster_url(cfg);
        let cur_dir = std::env::current_dir()?;

        let use_ts =
            Path::new("tsconfig.json").exists() && Path::new("migrations/deploy.ts").exists();

        if !Path::new(".anchor").exists() {
            fs::create_dir(".anchor")?;
        }
        std::env::set_current_dir(".anchor")?;

        let exit = if use_ts {
            let module_path = cur_dir.join("migrations/deploy.ts");
            let deploy_script_host_str =
                template::deploy_ts_script_host(&url, &module_path.display().to_string());
            fs::write("deploy.ts", deploy_script_host_str)?;
            std::process::Command::new("ts-node")
                .arg("deploy.ts")
                .env("ANCHOR_WALLET", cfg.provider.wallet.to_string())
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .output()?
        } else {
            let module_path = cur_dir.join("migrations/deploy.js");
            let deploy_script_host_str =
                template::deploy_js_script_host(&url, &module_path.display().to_string());
            fs::write("deploy.js", deploy_script_host_str)?;
            std::process::Command::new("node")
                .arg("deploy.js")
                .env("ANCHOR_WALLET", cfg.provider.wallet.to_string())
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .output()?
        };

        if !exit.status.success() {
            println!("Deploy failed.");
            std::process::exit(exit.status.code().unwrap());
        }

        println!("Deploy complete.");
        Ok(())
    })
}
