use super::idl::{write_idl, IdlTestMetadata};
use crate::config::{Config, ConfigOverride, Manifest, Test, WithPath};
use anchor_client::Cluster;
use anyhow::{anyhow, Result};
use solana_sdk::signature::Signer;
use std::path::{Path, PathBuf};
use std::string::ToString;

pub enum OutFile {
    Stdout,
    File(PathBuf),
}

pub fn cluster_url(cfg: &Config) -> String {
    let is_localnet = cfg.provider.cluster == Cluster::Localnet;
    match is_localnet {
        // Cluster is Localnet, assume the intent is to use the configuration
        // for solana-test-validator
        true => test_validator_rpc_url(cfg),
        false => cfg.provider.cluster.url().to_string(),
    }
}

// Return the URL that solana-test-validator should be running on given the
// configuration
pub fn test_validator_rpc_url(cfg: &Config) -> String {
    match &cfg.test.as_ref() {
        Some(Test {
            validator: Some(validator),
            ..
        }) => format!("http://{}:{}", validator.bind_address, validator.rpc_port),
        _ => "http://localhost:8899".to_string(),
    }
}

pub fn cd_member(cfg_override: &ConfigOverride, program_name: &str) -> Result<()> {
    // Change directories to the given `program_name`, if given.
    let cfg = Config::discover(cfg_override)?.expect("Not in workspace.");

    for program in cfg.read_all_programs()? {
        let cargo_toml = program.path.join("Cargo.toml");
        if !cargo_toml.exists() {
            return Err(anyhow!(
                "Did not find Cargo.toml at the path: {}",
                program.path.display()
            ));
        }
        let p_lib_name = Manifest::from_path(&cargo_toml)?.lib_name()?;
        if program_name == p_lib_name {
            std::env::set_current_dir(&program.path)?;
            return Ok(());
        }
    }
    return Err(anyhow!("{} is not part of the workspace", program_name,));
}

// with_workspace ensures the current working directory is always the top level
// workspace directory, i.e., where the `Anchor.toml` file is located, before
// and after the closure invocation.
//
// The closure passed into this function must never change the working directory
// to be outside the workspace. Doing so will have undefined behavior.
pub fn with_workspace<R>(
    cfg_override: &ConfigOverride,
    f: impl FnOnce(&WithPath<Config>) -> R,
) -> R {
    set_workspace_dir_or_exit();

    let cfg = Config::discover(cfg_override)
        .expect("Previously set the workspace dir")
        .expect("Anchor.toml must always exist");

    let r = f(&cfg);

    set_workspace_dir_or_exit();

    r
}

pub fn set_workspace_dir_or_exit() {
    let d = match Config::discover(&ConfigOverride::default()) {
        Err(err) => {
            println!("Workspace configuration error: {}", err);
            std::process::exit(1);
        }
        Ok(d) => d,
    };
    match d {
        None => {
            println!("Not in anchor workspace.");
            std::process::exit(1);
        }
        Some(cfg) => {
            match cfg.path().parent() {
                None => {
                    println!("Unable to make new program");
                }
                Some(parent) => {
                    if std::env::set_current_dir(&parent).is_err() {
                        println!("Not in anchor workspace.");
                        std::process::exit(1);
                    }
                }
            };
        }
    }
}

// Returns the solana-test-validator flags. This will embed the workspace
// programs in the genesis block so we don't have to deploy every time. It also
// allows control of other solana-test-validator features.
pub fn validator_flags(cfg: &WithPath<Config>) -> Result<Vec<String>> {
    let programs = cfg.programs.get(&Cluster::Localnet);

    let mut flags = Vec::new();
    for mut program in cfg.read_all_programs()? {
        let binary_path = program.binary_path().display().to_string();

        // Use the [programs.cluster] override and fallback to the keypair
        // files if no override is given.
        let address = programs
            .and_then(|m| m.get(&program.lib_name))
            .map(|deployment| Ok(deployment.address.to_string()))
            .unwrap_or_else(|| program.pubkey().map(|p| p.to_string()))?;

        flags.push("--bpf-program".to_string());
        flags.push(address.clone());
        flags.push(binary_path);

        if let Some(mut idl) = program.idl.as_mut() {
            // Add program address to the IDL.
            idl.metadata = Some(serde_json::to_value(IdlTestMetadata { address })?);

            // Persist it.
            let idl_out = PathBuf::from("target/idl")
                .join(&idl.name)
                .with_extension("json");
            write_idl(idl, OutFile::File(idl_out))?;
        }
    }

    if let Some(test) = cfg.test.as_ref() {
        if let Some(genesis) = &test.genesis {
            for entry in genesis {
                let program_path = Path::new(&entry.program);
                if !program_path.exists() {
                    return Err(anyhow!(
                        "Program in genesis configuration does not exist at path: {}",
                        program_path.display()
                    ));
                }
                flags.push("--bpf-program".to_string());
                flags.push(entry.address.clone());
                flags.push(entry.program.clone());
            }
        }
        if let Some(clone) = &test.clone {
            for entry in clone {
                flags.push("--clone".to_string());
                flags.push(entry.address.clone());
            }
        }
        if let Some(validator) = &test.validator {
            for (key, value) in serde_json::to_value(validator)?.as_object().unwrap() {
                if key == "ledger" {
                    continue;
                };
                flags.push(format!("--{}", key.replace("_", "-")));
                if let serde_json::Value::String(v) = value {
                    flags.push(v.to_string());
                } else {
                    flags.push(value.to_string());
                }
            }
        }
    }

    Ok(flags)
}
