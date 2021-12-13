use crate::config::{Config, ConfigOverride};
use anyhow::Result;
use clap::Clap;
use solana_sdk::signature::Signer;

#[derive(Debug, Clap)]
pub enum KeysCommand {
    List,
}

pub fn keys(cfg_override: &ConfigOverride, cmd: KeysCommand) -> Result<()> {
    match cmd {
        KeysCommand::List => keys_list(cfg_override),
    }
}

fn keys_list(cfg_override: &ConfigOverride) -> Result<()> {
    let cfg = Config::discover(cfg_override)?.expect("Not in workspace.");
    for program in cfg.read_all_programs()? {
        let pubkey = program.pubkey()?;
        println!("{}: {}", program.lib_name, pubkey);
    }
    Ok(())
}
