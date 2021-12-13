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

use super::shared::{cluster_url, with_workspace, OutFile};
use super::template;

#[derive(Debug, Clap)]
pub enum IdlCommand {
    /// Initializes a program's IDL account. Can only be run once.
    Init {
        program_id: Pubkey,
        #[clap(short, long)]
        filepath: String,
    },
    /// Writes an IDL into a buffer account. This can be used with SetBuffer
    /// to perform an upgrade.
    WriteBuffer {
        program_id: Pubkey,
        #[clap(short, long)]
        filepath: String,
    },
    /// Sets a new IDL buffer for the program.
    SetBuffer {
        program_id: Pubkey,
        /// Address of the buffer account to set as the idl on the program.
        #[clap(short, long)]
        buffer: Pubkey,
    },
    /// Upgrades the IDL to the new file. An alias for first writing and then
    /// then setting the idl buffer account.
    Upgrade {
        program_id: Pubkey,
        #[clap(short, long)]
        filepath: String,
    },
    /// Sets a new authority on the IDL account.
    SetAuthority {
        /// The IDL account buffer to set the authority of. If none is given,
        /// then the canonical IDL account is used.
        address: Option<Pubkey>,
        /// Program to change the IDL authority.
        #[clap(short, long)]
        program_id: Pubkey,
        /// New authority of the IDL account.
        #[clap(short, long)]
        new_authority: Pubkey,
    },
    /// Command to remove the ability to modify the IDL account. This should
    /// likely be used in conjection with eliminating an "upgrade authority" on
    /// the program.
    EraseAuthority {
        #[clap(short, long)]
        program_id: Pubkey,
    },
    /// Outputs the authority for the IDL account.
    Authority {
        /// The program to view.
        program_id: Pubkey,
    },
    /// Parses an IDL from source.
    Parse {
        /// Path to the program's interface definition.
        #[clap(short, long)]
        file: String,
        /// Output file for the IDL (stdout if not specified).
        #[clap(short, long)]
        out: Option<String>,
        /// Output file for the TypeScript IDL.
        #[clap(short = 't', long)]
        out_ts: Option<String>,
    },
    /// Fetches an IDL for the given address from a cluster.
    /// The address can be a program, IDL account, or IDL buffer.
    Fetch {
        address: Pubkey,
        /// Output file for the idl (stdout if not specified).
        #[clap(short, long)]
        out: Option<String>,
    },
}

#[derive(Debug, Serialize, Deserialize)]
pub struct IdlTestMetadata {
    pub address: String,
}

pub fn idl(cfg_override: &ConfigOverride, subcmd: IdlCommand) -> Result<()> {
    match subcmd {
        IdlCommand::Init {
            program_id,
            filepath,
        } => idl_init(cfg_override, program_id, filepath),
        IdlCommand::WriteBuffer {
            program_id,
            filepath,
        } => idl_write_buffer(cfg_override, program_id, filepath).map(|_| ()),
        IdlCommand::SetBuffer { program_id, buffer } => {
            idl_set_buffer(cfg_override, program_id, buffer)
        }
        IdlCommand::Upgrade {
            program_id,
            filepath,
        } => idl_upgrade(cfg_override, program_id, filepath),
        IdlCommand::SetAuthority {
            program_id,
            address,
            new_authority,
        } => idl_set_authority(cfg_override, program_id, address, new_authority),
        IdlCommand::EraseAuthority { program_id } => idl_erase_authority(cfg_override, program_id),
        IdlCommand::Authority { program_id } => idl_authority(cfg_override, program_id),
        IdlCommand::Parse { file, out, out_ts } => idl_parse(file, out, out_ts),
        IdlCommand::Fetch { address, out } => idl_fetch(cfg_override, address, out),
    }
}

