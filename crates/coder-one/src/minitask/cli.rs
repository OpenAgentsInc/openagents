//! `coder-one minitask …`: run an episode on a mini-task.

use std::path::PathBuf;
use std::time::Duration;

use serde_json::json;

use super::run::{ExecutorChoice, Options, default_runs_dir, run};
use super::{CATALOG, catalog_json, find};
use crate::delegate::Agent;
use crate::scripted::Script;
use crate::session::Controls;

/// The mini-task commands' usage.
pub const USAGE: &str = "usage: coder-one minitask list [--json]
       coder-one minitask setup ID DIR
       coder-one minitask grade ID DIR [--json]
       coder-one minitask run ID [--executor scripted|claude-code|codex|microluna]
                                 [--microluna single|requirements]
                                 [--script good|bad|FILE] [--model MODEL]
                                 [--jev off|live] [--speed X] [--deadline SECONDS]
                                 [--controls FILE] [--no-checks] [--monitor]
                                 [--out DIR] [--json]
                                 [--repair PROFILE [--repair-brief packet|plain]
                                  [--repair-trigger detected|checked|always]]

run sets the task up in a scratch directory, runs one episode with no
explore steps, and grades it. The scripted executor (the default) plays the
task's good or bad script, or a script file, on virtual time unless --speed
gives real milliseconds per scripted one. claude-code and codex run the real
CLI inside a coder-boundary filesystem boundary; codex with --model
gpt-6-luna is the Luna arm. microluna runs Microluna in this process on the
Codex login, with each command inside a coder-boundary boundary: --microluna
requirements (the default) runs short sessions one requirement group at a
time, with the checks and a Jev move between them, and --microluna single
runs one session on the briefing. --controls names a JSON file of session
controls (deadline_ms, tick_ms, steer, stop_when, resume) for the scripted
executor. verify.checks observes the workspace before the grader runs
unless --no-checks is given. --monitor watches the session with
control.monitor in shadow mode: its rules, and Jev with --jev live.
--repair runs one verify.repair session after the checks when one
contradicted a requirement (or on every candidate with --repair-trigger
always), within what is left of --deadline, from the diagnostic packets
(or, with --repair-brief plain, from the task alone), then reruns the
checks. PROFILE is a scripted repair profile (fix, fix-if-packet,
claim-only, break) or AGENT:MODEL, such as codex:gpt-6-luna. Runs record
under ~/.openagents/coder-one/minitasks unless --out names another
directory. The exit code is 0 when the grader passed.

setup writes the task's files to DIR and its instruction to
DIR.instruction.md, so `coder-one episode run` can run any policy manifest
on it from DIR; grade grades what DIR holds.";

