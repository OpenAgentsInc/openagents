//! Terminal agent runs on this computer, read through the public SDK.

use std::time::{SystemTime, UNIX_EPOCH};

use coder_lease::runs::{ClaimAction, RunStatus};
use serde::Serialize;
use serde_json::{Value, json};

use crate::tools::ToolDef;

pub const NAME: &str = "list_runs";
const DEFAULT_STALL_MINUTES: u64 = 20;

pub fn tool() -> ToolDef {
    ToolDef {
        name: NAME,
        description: "Terminal agent sessions on this computer: run id, kind, harness session ids, root process, whether it is alive, and last activity (Unix seconds). Match a claim's HARNESS:ID to kind and the bare ID in sessions (claude means claude-code). claim_action recommends report for a live run with recent activity, or release for a dead run or one silent for the stall window. A missing run does not mean its session is dead. Never type into the terminal.",
        input_schema: json!({
            "type": "object",
            "properties": {
                "stall_minutes": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": 1440,
                    "default": DEFAULT_STALL_MINUTES,
                    "description": "Minutes without activity before a live run's claim should be released. Use the agent definition's stall window."
                }
            }
        }),
    }
}

#[derive(Default, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Args {
    stall_minutes: Option<u64>,
}

#[derive(Serialize)]
struct Report {
    #[serde(flatten)]
    run: RunStatus,
    claim_action: ClaimAction,
}

/// Read local run files without blocking the async tool dispatcher.
pub async fn call(args: Value) -> Result<Value, String> {
    tokio::task::spawn_blocking(move || {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        report(args, now, coder_lease::runs::here)
    })
    .await
    .map_err(|error| format!("couldn't read terminal sessions: {error}"))?
}

fn report(
    args: Value,
    now: u64,
    load: impl FnOnce() -> Result<Vec<RunStatus>, coder_lease::Error>,
) -> Result<Value, String> {
    let args: Args = if args.is_null() {
        Args::default()
    } else {
        serde_json::from_value(args).map_err(|error| format!("invalid arguments: {error}"))?
    };
    let stall_minutes = args.stall_minutes.unwrap_or(DEFAULT_STALL_MINUTES);
    if !(1..=1440).contains(&stall_minutes) {
        return Err("stall_minutes must be between 1 and 1440".into());
    }
    let runs = load()
        .map_err(|error| format!("couldn't read terminal sessions: {error}"))?
        .into_iter()
        .map(|run| {
            let claim_action = run.claim_action(now, stall_minutes * 60);
            Report { run, claim_action }
        })
        .collect::<Vec<_>>();
    Ok(json!({ "runs": runs, "stall_minutes": stall_minutes }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_terminal_sessions_and_the_configured_claim_action() {
        let home = tempfile::tempdir().unwrap();
        let run_dir = home.path().join(".openagents/runs/terminal-1");
        std::fs::create_dir_all(&run_dir).unwrap();
        std::fs::write(
            run_dir.join("run.json"),
            json!({
                "id": "terminal-1", "kind": "claude-code", "host": "test-host",
                "sessions": ["session-1"], "root_pid": 123, "root_start": 0,
                "created": 100, "last_activity": 300, "ended": 400
            })
            .to_string(),
        )
        .unwrap();
        let result = report(Value::Null, 600, || coder_lease::runs::list(home.path())).unwrap();
        assert_eq!(result["stall_minutes"], 20);
        let run = &result["runs"][0];
        assert_eq!(run["id"], "terminal-1");
        assert_eq!(run["kind"], "claude-code");
        assert_eq!(run["host"], "test-host");
        assert_eq!(run["sessions"], json!(["session-1"]));
        assert_eq!(run["root_pid"], 123);
        assert_eq!(run["alive"], false);
        assert_eq!(run["last_activity"], 300);
        assert_eq!(run["claim_action"], "release");

        let mut runs = coder_lease::runs::list(home.path()).unwrap();
        runs[0].alive = true;
        let result = report(json!({ "stall_minutes": 10 }), 600, || Ok(runs)).unwrap();
        assert_eq!(result["runs"][0]["claim_action"], "report");
        let mut runs = coder_lease::runs::list(home.path()).unwrap();
        runs[0].alive = true;
        let result = report(json!({ "stall_minutes": 5 }), 600, || Ok(runs)).unwrap();
        assert_eq!(result["runs"][0]["claim_action"], "release");
    }

    #[test]
    fn rejects_invalid_arguments_before_reading_runs() {
        for args in [
            json!({ "stall_minutes": 0 }),
            json!({ "stall_minutes": 1441 }),
            json!({ "device": "remote" }),
        ] {
            assert!(report(args, 600, || panic!("must validate first")).is_err());
        }
    }
}
