//! `coder-one checks conformance …`.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use super::{Context, registry};
use crate::checks::contract::host::Local;
use crate::component::jev::{JevMode, Recorded, RecordedAnswer};
use crate::record::Recorder;

/// The conformance commands' usage.
pub const USAGE: &str = "usage: coder-one checks conformance registry [--json]
       coder-one checks conformance run --workspace DIR [--jev off|recorded|live]
                                        [--recorded FILE] [--out FILE]
       coder-one checks conformance offline TASK... --out DIR [--jobs DIR]...
                                        [--tasks DIR] [--image IMAGE]
                                        [--kinds snapshot,final,candidate,reconstruction]
                                        [--trials NAME,...] [--exclude-job TEXT]...
                                        [--jev off|recorded|live] [--recorded FILE]

registry validates the method registry (methods/) and prints each entry
with its digest. run finds the candidate functions in the workspace at
--workspace, has Jev identify the well-known method each implements, or
none, and runs each identified entry's property checks there inside a
writing boundary; it prints the report, or writes it to --out. offline
restores every retained workspace of each TASK that `coder-one accept
offline` reads under each --jobs directory (default:
~/.openagents/terminal-bench/jobs), identifies each distinct function
once, and runs the checks in a fresh container of the task's image with
no network; it writes <out>/<task>/conformance.json and labels.json. Jev
answers come from <out>/jev-recorded.json (or --recorded) where it holds
the request; --jev live asks for the rest and records them there, and
--jev recorded replays only.";

fn modes(word: &str, recorded: &Recorded) -> Result<(JevMode, Option<JevMode>), String> {
    let replay = JevMode::Recorded(recorded.clone());
    match word {
        "off" => Ok((JevMode::Off, None)),
        "recorded" => Ok((replay, None)),
        "live" => {
            let dir = crate::credentials::openagents_dir().ok_or("HOME is not set")?;
            let key = crate::credentials::jev_key(|name| std::env::var(name).ok(), &dir)?;
            let client = crate::credentials::jev_client(&key.secret)?;
            Ok((JevMode::Live(client), Some(replay)))
        }
        other => Err(format!("--jev is off, recorded, or live, not {other}")),
    }
}

/// Adds the live answers among `calls` to `recorded`; returns how many.
fn keep_live(calls: &[Value], source: &str, recorded: &mut Recorded) -> usize {
    let mut added = 0;
    for call in calls {
        if call["how"] != "live" || call["answers"].is_null() {
            continue;
        }
        let Some(key) = call["key"].as_str() else {
            continue;
        };
        recorded.entries.insert(
            key.to_string(),
            RecordedAnswer {
                name: "jev_method_conformance".to_string(),
                model: crate::credentials::JEV_MODEL.to_string(),
                answers: call["answers"].clone(),
                input_tokens: call["input_tokens"].as_u64(),
                output_tokens: call["output_tokens"].as_u64(),
                milliseconds: call["milliseconds"].as_u64(),
                source: format!(
                    "checks conformance: {}",
                    call["task"].as_str().unwrap_or(source)
                ),
            },
        );
        added += 1;
    }
    added
}

fn print_or_write(out: Option<String>, value: &Value) -> Result<(), String> {
    let text = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
    match out {
        Some(path) => {
            if let Some(parent) = Path::new(&path).parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            crate::record::write_atomic(Path::new(&path), format!("{text}\n").as_bytes())
        }
        None => {
            println!("{text}");
            Ok(())
        }
    }
}

