use crate::config::{AnchorPackage, BootstrapMode, Config, ConfigOverride};
use crate::handlers::build;
use anyhow::{anyhow, Result};
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use reqwest::blocking::multipart::{Form, Part};
use reqwest::blocking::Client;
use serde::Deserialize;
use std::fs::{self, File};
use std::io::prelude::*;
use std::path::Path;
use std::string::ToString;
use tar::Archive;

pub fn publish(
    cfg_override: &ConfigOverride,
    program_name: String,
    cargo_args: Vec<String>,
) -> Result<()> {
    // Discover the various workspace configs.
    let cfg = Config::discover(cfg_override)?.expect("Not in workspace.");

    let program = cfg
        .get_program(&program_name)?
        .ok_or_else(|| anyhow!("Workspace member not found"))?;

    let program_cargo_lock = pathdiff::diff_paths(
        program.path().join("Cargo.lock"),
        cfg.path().parent().unwrap(),
    )
    .ok_or_else(|| anyhow!("Unable to diff Cargo.lock path"))?;
    let cargo_lock = Path::new("Cargo.lock");

    // There must be a Cargo.lock
    if !program_cargo_lock.exists() && !cargo_lock.exists() {
        return Err(anyhow!("Cargo.lock must exist for a verifiable build"));
    }

    println!("Publishing will make your code public. Are you sure? Enter (yes)/no:");

    let answer = std::io::stdin().lock().lines().next().unwrap().unwrap();
    if answer != "yes" {
        println!("Aborting");
        return Ok(());
    }

    let anchor_package = AnchorPackage::from(program_name.clone(), &cfg)?;
    let anchor_package_bytes = serde_json::to_vec(&anchor_package)?;

    // Set directory to top of the workspace.
    let workspace_dir = cfg.path().parent().unwrap();
    std::env::set_current_dir(workspace_dir)?;

    // Create the workspace tarball.
    let dot_anchor = workspace_dir.join(".anchor");
    fs::create_dir_all(&dot_anchor)?;
    let tarball_filename = dot_anchor.join(format!("{}.tar.gz", program_name));
    let tar_gz = File::create(&tarball_filename)?;
    let enc = GzEncoder::new(tar_gz, Compression::default());
    let mut tar = tar::Builder::new(enc);

    // Files that will always be included if they exist.
    println!("PACKING: Anchor.toml");
    tar.append_path("Anchor.toml")?;
    if cargo_lock.exists() {
        println!("PACKING: Cargo.lock");
        tar.append_path(cargo_lock)?;
    }
    if Path::new("Cargo.toml").exists() {
        println!("PACKING: Cargo.toml");
        tar.append_path("Cargo.toml")?;
    }
    if Path::new("LICENSE").exists() {
        println!("PACKING: LICENSE");
        tar.append_path("LICENSE")?;
    }
    if Path::new("README.md").exists() {
        println!("PACKING: README.md");
        tar.append_path("README.md")?;
    }

    // All workspace programs.
    for path in cfg.get_program_list()? {
        let mut dirs = walkdir::WalkDir::new(&path)
            .into_iter()
            .filter_entry(|e| !is_hidden(e));

        // Skip the parent dir.
        let _ = dirs.next().unwrap()?;

        for entry in dirs {
            let e = entry.map_err(|e| anyhow!("{:?}", e))?;

            let e = pathdiff::diff_paths(e.path(), cfg.path().parent().unwrap())
                .ok_or_else(|| anyhow!("Unable to diff paths"))?;

            let path_str = e.display().to_string();

            // Skip target dir.
            if !path_str.contains("target/") && !path_str.contains("/target") {
                // Only add the file if it's not empty.
                let metadata = fs::File::open(&e)?.metadata()?;
                if metadata.len() > 0 {
                    println!("PACKING: {}", e.display());
                    if e.is_dir() {
                        tar.append_dir_all(&e, &e)?;
                    } else {
                        tar.append_path(&e)?;
                    }
                }
            }
        }
    }

    // Tar pack complete.
    tar.into_inner()?;

    // Create tmp directory for workspace.
    let ws_dir = dot_anchor.join("workspace");
    if Path::exists(&ws_dir) {
        fs::remove_dir_all(&ws_dir)?;
    }
    fs::create_dir_all(&ws_dir)?;

    // Unpack the archive into the new workspace directory.
    std::env::set_current_dir(&ws_dir)?;
    unpack_archive(&tarball_filename)?;

    // Build the program before sending it to the server.
    build(
        cfg_override,
        None,
        None,
        true,
        Some(program_name),
        None,
        None,
        BootstrapMode::None,
        None,
        None,
        cargo_args,
    )?;

    // Success. Now we can finally upload to the server without worrying
    // about a build failure.

    // Upload the tarball to the server.
    let token = registry_api_token(cfg_override)?;
    let form = Form::new()
        .part("manifest", Part::bytes(anchor_package_bytes))
        .part("workspace", {
            let file = File::open(&tarball_filename)?;
            Part::reader(file)
        });
    let client = Client::new();
    let resp = client
        .post(&format!("{}/api/v0/build", cfg.registry.url))
        .bearer_auth(token)
        .multipart(form)
        .send()?;

    if resp.status() == 200 {
        println!("Build triggered");
    } else {
        println!(
            "{:?}",
            resp.text().unwrap_or_else(|_| "Server error".to_string())
        );
    }

    Ok(())
}

fn is_hidden(entry: &walkdir::DirEntry) -> bool {
    entry
        .file_name()
        .to_str()
        .map(|s| s == "." || s.starts_with('.') || s == "target")
        .unwrap_or(false)
}

// Unpacks the tarball into the current directory.
fn unpack_archive(tar_path: impl AsRef<Path>) -> Result<()> {
    let tar = GzDecoder::new(std::fs::File::open(tar_path)?);
    let mut archive = Archive::new(tar);
    archive.unpack(".")?;
    archive.into_inner();

    Ok(())
}

fn registry_api_token(_cfg_override: &ConfigOverride) -> Result<String> {
    #[derive(Debug, Deserialize)]
    struct Registry {
        token: String,
    }
    #[derive(Debug, Deserialize)]
    struct Credentials {
        registry: Registry,
    }
    let filename = shellexpand::tilde("~/.config/anchor/credentials");
    let mut file = File::open(filename.to_string())?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;

    let credentials_toml: Credentials = toml::from_str(&contents)?;

    Ok(credentials_toml.registry.token)
}