pub fn idl_init(
    cfg_override: &ConfigOverride,
    program_id: Pubkey,
    idl_filepath: String,
) -> Result<()> {
    with_workspace(cfg_override, |cfg| {
        let keypair = cfg.provider.wallet.to_string();

        let bytes = fs::read(idl_filepath)?;
        let idl: Idl = serde_json::from_reader(&*bytes)?;

        let idl_address = create_idl_account(cfg, &keypair, &program_id, &idl)?;

        println!("Idl account created: {:?}", idl_address);
        Ok(())
    })
}

pub fn idl_write_buffer(
    cfg_override: &ConfigOverride,
    program_id: Pubkey,
    idl_filepath: String,
) -> Result<Pubkey> {
    with_workspace(cfg_override, |cfg| {
        let keypair = cfg.provider.wallet.to_string();

        let bytes = fs::read(idl_filepath)?;
        let idl: Idl = serde_json::from_reader(&*bytes)?;

        let idl_buffer = create_idl_buffer(cfg, &keypair, &program_id, &idl)?;
        idl_write(cfg, &program_id, &idl, idl_buffer)?;

        println!("Idl buffer created: {:?}", idl_buffer);

        Ok(idl_buffer)
    })
}

pub fn idl_set_buffer(
    cfg_override: &ConfigOverride,
    program_id: Pubkey,
    buffer: Pubkey,
) -> Result<()> {
    with_workspace(cfg_override, |cfg| {
        let keypair = solana_sdk::signature::read_keypair_file(&cfg.provider.wallet.to_string())
            .map_err(|_| anyhow!("Unable to read keypair file"))?;
        let url = cluster_url(cfg);
        let client = RpcClient::new(url);

        // Instruction to set the buffer onto the IdlAccount.
        let set_buffer_ix = {
            let accounts = vec![
                AccountMeta::new(buffer, false),
                AccountMeta::new(IdlAccount::address(&program_id), false),
                AccountMeta::new(keypair.pubkey(), true),
            ];
            let mut data = anchor_lang::idl::IDL_IX_TAG.to_le_bytes().to_vec();
            data.append(&mut IdlInstruction::SetBuffer.try_to_vec()?);
            Instruction {
                program_id,
                accounts,
                data,
            }
        };

        // Build the transaction.
        let (recent_hash, _fee_calc) = client.get_recent_blockhash()?;
        let tx = Transaction::new_signed_with_payer(
            &[set_buffer_ix],
            Some(&keypair.pubkey()),
            &[&keypair],
            recent_hash,
        );

        // Send the transaction.
        client.send_and_confirm_transaction_with_spinner_and_config(
            &tx,
            CommitmentConfig::confirmed(),
            RpcSendTransactionConfig {
                skip_preflight: true,
                ..RpcSendTransactionConfig::default()
            },
        )?;

        Ok(())
    })
}

pub fn idl_upgrade(
    cfg_override: &ConfigOverride,
    program_id: Pubkey,
    idl_filepath: String,
) -> Result<()> {
    let buffer = idl_write_buffer(cfg_override, program_id, idl_filepath)?;
    idl_set_buffer(cfg_override, program_id, buffer)
}

pub fn idl_authority(cfg_override: &ConfigOverride, program_id: Pubkey) -> Result<()> {
    with_workspace(cfg_override, |cfg| {
        let url = cluster_url(cfg);
        let client = RpcClient::new(url);
        let idl_address = {
            let account = client
                .get_account_with_commitment(&program_id, CommitmentConfig::processed())?
                .value
                .map_or(Err(anyhow!("Account not found")), Ok)?;
            if account.executable {
                IdlAccount::address(&program_id)
            } else {
                program_id
            }
        };

        let account = client.get_account(&idl_address)?;
        let mut data: &[u8] = &account.data;
        let idl_account: IdlAccount = AccountDeserialize::try_deserialize(&mut data)?;

        println!("{:?}", idl_account.authority);

        Ok(())
    })
}

