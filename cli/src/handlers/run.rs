use super::shared::{cluster_url, with_workspace};
use crate::config::ConfigOverride;
use anyhow::{anyhow, Result};
use std::process::Stdio;
use std::string::ToString;

pub fn run(cfg_override: &ConfigOverride, script: String) -> Result<()> {
    with_workspace(cfg_override, |cfg| {
        let url = cluster_url(cfg);
        let script = cfg
            .scripts
            .get(&script)
            .ok_or_else(|| anyhow!("Unable to find script"))?;
        let exit = std::process::Command::new("bash")
            .arg("-c")
            .arg(&script)
            .env("ANCHOR_PROVIDER_URL", url)
            .env("ANCHOR_WALLET", cfg.provider.wallet.to_string())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .output()
            .unwrap();
        if !exit.status.success() {
            std::process::exit(exit.status.code().unwrap_or(1));
        }
        Ok(())
    })
}
