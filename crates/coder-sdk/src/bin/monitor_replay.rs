//! Replays recorded Claude Code sessions through the monitor (#11488) and
//! shows where it would have fired.
//!
//! ```text
//! cargo run -p coder-sdk --bin monitor-replay -- [--live] PATH...
//! ```
//!
//! Each PATH is a Claude Code session log (`.jsonl`, as under
//! `~/.claude/projects/`) or a directory searched for them. Every tool
//! result becomes the `PostToolUse` call the backend would fire. Without
//! `--live` only the rule runs: a line per suspect window the monitor would
//! put to the Decisions API. With `--live` each such window is asked
//! (`openagents/decide`, Clef first) and the action it picks is shown.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use coder_sdk::decide::Decider;
use coder_sdk::hook_log::{Decision, How};
use coder_sdk::system_one::monitor;
use coder_sdk::{Call, DecisionHook, Event, Timing};
use serde_json::{Value, json};

/// Answers nothing: rule-only replays.
struct Offline;

impl Decider for Offline {
    fn decide(
        &self,
        _: jev::decisions::DecisionsRequest,
    ) -> futures::future::BoxFuture<'static, Result<jev::decisions::DecisionsResponse, String>>
    {
        Box::pin(async { Err("offline replay: the rule only".to_owned()) })
    }
}

fn logs(path: &Path, out: &mut Vec<PathBuf>) {
    if path.is_dir() {
        let Ok(entries) = std::fs::read_dir(path) else {
            return;
        };
        let mut entries: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
        entries.sort();
        for entry in entries {
            logs(&entry, out);
        }
    } else if path.extension().is_some_and(|ext| ext == "jsonl") {
        out.push(path.to_owned());
    }
}

fn text_of(content: &Value) -> String {
    match content {
        Value::String(text) => text.clone(),
        Value::Array(parts) => parts
            .iter()
            .filter_map(|part| part["text"].as_str())
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

/// The `PostToolUse` calls a session log holds, in order.
fn calls(path: &Path) -> Vec<Call> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let mut uses: HashMap<String, (String, Value)> = HashMap::new();
    let mut out = Vec::new();
    for line in text.lines() {
        let Ok(entry) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let Some(parts) = entry["message"]["content"].as_array() else {
            continue;
        };
        for part in parts {
            match part["type"].as_str() {
                Some("tool_use") => {
                    uses.insert(
                        part["id"].as_str().unwrap_or_default().to_owned(),
                        (
                            part["name"].as_str().unwrap_or_default().to_owned(),
                            part["input"].clone(),
                        ),
                    );
                }
                Some("tool_result") => {
                    let id = part["tool_use_id"].as_str().unwrap_or_default();
                    let Some((tool, input)) = uses.remove(id) else {
                        continue;
                    };
                    out.push(Call::tool(
                        Event::PostToolUse,
                        &tool,
                        json!({
                            "tool_input": input,
                            "tool_response": text_of(&part["content"]),
                            "failed": part["is_error"].as_bool().unwrap_or(false),
                        }),
                    ));
                }
                _ => {}
            }
        }
    }
    out
}

fn main() {
    let mut live = false;
    let mut paths = Vec::new();
    for arg in std::env::args().skip(1) {
        if arg == "--live" {
            live = true;
        } else {
            logs(Path::new(&arg), &mut paths);
        }
    }
    if paths.is_empty() {
        eprintln!("usage: monitor-replay [--live] PATH...");
        std::process::exit(2);
    }
    let decider: Arc<dyn Decider> = if live {
        Arc::new(coder_sdk::decide::Api)
    } else {
        Arc::new(Offline)
    };
    let mut totals: BTreeMap<String, usize> = BTreeMap::new();
    let (mut sessions, mut tool_calls, mut fired_sessions) = (0usize, 0usize, 0usize);
    for path in &paths {
        let calls = calls(path);
        if calls.is_empty() {
            continue;
        }
        sessions += 1;
        tool_calls += calls.len();
        let lines: Arc<Mutex<Vec<(usize, Decision)>>> = Arc::default();
        let at = Arc::new(Mutex::new(0usize));
        let recorder = {
            let (lines, at) = (lines.clone(), at.clone());
            Arc::new(move |line: &Decision| {
                let index = *at.lock().unwrap();
                lines.lock().unwrap().push((index, line.clone()));
            })
        };
        let hook = DecisionHook {
            timing: Timing::Blocking,
            ..monitor::hook(true, None, decider.clone(), recorder)
        };
        for (index, call) in calls.iter().enumerate() {
            *at.lock().unwrap() = index + 1;
            let _ = hook.fire(call);
        }
        let lines = lines.lock().unwrap();
        if lines.is_empty() {
            continue;
        }
        fired_sessions += 1;
        println!("{} ({} tool calls)", path.display(), calls.len());
        for (index, line) in lines.iter() {
            let action = match line.how {
                How::Fallback if !live => "would ask".to_owned(),
                _ => line.action.clone(),
            };
            *totals.entry(action.clone()).or_default() += 1;
            let features = &line.detail["memo"]["features"];
            println!(
                "  call {index:>3}: {action:<15} p={} repeated_failures={} repeated_calls={} since_edit={} | {}",
                line.p.map_or_else(|| "-".to_owned(), |p| format!("{p:.2}")),
                features["repeated_failures"],
                features["repeated_commands"],
                features["calls_since_edit"],
                line.detail["memo"]["evidence"].as_str().unwrap_or_default(),
            );
        }
    }
    println!(
        "\n{sessions} sessions, {tool_calls} tool calls; the monitor would have asked in {fired_sessions} sessions: {totals:?}"
    );
}
