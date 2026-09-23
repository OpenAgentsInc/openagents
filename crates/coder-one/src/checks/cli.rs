//! `coder-one checks …`: run `verify.checks` on a synthetic case, an input
//! file, or the candidates recovered from retained trials.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use super::{Input, Report, check, recover, synthetic};
use crate::record::Recorder;

/// The checks commands' usage.
pub const USAGE: &str = "usage: coder-one checks synthetic [NAME] [--json]
       coder-one checks run --input FILE [--json]
       coder-one checks recover --traces DIR [--arm ARM|all] [--out DIR] [--json]
       coder-one checks replay [--jobs DIR] [--match TEXT] [--policy FILE]
                               [--write-fixtures DIR] [--json]

synthetic runs the known-good and known-bad candidates and says whether each
scenario tells them apart. run checks the candidate in an input file
(openagents.coder-one.checks input: task, candidate, observed samples).
recover rebuilds candidates from retained native streams under --traces
(the v3 Luna arm by default), checks each one with repair disabled, and
writes <out>/<job>/<trial>/checks.json for the Gym; --out defaults to
~/.openagents/coder-one/checks. Scenarios need python3 on PATH.

replay reads composed Terminal-Bench trials (the jobs whose names contain
--match, tb4--coder-one- by default, under --jobs,
~/.openagents/terminal-bench/jobs by default; the checked-in fixtures with
--jobs fixtures) and says what verify.self_report, verify.optional_outputs,
and the support budget of --policy (tunable-v4.json by default) would have
done on each first check. It runs nothing and asks Jev nothing.
--write-fixtures writes each trial as a fixture.";

/// The schema of a recovery summary.
pub const RECOVERY_SCHEMA: &str = "openagents.coder-one.checks-recovery.v1";

/// Where recovered checks are written by default.
#[must_use]
pub fn default_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .filter(|home| !home.is_empty())
        .map(|home| PathBuf::from(home).join(".openagents/coder-one/checks"))
}

fn scratch(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "coder-one-checks-{label}-{}-{}",
        std::process::id(),
        atif::now_ms()
    ))
}

fn line(report: &Report) -> String {
    report
        .verdicts
        .iter()
        .map(|v| format!("{} {}", v.scenario, v.verdict))
        .collect::<Vec<_>>()
        .join(" · ")
}

