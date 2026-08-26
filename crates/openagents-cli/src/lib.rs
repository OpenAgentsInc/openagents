/// The version this binary was published as.
///
/// The crate manifest names the version of the source. A release candidate is
/// built from a crate at `0.1.0` and published as `0.1.0-rc.2`, and it is the
/// published name that `oa update` compares against the channel pointer, so
/// `ops/release-cli.sh` threads that name in at build time. A build that is
/// not a release falls back to the manifest, which is the honest answer for
/// one.
pub const VERSION: &str = match option_env!("OPENAGENTS_CLI_RELEASE_VERSION") {
    Some(version) => version,
    None => env!("CARGO_PKG_VERSION"),
};

pub mod acp;
pub mod api_passthrough;
pub mod auth;
pub mod box_client;
pub mod cli;
pub mod composer;
pub mod computer;
pub mod delegate;
pub mod diag;
pub mod diff;
pub mod errors;
pub mod fleet;
pub mod foreign_resume;
pub mod forum;
pub mod interactive;
pub mod markdown;
pub mod memory_client;
pub mod plugins;
pub mod provider;
pub mod pty;
pub mod repo;
pub mod resume;
pub mod runtime;
pub mod signals;
pub mod tools;
pub mod trace;
pub mod trace_client;
pub mod tracker;
pub mod trailing_args;
pub mod tui;
pub mod update;
pub mod workspace;
