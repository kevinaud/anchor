use super::shared::with_workspace;
use crate::config::ConfigOverride;
use crate::handlers::idl::{write_idl, IdlTestMetadata};
use crate::handlers::shared::{cluster_url, OutFile};
use anyhow::Result;
use std::path::PathBuf;
use std::process::Stdio;

pub fn deploy(cfg_override: &ConfigOverride, program_str: Option<String>) -> Result<()> {
    with_workspace(cfg_override, |cfg| {
        let url = cluster_url(cfg);
        let keypair = cfg.provider.wallet.to_string();

        // Deploy the programs.
        println!("Deploying workspace: {}", url);
        println!("Upgrade authority: {}", keypair);

        for mut program in cfg.read_all_programs()? {
            if let Some(single_prog_str) = &program_str {
                let program_name = program.path.file_name().unwrap().to_str().unwrap();
                if single_prog_str.as_str() != program_name {
                    continue;
                }
            }
            let binary_path = program.binary_path().display().to_string();

            println!(
                "Deploying program {:?}...",
                program.path.file_name().unwrap().to_str().unwrap()
            );
            println!("Program path: {}...", binary_path);

            let file = program.keypair_file()?;

            // Send deploy transactions.
            let exit = std::process::Command::new("solana")
                .arg("program")
                .arg("deploy")
                .arg("--url")
                .arg(&url)
                .arg("--keypair")
                .arg(&keypair)
                .arg("--program-id")
                .arg(file.path().display().to_string())
                .arg(&binary_path)
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .output()
                .expect("Must deploy");
            if !exit.status.success() {
                println!("There was a problem deploying: {:?}.", exit);
                std::process::exit(exit.status.code().unwrap_or(1));
            }

            let program_pubkey = program.pubkey()?;
            if let Some(mut idl) = program.idl.as_mut() {
                // Add program address to the IDL.
                idl.metadata = Some(serde_json::to_value(IdlTestMetadata {
                    address: program_pubkey.to_string(),
                })?);

                // Persist it.
                let idl_out = PathBuf::from("target/idl")
                    .join(&idl.name)
                    .with_extension("json");
                write_idl(idl, OutFile::File(idl_out))?;
            }
        }

        println!("Deploy success");

        Ok(())
    })
}