/// Runs a checks command and returns the exit code.
///
/// # Errors
///
/// Returns a message for bad arguments or unreadable inputs.
pub async fn command(args: &[String]) -> Result<i32, String> {
    let Some((verb, rest)) = args.split_first() else {
        return Err(USAGE.to_string());
    };
    let mut positional = Vec::new();
    let mut input = None;
    let mut traces = None;
    let mut arm = "coder-one-jevprobe3-luna".to_string();
    let mut out = None;
    let mut jobs = None;
    let mut matching = "tb4--coder-one-".to_string();
    let mut policy = None;
    let mut fixtures_out = None;
    let mut json_output = false;
    let mut iter = rest.iter();
    while let Some(arg) = iter.next() {
        let mut value = |name: &str| {
            iter.next()
                .cloned()
                .ok_or_else(|| format!("{name} needs a value"))
        };
        match arg.as_str() {
            "--input" => input = Some(PathBuf::from(value("--input")?)),
            "--traces" => traces = Some(PathBuf::from(value("--traces")?)),
            "--arm" => arm = value("--arm")?,
            "--out" => out = Some(PathBuf::from(value("--out")?)),
            "--jobs" => jobs = Some(value("--jobs")?),
            "--match" => matching = value("--match")?,
            "--policy" => policy = Some(PathBuf::from(value("--policy")?)),
            "--write-fixtures" => fixtures_out = Some(PathBuf::from(value("--write-fixtures")?)),
            "--json" => json_output = true,
            other if other.starts_with("--") => return Err(format!("unknown option {other}")),
            other => positional.push(other.to_string()),
        }
    }
    match verb.as_str() {
        "synthetic" => {
            let mut all = true;
            let mut results = Vec::new();
            for case in synthetic::cases() {
                if positional.first().is_some_and(|name| name != case.name) {
                    continue;
                }
                let report = check(&case.input, &Recorder::default(), &scratch(case.name)).await;
                let verdict = |id: &str| {
                    report
                        .verdicts
                        .iter()
                        .find(|v| v.scenario == id)
                        .map_or("not run".to_string(), |v| v.verdict.clone())
                };
                let separates = case.fails.iter().all(|id| verdict(id) == "failed")
                    && case.passes.iter().all(|id| verdict(id) == "passed");
                all &= separates;
                if !json_output {
                    println!(
                        "{:<24} {:<5} {}  {}",
                        case.name,
                        if case.good { "good" } else { "bad" },
                        if separates {
                            "as expected"
                        } else {
                            "NOT AS EXPECTED"
                        },
                        line(&report)
                    );
                }
                results.push(json!({ "case": case.name, "good": case.good, "separates": separates, "report": report }));
            }
            if json_output {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({ "cases": results }))
                        .map_err(|e| e.to_string())?
                );
            }
            Ok(i32::from(!all))
        }
        "run" => {
            let path = input.ok_or("checks run needs --input FILE")?;
            let text = std::fs::read_to_string(&path)
                .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
            let input: Input = serde_json::from_str(&text)
                .map_err(|error| format!("{} is not a checks input: {error}", path.display()))?;
            let report = check(&input, &Recorder::default(), &scratch("run")).await;
            if json_output {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?
                );
            } else {
                for l in report_lines(&report) {
                    println!("{l}");
                }
            }
            Ok(i32::from(report.detected()))
        }
        "recover" => {
            let traces = traces.ok_or("checks recover needs --traces DIR")?;
            let out = out.or_else(default_dir).ok_or("no --out and no HOME")?;
            let summary = recover_and_check(&traces, &arm, &out).await?;
            if json_output {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&summary).map_err(|e| e.to_string())?
                );
            } else {
                for trial in summary["trials"].as_array().into_iter().flatten() {
                    println!(
                        "{:<62} reward {:<4} {}",
                        format!(
                            "{} / {}",
                            trial["job"].as_str().unwrap_or_default(),
                            trial["trial"].as_str().unwrap_or_default()
                        ),
                        trial["reward"]
                            .as_f64()
                            .map_or("—".to_string(), |r| r.to_string()),
                        trial["unavailable"].as_str().map_or_else(
                            || trial["verdicts"].as_str().unwrap_or_default().to_string(),
                            |why| format!("unavailable: {why}")
                        )
                    );
                }
                let t = &summary["totals"];
                println!(
                    "failed trials: {} · candidate recovered {} · detected {}\npassing trials: {} · candidate recovered {} · false alarms {}\nwritten to {}",
                    t["failed"],
                    t["failed_recovered"],
                    t["detected"],
                    t["passed"],
                    t["passed_recovered"],
                    t["false_alarms"],
                    out.display()
                );
            }
            Ok(0)
        }
        "replay" => {
            let params = replay_params(policy.as_deref())?;
            let trials = match jobs.as_deref() {
                Some("fixtures") => super::replay::fixtures(),
                Some(dir) => super::replay::load_jobs(Path::new(dir), &matching)?,
                None => {
                    let dir = std::env::var_os("HOME")
                        .map(|home| PathBuf::from(home).join(".openagents/terminal-bench/jobs"))
                        .ok_or("no --jobs and no HOME")?;
                    super::replay::load_jobs(&dir, &matching)?
                }
            };
            if let Some(dir) = &fixtures_out {
                std::fs::create_dir_all(dir)
                    .map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
                for trial in &trials {
                    let path = dir.join(format!("{}.json", trial.trial));
                    let text = serde_json::to_string_pretty(trial).map_err(|e| e.to_string())?;
                    std::fs::write(&path, text + "\n")
                        .map_err(|e| format!("cannot write {}: {e}", path.display()))?;
                }
            }
            let replayed: Vec<_> = trials
                .iter()
                .map(|trial| super::replay::replay(trial, params))
                .collect();
            if json_output {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&super::replay::to_json(&replayed, params))
                        .map_err(|e| e.to_string())?
                );
            } else {
                for line in super::replay::lines(&replayed, params) {
                    println!("{line}");
                }
            }
            Ok(0)
        }
        _ => Err(USAGE.to_string()),
    }
}

/// The long-task support parameters of the policy at `path`, or of
/// `tunable-v4.json`.
fn replay_params(path: Option<&Path>) -> Result<crate::support::Params, String> {
    let text = match path {
        Some(path) => std::fs::read_to_string(path)
            .map_err(|e| format!("cannot read {}: {e}", path.display()))?,
        None => crate::policy::REFERENCE
            .iter()
            .find(|(name, _)| *name == "tunable-v4.json")
            .map(|(_, text)| (*text).to_string())
            .ok_or("no tunable-v4.json in this build")?,
    };
    let manifest = crate::policy::Manifest::parse(&text)?;
    Ok(manifest
        .policy
        .verify
        .map(|verify| verify.support_params(true))
        .unwrap_or_default())
}