/// Runs a conformance command.
///
/// # Errors
///
/// A message for bad arguments or unreadable inputs.
#[allow(clippy::too_many_lines)]
pub async fn command(args: &[String]) -> Result<i32, String> {
    let Some((verb, rest)) = args.split_first() else {
        return Err(USAGE.to_string());
    };
    let mut positional = Vec::new();
    let mut flags: Vec<(String, String)> = Vec::new();
    let mut json_out = false;
    let mut rest = rest.iter();
    while let Some(arg) = rest.next() {
        if arg == "--json" {
            json_out = true;
        } else if arg.starts_with("--") {
            let value = rest.next().ok_or(format!("{arg} needs a value"))?;
            flags.push((arg.clone(), value.clone()));
        } else {
            positional.push(arg.clone());
        }
    }
    let known = [
        "--workspace",
        "--jev",
        "--recorded",
        "--out",
        "--jobs",
        "--tasks",
        "--image",
        "--kinds",
        "--trials",
        "--exclude-job",
    ];
    if let Some((unknown, _)) = flags.iter().find(|(k, _)| !known.contains(&k.as_str())) {
        return Err(format!("unknown option {unknown}\n{USAGE}"));
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
    let jev = one("--jev").unwrap_or_else(|| "recorded".to_string());
    match verb.as_str() {
        "registry" => {
            let registry = registry();
            if json_out {
                let value = json!({
                    "digest": registry.digest,
                    "question_set": super::question_set().digest,
                    "entries": registry.entries.iter().map(|l| json!({
                        "slug": l.entry.slug,
                        "name": l.entry.name,
                        "digest": l.digest,
                        "properties": l.entry.properties.len(),
                        "source_tasks": l.entry.provenance.source_tasks,
                        "admission": l.entry.admission.status,
                    })).collect::<Vec<_>>(),
                });
                println!(
                    "{}",
                    serde_json::to_string_pretty(&value).map_err(|e| e.to_string())?
                );
            } else {
                println!(
                    "methods registry {} ({} entries)",
                    registry.digest,
                    registry.entries.len()
                );
                for l in &registry.entries {
                    println!(
                        "{:30} {} properties  {}  {}",
                        l.entry.slug,
                        l.entry.properties.len(),
                        l.entry.admission.status,
                        l.digest
                    );
                }
            }
            Ok(0)
        }
        "run" => {
            let workspace = PathBuf::from(one("--workspace").ok_or("run needs --workspace")?);
            let recorded_path =
                PathBuf::from(one("--recorded").unwrap_or_else(|| "jev-recorded.json".to_string()));
            let mut recorded = Recorded::load(&recorded_path)?;
            let (mode, replay) = modes(&jev, &recorded)?;
            let host = Local {
                workdir: workspace.clone(),
            };
            let report = super::run_with(
                &mode,
                replay.as_ref(),
                &Recorder::default(),
                &Context {
                    component: super::COMPONENT,
                    id: "jev-conformance".to_string(),
                    deadline: None,
                },
                &workspace,
                &host,
                &workspace.display().to_string(),
            )
            .await;
            if keep_live(&report.jev, "run", &mut recorded) > 0 {
                recorded.save(&recorded_path)?;
            }
            let value = serde_json::to_value(&report).map_err(|e| e.to_string())?;
            print_or_write(one("--out"), &value)?;
            Ok(i32::from(!report.failures.is_empty()))
        }
        "offline" => {
            if positional.is_empty() {
                return Err(format!("offline needs at least one TASK\n{USAGE}"));
            }
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
            let jobs = many("--jobs");
            let options = super::offline::Options {
                jobs: if jobs.is_empty() {
                    vec![home.join("jobs")]
                } else {
                    jobs.into_iter().map(PathBuf::from).collect()
                },
                tasks_dir: one("--tasks").map_or_else(
                    || home.join("upstream/terminal-bench-v4.0.0/tasks"),
                    PathBuf::from,
                ),
                out: out.clone(),
                image: one("--image"),
                only: one("--trials")
                    .map(|t| t.split(',').map(str::to_string).collect())
                    .unwrap_or_default(),
                kinds,
                exclude_jobs: many("--exclude-job"),
            };
            crate::accept::runner::name_containers("method-conformance");
            let recorded_path =
                one("--recorded").map_or_else(|| out.join("jev-recorded.json"), PathBuf::from);
            let mut recorded = Recorded::load(&recorded_path)?;
            let mut failed = 0;
            for task in &positional {
                let (mode, replay) = modes(&jev, &recorded)?;
                match super::offline::task(task, &mode, replay.as_ref(), &options).await {
                    Ok(calls) => {
                        if keep_live(&calls, task, &mut recorded) > 0 {
                            recorded.save(&recorded_path)?;
                        }
                    }
                    Err(error) => {
                        failed += 1;
                        crate::say::line(&format!("conformance ▸ {task}: {error}"));
                    }
                }
            }
            Ok(i32::from(failed > 0))
        }
        _ => Err(USAGE.to_string()),
    }
}
