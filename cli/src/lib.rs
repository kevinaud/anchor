use crate::config::{BootstrapMode, ConfigOverride};

use anyhow::Result;
use clap::Clap;

use handlers::cluster::ClusterCommand;
use handlers::idl::IdlCommand;
use handlers::keys::KeysCommand;

use solana_sdk::pubkey::Pubkey;

pub mod config;

mod handlers;

// Version of the docker image.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const DOCKER_BUILDER_VERSION: &str = VERSION;

#[derive(Debug, Clap)]
#[clap(version = VERSION)]
pub struct Opts {
    #[clap(flatten)]
    pub cfg_override: ConfigOverride,
    #[clap(subcommand)]
    pub command: Command,
}

#[derive(Debug, Clap)]
pub enum Command {
    /// Initializes a workspace.
    Init {
        name: String,
        #[clap(short, long)]
        javascript: bool,
    },
    /// Builds the workspace.
    Build {
        /// Output directory for the IDL.
        #[clap(short, long)]
        idl: Option<String>,
        /// Output directory for the TypeScript IDL.
        #[clap(short = 't', long)]
        idl_ts: Option<String>,
        /// True if the build artifact needs to be deterministic and verifiable.
        #[clap(short, long)]
        verifiable: bool,
        #[clap(short, long)]
        program_name: Option<String>,
        /// Version of the Solana toolchain to use. For --verifiable builds
        /// only.
        #[clap(short, long)]
        solana_version: Option<String>,
        /// Docker image to use. For --verifiable builds only.
        #[clap(short, long)]
        docker_image: Option<String>,
        /// Bootstrap docker image from scratch, installing all requirements for
        /// verifiable builds. Only works for debian-based images.
        #[clap(arg_enum, short, long, default_value = "none")]
        bootstrap: BootstrapMode,
        /// Arguments to pass to the underlying `cargo build-bpf` command
        #[clap(
            required = false,
            takes_value = true,
            multiple_values = true,
            last = true
        )]
        cargo_args: Vec<String>,
    },
    /// Verifies the on-chain bytecode matches the locally compiled artifact.
    /// Run this command inside a program subdirectory, i.e., in the dir
    /// containing the program's Cargo.toml.
    Verify {
        /// The deployed program to compare against.
        program_id: Pubkey,
        #[clap(short, long)]
        program_name: Option<String>,
        /// Version of the Solana toolchain to use. For --verifiable builds
        /// only.
        #[clap(short, long)]
        solana_version: Option<String>,
        /// Docker image to use. For --verifiable builds only.
        #[clap(short, long)]
        docker_image: Option<String>,
        /// Bootstrap docker image from scratch, installing all requirements for
        /// verifiable builds. Only works for debian-based images.
        #[clap(arg_enum, short, long, default_value = "none")]
        bootstrap: BootstrapMode,
        /// Arguments to pass to the underlying `cargo build-bpf` command.
        #[clap(
            required = false,
            takes_value = true,
            multiple_values = true,
            last = true
        )]
        cargo_args: Vec<String>,
    },
    /// Runs integration tests against a localnetwork.
    Test {
        /// Use this flag if you want to run tests against previously deployed
        /// programs.
        #[clap(long)]
        skip_deploy: bool,
        /// Flag to skip starting a local validator, if the configured cluster
        /// url is a localnet.
        #[clap(long)]
        skip_local_validator: bool,
        /// Flag to skip building the program in the workspace,
        /// use this to save time when running test and the program code is not altered.
        #[clap(long)]
        skip_build: bool,
        /// Flag to keep the local validator running after tests
        /// to be able to check the transactions.
        #[clap(long)]
        detach: bool,
        #[clap(multiple_values = true)]
        args: Vec<String>,
        /// Arguments to pass to the underlying `cargo build-bpf` command.
        #[clap(
            required = false,
            takes_value = true,
            multiple_values = true,
            last = true
        )]
        cargo_args: Vec<String>,
    },
    /// Creates a new program.
    New { name: String },
    /// Commands for interacting with interface definitions.
    Idl {
        #[clap(subcommand)]
        subcmd: IdlCommand,
    },
    /// Deploys each program in the workspace.
    Deploy {
        #[clap(short, long)]
        program_name: Option<String>,
    },
    /// Runs the deploy migration script.
    Migrate,
    /// Deploys, initializes an IDL, and migrates all in one command.
    /// Upgrades a single program. The configured wallet must be the upgrade
    /// authority.
    Upgrade {
        /// The program to upgrade.
        #[clap(short, long)]
        program_id: Pubkey,
        /// Filepath to the new program binary.
        program_filepath: String,
    },
    #[cfg(feature = "dev")]
    /// Runs an airdrop loop, continuously funding the configured wallet.
    Airdrop {
        #[clap(short, long)]
        url: Option<String>,
    },
    /// Cluster commands.
    Cluster {
        #[clap(subcommand)]
        subcmd: ClusterCommand,
    },
    /// Starts a node shell with an Anchor client setup according to the local
    /// config.
    Shell,
    /// Runs the script defined by the current workspace's Anchor.toml.
    Run {
        /// The name of the script to run.
        script: String,
    },
    /// Saves an api token from the registry locally.
    Login {
        /// API access token.
        token: String,
    },
    /// Publishes a verified build to the Anchor registry.
    Publish {
        /// The name of the program to publish.
        program: String,
        /// Arguments to pass to the underlying `cargo build-bpf` command.
        #[clap(
            required = false,
            takes_value = true,
            multiple_values = true,
            last = true
        )]
        cargo_args: Vec<String>,
    },
    /// Keypair commands.
    Keys {
        #[clap(subcommand)]
        subcmd: KeysCommand,
    },
    /// Localnet commands.
    Localnet {
        /// Flag to skip building the program in the workspace,
        /// use this to save time when running test and the program code is not altered.
        #[clap(long)]
        skip_build: bool,
        /// Use this flag if you want to run tests against previously deployed
        /// programs.
        #[clap(long)]
        skip_deploy: bool,
        /// Arguments to pass to the underlying `cargo build-bpf` command.
        #[clap(
            required = false,
            takes_value = true,
            multiple_values = true,
            last = true
        )]
        cargo_args: Vec<String>,
    },
}

