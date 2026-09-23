//! `coder-one repair …`: the conditional recovery study, and a run's
//! delta brief.

use std::path::PathBuf;
use std::time::Duration;

use serde_json::{Value, json};

use super::study::{self, Options};
use super::{Profile, Trigger, gaps, packet_brief};
use crate::checks::{self, Report};
use crate::minitask::{CATALOG, find};

/// The repair commands' usage.
pub const USAGE: &str =
    "usage: coder-one repair study [--tasks ID,ID…] [--same PROFILE] [--other PROFILE]
                              [--trigger detected|checked|always] [--allowance SECONDS]
                              [--out DIR] [--json]
       coder-one repair brief --run DIR

study runs each mini-task's good and bad scripts once with verify.checks,
preserves their workspaces, and runs four repair arms on an isolated copy
of each: none, fresh (the same profile with no packet), packet-same, and
packet-other. It reports failures recovered, passes damaged, and every
dispatch's cost per arm, and writes study.json under --out (default
~/.openagents/coder-one/repair/study-<ms>). A PROFILE is a scripted repair
(fix, fix-if-packet, claim-only, break) or AGENT:MODEL; the defaults are
fix-if-packet for the same profile and fix for the other. --trigger always
repairs passing candidates too, to count damage. brief prints the delta
brief a mini-task run's checks would give a repair.";

/// Runs a repair command and returns the exit code.
///
/// # Errors
///
/// Returns a message for bad arguments or a study that can't run.
pub async fn command(args: &[String]) -> Result<i32, String> {
    let Some((verb, rest)) = args.split_first() else {
        return Err(USAGE.to_string());
    };
    let mut tasks = None;
    let mut same = "fix-if-packet".to_string();
    let mut other = "fix".to_string();
    let mut trigger = "detected".to_string();
    let mut allowance = 60u64;
    let mut out = None;
    let mut run_dir = None;
    let mut json_output = false;
    let mut iter = rest.iter();
    while let Some(arg) = iter.next() {
        let mut value = |name: &str| {
            iter.next()
                .cloned()
                .ok_or_else(|| format!("{name} needs a value"))
        };
        match arg.as_str() {
            "--tasks" => tasks = Some(value("--tasks")?),
            "--same" => same = value("--same")?,
            "--other" => other = value("--other")?,
            "--trigger" => trigger = value("--trigger")?,
            "--allowance" => {
                allowance = value("--allowance")?
                    .parse()
                    .map_err(|_| "--allowance takes whole seconds")?;
            }
            "--out" => out = Some(PathBuf::from(value("--out")?)),
            "--run" => run_dir = Some(PathBuf::from(value("--run")?)),
            "--json" => json_output = true,
            other => return Err(format!("unknown option {other}\n\n{USAGE}")),
        }
    }
    match verb.as_str() {
        "study" => {
            let tasks = match tasks {
                None => CATALOG.to_vec(),
                Some(list) => list.split(',').map(find).collect::<Result<Vec<_>, _>>()?,
            };
            let out = match out {
                Some(out) => out,
                None => study::default_dir()
                    .ok_or("no --out and no HOME")?
                    .join(format!("study-{}", atif::now_ms())),
            };
            let options = Options {
                tasks,
                same: Profile::parse(&same)?,
                other: Profile::parse(&other)?,
                trigger: Trigger::parse(&trigger)?,
                allowance: Duration::from_secs(allowance),
                out: out.clone(),
            };
            let result = study::run(&options).await?;
            if json_output {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&result).map_err(|e| e.to_string())?
                );
            } else {
                for line in lines(&result) {
                    println!("{line}");
                }
                println!("written to {}", out.join("study.json").display());
            }
            Ok(0)
        }
        "brief" => {
            let dir = run_dir.ok_or("repair brief needs --run DIR")?;
            let manifest: Value = serde_json::from_str(
                &std::fs::read_to_string(dir.join("manifest.json"))
                    .map_err(|e| format!("cannot read the run's manifest: {e}"))?,
            )
            .map_err(|e| e.to_string())?;
            let task = find(manifest["task"]["id"].as_str().unwrap_or_default())?;
            let report: Report = serde_json::from_str(
                &std::fs::read_to_string(dir.join(checks::COVERAGE_FILE))
                    .map_err(|e| format!("the run has no checks: {e}"))?,
            )
            .map_err(|e| e.to_string())?;
            let input = checks::workspace_input(&task, &dir.join("work"));
            let found = gaps(&report, None);
            if found.is_empty() {
                println!("No check contradicted a requirement, so there is nothing to repair.");
            } else {
                print!(
                    "{}",
                    packet_brief(&input.task, &input.candidate, &report, &found).text
                );
            }
            Ok(0)
        }
        _ => Err(USAGE.to_string()),
    }
}

/// A study as text: per arm, then per candidate.
#[must_use]
pub fn lines(study: &Value) -> Vec<String> {
    let mut lines = vec![format!(
        "Conditional recovery study · trigger {} · same {} · other {} · allowance {}s",
        study["trigger"].as_str().unwrap_or_default(),
        study["same"].as_str().unwrap_or_default(),
        study["other"].as_str().unwrap_or_default(),
        study["allowance_sec"],
    )];
    lines.push(format!(
        "  {:<14} {:<30} {:>18} {:>16} {:>8} {:>10}",
        "arm", "brief · profile", "recovered", "damaged", "repairs", "cost"
    ));
    for arm in study["arms"].as_array().into_iter().flatten() {
        lines.push(format!(
            "  {:<14} {:<30} {:>18} {:>16} {:>8} {:>10}",
            arm["arm"].as_str().unwrap_or_default(),
            format!(
                "{} · {}",
                arm["brief"].as_str().unwrap_or("—"),
                arm["profile"].as_str().unwrap_or("—")
            ),
            format!(
                "{} of {} failed",
                arm["recovered"], arm["failed_candidates"]
            ),
            format!(
                "{} of {} passing",
                arm["damaged"], arm["passing_candidates"]
            ),
            arm["repairs_run"].to_string(),
            arm["cost_usd"]
                .as_f64()
                .map_or("unknown".to_string(), |c| format!("${c:.4}")),
        ));
    }
    for row in study["rows"].as_array().into_iter().flatten() {
        lines.push(format!(
            "    {:<28} {:<13} {} → {}{}",
            row["candidate"].as_str().unwrap_or_default(),
            row["arm"].as_str().unwrap_or_default(),
            word(&row["before"]),
            word(&row["after"]),
            if row["triggered"] == json!(true) {
                format!(
                    " · repaired{}",
                    if row["changed"] == json!(true) {
                        ", changed"
                    } else {
                        ", unchanged"
                    }
                )
            } else {
                String::new()
            }
        ));
    }
    lines
}

fn word(value: &Value) -> &'static str {
    match value.as_bool() {
        Some(true) => "pass",
        Some(false) => "fail",
        None => "unavailable",
    }
}
