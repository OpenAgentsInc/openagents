//! `atif` reads a recorded session and answers what it did: `stats` for
//! the totals, `anomalies` for what repeated, stalled, failed, or was
//! refused.
//!
//! ```sh
//! atif stats ~/.openagents/exports/run.atif.json
//! atif anomalies ~/.openagents/coder-new/solver-runs/8205-…/coder-0-events.jsonl
//! atif stats ~/.openagents/coder-new/solver-runs/8205-…     # a run directory
//! ```

use std::path::Path;
use std::process::ExitCode;

use serde_json::json;

const USAGE: &str = "Usage: atif stats FILE|DIR [--json]
       atif anomalies FILE|DIR [--json]

stats      What the session did: calls per tool, tokens, wall time,
           repeated identical calls, the largest tool results.
anomalies  What went wrong or looped: identical call runs, identical
           failures, refused calls, stalls, outlier prompts, empty turns,
           log faults.

A FILE is a session log (.atif.jsonl), an exported document
(.atif.json), or an SDK event stream (*-events.jsonl). A DIR is a run
folder with a summary.json and events files, as a solver run leaves.";

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let Some(verb) = args.next() else {
        eprintln!("{USAGE}");
        return ExitCode::from(64);
    };
    let Some(target) = args.next() else {
        eprintln!("{USAGE}");
        return ExitCode::from(64);
    };
    let json_out = args.any(|arg| arg == "--json");
    let path = Path::new(&target);
    if path.is_dir() {
        return directory(verb.as_str(), path, json_out);
    }
    let trace = match atif::analysis::read(path) {
        Ok(trace) => trace,
        Err(why) => {
            eprintln!("{why}");
            return ExitCode::FAILURE;
        }
    };
    match verb.as_str() {
        "stats" => {
            if json_out {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&atif::analysis::stats(&trace)).unwrap()
                );
            } else {
                print!("{}", atif::analysis::stats_text(&trace));
            }
            ExitCode::SUCCESS
        }
        "anomalies" => {
            if json_out {
                let found: Vec<serde_json::Value> = atif::analysis::anomalies(&trace)
                    .iter()
                    .map(|a| json!({"kind": a.kind, "step": a.step, "detail": a.detail}))
                    .collect();
                println!("{}", serde_json::to_string_pretty(&found).unwrap());
            } else {
                print!("{}", atif::analysis::anomalies_text(&trace));
            }
            ExitCode::SUCCESS
        }
        _ => {
            eprintln!("{USAGE}");
            ExitCode::from(64)
        }
    }
}

/// A run directory: the summary's hands table plus a per-file rollup of
/// each events file it holds.
fn directory(verb: &str, path: &Path, json_out: bool) -> ExitCode {
    let dir = match atif::analysis::read_dir(path) {
        Ok(dir) => dir,
        Err(why) => {
            eprintln!("{why}");
            return ExitCode::FAILURE;
        }
    };
    match verb {
        "stats" => {
            if json_out {
                let traces: Vec<serde_json::Value> = dir
                    .traces
                    .iter()
                    .map(|(file, trace)| {
                        let mut report = atif::analysis::stats(trace);
                        report["file"] = json!(file.display().to_string());
                        report
                    })
                    .collect();
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({
                        "summary": dir.summary,
                        "traces": traces,
                    }))
                    .unwrap()
                );
            } else {
                print!("{}", atif::analysis::run_dir_text(&dir, path));
            }
            ExitCode::SUCCESS
        }
        "anomalies" => {
            if json_out {
                let mut by_file = serde_json::Map::new();
                for (file, trace) in &dir.traces {
                    let found: Vec<serde_json::Value> = atif::analysis::anomalies(trace)
                        .iter()
                        .map(|a| json!({"kind": a.kind, "step": a.step, "detail": a.detail}))
                        .collect();
                    by_file.insert(file.display().to_string(), json!(found));
                }
                println!("{}", serde_json::to_string_pretty(&by_file).unwrap());
            } else {
                for (file, trace) in &dir.traces {
                    println!("{}:", file.display());
                    print!("{}", atif::analysis::anomalies_text(trace));
                }
            }
            ExitCode::SUCCESS
        }
        _ => {
            eprintln!("{USAGE}");
            ExitCode::from(64)
        }
    }
}
