//! `coder-one accept …`: define, run, and check acceptance suites, run the
//! mini-task loop, and measure offline validity.

use std::path::{Path, PathBuf};

use serde_json::json;

use super::minitask::{LoopOptions, default_out, run_minitask};
use super::offline::{self, TaskOptions};
use super::{AcceptanceSuite, Docker, Local, run};
use crate::component::jev::JevMode;

/// The accept commands' usage.
pub const USAGE: &str = "usage: coder-one accept minitask ID [--sessions N] [--rounds N]
                                  [--jev live|off] [--out DIR] [--echo]
       coder-one accept offline TASK [--trials NAME,…] [--image IMAGE]
                                  [--rounds N] [--jev live|off] [--out DIR]
                                  [--reuse] [--echo]
       coder-one accept validity DIR [--rows FILE] [--json]
       coder-one accept run RECORD WORKSPACE [--docker IMAGE --workdir DIR
                                  [--candidate DIR]] [--json]
       coder-one accept check RECORD

minitask sets a mini-task up, extracts its requirement map with Jev,
writes and freezes an acceptance suite with Microluna (accept.define),
then runs Microluna edit sessions until the suite is green or the
sessions run out, and grades the result with the task's own grader.

offline writes a suite for a Terminal-Bench task from its instruction, in
its warm environment image, and runs it against every retained trial's
post-executor snapshot. validity joins the offline records under DIR with
the check-truth label rows and prints how often a green suite, today's
checks, and the combined verdict agree with the verifier.

run runs a frozen suite, from its record, on a workspace on this host
(inside a coder-boundary writing boundary) or in a Docker image. check
reports whether the suite was edited since its freeze; the exit code is 1
when it was.";

fn jev_mode(word: &str) -> Result<JevMode, String> {
    match word {
        "live" => Ok(JevMode::Live(crate::component::cli::live_client()?)),
        "off" => Ok(JevMode::Off),
        other => Err(format!("--jev takes live or off, not {other}")),
    }
}

