//! `decision-advertise` — publish the decision service's NIP-CAP
//! `kind:30180` manifest to a relay.
//!
//! Reads one `decision-advertise.json` — relay, publisher secret, slug,
//! lanes, registry, limits, schema references, and an optional
//! expiration — and signs and publishes the service manifest the
//! NIP-CAP decision-service contract defines. What it advertises is
//! derived: doors come from the registry's shared set, lanes from the
//! operator's own configuration. See
//! `docs/decision-models/service/decision-advertise.md` for the
//! operator flow.
//!
//! Usage: `decision-advertise <decision-advertise.json>`

use std::process::ExitCode;

#[tokio::main]
async fn main() -> ExitCode {
    let Some(path) = std::env::args().nth(1) else {
        eprintln!("usage: decision-advertise <decision-advertise.json>");
        return ExitCode::from(2);
    };
    let body = match std::fs::read_to_string(&path) {
        Ok(body) => body,
        Err(error) => {
            eprintln!("decision-advertise: cannot read {path}: {error}");
            return ExitCode::from(2);
        }
    };
    let config: gateway::advertise::AdvertiseConfig = match serde_json::from_str(&body) {
        Ok(config) => config,
        Err(error) => {
            eprintln!("decision-advertise: {path} does not parse: {error}");
            return ExitCode::from(2);
        }
    };
    match gateway::advertise::run(config).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("decision-advertise: {error}");
            ExitCode::FAILURE
        }
    }
}
