//! `coder-one checks oracle …`.

use std::path::{Path, PathBuf};
use std::time::Duration;

use super::offline::{self, Writing};
use super::write::Bounds;
use crate::component::jev::{JevMode, Recorded, RecordedAnswer};

/// The oracle commands' usage.
pub const USAGE: &str = "usage: coder-one checks oracle offline TASK... --out DIR [--jobs DIR] [--tasks DIR]
                                     [--image IMAGE] [--grades DIR]... [--kinds snapshot,final,candidate,reconstruction]
                                     [--trials NAME,...] [--exclude-job TEXT]... [--workers N]
                                     [--jev off|recorded|live] [--recorded FILE]
                                     [--write off|luna] [--budget-usd N] [--session-usd N]
                                     [--writer-turns N] [--writer-sec N] [--effort LEVEL] [--reuse-spec]

offline finds or writes each task's oracle in a fresh container of its
image and runs it on every retained workspace that `coder-one accept
offline` reads, each in its own container with the network off. Code looks
for a checker the instruction names first. When there's none, Jev picks the
task's stated definition, parameters, and boundary inputs (recorded in
<out>/jev-recorded.json, or --recorded, and replayed from it), and with
--write luna a Luna session that sees only that spec writes oracle.py. An
oracle already in <out>/<task>/oracle.json for the same spec is reused, so
a rerun makes no Luna call. --budget-usd bounds Luna across all tasks
(default 1.00), --session-usd each session (default 0.08). It writes
<out>/<task>/find.json, spec.json, oracle.json, oracle/, writer.json,
results.json, and labels.json. Containers are named oracle-9656-*.";