/// Runs an accept command and returns the exit code.
///
/// # Errors
///
/// A message for bad arguments or a run that can't start.
#[allow(clippy::too_many_lines)]
pub async fn command(args: &[String]) -> Result<i32, String> {
    let Some((verb, rest)) = args.split_first() else {
        return Err(USAGE.to_string());
    };
    let mut positional = Vec::new();
    let mut sessions = 4u32;
    let mut rounds = None;
    let mut jev = "live".to_string();
    let mut out = None;
    let mut echo = false;
    let mut json_output = false;
    let mut trials = Vec::new();
    let mut image = None;
    let mut reuse = false;
    let mut rows = None;
    let mut docker_image = None;
    let mut workdir = "/app".to_string();
    let mut candidate = None;
    let mut iter = rest.iter();
    while let Some(arg) = iter.next() {
        let mut value = |name: &str| {
            iter.next()
                .cloned()
                .ok_or_else(|| format!("{name} needs a value"))
        };
        match arg.as_str() {
            "--sessions" => {
                sessions = value("--sessions")?
                    .parse()
                    .map_err(|_| "--sessions takes a count")?;
            }
            "--rounds" => {
                rounds = Some(
                    value("--rounds")?
                        .parse::<u32>()
                        .map_err(|_| "--rounds takes a count")?,
                );
            }
            "--jev" => jev = value("--jev")?,
            "--out" => out = Some(PathBuf::from(value("--out")?)),
            "--echo" => echo = true,
            "--json" => json_output = true,
            "--trials" => {
                trials = value("--trials")?.split(',').map(str::to_string).collect();
            }
            "--image" => image = Some(value("--image")?),
            "--reuse" => reuse = true,
            "--rows" => rows = Some(PathBuf::from(value("--rows")?)),
            "--docker" => docker_image = Some(value("--docker")?),
            "--workdir" => workdir = value("--workdir")?,
            "--candidate" => candidate = Some(PathBuf::from(value("--candidate")?)),
            other if other.starts_with("--") => return Err(format!("unknown option {other}")),
            other => positional.push(other.to_string()),
        }
    }
    match verb.as_str() {
        "minitask" => {
            let [id] = positional.as_slice() else {
                return Err(USAGE.to_string());
            };
            let out = out
                .or_else(|| default_out(id))
                .ok_or("no home directory for the run; pass --out")?;
            let mut options = LoopOptions {
                sessions,
                echo,
                ..LoopOptions::default()
            };
            if let Some(rounds) = rounds {
                options.define.max_rounds = rounds;
            }
            let result = run_minitask(id, &out, &jev_mode(&jev)?, &options).await?;
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "task": result["task"],
                    "suite": result["suite"]["headline"],
                    "runs": result["runs"],
                    "stopped": result["stopped"],
                    "green": result["green"],
                    "grade": result["grade"],
                    "green_agrees_with_grader": result["green_agrees_with_grader"],
                    "spend_usd": result["spend_usd"],
                    "record": out.join("result.json"),
                }))
                .unwrap_or_default()
            );
            Ok(if result["grade"]["verdict"] == "passed" {
                0
            } else {
                1
            })
        }
        "offline" => {
            let [task] = positional.as_slice() else {
                return Err(USAGE.to_string());
            };
            let home = crate::credentials::openagents_dir()
                .ok_or("no home directory; pass --out")?
                .join("terminal-bench");
            let mut options = TaskOptions {
                jobs: home.join("jobs"),
                tasks_dir: home.join("upstream/terminal-bench-v4.0.0/tasks"),
                out: out.unwrap_or_else(|| home.join("accept-offline")),
                image,
                reuse,
                only: trials,
                define: super::Options::default(),
                model: "gpt-6-luna".to_string(),
                writer_turns: 50,
                writer_sec: 900,
                echo,
            };
            if let Some(rounds) = rounds {
                options.define.max_rounds = rounds;
            }
            let value = offline::task(task, &jev_mode(&jev)?, &options).await?;
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "task": value["task"],
                    "suite": value["suite"]["headline"],
                    "writer_usd": value["suite"]["writer_usd"],
                    "jev_usd": value["suite"]["jev_usd"],
                    "trials": value["trials"].as_array().map(|t| t.iter().map(|e| json!({
                        "trial": e["trial"]["trial"],
                        "reward": e["trial"]["reward"],
                        "snapshot_graded": e["trial"]["snapshot_graded"],
                        "passed": e["run"]["passed"],
                        "total": e["run"]["total"],
                        "error": e["error"],
                    })).collect::<Vec<_>>()),
                }))
                .unwrap_or_default()
            );
            Ok(0)
        }
        "validity" => {
            let [dir] = positional.as_slice() else {
                return Err(USAGE.to_string());
            };
            let rows = rows.unwrap_or_else(|| {
                PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/truth/rows.jsonl")
            });
            let joined = offline::join(Path::new(dir), &rows)?;
            let value = offline::validity(&joined);
            if json_output {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&value).unwrap_or_default()
                );
            } else {
                for (set, label) in [
                    ("snapshot_graded", "trials whose snapshot was graded"),
                    ("all_snapshots", "every trial with a snapshot"),
                ] {
                    println!("{label}:");
                    for signal in ["suite_green", "todays_checks", "combined_verdict"] {
                        let a = &value[set][signal];
                        println!(
                            "  {signal:<17} spoke {}/{}, agreed {}, fail right {}/{}, pass right {}/{}, failures caught {}/{}",
                            a["spoke"],
                            a["trials"],
                            a["agreed"],
                            a["fail_right"],
                            a["fail_called"],
                            a["pass_right"],
                            a["pass_called"],
                            a["fail_right"],
                            a["failures"]
                        );
                    }
                }
                for j in &joined {
                    println!(
                        "{:<44} reward {:<4} graded {:<5} suite {:<5} ({}/{}) checks {:<5} verdict {}",
                        j.trial,
                        j.reward.map_or("?".to_string(), |r| r.to_string()),
                        j.snapshot_graded,
                        word(j.suite),
                        j.passed,
                        j.total,
                        word(j.checks),
                        word(j.verdict)
                    );
                }
            }
            Ok(0)
        }
        "run" => {
            let [record, workspace] = positional.as_slice() else {
                return Err(USAGE.to_string());
            };
            let suite = AcceptanceSuite::load(Path::new(record))?;
            let workspace = PathBuf::from(workspace);
            let result = match docker_image {
                Some(image) => {
                    let runner = Docker {
                        image,
                        workdir: workdir.clone(),
                        candidate,
                        test_sec: 120,
                        dev: None,
                    };
                    run(&suite, &workspace, &runner, None, "cli").await
                }
                None => run(&suite, &workspace, &Local::writing(120), None, "cli").await,
            }
            .map_err(|tampered| tampered.to_string())?;
            if json_output {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&result).unwrap_or_default()
                );
            } else {
                for test in &result.tests {
                    println!(
                        "{} {} ({})",
                        if test.green { "GREEN" } else { "RED  " },
                        test.id,
                        test.requirements.join(", ")
                    );
                }
                println!("{} of {} green", result.passed, result.total);
            }
            Ok(i32::from(!result.green))
        }
        "check" => {
            let [record] = positional.as_slice() else {
                return Err(USAGE.to_string());
            };
            let suite = AcceptanceSuite::load(Path::new(record))?;
            let integrity = suite.integrity();
            println!(
                "{}",
                serde_json::to_string_pretty(&integrity).unwrap_or_default()
            );
            Ok(i32::from(!integrity.intact))
        }
        _ => Err(USAGE.to_string()),
    }
}

fn word(says: Option<crate::checks::truth::Says>) -> &'static str {
    match says {
        Some(crate::checks::truth::Says::Pass) => "pass",
        Some(crate::checks::truth::Says::Fail) => "fail",
        None => "-",
    }
}