/// A report as text: coverage per requirement, then each packet.
#[must_use]
pub fn report_lines(report: &Report) -> Vec<String> {
    let mut lines = vec![format!(
        "verify.checks · {} ({}) · candidate {}",
        report.candidate["label"].as_str().unwrap_or_default(),
        report.candidate["origin"].as_str().unwrap_or_default(),
        &report.candidate["digest"].as_str().unwrap_or_default()[..12.min(
            report.candidate["digest"]
                .as_str()
                .unwrap_or_default()
                .len()
        )]
    )];
    for covered in &report.coverage {
        if covered.scenarios.is_empty() {
            continue;
        }
        lines.push(format!(
            "  {} {:<13} {}",
            covered.id,
            covered.state,
            crate::judge::clip(&covered.text, 90)
        ));
        for scenario in &covered.scenarios {
            lines.push(format!(
                "      {} {}",
                scenario["id"].as_str().unwrap_or_default(),
                scenario["verdict"].as_str().unwrap_or_default()
            ));
        }
    }
    for packet in &report.packets {
        lines.push(format!(
            "  packet · {} · {}",
            packet.requirement, packet.scenario
        ));
        lines.push(format!("      expected: {}", packet.expected.statement));
        for hypothesis in &packet.hypotheses {
            lines.push(format!("      hypothesis: {hypothesis}"));
        }
    }
    for ineligible in &report.ineligible {
        lines.push(format!(
            "  not applicable · {}: {}",
            ineligible.kind, ineligible.why
        ));
    }
    lines
}

/// Recovers candidates under `traces`, checks each, writes the reports
/// under `out`, and returns the summary.
///
/// # Errors
///
/// Returns a message when the traces or the output don't work.
pub async fn recover_and_check(traces: &Path, arm: &str, out: &Path) -> Result<Value, String> {
    let recovered = recover::recover_tree(traces, arm)?;
    let mut trials = Vec::new();
    let (mut failed, mut failed_recovered, mut detected) = (0, 0, 0);
    let (mut passed, mut passed_recovered, mut false_alarms) = (0, 0, 0);
    for one in &recovered {
        let dir = out.join(&one.job).join(&one.trial);
        std::fs::create_dir_all(&dir)
            .map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
        let failing = one.reward == Some(0.0);
        let passing = one.reward == Some(1.0);
        failed += usize::from(failing);
        passed += usize::from(passing);
        let mut entry = json!({
            "job": one.job, "trial": one.trial, "arm": one.arm, "task": one.task,
            "reward": one.reward, "unavailable": one.unavailable, "sources": one.sources,
        });
        if let Some(input) = &one.input {
            crate::record::write_atomic(
                &dir.join("input.json"),
                serde_json::to_string_pretty(input)
                    .map_err(|e| e.to_string())?
                    .as_bytes(),
            )?;
            let report = check(input, &Recorder::default(), &scratch(&one.trial)).await;
            let caught = report.detected();
            let conclusive = report
                .verdicts
                .iter()
                .any(|v| v.verdict == "passed" || v.verdict == "failed");
            if conclusive {
                failed_recovered += usize::from(failing);
                passed_recovered += usize::from(passing);
                detected += usize::from(failing && caught);
                false_alarms += usize::from(passing && caught);
            } else if report.verdicts.is_empty() {
                entry["unavailable"] = json!(format!(
                    "no scenario applies: {}",
                    report
                        .ineligible
                        .iter()
                        .map(|i| format!("{}: {}", i.kind, i.why))
                        .collect::<Vec<_>>()
                        .join("; ")
                ));
            } else {
                // Each scenario's last limit is the one that says why.
                let mut why: Vec<String> = report
                    .verdicts
                    .iter()
                    .filter_map(|v| v.coverage.last().cloned())
                    .collect();
                why.dedup();
                entry["unavailable"] =
                    json!(format!("no scenario ran to a verdict: {}", why.join("; ")));
            }
            entry["verdicts"] = json!(line(&report));
            entry["detected"] = json!(caught);
            entry["summary"] = report.summary();
            let text = serde_json::to_string_pretty(&json!({ "attempt": { "job": one.job, "trial": one.trial, "reward": one.reward }, "report": report }))
                .map_err(|e| e.to_string())?;
            crate::record::write_atomic(&dir.join("checks.json"), text.as_bytes())?;
        } else {
            let text = serde_json::to_string_pretty(&json!({ "attempt": { "job": one.job, "trial": one.trial, "reward": one.reward }, "unavailable": one.unavailable }))
                .map_err(|e| e.to_string())?;
            crate::record::write_atomic(&dir.join("checks.json"), text.as_bytes())?;
        }
        trials.push(entry);
    }
    let summary = json!({
        "schema": RECOVERY_SCHEMA,
        "arm": arm,
        "implementation": super::implementation(),
        "repair": "disabled",
        "trials": trials,
        "totals": {
            "failed": failed, "failed_recovered": failed_recovered, "detected": detected,
            "passed": passed, "passed_recovered": passed_recovered, "false_alarms": false_alarms,
        },
    });
    crate::record::write_atomic(
        &out.join(format!("summary-{arm}.json")),
        serde_json::to_string_pretty(&summary)
            .map_err(|e| e.to_string())?
            .as_bytes(),
    )?;
    Ok(summary)
}
