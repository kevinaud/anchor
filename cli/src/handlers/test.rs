use crate::config::{
    AnchorPackage, BootstrapMode, BuildConfig, Config, ConfigOverride, Manifest, ProgramDeployment,
    ProgramWorkspace, Test, WithPath,
};
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

use super::localnet::{start_test_validator, stream_logs};
use super::shared::{cluster_url, validator_flags, with_workspace};
use super::{build, deploy};
// Builds, deploys, and tests all workspace programs in a single command.
pub fn test(
    cfg_override: &ConfigOverride,
    skip_deploy: bool,
    skip_local_validator: bool,
    skip_build: bool,
    detach: bool,
    extra_args: Vec<String>,
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

        // Run the deploy against the cluster in two cases:
        //
        // 1. The cluster is not localnet.
        // 2. The cluster is localnet, but we're not booting a local validator.
        //
        // In either case, skip the deploy if the user specifies.
        let is_localnet = cfg.provider.cluster == Cluster::Localnet;
        if (!is_localnet || skip_local_validator) && !skip_deploy {
            deploy(cfg_override, None)?;
        }
        // Start local test validator, if needed.
        let mut validator_handle = None;
        if is_localnet && (!skip_local_validator) {
            let flags = match skip_deploy {
                true => None,
                false => Some(validator_flags(cfg)?),
            };
            validator_handle = Some(start_test_validator(cfg, flags, true)?);
        }

        let url = cluster_url(cfg);

        let node_options = format!(
            "{} {}",
            match std::env::var_os("NODE_OPTIONS") {
                Some(value) => value
                    .into_string()
                    .map_err(std::env::VarError::NotUnicode)?,
                None => "".to_owned(),
            },
            get_node_dns_option()?,
        );

        // Setup log reader.
        let log_streams = stream_logs(cfg, &url);

        // Run the tests.
        let test_result: Result<_> = {
            let cmd = cfg
                .scripts
                .get("test")
                .expect("Not able to find command for `test`")
                .clone();
            let mut args: Vec<&str> = cmd
                .split(' ')
                .chain(extra_args.iter().map(|arg| arg.as_str()))
                .collect();
            let program = args.remove(0);

            std::process::Command::new(program)
                .args(args)
                .env("ANCHOR_PROVIDER_URL", url)
                .env("ANCHOR_WALLET", cfg.provider.wallet.to_string())
                .env("NODE_OPTIONS", node_options)
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .output()
                .map_err(anyhow::Error::from)
                .context(cmd)
        };

        // Keep validator running if needed.
        if test_result.is_ok() && detach {
            println!("Local validator still running. Press Ctrl + C quit.");
            std::io::stdin().lock().lines().next().unwrap().unwrap();
        }

        // Check all errors and shut down.
        if let Some(mut child) = validator_handle {
            if let Err(err) = child.kill() {
                println!("Failed to kill subprocess {}: {}", child.id(), err);
            }
        }
        for mut child in log_streams? {
            if let Err(err) = child.kill() {
                println!("Failed to kill subprocess {}: {}", child.id(), err);
            }
        }

        // Must exist *after* shutting down the validator and log streams.
        match test_result {
            Ok(exit) => {
                if !exit.status.success() {
                    std::process::exit(exit.status.code().unwrap());
                }
            }
            Err(err) => {
                println!("Failed to run test: {:#}", err)
            }
        }

        Ok(())
    })
}

fn get_node_dns_option() -> Result<&'static str> {
    let version = get_node_version()?;
    let req = VersionReq::parse(">=16.4.0").unwrap();
    let option = match req.matches(&version) {
        true => "--dns-result-order=ipv4first",
        false => "",
    };
    Ok(option)
}

fn get_node_version() -> Result<Version> {
    let node_version = std::process::Command::new("node")
        .arg("--version")
        .stderr(Stdio::inherit())
        .output()
        .map_err(|e| anyhow::format_err!("node failed: {}", e.to_string()))?;
    let output = std::str::from_utf8(&node_version.stdout)?
        .strip_prefix('v')
        .unwrap()
        .trim();
    Version::parse(output).map_err(Into::into)
}
