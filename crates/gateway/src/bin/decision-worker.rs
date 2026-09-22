//! The NIP-CJ decision worker: `decision-worker <config.json>` serves
//! kind-`25910` decision jobs from a Nostr relay and fronts the same
//! `POST /v1/systemone` admission path the `gateway` binary serves over
//! HTTP. `docs/decision-models/service/decision-worker.md` is the
//! operator's guide.

use std::process::ExitCode;

use gateway::relay_worker::{self, WorkerConfig};

#[tokio::main]
async fn main() -> ExitCode {
    let Some(path) = std::env::args().nth(1) else {
        eprintln!("usage: decision-worker <config.json>");
        return ExitCode::from(2);
    };
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) => {
            eprintln!("decision-worker: cannot read {path}: {error}");
            return ExitCode::from(2);
        }
    };
    let config: WorkerConfig = match serde_json::from_str(&text) {
        Ok(config) => config,
        Err(error) => {
            eprintln!("decision-worker: {path} does not parse: {error}");
            return ExitCode::from(2);
        }
    };
    match relay_worker::run(config).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("decision-worker: {error}");
            ExitCode::FAILURE
        }
    }
}
