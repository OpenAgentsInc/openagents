//! The Boat providers the packaged owners run on.
//!
//! One Boat client (`BOAT_API_KEY` or Secret Manager `boat-api-key`, and
//! `BOAT_API_BASE` when set) serves three providers. Setup and builder
//! machines carry the configured named credentials, read once from the
//! operator's process environment; a computer is still checked to hold
//! only the names it selected. Verifier machines carry none.

use crate::{Config, Providers};
use coder_cloud::runtime::Credentials;
use coder_working_computer::boat::BoatProvider;

/// Build the three Boat providers for `config`.
pub async fn providers(config: &Config) -> Result<Providers<BoatProvider>, String> {
    config.validate()?;
    let client = boat::Client::from_env()
        .await
        .map_err(|e| format!("The Boat client is unavailable: {e}"))?;
    let names: Vec<String> = config.credential_names.iter().cloned().collect();
    let credentials = Credentials::from_names(&names, |n| std::env::var(n).ok())?;
    let with = |credentials: Credentials| BoatProvider {
        client: client.clone(),
        credentials,
        template: config.template.clone(),
        workdir: config.workdir.clone(),
    };
    Ok(Providers {
        setup: with(credentials.clone()),
        build: with(credentials),
        verify: with(Credentials::default()),
    })
}

/// The newest ready interactive runtime template (`oa-coder-runtime-*`),
/// which carries the headless Coder runtime and Claude Code; `None` when
/// Boat holds none.
pub async fn runtime_template() -> Result<Option<String>, String> {
    let client = boat::Client::from_env()
        .await
        .map_err(|e| format!("The Boat client is unavailable: {e}"))?;
    let snapshots = client
        .list_named_snapshots()
        .await
        .map_err(|e| format!("Boat's templates couldn't be listed: {e}"))?
        .snapshots;
    Ok(newest_runtime(
        snapshots
            .iter()
            .map(|s| (s.name.as_str(), s.status.as_str())),
    ))
}

/// The newest ready name in the interactive runtime namespace.
pub fn newest_runtime<'a>(snapshots: impl Iterator<Item = (&'a str, &'a str)>) -> Option<String> {
    snapshots
        .filter(|(name, status)| *status == "ready" && name.starts_with("oa-coder-runtime-"))
        .map(|(name, _)| name)
        .max()
        .map(str::to_owned)
}
