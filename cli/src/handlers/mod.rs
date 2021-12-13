mod airdrop;
mod build;
mod deploy;
mod init;
mod localnet;
mod login;
mod migrate;
mod new;
mod publish;
mod run;
mod shell;
mod test;
mod upgrade;
mod verify;

mod shared;
mod template;

pub mod cluster;
pub mod idl;
pub mod keys;

#[cfg(feature = "dev")]
pub use airdrop::airdrop;
pub use build::build;
pub use cluster::cluster;
pub use deploy::deploy;
pub use idl::idl;
pub use init::init;
pub use keys::keys;
pub use localnet::localnet;
pub use login::login;
pub use migrate::migrate;
pub use new::new;
pub use publish::publish;
pub use run::run;
pub use shell::shell;
pub use test::test;
pub use upgrade::upgrade;
pub use verify::verify;