pub fn idl_set_authority(
    cfg_override: &ConfigOverride,
    program_id: Pubkey,
    address: Option<Pubkey>,
    new_authority: Pubkey,
) -> Result<()> {
    with_workspace(cfg_override, |cfg| {
        // Misc.
        let idl_address = match address {
            None => IdlAccount::address(&program_id),
            Some(addr) => addr,
        };
        let keypair = solana_sdk::signature::read_keypair_file(&cfg.provider.wallet.to_string())
            .map_err(|_| anyhow!("Unable to read keypair file"))?;
        let url = cluster_url(cfg);
        let client = RpcClient::new(url);

        // Instruction data.
        let data =
            serialize_idl_ix(anchor_lang::idl::IdlInstruction::SetAuthority { new_authority })?;

        // Instruction accounts.
        let accounts = vec![
            AccountMeta::new(idl_address, false),
            AccountMeta::new_readonly(keypair.pubkey(), true),
        ];

        // Instruction.
        let ix = Instruction {
            program_id,
            accounts,
            data,
        };
        // Send transaction.
        let (recent_hash, _fee_calc) = client.get_recent_blockhash()?;
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&keypair.pubkey()),
            &[&keypair],
            recent_hash,
        );
        client.send_and_confirm_transaction_with_spinner_and_config(
            &tx,
            CommitmentConfig::confirmed(),
            RpcSendTransactionConfig {
                skip_preflight: true,
                ..RpcSendTransactionConfig::default()
            },
        )?;

        println!("Authority update complete.");

        Ok(())
    })
}

pub fn idl_erase_authority(cfg_override: &ConfigOverride, program_id: Pubkey) -> Result<()> {
    println!("Are you sure you want to erase the IDL authority: [y/n]");

    let stdin = std::io::stdin();
    let mut stdin_lines = stdin.lock().lines();
    let input = stdin_lines.next().unwrap().unwrap();
    if input != "y" {
        println!("Not erasing.");
        return Ok(());
    }

    // Program will treat the zero authority as erased.
    let new_authority = Pubkey::new_from_array([0u8; 32]);
    idl_set_authority(cfg_override, program_id, None, new_authority)?;

    Ok(())
}

// Write the idl to the account buffer, chopping up the IDL into pieces
// and sending multiple transactions in the event the IDL doesn't fit into
// a single transaction.
pub fn idl_write(cfg: &Config, program_id: &Pubkey, idl: &Idl, idl_address: Pubkey) -> Result<()> {
    // Remove the metadata before deploy.
    let mut idl = idl.clone();
    idl.metadata = None;

    // Misc.
    let keypair = solana_sdk::signature::read_keypair_file(&cfg.provider.wallet.to_string())
        .map_err(|_| anyhow!("Unable to read keypair file"))?;
    let url = cluster_url(cfg);
    let client = RpcClient::new(url);

    // Serialize and compress the idl.
    let idl_data = {
        let json_bytes = serde_json::to_vec(&idl)?;
        let mut e = ZlibEncoder::new(Vec::new(), Compression::default());
        e.write_all(&json_bytes)?;
        e.finish()?
    };

    const MAX_WRITE_SIZE: usize = 1000;
    let mut offset = 0;
    while offset < idl_data.len() {
        // Instruction data.
        let data = {
            let start = offset;
            let end = std::cmp::min(offset + MAX_WRITE_SIZE, idl_data.len());
            serialize_idl_ix(anchor_lang::idl::IdlInstruction::Write {
                data: idl_data[start..end].to_vec(),
            })?
        };
        // Instruction accounts.
        let accounts = vec![
            AccountMeta::new(idl_address, false),
            AccountMeta::new_readonly(keypair.pubkey(), true),
        ];
        // Instruction.
        let ix = Instruction {
            program_id: *program_id,
            accounts,
            data,
        };
        // Send transaction.
        let (recent_hash, _fee_calc) = client.get_recent_blockhash()?;
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&keypair.pubkey()),
            &[&keypair],
            recent_hash,
        );
        client.send_and_confirm_transaction_with_spinner_and_config(
            &tx,
            CommitmentConfig::confirmed(),
            RpcSendTransactionConfig {
                skip_preflight: true,
                ..RpcSendTransactionConfig::default()
            },
        )?;
        offset += MAX_WRITE_SIZE;
    }
    Ok(())
}

