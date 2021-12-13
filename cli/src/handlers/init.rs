use crate::config::{Config, ConfigOverride, ProgramDeployment};
use crate::handlers::new::new_program;
use crate::handlers::template;
use anchor_client::Cluster;
use anyhow::{anyhow, Result};
use heck::SnakeCase;
use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::prelude::*;
use std::process::Stdio;
use std::string::ToString;

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
