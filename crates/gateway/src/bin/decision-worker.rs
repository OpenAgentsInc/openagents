//! The NIP-CJ decision worker: `decision-worker <config.json>` serves
//! kind-`25910` decision jobs from a Nostr relay and fronts the same
//! `POST /v1/systemone` admission path the `gateway` binary serves over
//! HTTP. `docs/decision-models/service/decision-worker.md` is the
//! operator's guide.
//!
//! `decision-worker usage [--since YYYY-MM-DD] [--by key|lane|model|door|day|outcome]
//! [--json] [--dir DIR]` summarizes the usage log: one line per job, in
//! `usage/` under the worker's `jobs_dir` (default
//! `/var/lib/decision-worker/usage`, or `DECISION_WORKER_USAGE_DIR`).

use std::process::ExitCode;

use gateway::relay_worker::{self, WorkerConfig};

#[tokio::main]
async fn main() -> ExitCode {
    let Some(path) = std::env::args().nth(1) else {
        eprintln!(
            "usage: decision-worker <config.json> | usage [--since YYYY-MM-DD] [--by key|lane|model|door|day|outcome] [--json] [--dir DIR]"
        );
        return ExitCode::from(2);
    };
    if path == "usage" {
        let args: Vec<String> = std::env::args().skip(2).collect();
        return match usage(&args) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("decision-worker: {error}");
                ExitCode::from(2)
            }
        };
    }
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

/// `decision-worker usage`: the usage log, grouped. It reads the log only,
/// so it needs no secret or config.
fn usage(args: &[String]) -> Result<(), String> {
    use gateway::decision_usage::{By, read, stats, table};
    let mut since = None;
    let mut by = By::Day;
    let mut json = false;
    let mut dir = std::env::var_os("DECISION_WORKER_USAGE_DIR").map_or_else(
        || std::path::PathBuf::from("/var/lib/decision-worker/usage"),
        std::path::PathBuf::from,
    );
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        let mut value = || {
            args.next()
                .cloned()
                .ok_or_else(|| format!("{arg} needs a value"))
        };
        match arg.as_str() {
            "--since" => since = Some(value()?),
            "--by" => by = By::parse(&value()?)?,
            "--dir" => dir = std::path::PathBuf::from(value()?),
            "--json" => json = true,
            other => return Err(format!("usage: unknown argument {other}")),
        }
    }
    let found = read(&dir, since.as_deref())?;
    let rows = stats(&found.records, by);
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&rows).map_err(|e| e.to_string())?
        );
    } else {
        print!("{}", table(&rows, by));
    }
    if found.unreadable > 0 {
        eprintln!("{} lines did not read as records", found.unreadable);
    }
    Ok(())
}
