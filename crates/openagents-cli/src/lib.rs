/// The version this binary reports.
///
/// The crate manifest names the version of the source. A release candidate is
/// built from a crate at `0.1.0` and published as `0.1.0-rc.2`, and it is the
/// published name that `oa update` compares against the channel pointer, so
/// `ops/release-cli.sh` threads that name in at build time. Non-release builds
/// must not fall back to the manifest: the manifest still says `0.1.0`, which
/// names a withdrawn release and sorts ahead of the active `0.0.x` series.
/// `0.0.0-dev` is accepted by the update parser, sorts below every published
/// release, and cannot be mistaken for a release build.
pub const VERSION: &str = match option_env!("OPENAGENTS_CLI_RELEASE_VERSION") {
    Some(version) => version,
    None => "0.0.0-dev",
};

pub mod acp;
pub mod api_passthrough;
pub mod auth;
pub mod box_client;
pub mod checks;
pub mod cli;
pub mod coder;
pub mod coder_dev;
pub mod composer;
pub mod computer;
pub mod delegate;
pub mod delegate_result;
pub mod diag;
pub mod diff;
pub mod entry;
pub mod errors;
pub mod fleet;
pub mod foreign_resume;
pub mod forum;
pub mod gym;
pub mod interactive;
pub mod markdown;
pub mod memory_client;
pub mod plugins;
pub mod provider;
pub mod pty;
pub mod repo;
pub mod resume;
pub mod runtime;
pub mod session_store;
pub mod signals;
pub mod surfaces;
pub mod swarm;
pub mod swarm_args;
pub mod tools;
pub mod trace;
pub mod trace_client;
pub mod tracker;
pub mod trailing_args;
pub mod tui;
pub mod update;
pub mod workspace;
