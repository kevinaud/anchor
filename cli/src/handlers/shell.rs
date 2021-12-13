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
use super::template;
pub fn shell(cfg_override: &ConfigOverride) -> Result<()> {
    with_workspace(cfg_override, |cfg| {
        let programs = {
            // Create idl map from all workspace programs.
            let mut idls: HashMap<String, Idl> = cfg
                .read_all_programs()?
                .iter()
                .filter(|program| program.idl.is_some())
                .map(|program| {
                    (
                        program.idl.as_ref().unwrap().name.clone(),
                        program.idl.clone().unwrap(),
                    )
                })
                .collect();
            // Insert all manually specified idls into the idl map.
            if let Some(programs) = cfg.programs.get(&cfg.provider.cluster) {
                let _ = programs
                    .iter()
                    .map(|(name, pd)| {
                        if let Some(idl_fp) = &pd.idl {
                            let file_str =
                                fs::read_to_string(idl_fp).expect("Unable to read IDL file");
                            let idl = serde_json::from_str(&file_str).expect("Idl not readable");
                            idls.insert(name.clone(), idl);
                        }
                    })
                    .collect::<Vec<_>>();
            }

            // Finalize program list with all programs with IDLs.
            match cfg.programs.get(&cfg.provider.cluster) {
                None => Vec::new(),
                Some(programs) => programs
                    .iter()
                    .filter_map(|(name, program_deployment)| {
                        Some(ProgramWorkspace {
                            name: name.to_string(),
                            program_id: program_deployment.address,
                            idl: match idls.get(name) {
                                None => return None,
                                Some(idl) => idl.clone(),
                            },
                        })
                    })
                    .collect::<Vec<ProgramWorkspace>>(),
            }
        };
        let url = cluster_url(cfg);
        let js_code = template::node_shell(&url, &cfg.provider.wallet.to_string(), programs)?;
        let mut child = std::process::Command::new("node")
            .args(&["-e", &js_code, "-i", "--experimental-repl-await"])
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|e| anyhow::format_err!("{}", e.to_string()))?;

        if !child.wait()?.success() {
            println!("Error running node shell");
            return Ok(());
        }
        Ok(())
    })
}
