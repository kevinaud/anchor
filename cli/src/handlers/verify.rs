use crate::config::{
    AnchorPackage, BootstrapMode, BuildConfig, Config, ConfigOverride, Manifest, ProgramDeployment,
    ProgramWorkspace, Test, WithPath,
};
use crate::handlers::build;
use crate::handlers::idl::{extract_idl, fetch_idl};
use crate::handlers::shared::{cd_member, cluster_url};
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

#[derive(PartialEq)]
pub struct BinVerification {
    pub state: BinVerificationState,
    pub is_verified: bool,
}

#[derive(PartialEq)]
pub enum BinVerificationState {
    Buffer,
    ProgramData {
        slot: u64,
        upgrade_authority_address: Option<Pubkey>,
    },
}

pub fn verify(
    cfg_override: &ConfigOverride,
    program_id: Pubkey,
    program_name: Option<String>,
    solana_version: Option<String>,
    docker_image: Option<String>,
    bootstrap: BootstrapMode,
    cargo_args: Vec<String>,
) -> Result<()> {
    // Change to the workspace member directory, if needed.
    if let Some(program_name) = program_name.as_ref() {
        cd_member(cfg_override, program_name)?;
    }

    // Proceed with the command.
    let cfg = Config::discover(cfg_override)?.expect("Not in workspace.");
    let cargo = Manifest::discover()?.ok_or_else(|| anyhow!("Cargo.toml not found"))?;

    // Build the program we want to verify.
    let cur_dir = std::env::current_dir()?;
    build(
        cfg_override,
        None,                                                  // idl
        None,                                                  // idl ts
        true,                                                  // verifiable
        None,                                                  // program name
        solana_version.or_else(|| cfg.solana_version.clone()), // solana version
        docker_image,                                          // docker image
        bootstrap,                                             // bootstrap docker image
        None,                                                  // stdout
        None,                                                  // stderr
        cargo_args,
    )?;
    std::env::set_current_dir(&cur_dir)?;

    // Verify binary.
    let binary_name = cargo.lib_name()?;
    let bin_path = cfg
        .path()
        .parent()
        .ok_or_else(|| anyhow!("Unable to find workspace root"))?
        .join("target/verifiable/")
        .join(format!("{}.so", binary_name));

    let url = cluster_url(&cfg);
    let bin_ver = verify_bin(program_id, &bin_path, &url)?;
    if !bin_ver.is_verified {
        println!("Error: Binaries don't match");
        std::process::exit(1);
    }

    // Verify IDL (only if it's not a buffer account).
    if let Some(local_idl) = extract_idl("src/lib.rs")? {
        if bin_ver.state != BinVerificationState::Buffer {
            let deployed_idl = fetch_idl(cfg_override, program_id)?;
            if local_idl != deployed_idl {
                println!("Error: IDLs don't match");
                std::process::exit(1);
            }
        }
    }

    println!("{} is verified.", program_id);

    Ok(())
}

pub fn verify_bin(program_id: Pubkey, bin_path: &Path, cluster: &str) -> Result<BinVerification> {
    let client = RpcClient::new(cluster.to_string());

    // Get the deployed build artifacts.
    let (deployed_bin, state) = {
        let account = client
            .get_account_with_commitment(&program_id, CommitmentConfig::default())?
            .value
            .map_or(Err(anyhow!("Account not found")), Ok)?;
        if account.owner == bpf_loader::id() || account.owner == bpf_loader_deprecated::id() {
            let bin = account.data.to_vec();
            let state = BinVerificationState::ProgramData {
                slot: 0, // Need to look through the transaction history.
                upgrade_authority_address: None,
            };
            (bin, state)
        } else if account.owner == bpf_loader_upgradeable::id() {
            match account.state()? {
                UpgradeableLoaderState::Program {
                    programdata_address,
                } => {
                    let account = client
                        .get_account_with_commitment(
                            &programdata_address,
                            CommitmentConfig::default(),
                        )?
                        .value
                        .map_or(Err(anyhow!("Account not found")), Ok)?;
                    let bin = account.data
                        [UpgradeableLoaderState::programdata_data_offset().unwrap_or(0)..]
                        .to_vec();

                    if let UpgradeableLoaderState::ProgramData {
                        slot,
                        upgrade_authority_address,
                    } = account.state()?
                    {
                        let state = BinVerificationState::ProgramData {
                            slot,
                            upgrade_authority_address,
                        };
                        (bin, state)
                    } else {
                        return Err(anyhow!("Expected program data"));
                    }
                }
                UpgradeableLoaderState::Buffer { .. } => {
                    let offset = UpgradeableLoaderState::buffer_data_offset().unwrap_or(0);
                    (
                        account.data[offset..].to_vec(),
                        BinVerificationState::Buffer,
                    )
                }
                _ => {
                    return Err(anyhow!(
                        "Invalid program id, not a buffer or program account"
                    ))
                }
            }
        } else {
            return Err(anyhow!(
                "Invalid program id, not owned by any loader program"
            ));
        }
    };
    let mut local_bin = {
        let mut f = File::open(bin_path)?;
        let mut contents = vec![];
        f.read_to_end(&mut contents)?;
        contents
    };

    // The deployed program probably has zero bytes appended. The default is
    // 2x the binary size in case of an upgrade.
    if local_bin.len() < deployed_bin.len() {
        local_bin.append(&mut vec![0; deployed_bin.len() - local_bin.len()]);
    }

    // Finally, check the bytes.
    let is_verified = local_bin == deployed_bin;

    Ok(BinVerification { state, is_verified })
}