/// Runs an oracle command.
///
/// # Errors
///
/// A message for bad arguments or unreadable inputs.
#[allow(clippy::too_many_lines)]
pub async fn command(args: &[String]) -> Result<i32, String> {
    let Some((verb, rest)) = args.split_first() else {
        return Err(USAGE.to_string());
    };
    if verb != "offline" {
        return Err(USAGE.to_string());
    }
    let mut positional = Vec::new();
    let mut flags: Vec<(String, String)> = Vec::new();
    let mut reuse = false;
    let mut rest = rest.iter();
    while let Some(arg) = rest.next() {
        if arg == "--reuse-spec" {
            reuse = true;
        } else if arg.starts_with("--") {
            let value = rest.next().ok_or(format!("{arg} needs a value"))?;
            flags.push((arg.clone(), value.clone()));
        } else {
            positional.push(arg.clone());
        }
    }
    let one = |name: &str| {
        flags
            .iter()
            .rev()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.clone())
    };
    let many = |name: &str| {
        flags
            .iter()
            .filter(|(k, _)| k == name)
            .map(|(_, v)| v.clone())
            .collect::<Vec<_>>()
    };
    let known = [
        "--out",
        "--jobs",
        "--tasks",
        "--image",
        "--kinds",
        "--trials",
        "--exclude-job",
        "--workers",
        "--jev",
        "--recorded",
        "--write",
        "--budget-usd",
        "--session-usd",
        "--writer-turns",
        "--writer-sec",
        "--effort",
        "--grades",
    ];
    if let Some((unknown, _)) = flags.iter().find(|(k, _)| !known.contains(&k.as_str())) {
        return Err(format!("unknown option {unknown}\n{USAGE}"));
    }
    if positional.is_empty() {
        return Err(format!("offline needs at least one TASK\n{USAGE}"));
    }
    let number = |name: &str, default: f64| -> Result<f64, String> {
        one(name).map_or(Ok(default), |v| {
            v.parse::<f64>()
                .ok()
                .filter(|n| *n >= 0.0)
                .ok_or(format!("{name} is a number, not {v}"))
        })
    };
    let out = PathBuf::from(one("--out").ok_or("offline needs --out")?);
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or("HOME is not set")?
        .join(".openagents/terminal-bench");
    let kinds = match one("--kinds") {
        Some(list) => list
            .split(',')
            .map(crate::accept::offline::Kind::parse)
            .collect::<Result<Vec<_>, _>>()?,
        None => Vec::new(),
    };
    let options = crate::checks::contract::offline::Options {
        jobs: one("--jobs").map_or_else(|| home.join("jobs"), PathBuf::from),
        tasks_dir: one("--tasks").map_or_else(
            || home.join("upstream/terminal-bench-v4.0.0/tasks"),
            PathBuf::from,
        ),
        out: out.clone(),
        image: one("--image"),
        grades: many("--grades").into_iter().map(PathBuf::from).collect(),
        reconstructions: Vec::new(),
        only: one("--trials")
            .map(|t| t.split(',').map(str::to_string).collect())
            .unwrap_or_default(),
        kinds,
        exclude_jobs: many("--exclude-job"),
        workers: one("--workers").and_then(|w| w.parse().ok()).unwrap_or(2),
        reuse_plan: reuse,
    };
    let writing = Writing {
        luna: match one("--write").as_deref() {
            None | Some("off") => false,
            Some("luna") => true,
            Some(other) => return Err(format!("--write is off or luna, not {other}")),
        },
        bounds: Bounds {
            turns: number("--writer-turns", 30.0)? as usize,
            wall: Duration::from_secs(number("--writer-sec", 600.0)? as u64),
            usd: number("--session-usd", 0.08)?,
            effort: Some(one("--effort").unwrap_or_else(|| "high".to_string())),
        },
        budget_usd: number("--budget-usd", 1.0)?,
    };
    let jev = one("--jev").unwrap_or_else(|| "recorded".to_string());
    let recorded_path =
        one("--recorded").map_or_else(|| out.join("jev-recorded.json"), PathBuf::from);
    let mut recorded = Recorded::load(&recorded_path)?;
    let spent_path = out.join("luna-spend.json");
    let mut spent: serde_json::Map<String, serde_json::Value> =
        std::fs::read_to_string(&spent_path)
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default();
    let mut failed = 0;
    for task in &positional {
        let total: f64 = spent.values().filter_map(serde_json::Value::as_f64).sum();
        let replay = JevMode::Recorded(recorded.clone());
        let mode = match jev.as_str() {
            "off" => JevMode::Off,
            "recorded" => replay.clone(),
            "live" => {
                let dir = crate::credentials::openagents_dir().ok_or("HOME is not set")?;
                let key = crate::credentials::jev_key(|name| std::env::var(name).ok(), &dir)?;
                JevMode::Live(crate::credentials::jev_client(&key.secret)?)
            }
            other => return Err(format!("--jev is off, recorded, or live, not {other}")),
        };
        match offline::task(task, &mode, Some(&replay), &options, &writing, total).await {
            Ok(usd) => {
                if usd > 0.0 {
                    let before = spent
                        .get(task)
                        .and_then(serde_json::Value::as_f64)
                        .unwrap_or(0.0);
                    spent.insert(task.clone(), serde_json::json!(before + usd));
                    write(&spent_path, &spent)?;
                }
                let mut added = 0;
                for (key, call) in offline::recorded_from(&out, std::slice::from_ref(task)) {
                    if recorded.entries.contains_key(&key) {
                        continue;
                    }
                    recorded.entries.insert(
                        key,
                        RecordedAnswer {
                            name: "jev_oracle_define".to_string(),
                            model: crate::credentials::JEV_MODEL.to_string(),
                            answers: call["answers"].clone(),
                            input_tokens: call["input_tokens"].as_u64(),
                            output_tokens: call["output_tokens"].as_u64(),
                            milliseconds: call["milliseconds"].as_u64(),
                            source: format!("checks oracle: {task}"),
                        },
                    );
                    added += 1;
                }
                if added > 0 {
                    recorded.save(&recorded_path)?;
                }
            }
            Err(error) => {
                failed += 1;
                crate::say::line(&format!("oracle ▸ {task}: {error}"));
            }
        }
    }
    Ok(i32::from(failed > 0))
}

fn write(path: &Path, value: &impl serde::Serialize) -> Result<(), String> {
    let text = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
    crate::record::write_atomic(path, format!("{text}\n").as_bytes())
}