pub fn idl_parse(file: String, out: Option<String>, out_ts: Option<String>) -> Result<()> {
    let idl = extract_idl(&file)?.ok_or_else(|| anyhow!("IDL not parsed"))?;
    let out = match out {
        None => OutFile::Stdout,
        Some(out) => OutFile::File(PathBuf::from(out)),
    };
    write_idl(&idl, out)?;

    // Write out the TypeScript IDL.
    if let Some(out) = out_ts {
        fs::write(out, template::idl_ts(&idl)?)?;
    }

    Ok(())
}

pub fn idl_fetch(
    cfg_override: &ConfigOverride,
    address: Pubkey,
    out: Option<String>,
) -> Result<()> {
    let idl = fetch_idl(cfg_override, address)?;
    let out = match out {
        None => OutFile::Stdout,
        Some(out) => OutFile::File(PathBuf::from(out)),
    };
    write_idl(&idl, out)
}

pub fn write_idl(idl: &Idl, out: OutFile) -> Result<()> {
    let idl_json = serde_json::to_string_pretty(idl)?;
    match out {
        OutFile::Stdout => println!("{}", idl_json),
        OutFile::File(out) => fs::write(out, idl_json)?,
    };

    Ok(())
}

fn create_idl_account(
    cfg: &Config,
    keypair_path: &str,
    program_id: &Pubkey,
    idl: &Idl,
) -> Result<Pubkey> {
    // Misc.
    let idl_address = IdlAccount::address(program_id);
    let keypair = solana_sdk::signature::read_keypair_file(keypair_path)
        .map_err(|_| anyhow!("Unable to read keypair file"))?;
    let url = cluster_url(cfg);
    let client = RpcClient::new(url);
    let idl_data = serialize_idl(idl)?;

    // Run `Create instruction.
    {
        let data = serialize_idl_ix(anchor_lang::idl::IdlInstruction::Create {
            data_len: (idl_data.len() as u64) * 2, // Double for future growth.
        })?;
        let program_signer = Pubkey::find_program_address(&[], program_id).0;
        let accounts = vec![
            AccountMeta::new_readonly(keypair.pubkey(), true),
            AccountMeta::new(idl_address, false),
            AccountMeta::new_readonly(program_signer, false),
            AccountMeta::new_readonly(solana_program::system_program::ID, false),
            AccountMeta::new_readonly(*program_id, false),
            AccountMeta::new_readonly(solana_program::sysvar::rent::ID, false),
        ];
        let ix = Instruction {
            program_id: *program_id,
            accounts,
            data,
        };
        let (recent_hash, _fee_calc) = client.get_recent_blockhash()?;
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&keypair.pubkey()),
            &[&keypair],
            recent_hash,
        );
        client.send_and_confirm_transaction_with_spinner_and_config(
            &tx,
            CommitmentConfig::confirmed(),
            RpcSendTransactionConfig {
                skip_preflight: true,
                ..RpcSendTransactionConfig::default()
            },
        )?;
    }

    // Write directly to the IDL account buffer.
    idl_write(cfg, program_id, idl, IdlAccount::address(program_id))?;

    Ok(idl_address)
}

