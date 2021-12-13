use super::shared::with_workspace;
use super::template;
use crate::config::ConfigOverride;
use anyhow::Result;
use std::fs::{self, File};
use std::io::prelude::*;

// Creates a new program crate in the `programs/<name>` directory.
pub fn new(cfg_override: &ConfigOverride, name: String) -> Result<()> {
    with_workspace(cfg_override, |cfg| {
        match cfg.path().parent() {
            None => {
                println!("Unable to make new program");
            }
            Some(parent) => {
                std::env::set_current_dir(&parent)?;
                new_program(&name)?;
                println!("Created new program.");
            }
        };
        Ok(())
    })
}

// Creates a new program crate in the current directory with `name`.
pub fn new_program(name: &str) -> Result<()> {
    fs::create_dir(&format!("programs/{}", name))?;
    fs::create_dir(&format!("programs/{}/src/", name))?;
    let mut cargo_toml = File::create(&format!("programs/{}/Cargo.toml", name))?;
    cargo_toml.write_all(template::cargo_toml(name).as_bytes())?;
    let mut xargo_toml = File::create(&format!("programs/{}/Xargo.toml", name))?;
    xargo_toml.write_all(template::xargo_toml().as_bytes())?;
    let mut lib_rs = File::create(&format!("programs/{}/src/lib.rs", name))?;
    lib_rs.write_all(template::lib_rs(name).as_bytes())?;
    Ok(())
}
