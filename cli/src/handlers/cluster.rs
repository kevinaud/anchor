use anyhow::Result;
use clap::Clap;

#[derive(Debug, Clap)]
pub enum ClusterCommand {
    /// Prints common cluster urls.
    List,
}

pub fn cluster(_cmd: ClusterCommand) -> Result<()> {
    println!("Cluster Endpoints:\n");
    println!("* Mainnet - https://solana-api.projectserum.com");
    println!("* Mainnet - https://api.mainnet-beta.solana.com");
    println!("* Devnet  - https://api.devnet.solana.com");
    println!("* Testnet - https://api.testnet.solana.com");
    Ok(())
}