fn create_idl_buffer(
    cfg: &Config,
    keypair_path: &str,
    program_id: &Pubkey,
    idl: &Idl,
) -> Result<Pubkey> {
    let keypair = solana_sdk::signature::read_keypair_file(keypair_path)
        .map_err(|_| anyhow!("Unable to read keypair file"))?;
    let url = cluster_url(cfg);
    let client = RpcClient::new(url);

    let buffer = Keypair::generate(&mut OsRng);

    // Creates the new buffer account with the system program.
    let create_account_ix = {
        let space = 8 + 32 + 4 + serialize_idl(idl)?.len() as usize;
        let lamports = client.get_minimum_balance_for_rent_exemption(space)?;
        solana_sdk::system_instruction::create_account(
            &keypair.pubkey(),
            &buffer.pubkey(),
            lamports,
            space as u64,
            program_id,
        )
    };

    // Program instruction to create the buffer.
    let create_buffer_ix = {
        let accounts = vec![
            AccountMeta::new(buffer.pubkey(), false),
            AccountMeta::new_readonly(keypair.pubkey(), true),
            AccountMeta::new_readonly(sysvar::rent::ID, false),
        ];
        let mut data = anchor_lang::idl::IDL_IX_TAG.to_le_bytes().to_vec();
        data.append(&mut IdlInstruction::CreateBuffer.try_to_vec()?);
        Instruction {
            program_id: *program_id,
            accounts,
            data,
        }
    };

    // Build the transaction.
    let (recent_hash, _fee_calc) = client.get_recent_blockhash()?;
    let tx = Transaction::new_signed_with_payer(
        &[create_account_ix, create_buffer_ix],
        Some(&keypair.pubkey()),
        &[&keypair, &buffer],
        recent_hash,
    );

    // Send the transaction.
    client.send_and_confirm_transaction_with_spinner_and_config(
        &tx,
        CommitmentConfig::confirmed(),
        RpcSendTransactionConfig {
            skip_preflight: true,
            ..RpcSendTransactionConfig::default()
        },
    )?;

    Ok(buffer.pubkey())
}

pub fn extract_idl(file: &str) -> Result<Option<Idl>> {
    let file = shellexpand::tilde(file);
    let manifest_from_path =
        std::env::current_dir()?.join(PathBuf::from(&*file).parent().unwrap().to_path_buf());
    let cargo = Manifest::discover_from_path(manifest_from_path)?
        .ok_or_else(|| anyhow!("Cargo.toml not found"))?;
    anchor_syn::idl::file::parse(&*file, cargo.version())
}

// Fetches an IDL for the given program_id.
pub fn fetch_idl(cfg_override: &ConfigOverride, idl_addr: Pubkey) -> Result<Idl> {
    let cfg = Config::discover(cfg_override)?.expect("Inside a workspace");
    let url = cluster_url(&cfg);
    let client = RpcClient::new(url);

    let mut account = client
        .get_account_with_commitment(&idl_addr, CommitmentConfig::processed())?
        .value
        .map_or(Err(anyhow!("Account not found")), Ok)?;

    if account.executable {
        let idl_addr = IdlAccount::address(&idl_addr);
        account = client
            .get_account_with_commitment(&idl_addr, CommitmentConfig::processed())?
            .value
            .map_or(Err(anyhow!("Account not found")), Ok)?;
    }

    // Cut off account discriminator.
    let mut d: &[u8] = &account.data[8..];
    let idl_account: IdlAccount = AnchorDeserialize::deserialize(&mut d)?;

    let mut z = ZlibDecoder::new(&idl_account.data[..]);
    let mut s = Vec::new();
    z.read_to_end(&mut s)?;
    serde_json::from_slice(&s[..]).map_err(Into::into)
}

// Serialize and compress the idl.
fn serialize_idl(idl: &Idl) -> Result<Vec<u8>> {
    let json_bytes = serde_json::to_vec(idl)?;
    let mut e = ZlibEncoder::new(Vec::new(), Compression::default());
    e.write_all(&json_bytes)?;
    e.finish().map_err(Into::into)
}

fn serialize_idl_ix(ix_inner: anchor_lang::idl::IdlInstruction) -> Result<Vec<u8>> {
    let mut data = anchor_lang::idl::IDL_IX_TAG.to_le_bytes().to_vec();
    data.append(&mut ix_inner.try_to_vec()?);
    Ok(data)
}
