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
