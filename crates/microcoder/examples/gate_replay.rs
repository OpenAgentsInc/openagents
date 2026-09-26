//! Replays the end-of-run gates on retained Microcoder runs, offline.
//!
//! ```sh
//! cargo run --release -p microcoder --example gate_replay -- RUN_DIR...
//! ```
//!
//! For each run directory, it rebuilds the state the run ended in (the task,
//! the frozen tests, and the model's rationales), asks each Jev-backed gate
//! in [`microcoder::gate`] on its own (`requirements`, `target`, and
//! `credible`), and prints one JSON line per run: the run's ending and
//! reward, and whether each gate would have sent it back. No model is
//! called and no container runs; only Jev is billed. Runs that didn't end
//! with every frozen test passing, or froze no tests, are skipped.

use microcoder::gate::{GateState, Gates, Used, check};
use microcoder::models::JevJudge;
use microcoder::state::{Action, State, Test};

fn jev_client() -> Result<jev::Client, String> {
    let from_env = std::env::var("TYPESAFE_API_KEY")
        .ok()
        .filter(|k| !k.trim().is_empty());
    let key = from_env.or_else(|| {
        let path = std::path::PathBuf::from(std::env::var_os("HOME")?).join(".openagents/jev.json");
        let value: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()?;
        value["api_key"].as_str().map(str::to_string)
    });
    let key = key.ok_or("no Jev key")?;
    jev::Client::new(jev::Config::new().api_key(key.trim())).map_err(|e| e.to_string())
}

/// The state a run ended in, or why it's skipped.
fn end_state(dir: &std::path::Path) -> Result<(State, serde_json::Value), String> {
    let summary: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(dir.join("summary.json")).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let ending = summary["outcome"]["ending"]["reason"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    if ending != "finished" && ending != "tests_held" {
        return Err(format!("ended by {ending}"));
    }
    let tests: Vec<Test> = summary["acceptance_tests"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|t| Test {
                    name: t["name"].as_str().unwrap_or_default().to_string(),
                    script: t["script"].as_str().unwrap_or_default().to_string(),
                    passed_at_freeze: t["passed_at_freeze"].as_bool(),
                })
                .collect()
        })
        .unwrap_or_default();
    if tests.is_empty() {
        return Err("no frozen tests".to_string());
    }
    let name = summary["task"].as_str().unwrap_or_default();
    let task = microcoder::tbench::find(&microcoder::tbench::tasks_dir(), name)?;
    let mut actions = Vec::new();
    let events = std::fs::read_to_string(dir.join("events.jsonl")).map_err(|e| e.to_string())?;
    for line in events.lines() {
        let Ok(event) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if event["event"] == "generated"
            && let Some(rationale) = event["generated"]["action"]["Ok"]["rationale"].as_str()
        {
            actions.push(Action {
                step: event["step"].as_u64().unwrap_or(0) as usize,
                rationale: rationale.to_string(),
                results: Vec::new(),
                skipped: Vec::new(),
            });
        }
    }
    let state = State {
        task: task.instruction.clone(),
        tests,
        actions,
        ..State::default()
    };
    let facts = serde_json::json!({
        "run": dir.file_name().map(|n| n.to_string_lossy().to_string()),
        "task": name,
        "ending": ending,
        "reward": summary["reward"],
    });
    Ok((state, facts))
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let judge = JevJudge {
        client: jev_client().expect("a Jev key"),
    };
    let one = |f: fn(&mut Gates)| {
        let mut gates = Gates::default();
        f(&mut gates);
        gates
    };
    let gates = [
        ("requirements", one(|g| g.requirements = true)),
        ("target", one(|g| g.target = true)),
        ("credible", one(|g| g.credible = true)),
    ];
    for dir in std::env::args().skip(1) {
        let dir = std::path::PathBuf::from(dir);
        let (state, mut facts) = match end_state(&dir) {
            Ok(found) => found,
            Err(why) => {
                eprintln!("skipped {}: {why}", dir.display());
                continue;
            }
        };
        for (name, gate) in &gates {
            let (checks, note) = check(
                gate,
                &mut GateState::default(),
                &judge,
                &state,
                Used {
                    time: 0.0,
                    spend: 0.0,
                },
                "/tmp/acceptance",
            )
            .await;
            facts[*name] = serde_json::json!({
                "refused": note.is_some(),
                "detail": checks.iter().flat_map(|c| c.detail.clone()).collect::<Vec<_>>(),
                "answers": checks.iter().flat_map(|c| c.judgments.iter().flat_map(|j| j.answers.clone())).collect::<Vec<_>>(),
                "errors": checks.iter().flat_map(|c| c.judgments.iter().filter_map(|j| j.error.clone())).collect::<Vec<_>>(),
            });
        }
        println!("{facts}");
    }
}