/// Runs a mini-task command and returns the exit code.
///
/// # Errors
///
/// Returns a message for bad arguments or a run that can't start.
pub async fn command(args: &[String]) -> Result<i32, String> {
    let Some((verb, rest)) = args.split_first() else {
        return Err(USAGE.to_string());
    };
    let mut positional = Vec::new();
    let mut executor = "scripted".to_string();
    let mut script = "good".to_string();
    let mut model = None;
    let mut jev = "off".to_string();
    let mut speed = 0.0;
    let mut deadline = 600u64;
    let mut controls = None;
    let mut out = None;
    let mut json_output = false;
    let mut checks = true;
    let mut monitor = false;
    let mut repair = None;
    let mut repair_brief = "packet".to_string();
    let mut repair_trigger = "detected".to_string();
    let mut microluna_mode = "requirements".to_string();
    let mut iter = rest.iter();
    while let Some(arg) = iter.next() {
        let mut value = |name: &str| {
            iter.next()
                .cloned()
                .ok_or_else(|| format!("{name} needs a value"))
        };
        match arg.as_str() {
            "--executor" => executor = value("--executor")?,
            "--script" => script = value("--script")?,
            "--model" => model = Some(value("--model")?),
            "--jev" => jev = value("--jev")?,
            "--speed" => {
                speed = value("--speed")?
                    .parse()
                    .map_err(|_| "--speed takes a number")?;
            }
            "--deadline" => {
                deadline = value("--deadline")?
                    .parse()
                    .map_err(|_| "--deadline takes whole seconds")?;
            }
            "--controls" => controls = Some(PathBuf::from(value("--controls")?)),
            "--out" => out = Some(PathBuf::from(value("--out")?)),
            "--json" => json_output = true,
            "--no-checks" => checks = false,
            "--monitor" => monitor = true,
            "--repair" => repair = Some(value("--repair")?),
            "--repair-brief" => repair_brief = value("--repair-brief")?,
            "--repair-trigger" => repair_trigger = value("--repair-trigger")?,
            "--microluna" => microluna_mode = value("--microluna")?,
            other if other.starts_with("--") => return Err(format!("unknown option {other}")),
            other => positional.push(other.to_string()),
        }
    }
    match verb.as_str() {
        "list" => {
            if json_output {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({ "tasks": catalog_json() }))
                        .map_err(|error| error.to_string())?
                );
            } else {
                for task in CATALOG {
                    println!("{:<22} {}", task.id, task.family);
                }
            }
            Ok(0)
        }
        // A task's files in a directory, and its instruction beside them, so
        // `coder-one episode run` can run any policy on it; `grade` then
        // grades what the episode left.
        "setup" | "grade" => {
            let [id, dir] = positional.as_slice() else {
                return Err(format!(
                    "minitask {verb} needs a task ID and a directory\n{USAGE}"
                ));
            };
            let task = find(id)?;
            let dir = std::path::Path::new(dir);
            if verb == "setup" {
                crate::minitask::setup(&task, dir)?;
                let instruction = dir.with_extension("instruction.md");
                std::fs::write(&instruction, task.instruction)
                    .map_err(|error| format!("{}: {error}", instruction.display()))?;
                println!("{}", instruction.display());
                return Ok(0);
            }
            let scratch = dir.with_extension("grader");
            let grade = crate::minitask::grade(&task, dir, &scratch).await;
            let _ = std::fs::remove_dir_all(&scratch);
            if json_output {
                println!(
                    "{}",
                    serde_json::to_string(&grade).map_err(|error| error.to_string())?
                );
            } else {
                println!("{}: {}", grade.verdict, grade.detail);
            }
            Ok(i32::from(grade.verdict != "passed"))
        }
        "run" => {
            let [id] = positional.as_slice() else {
                return Err(format!("minitask run needs one task ID\n{USAGE}"));
            };
            let task = find(id)?;
            let choice = match executor.as_str() {
                "scripted" => {
                    let custom = if script == "good" || script == "bad" {
                        None
                    } else {
                        Some(Script::load(std::path::Path::new(&script))?)
                    };
                    ExecutorChoice::Scripted {
                        variant: if custom.is_some() {
                            "custom".to_string()
                        } else {
                            script.clone()
                        },
                        script: custom,
                    }
                }
                "microluna" => ExecutorChoice::Microluna {
                    model: model
                        .clone()
                        .unwrap_or_else(|| Agent::Microluna.default_model().to_string()),
                    policy: crate::micro::Policy {
                        mode: match microluna_mode.as_str() {
                            "single" => crate::micro::Mode::Single,
                            "requirements" => crate::micro::Mode::Requirements,
                            other => {
                                return Err(format!(
                                    "--microluna takes single or requirements, not {other}"
                                ));
                            }
                        },
                        session_sec: deadline.min(600),
                        ..crate::micro::Policy::default()
                    },
                },
                word => {
                    let agent = Agent::parse(word)?;
                    ExecutorChoice::Cli {
                        agent,
                        model: model
                            .clone()
                            .unwrap_or_else(|| agent.default_model().to_string()),
                    }
                }
            };
            let jev = match jev.as_str() {
                "off" => None,
                "live" => {
                    let dir = crate::credentials::openagents_dir()
                        .unwrap_or_else(|| PathBuf::from("/nonexistent"));
                    let env = |name: &str| {
                        std::env::var(name)
                            .ok()
                            .map(|value| value.trim().to_string())
                            .filter(|value| !value.is_empty())
                    };
                    let key = crate::credentials::jev_key(env, &dir)
                        .map_err(|error| format!("--jev live needs a Jev key: {error}"))?;
                    Some(crate::credentials::jev_client(&key.secret)?)
                }
                other => return Err(format!("--jev takes off or live, not {other}")),
            };
            let controls = match controls {
                Some(path) => serde_json::from_str::<Controls>(
                    &std::fs::read_to_string(&path)
                        .map_err(|error| format!("cannot read {}: {error}", path.display()))?,
                )
                .map_err(|error| format!("{} is not session controls: {error}", path.display()))?,
                None => Controls::default(),
            };
            let out = out
                .or_else(default_runs_dir)
                .ok_or("no --out and no HOME to record under")?;
            let repair = match repair {
                None => None,
                Some(profile) => Some(super::run::Repair {
                    profile: crate::repair::Profile::parse(&profile)?,
                    policy: crate::repair::Policy {
                        kind: match repair_brief.as_str() {
                            "packet" => crate::repair::BriefKind::Packet,
                            "plain" => crate::repair::BriefKind::Plain,
                            other => {
                                return Err(format!(
                                    "--repair-brief takes packet or plain, not {other}"
                                ));
                            }
                        },
                        trigger: crate::repair::Trigger::parse(&repair_trigger)?,
                        allowance: Duration::from_secs(deadline),
                    },
                }),
            };
            let ran = run(Options {
                task,
                executor: choice,
                out,
                jev,
                speed,
                deadline: Duration::from_secs(deadline),
                controls,
                checks,
                brief: None,
                monitor: monitor.then(crate::monitor::Params::default),
                repair,
            })
            .await?;
            if json_output {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({
                        "dir": ran.dir.display().to_string(),
                        "manifest": ran.manifest,
                    }))
                    .map_err(|error| error.to_string())?
                );
            } else {
                println!(
                    "\n── mini-task {} · {} · {} ──\n{}: {}\nrecorded in {} ({} ms)",
                    task.id,
                    ran.manifest["executor"]["label"]
                        .as_str()
                        .unwrap_or_default(),
                    ran.manifest["outcome"].as_str().unwrap_or_default(),
                    ran.grade.verdict,
                    ran.grade.detail,
                    ran.dir.display(),
                    ran.milliseconds
                );
            }
            Ok(i32::from(ran.grade.verdict != "passed"))
        }
        _ => Err(USAGE.to_string()),
    }
}
