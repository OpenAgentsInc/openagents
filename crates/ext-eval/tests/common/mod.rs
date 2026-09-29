//! Shared fixtures: the tiny suite, its trajectories, and scenario runs.

#![allow(dead_code)]

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use ext_eval::artifact::JSON;
use ext_eval::door::fake::FakeDecisionDoor;
use ext_eval::{
    Arm, ArmSetup, ArtifactRef, DecisionAnswer, Identity, LoadOptions, Plan, RunFailure,
    RunOutcome, RunRecord, Suite, Trajectory,
};
use serde_json::{Value, json};

/// The suite author, a fixed test key.
pub const AUTHOR: &str = "5be6446aef0a9a6b1f2c3d4e5f6071829a4b5c6d7e8f90112233445566778899";
/// The evaluator, a second fixed key.
pub const EVALUATOR: &str = "0b1c2d3e4f506172894a5b6c7d8e9f00112233445566778899aabbccddeeff00";
/// The operation the extension under test supplies.
pub const MAP: &str = "repo-map.map";
/// The answer that names the callers.
pub const RIGHT: &str = "The callers of `parse_case` are `load` and `parse_all`.";

pub fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

pub fn suite() -> Suite {
    Suite::load(&fixtures().join("suite/evals"), LoadOptions::default()).expect("the suite loads")
}

pub fn plan(runs: Option<u32>) -> Plan {
    Plan {
        baseline: true,
        runs,
        extension_operations: BTreeSet::from([MAP.to_string()]),
    }
}

pub fn trajectory(name: &str) -> Trajectory {
    let path = fixtures().join(format!("trajectories/{name}.json"));
    let bytes = std::fs::read(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    Trajectory::from_bytes(&bytes).expect("a valid trajectory")
}

pub fn definition(id: &str, bytes: &[u8]) -> Value {
    json!({
        "id": format!("{AUTHOR}:{id}"),
        "artifact": ArtifactRef::of(bytes, JSON, Some("openagents.ext-package.v1")).value(),
    })
}

pub fn identity() -> Identity {
    Identity {
        author: AUTHOR.into(),
        package: "repo-map".into(),
        component: "eval-suite".into(),
        evaluator: EVALUATOR.into(),
        subject: ArmSetup {
            definition: definition("repo-map/repo-map", b"repo-map package"),
            lock: ArtifactRef::of(b"subject lock", JSON, Some("openagents.lock.v1")),
            door: "fixture-door".into(),
            run: json!({ "shell_rounds": 8 }),
        },
        baseline: Some(ArmSetup {
            definition: definition("coder/coder", b"coder"),
            lock: ArtifactRef::of(b"baseline lock", JSON, Some("openagents.lock.v1")),
            door: "fixture-door".into(),
            run: json!({ "shell_rounds": 8 }),
        }),
        started_at: 1_790_000_000,
        ended_at: 1_790_000_900,
        requester: None,
        suite_release: None,
        environment: None,
        partial: None,
    }
}

/// A decision door that says yes (0.9) when the run's last message names
/// the callers, and no (0.1) otherwise. Test-only.
pub fn decision_door() -> FakeDecisionDoor {
    FakeDecisionDoor::answering(|state, _| {
        Ok(DecisionAnswer::Noul(if state["run"] == RIGHT {
            0.9
        } else {
            0.1
        }))
    })
}

/// A scenario's records and planned runs.
pub struct Scenario {
    pub runs: Option<u32>,
    pub records: Vec<RunRecord>,
}

pub fn scenario(name: &str) -> Scenario {
    let path = fixtures().join(format!("scenarios/{name}.json"));
    let manifest: Value =
        serde_json::from_slice(&std::fs::read(&path).expect("the scenario reads")).expect("json");
    let runs = manifest["runs"]
        .as_u64()
        .map(|runs| u32::try_from(runs).unwrap());
    let mut records = Vec::new();
    for (case, arms) in manifest["cases"].as_object().expect("cases") {
        for (arm_word, entries) in arms.as_object().expect("arms") {
            let arm = if arm_word == "subject" {
                Arm::Subject
            } else {
                Arm::Baseline
            };
            for (index, entry) in entries.as_array().expect("runs").iter().enumerate() {
                let attempt = u32::try_from(index + 1).unwrap();
                records.push(record(case, arm, attempt, entry, &manifest, arm_word));
            }
        }
    }
    Scenario { runs, records }
}

fn record(
    case: &str,
    arm: Arm,
    attempt: u32,
    entry: &Value,
    manifest: &Value,
    arm_word: &str,
) -> RunRecord {
    let entry = match entry {
        Value::String(name) => json!({ "trajectory": name }),
        other => other.clone(),
    };
    let outcome = match entry["outcome"].as_str() {
        None | Some("completed") => RunOutcome::Completed,
        Some("timeout") => RunOutcome::Errored(RunFailure::Timeout),
        Some("cancelled") => RunOutcome::Cancelled,
        Some("unknown") => RunOutcome::Unknown,
        Some(other) => panic!("unknown outcome {other}"),
    };
    let mut record = RunRecord::new(case, arm, attempt, outcome);
    if let Some(name) = entry["trajectory"].as_str() {
        record.trajectory = Some(trajectory(name));
    }
    if let Some(created) = entry["created"].as_array() {
        record.created_files = created
            .iter()
            .map(|file| file.as_str().unwrap().to_string())
            .collect();
    }
    if let Some(workspace) = entry["workspace"].as_str() {
        record.workspace = Some(fixtures().join("workspaces").join(workspace));
    }
    if outcome.scored() {
        record.cost_usd = match entry.get("cost_usd") {
            Some(Value::Null) => None,
            Some(value) => value.as_f64(),
            None => manifest["cost_usd"][arm_word].as_f64(),
        };
        record.seconds = entry
            .get("seconds")
            .and_then(Value::as_f64)
            .or_else(|| manifest["seconds"][arm_word].as_f64());
    }
    record
}
