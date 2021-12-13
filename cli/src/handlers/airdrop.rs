#[cfg(feature = "dev")]
pub fn airdrop(cfg_override: &crate::config::ConfigOverride) -> anyhow::Result<()> {
    let url = cfg_override
        .cluster
        .unwrap_or_else(|| "https://api.devnet.solana.com".to_string());
    loop {
        let exit = std::process::Command::new("solana")
            .arg("airdrop")
            .arg("10")
            .arg("--url")
            .arg(&url)
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit())
            .output()
            .expect("Must airdrop");
        if !exit.status.success() {
            println!("There was a problem airdropping: {:?}.", exit);
            std::process::exit(exit.status.code().unwrap_or(1));
        }
        std::thread::sleep(std::time::Duration::from_millis(10000));
    }
}
