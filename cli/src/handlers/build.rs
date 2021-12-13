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

use super::idl::{extract_idl, write_idl};
use super::shared::{cd_member, set_workspace_dir_or_exit, OutFile};
use super::template;

#[allow(clippy::too_many_arguments)]
pub fn build(
    cfg_override: &ConfigOverride,
    idl: Option<String>,
    idl_ts: Option<String>,
    verifiable: bool,
    program_name: Option<String>,
    solana_version: Option<String>,
    docker_image: Option<String>,
    bootstrap: BootstrapMode,
    stdout: Option<File>, // Used for the package registry server.
    stderr: Option<File>, // Used for the package registry server.
    cargo_args: Vec<String>,
) -> Result<()> {
    // Change to the workspace member directory, if needed.
    if let Some(program_name) = program_name.as_ref() {
        cd_member(cfg_override, program_name)?;
    }

    let cfg = Config::discover(cfg_override)?.expect("Not in workspace.");
    let build_config = BuildConfig {
        verifiable,
        solana_version: solana_version.or_else(|| cfg.solana_version.clone()),
        docker_image: docker_image.unwrap_or_else(|| cfg.docker()),
        bootstrap,
    };
    let cfg_parent = cfg.path().parent().expect("Invalid Anchor.toml");

    let cargo = Manifest::discover()?;

    let idl_out = match idl {
        Some(idl) => Some(PathBuf::from(idl)),
        None => Some(cfg_parent.join("target/idl")),
    };
    fs::create_dir_all(idl_out.as_ref().unwrap())?;

    let idl_ts_out = match idl_ts {
        Some(idl_ts) => Some(PathBuf::from(idl_ts)),
        None => Some(cfg_parent.join("target/types")),
    };
    fs::create_dir_all(idl_ts_out.as_ref().unwrap())?;

    if !&cfg.workspace.types.is_empty() {
        fs::create_dir_all(cfg_parent.join(&cfg.workspace.types))?;
    };

    match cargo {
        // No Cargo.toml so build the entire workspace.
        None => build_all(
            &cfg,
            cfg.path(),
            idl_out,
            idl_ts_out,
            &build_config,
            stdout,
            stderr,
            cargo_args,
        )?,
        // If the Cargo.toml is at the root, build the entire workspace.
        Some(cargo) if cargo.path().parent() == cfg.path().parent() => build_all(
            &cfg,
            cfg.path(),
            idl_out,
            idl_ts_out,
            &build_config,
            stdout,
            stderr,
            cargo_args,
        )?,
        // Cargo.toml represents a single package. Build it.
        Some(cargo) => build_cwd(
            &cfg,
            cargo.path().to_path_buf(),
            idl_out,
            idl_ts_out,
            &build_config,
            stdout,
            stderr,
            cargo_args,
        )?,
    }

    set_workspace_dir_or_exit();

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn build_all(
    cfg: &WithPath<Config>,
    cfg_path: &Path,
    idl_out: Option<PathBuf>,
    idl_ts_out: Option<PathBuf>,
    build_config: &BuildConfig,
    stdout: Option<File>, // Used for the package registry server.
    stderr: Option<File>, // Used for the package registry server.
    cargo_args: Vec<String>,
) -> Result<()> {
    let cur_dir = std::env::current_dir()?;
    let r = match cfg_path.parent() {
        None => Err(anyhow!("Invalid Anchor.toml at {}", cfg_path.display())),
        Some(_parent) => {
            for p in cfg.get_program_list()? {
                build_cwd(
                    cfg,
                    p.join("Cargo.toml"),
                    idl_out.clone(),
                    idl_ts_out.clone(),
                    build_config,
                    stdout.as_ref().map(|f| f.try_clone()).transpose()?,
                    stderr.as_ref().map(|f| f.try_clone()).transpose()?,
                    cargo_args.clone(),
                )?;
            }
            Ok(())
        }
    };
    std::env::set_current_dir(cur_dir)?;
    r
}

// Runs the build command outside of a workspace.
#[allow(clippy::too_many_arguments)]
fn build_cwd(
    cfg: &WithPath<Config>,
    cargo_toml: PathBuf,
    idl_out: Option<PathBuf>,
    idl_ts_out: Option<PathBuf>,
    build_config: &BuildConfig,
    stdout: Option<File>,
    stderr: Option<File>,
    cargo_args: Vec<String>,
) -> Result<()> {
    match cargo_toml.parent() {
        None => return Err(anyhow!("Unable to find parent")),
        Some(p) => std::env::set_current_dir(&p)?,
    };
    match build_config.verifiable {
        false => _build_cwd(cfg, idl_out, idl_ts_out, cargo_args),
        true => build_cwd_verifiable(cfg, cargo_toml, build_config, stdout, stderr, cargo_args),
    }
}

fn _build_cwd(
    cfg: &WithPath<Config>,
    idl_out: Option<PathBuf>,
    idl_ts_out: Option<PathBuf>,
    cargo_args: Vec<String>,
) -> Result<()> {
    let exit = std::process::Command::new("cargo")
        .arg("build-bpf")
        .args(cargo_args)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .output()
        .map_err(|e| anyhow::format_err!("{}", e.to_string()))?;
    if !exit.status.success() {
        std::process::exit(exit.status.code().unwrap_or(1));
    }

    // Always assume idl is located at src/lib.rs.
    if let Some(idl) = extract_idl("src/lib.rs")? {
        // JSON out path.
        let out = match idl_out {
            None => PathBuf::from(".").join(&idl.name).with_extension("json"),
            Some(o) => PathBuf::from(&o.join(&idl.name).with_extension("json")),
        };
        // TS out path.
        let ts_out = match idl_ts_out {
            None => PathBuf::from(".").join(&idl.name).with_extension("ts"),
            Some(o) => PathBuf::from(&o.join(&idl.name).with_extension("ts")),
        };

        // Write out the JSON file.
        write_idl(&idl, OutFile::File(out))?;
        // Write out the TypeScript type.
        fs::write(&ts_out, template::idl_ts(&idl)?)?;
        // Copy out the TypeScript type.
        let cfg_parent = cfg.path().parent().expect("Invalid Anchor.toml");
        if !&cfg.workspace.types.is_empty() {
            fs::copy(
                &ts_out,
                cfg_parent
                    .join(&cfg.workspace.types)
                    .join(&idl.name)
                    .with_extension("ts"),
            )?;
        }
    }

    Ok(())
}

// Builds an anchor program in a docker image and copies the build artifacts
// into the `target/` directory.
fn build_cwd_verifiable(
    cfg: &WithPath<Config>,
    cargo_toml: PathBuf,
    build_config: &BuildConfig,
    stdout: Option<File>,
    stderr: Option<File>,
    cargo_args: Vec<String>,
) -> Result<()> {
    // Create output dirs.
    let workspace_dir = cfg.path().parent().unwrap().canonicalize()?;
    fs::create_dir_all(workspace_dir.join("target/verifiable"))?;
    fs::create_dir_all(workspace_dir.join("target/idl"))?;
    fs::create_dir_all(workspace_dir.join("target/types"))?;
    if !&cfg.workspace.types.is_empty() {
        fs::create_dir_all(workspace_dir.join(&cfg.workspace.types))?;
    }

    let container_name = "anchor-program";

    // Build the binary in docker.
    let result = docker_build(
        cfg,
        container_name,
        cargo_toml,
        build_config,
        stdout,
        stderr,
        cargo_args,
    );

    match &result {
        Err(e) => {
            eprintln!("Error during Docker build: {:?}", e);
        }
        Ok(_) => {
            // Build the idl.
            println!("Extracting the IDL");
            if let Ok(Some(idl)) = extract_idl("src/lib.rs") {
                // Write out the JSON file.
                println!("Writing the IDL file");
                let out_file = workspace_dir.join(format!("target/idl/{}.json", idl.name));
                write_idl(&idl, OutFile::File(out_file))?;

                // Write out the TypeScript type.
                println!("Writing the .ts file");
                let ts_file = workspace_dir.join(format!("target/types/{}.ts", idl.name));
                fs::write(&ts_file, template::idl_ts(&idl)?)?;

                // Copy out the TypeScript type.
                if !&cfg.workspace.types.is_empty() {
                    fs::copy(
                        ts_file,
                        workspace_dir
                            .join(&cfg.workspace.types)
                            .join(idl.name)
                            .with_extension("ts"),
                    )?;
                }
            }
            println!("Build success");
        }
    }

    result
}

fn docker_build(
    cfg: &WithPath<Config>,
    container_name: &str,
    cargo_toml: PathBuf,
    build_config: &BuildConfig,
    stdout: Option<File>,
    stderr: Option<File>,
    cargo_args: Vec<String>,
) -> Result<()> {
    let binary_name = Manifest::from_path(&cargo_toml)?.lib_name()?;

    // Docker vars.
    let workdir = Path::new("/workdir");
    let volume_mount = format!(
        "{}:{}",
        cfg.path().parent().unwrap().canonicalize()?.display(),
        workdir.to_str().unwrap(),
    );
    println!("Using image {:?}", build_config.docker_image);

    // Start the docker image running detached in the background.
    let target_dir = workdir.join("docker-target");
    println!("Run docker image");
    let exit = std::process::Command::new("docker")
        .args(&[
            "run",
            "-it",
            "-d",
            "--name",
            container_name,
            "--env",
            &format!(
                "CARGO_TARGET_DIR={}",
                target_dir.as_path().to_str().unwrap()
            ),
            "-v",
            &volume_mount,
            "-w",
            workdir.to_str().unwrap(),
            &build_config.docker_image,
            "bash",
        ])
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .output()
        .map_err(|e| anyhow::format_err!("Docker build failed: {}", e.to_string()))?;
    if !exit.status.success() {
        return Err(anyhow!("Failed to build program"));
    }

    let result = docker_prep(container_name, build_config).and_then(|_| {
        let cfg_parent = cfg.path().parent().unwrap();
        docker_build_bpf(
            container_name,
            cargo_toml.as_path(),
            cfg_parent,
            target_dir.as_path(),
            binary_name,
            stdout,
            stderr,
            cargo_args,
        )
    });

    // Cleanup regardless of errors
    docker_cleanup(container_name, target_dir.as_path())?;

    // Done.
    result
}

fn docker_prep(container_name: &str, build_config: &BuildConfig) -> Result<()> {
    // Set the solana version in the container, if given. Otherwise use the
    // default.
    match build_config.bootstrap {
        BootstrapMode::Debian => {
            // Install build requirements
            docker_exec(container_name, &["apt", "update"])?;
            docker_exec(
                container_name,
                &["apt", "install", "-y", "curl", "build-essential"],
            )?;

            // Install Rust
            docker_exec(
                container_name,
                &["curl", "https://sh.rustup.rs", "-sfo", "rustup.sh"],
            )?;
            docker_exec(container_name, &["sh", "rustup.sh", "-y"])?;
            docker_exec(container_name, &["rm", "-f", "rustup.sh"])?;
        }
        BootstrapMode::None => {}
    }

    if let Some(solana_version) = &build_config.solana_version {
        println!("Using solana version: {}", solana_version);

        // Install Solana CLI
        docker_exec(
            container_name,
            &[
                "curl",
                "-sSfL",
                &format!("https://release.solana.com/v{0}/install", solana_version,),
                "-o",
                "solana_installer.sh",
            ],
        )?;
        docker_exec(container_name, &["sh", "solana_installer.sh"])?;
        docker_exec(container_name, &["rm", "-f", "solana_installer.sh"])?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn docker_build_bpf(
    container_name: &str,
    cargo_toml: &Path,
    cfg_parent: &Path,
    target_dir: &Path,
    binary_name: String,
    stdout: Option<File>,
    stderr: Option<File>,
    cargo_args: Vec<String>,
) -> Result<()> {
    let manifest_path =
        pathdiff::diff_paths(cargo_toml.canonicalize()?, cfg_parent.canonicalize()?)
            .ok_or_else(|| anyhow!("Unable to diff paths"))?;
    println!(
        "Building {} manifest: {:?}",
        binary_name,
        manifest_path.display().to_string()
    );

    // Execute the build.
    let exit = std::process::Command::new("docker")
        .args(&[
            "exec",
            "--env",
            "PATH=/root/.local/share/solana/install/active_release/bin:/root/.cargo/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin",
            container_name,
            "cargo",
            "build-bpf",
            "--manifest-path",
            &manifest_path.display().to_string(),
        ])
        .args(cargo_args)
        .stdout(match stdout {
            None => Stdio::inherit(),
            Some(f) => f.into(),
        })
        .stderr(match stderr {
            None => Stdio::inherit(),
            Some(f) => f.into(),
        })
        .output()
        .map_err(|e| anyhow::format_err!("Docker build failed: {}", e.to_string()))?;
    if !exit.status.success() {
        return Err(anyhow!("Failed to build program"));
    }

    // Copy the binary out of the docker image.
    println!("Copying out the build artifacts");
    let out_file = cfg_parent
        .canonicalize()?
        .join(format!("target/verifiable/{}.so", binary_name))
        .display()
        .to_string();

    // This requires the target directory of any built program to be located at
    // the root of the workspace.
    let mut bin_path = target_dir.join("deploy");
    bin_path.push(format!("{}.so", binary_name));
    let bin_artifact = format!(
        "{}:{}",
        container_name,
        bin_path.as_path().to_str().unwrap()
    );
    let exit = std::process::Command::new("docker")
        .args(&["cp", &bin_artifact, &out_file])
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .output()
        .map_err(|e| anyhow::format_err!("{}", e.to_string()))?;
    if !exit.status.success() {
        Err(anyhow!(
            "Failed to copy binary out of docker. Is the target directory set correctly?"
        ))
    } else {
        Ok(())
    }
}

fn docker_cleanup(container_name: &str, target_dir: &Path) -> Result<()> {
    // Wipe the generated docker-target dir.
    println!("Cleaning up the docker target directory");
    docker_exec(container_name, &["rm", "-rf", target_dir.to_str().unwrap()])?;

    // Remove the docker image.
    println!("Removing the docker image");
    let exit = std::process::Command::new("docker")
        .args(&["rm", "-f", container_name])
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .output()
        .map_err(|e| anyhow::format_err!("{}", e.to_string()))?;
    if !exit.status.success() {
        println!("Unable to remove docker container");
        std::process::exit(exit.status.code().unwrap_or(1));
    }
    Ok(())
}

fn docker_exec(container_name: &str, args: &[&str]) -> Result<()> {
    let exit = std::process::Command::new("docker")
        .args([&["exec", container_name], args].concat())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .output()
        .map_err(|e| anyhow!("Failed to run command \"{:?}\": {:?}", args, e))?;
    if !exit.status.success() {
        Err(anyhow!("Failed to run command: {:?}", args))
    } else {
        Ok(())
    }
}
