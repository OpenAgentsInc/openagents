//! Asks a local Laya checkpoint whether each held-out prompt line is a
//! shell command or a request (#10693), and prints one JSON line per case
//! with the answer, its confidence, and the warm latency, then a summary.
//!
//! ```text
//! cargo run --release -p laya --example line_kind -- \
//!     ~/work/laya-artifacts/typed-decisions crates/terminal-core/fixtures/line-kinds.json
//! ```
//!
//! The line never leaves the machine: the checkpoint runs in process.

use std::process::ExitCode;
use std::time::{Duration, Instant};

use laya::api::SystemOneRequest;
use laya::decision::DecisionModel;
use serde_json::json;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("usage: line_kind <checkpoint dir> <line-kinds fixture>");
        return ExitCode::from(2);
    }
    let started = Instant::now();
    let model = DecisionModel::load(std::path::Path::new(&args[1]), candle_core::Device::Cpu)
        .expect("checkpoint loads");
    eprintln!("load: {:?}", started.elapsed());
    let fixture: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&args[2]).expect("fixture")).unwrap();
    let mut times: Vec<Duration> = Vec::new();
    let (mut cases, mut right) = (0, 0);
    for case in fixture["cases"].as_array().unwrap() {
        let request: SystemOneRequest = serde_json::from_value(json!({
            "state": {
                "line": case["line"],
                "first_word_is": case["first"],
            },
            "questions": {
                "kind": {
                    "type": "choice",
                    "instructions": "The person typed this line at a shell prompt. Did they type a shell command to run, or a request in plain language for an assistant?",
                    "criteria": {
                        "command": "a shell command to run as typed",
                        "request": "a request in plain language for an assistant",
                    },
                },
            },
        }))
        .unwrap();
        // One warm-up, then the timed call.
        model.system_one(&request).expect("system_one");
        let started = Instant::now();
        let answers = model.system_one(&request).expect("system_one");
        times.push(started.elapsed());
        let choice = &answers["answers"]["kind"];
        let said = if choice["choice"] == "command" {
            "shell"
        } else {
            "ask"
        };
        cases += 1;
        right += u32::from(said == case["kind"].as_str().unwrap());
        println!(
            "{}",
            json!({
                "line": case["line"],
                "kind": case["kind"],
                "said": said,
                "confidence": choice["confidence"],
                "micros": times.last().unwrap().as_micros(),
            })
        );
    }
    times.sort();
    eprintln!(
        "{right}/{cases} right; latency p50 {:?} p99 {:?}",
        times[times.len() / 2],
        times[((times.len() - 1) as f64 * 0.99).round() as usize]
    );
    ExitCode::SUCCESS
}
