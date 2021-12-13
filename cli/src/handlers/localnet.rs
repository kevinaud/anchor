use super::build::build;
use super::idl::IdlTestMetadata;
use super::shared::{test_validator_rpc_url, validator_flags, with_workspace};
use crate::config::{BootstrapMode, Config, ConfigOverride, Test, WithPath};
use anchor_syn::idl::Idl;
use anyhow::{anyhow, Result};
use solana_client::rpc_client::RpcClient;
use solana_sdk::signature::Signer;
use std::fs::{self, File};
use std::io::prelude::*;
use std::path::Path;
use std::process::{Child, Stdio};
use std::string::ToString;

pub fn localnet(
    cfg_override: &ConfigOverride,
    skip_build: bool,
    skip_deploy: bool,
    cargo_args: Vec<String>,
) -> Result<()> {
    with_workspace(cfg_override, |cfg| {
        // Build if needed.
        if !skip_build {
            build(
                cfg_override,
                None,
                None,
                false,
                None,
                None,
                None,
                BootstrapMode::None,
                None,
                None,
                cargo_args,
            )?;
        }

        let flags = match skip_deploy {
            true => None,
            false => Some(validator_flags(cfg)?),
        };

        let validator_handle = &mut start_test_validator(cfg, flags, false)?;

        // Setup log reader.
        let url = test_validator_rpc_url(cfg);
        let log_streams = stream_logs(cfg, &url);

        std::io::stdin().lock().lines().next().unwrap().unwrap();

        // Check all errors and shut down.
        if let Err(err) = validator_handle.kill() {
            println!(
                "Failed to kill subprocess {}: {}",
                validator_handle.id(),
                err
            );
        }

        for mut child in log_streams? {
            if let Err(err) = child.kill() {
                println!("Failed to kill subprocess {}: {}", child.id(), err);
            }
        }

        Ok(())
    })
}

pub fn start_test_validator(
    cfg: &Config,
    flags: Option<Vec<String>>,
    test_log_stdout: bool,
) -> Result<Child> {
    //
    let (test_ledger_directory, test_ledger_log_filename) = test_validator_file_paths(cfg);

    // Start a validator for testing.
    let (test_validator_stdout, test_validator_stderr) = match test_log_stdout {
        true => {
            let test_validator_stdout_file = File::create(&test_ledger_log_filename)?;
            let test_validator_sterr_file = test_validator_stdout_file.try_clone()?;
            (
                Stdio::from(test_validator_stdout_file),
                Stdio::from(test_validator_sterr_file),
            )
        }
        false => (Stdio::inherit(), Stdio::inherit()),
    };

    let rpc_url = test_validator_rpc_url(cfg);

    let mut validator_handle = std::process::Command::new("solana-test-validator")
        .arg("--ledger")
        .arg(test_ledger_directory)
        .arg("--mint")
        .arg(cfg.wallet_kp()?.pubkey().to_string())
        .args(flags.unwrap_or_default())
        .stdout(test_validator_stdout)
        .stderr(test_validator_stderr)
        .spawn()
        .map_err(|e| anyhow::format_err!("{}", e.to_string()))?;

    // Wait for the validator to be ready.
    let client = RpcClient::new(rpc_url);
    let mut count = 0;
    let ms_wait = cfg
        .test
        .as_ref()
        .and_then(|test| test.startup_wait)
        .unwrap_or(5_000);
    while count < ms_wait {
        let r = client.get_recent_blockhash();
        if r.is_ok() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
        count += 1;
    }
    if count == ms_wait {
        eprintln!(
            "Unable to start test validator. Check {} for errors.",
            test_ledger_log_filename
        );
        validator_handle.kill()?;
        std::process::exit(1);
    }
    Ok(validator_handle)
}

// Setup and return paths to the solana-test-validator ledger directory and log
// files given the configuration
fn test_validator_file_paths(cfg: &Config) -> (String, String) {
    let ledger_directory = match &cfg.test.as_ref() {
        Some(Test {
            validator: Some(validator),
            ..
        }) => &validator.ledger,
        _ => ".anchor/test-ledger",
    };

    if !Path::new(&ledger_directory).is_relative() {
        // Prevent absolute paths to avoid someone using / or similar, as the
        // directory gets removed
        eprintln!("Ledger directory {} must be relative", ledger_directory);
        std::process::exit(1);
    }
    if Path::new(&ledger_directory).exists() {
        fs::remove_dir_all(&ledger_directory).unwrap();
    }

    fs::create_dir_all(&ledger_directory).unwrap();

    (
        ledger_directory.to_string(),
        format!("{}/test-ledger-log.txt", ledger_directory),
    )
}

pub fn stream_logs(config: &WithPath<Config>, rpc_url: &str) -> Result<Vec<std::process::Child>> {
    let program_logs_dir = ".anchor/program-logs";
    if Path::new(program_logs_dir).exists() {
        fs::remove_dir_all(program_logs_dir)?;
    }
    fs::create_dir_all(program_logs_dir)?;
    let mut handles = vec![];
    for program in config.read_all_programs()? {
        let mut file = File::open(&format!("target/idl/{}.json", program.lib_name))?;
        let mut contents = vec![];
        file.read_to_end(&mut contents)?;
        let idl: Idl = serde_json::from_slice(&contents)?;
        let metadata = idl
            .metadata
            .ok_or_else(|| anyhow!("Program address not found."))?;
        let metadata: IdlTestMetadata = serde_json::from_value(metadata)?;

        let log_file = File::create(format!(
            "{}/{}.{}.log",
            program_logs_dir, metadata.address, program.lib_name,
        ))?;
        let stdio = std::process::Stdio::from(log_file);
        let child = std::process::Command::new("solana")
            .arg("logs")
            .arg(metadata.address)
            .arg("--url")
            .arg(rpc_url)
            .stdout(stdio)
            .spawn()?;
        handles.push(child);
    }
    if let Some(test) = config.test.as_ref() {
        if let Some(genesis) = &test.genesis {
            for entry in genesis {
                let log_file = File::create(format!("{}/{}.log", program_logs_dir, entry.address))?;
                let stdio = std::process::Stdio::from(log_file);
                let child = std::process::Command::new("solana")
                    .arg("logs")
                    .arg(entry.address.clone())
                    .arg("--url")
                    .arg(rpc_url)
                    .stdout(stdio)
                    .spawn()?;
                handles.push(child);
            }
        }
    }

    Ok(handles)
}
