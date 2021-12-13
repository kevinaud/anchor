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

use super::shared::{cluster_url, with_workspace};
pub fn upgrade(
    cfg_override: &ConfigOverride,
    program_id: Pubkey,
    program_filepath: String,
) -> Result<()> {
    let path: PathBuf = program_filepath.parse().unwrap();
    let program_filepath = path.canonicalize()?.display().to_string();

    with_workspace(cfg_override, |cfg| {
        let url = cluster_url(cfg);
        let exit = std::process::Command::new("solana")
            .arg("program")
            .arg("deploy")
            .arg("--url")
            .arg(url)
            .arg("--keypair")
            .arg(&cfg.provider.wallet.to_string())
            .arg("--program-id")
            .arg(program_id.to_string())
            .arg(&program_filepath)
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .output()
            .expect("Must deploy");
        if !exit.status.success() {
            println!("There was a problem deploying: {:?}.", exit);
            std::process::exit(exit.status.code().unwrap_or(1));
        }
        Ok(())
    })
}
