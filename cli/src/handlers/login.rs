use super::template;
use crate::config::ConfigOverride;
use anyhow::Result;
use std::fs::{self, File};
use std::io::prelude::*;
use std::path::Path;
use std::string::ToString;

pub fn login(_cfg_override: &ConfigOverride, token: String) -> Result<()> {
    let dir = shellexpand::tilde("~/.config/anchor");
    if !Path::new(&dir.to_string()).exists() {
        fs::create_dir(dir.to_string())?;
    }

    std::env::set_current_dir(dir.to_string())?;

    // Freely overwrite the entire file since it's not used for anything else.
    let mut file = File::create("credentials")?;
    file.write_all(template::credentials(&token).as_bytes())?;
    Ok(())
}