pub fn entry(opts: Opts) -> Result<()> {
    match opts.command {
        Command::Init { name, javascript } => handlers::init(&opts.cfg_override, name, javascript),
        Command::New { name } => handlers::new(&opts.cfg_override, name),
        Command::Build {
            idl,
            idl_ts,
            verifiable,
            program_name,
            solana_version,
            docker_image,
            bootstrap,
            cargo_args,
        } => handlers::build(
            &opts.cfg_override,
            idl,
            idl_ts,
            verifiable,
            program_name,
            solana_version,
            docker_image,
            bootstrap,
            None,
            None,
            cargo_args,
        ),
        Command::Verify {
            program_id,
            program_name,
            solana_version,
            docker_image,
            bootstrap,
            cargo_args,
        } => handlers::verify(
            &opts.cfg_override,
            program_id,
            program_name,
            solana_version,
            docker_image,
            bootstrap,
            cargo_args,
        ),
        Command::Deploy { program_name } => handlers::deploy(&opts.cfg_override, program_name),
        Command::Upgrade {
            program_id,
            program_filepath,
        } => handlers::upgrade(&opts.cfg_override, program_id, program_filepath),
        Command::Idl { subcmd } => handlers::idl(&opts.cfg_override, subcmd),
        Command::Migrate => handlers::migrate(&opts.cfg_override),
        Command::Test {
            skip_deploy,
            skip_local_validator,
            skip_build,
            detach,
            args,
            cargo_args,
        } => handlers::test(
            &opts.cfg_override,
            skip_deploy,
            skip_local_validator,
            skip_build,
            detach,
            args,
            cargo_args,
        ),
        #[cfg(feature = "dev")]
        Command::Airdrop => handlers::airdrop(cfg_override),
        Command::Cluster { subcmd } => handlers::cluster(subcmd),
        Command::Shell => handlers::shell(&opts.cfg_override),
        Command::Run { script } => handlers::run(&opts.cfg_override, script),
        Command::Login { token } => handlers::login(&opts.cfg_override, token),
        Command::Publish {
            program,
            cargo_args,
        } => handlers::publish(&opts.cfg_override, program, cargo_args),
        Command::Keys { subcmd } => handlers::keys(&opts.cfg_override, subcmd),
        Command::Localnet {
            skip_build,
            skip_deploy,
            cargo_args,
        } => handlers::localnet(&opts.cfg_override, skip_build, skip_deploy, cargo_args),
    }
}
