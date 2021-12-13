use super::shared::with_workspace;
use crate::config::ConfigOverride;
use crate::handlers::shared::cluster_url;
use crate::handlers::template;
use anyhow::Result;
use std::fs::{self};
use std::path::Path;
use std::process::Stdio;
use std::string::ToString;

pub fn migrate(cfg_override: &ConfigOverride) -> Result<()> {
    with_workspace(cfg_override, |cfg| {
        println!("Running migration deploy script");

        let url = cluster_url(cfg);
        let cur_dir = std::env::current_dir()?;

        let use_ts =
            Path::new("tsconfig.json").exists() && Path::new("migrations/deploy.ts").exists();

        if !Path::new(".anchor").exists() {
            fs::create_dir(".anchor")?;
        }
        std::env::set_current_dir(".anchor")?;

        let exit = if use_ts {
            let module_path = cur_dir.join("migrations/deploy.ts");
            let deploy_script_host_str =
                template::deploy_ts_script_host(&url, &module_path.display().to_string());
            fs::write("deploy.ts", deploy_script_host_str)?;
            std::process::Command::new("ts-node")
                .arg("deploy.ts")
                .env("ANCHOR_WALLET", cfg.provider.wallet.to_string())
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .output()?
        } else {
            let module_path = cur_dir.join("migrations/deploy.js");
            let deploy_script_host_str =
                template::deploy_js_script_host(&url, &module_path.display().to_string());
            fs::write("deploy.js", deploy_script_host_str)?;
            std::process::Command::new("node")
                .arg("deploy.js")
                .env("ANCHOR_WALLET", cfg.provider.wallet.to_string())
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .output()?
        };

        if !exit.status.success() {
            println!("Deploy failed.");
            std::process::exit(exit.status.code().unwrap());
        }

        println!("Deploy complete.");
        Ok(())
    })
}
