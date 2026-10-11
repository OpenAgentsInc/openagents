//! The Boat providers the packaged owners run on.
//!
//! One Boat client (`BOAT_API_KEY` or Secret Manager `oa-boat-api-key`, and
//! `BOAT_API_BASE` when set) serves three providers. Setup and builder
//! machines carry the configured named credentials, read through
//! [`resolver`] each time they are applied (every create and resume on
//! Boat, every command step on GCE), so a short-lived token a minter keeps
//! current under `OA_CREDENTIAL_DIR` never expires mid-build; a computer is
//! still checked to hold only the names it selected. Verifier machines
//! carry none.

use crate::{Config, Providers};
use coder_cloud::runtime::Credentials;
use coder_working_computer::boat::BoatProvider;
use coder_working_computer::provider::{Resolve, resolve_from};

/// Where a token minter may keep current credential values, one file per
/// name; a name with no file is read from the process environment.
pub const CREDENTIAL_DIR: &str = "OA_CREDENTIAL_DIR";

/// Current credential values: `$OA_CREDENTIAL_DIR/<NAME>`, else the
/// process environment.
pub fn resolver() -> Resolve {
    resolve_from(std::env::var_os(CREDENTIAL_DIR).map(Into::into))
}

/// Build the three Boat providers for `config`.
pub async fn providers(config: &Config) -> Result<Providers<BoatProvider>, String> {
    config.validate()?;
    let client = boat::Client::from_env()
        .await
        .map_err(|e| format!("The Boat client is unavailable: {e}"))?;
    let names: Vec<String> = config.credential_names.iter().cloned().collect();
    let resolve = resolver();
    let credentials = Credentials::from_names(&names, |n| resolve(n))?;
    let with = |credentials: Credentials, fresh: Option<Resolve>| BoatProvider {
        client: client.clone(),
        credentials,
        fresh,
        template: config.template.clone(),
        workdir: config.workdir.clone(),
    };
    Ok(Providers {
        setup: with(credentials.clone(), Some(resolve.clone())),
        build: with(credentials, Some(resolve)),
        verify: with(Credentials::default(), None),
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
