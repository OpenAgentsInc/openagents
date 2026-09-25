//! `coder-one checks …`: run `verify.checks` on a synthetic case, an input
//! file, or the candidates recovered from retained trials.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use super::{Input, Report, check, recover, synthetic};
use crate::record::Recorder;

/// The checks commands' usage.
pub const USAGE: &str = "usage: coder-one checks readiness --input FILE --out DIR
       coder-one checks review --input FILE --out DIR [--model MODEL] [--scope strict]
       coder-one checks report-audit --rows FILE --jobs DIR --out DIR
                                      [--partition calibration|held-out|all]
       coder-one checks report-audit --trial-dir DIR --input FILE --out DIR
       coder-one checks public-program --input FILE --out DIR
       coder-one checks contract plan|run|offline …    (coder-one checks contract help)
       coder-one checks conformance registry|run|offline …    (coder-one checks conformance help)
       coder-one checks oracle offline …    (coder-one checks oracle help)
       coder-one checks metric-target extract|offline|measure …    (coder-one checks metric-target help)
       coder-one checks synthetic [NAME] [--json]
       coder-one checks run --input FILE [--json]
       coder-one checks recover --traces DIR [--arm ARM|all] [--out DIR] [--json]
       coder-one checks replay [--jobs DIR] [--match TEXT] [--policy FILE]
                               [--write-fixtures DIR] [--json]
       coder-one checks recall [ARM...] [TASK...] [--jobs DIR] [--match TEXT]
                               [--out DIR] [--json]
       coder-one checks truth [--jobs DIR|none] [--traces DIR] [--match TEXT] [--rows FILE]
                              [--jev live|recorded|off] [--set all|calibration|held_out]
                              [--out DIR] [--json]

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
--write-fixtures writes each trial as a fixture.

recall builds the labeled set from finished Terminal-Bench trials (the jobs
whose names contain --match, tb4--coder-one-tunable-v by default): for each
graded trial it rebuilds the task's filesystem from the task image's public
files and the outputs Harbor collected, runs the checks against it in a
bwrap sandbox under each ARM (v6: self-report and optional outputs; v7:
also the behavior scenarios; both by default), and reports recall on the
verifier's failures and false alarms on its passes, beside the episode's
own first check. TASK names limit it to those tasks. Reports go to
<out>/<job>/<trial>/checks-<arm>.json and the table to <out>/summary.json;
--out defaults to ~/.openagents/coder-one/checks-recall.

truth builds the truthful-checks label set: every graded Coder One trial
with a composition record under --jobs (~/.openagents/terminal-bench/jobs by
default) and --traces (bench/terminal-bench/traces by default), each labeled
with its verifier reward and split by task into calibration and held-out
halves. It asks Jev the report questions over each trial's task and final
report (--jev recorded replays <out>/jev-recorded.json only; live, the
default, asks for what isn't recorded), measures every signal's fail
precision, failure recall, and pass rate with Wilson intervals, and scores
the combined verdict. --rows FILE reads rows a previous run wrote instead
of scanning. It writes <out>/rows.jsonl and <out>/summary.json; --out
defaults to ~/.openagents/coder-one/checks-truth.";

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
    if verb == "contract" {
        return super::contract::cli::command(rest).await;
    }
    if verb == "oracle" {
        return super::oracle::cli::command(rest).await;
    }
    if verb == "conformance" {
        return super::conformance::cli::command(rest).await;
    }
    if verb == "metric-target" {
        return super::metric_target::cli::command(rest).await;
    }
    if verb == "execution-audit" {
        return super::execution_audit::command(rest).await;
    }
    if verb == "reproduced-review" {
        return super::reproduced::command(rest).await;
    }
    if verb == "reproduced-rejudge" {
        return super::reproduced::rejudge_command(rest).await;
    }
    if verb == "readiness" {
        return super::readiness::command(rest).await;
    }
    if verb == "public-program" {
        return super::public_program::command(rest).await;
    }
    if verb == "report-audit" {
        return super::report_audit::command(rest).await;
    }
    if verb == "review" {
        return super::review::command(rest).await;
    }
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
    let mut truth_options: Vec<String> = Vec::new();
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
            "--match" => {
                matching = value("--match")?;
                truth_options.push(format!("match={matching}"));
            }
            "--policy" => policy = Some(PathBuf::from(value("--policy")?)),
            "--write-fixtures" => fixtures_out = Some(PathBuf::from(value("--write-fixtures")?)),
            "--json" => json_output = true,
            "--jev" => truth_options.push(format!("jev={}", value("--jev")?)),
            "--set" => truth_options.push(format!("set={}", value("--set")?)),
            "--rows" => input = Some(PathBuf::from(value("--rows")?)),
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
        "recall" => {
            let dir = match jobs.as_deref() {
                Some(dir) => PathBuf::from(dir),
                None => std::env::var_os("HOME")
                    .map(|home| PathBuf::from(home).join(".openagents/terminal-bench/jobs"))
                    .ok_or("no --jobs and no HOME")?,
            };
            let matching = if matching == "tb4--coder-one-" {
                "tb4--coder-one-tunable-v".to_string()
            } else {
                matching
            };
            let out = out.or_else(recall_dir).ok_or("no --out and no HOME")?;
            let arms: Vec<String> = positional
                .iter()
                .filter(|p| super::labeled::arm_options(p).is_some())
                .cloned()
                .collect();
            let arms = if arms.is_empty() {
                vec!["v6".to_string(), "v7".to_string()]
            } else {
                arms
            };
            let only: Vec<&String> = positional
                .iter()
                .filter(|p| super::labeled::arm_options(p).is_none())
                .collect();
            let mut rows = Vec::new();
            for labeled in super::labeled::scan(&dir, &matching) {
                if !only.is_empty() && !only.iter().any(|t| **t == labeled.label.task) {
                    continue;
                }
                if !json_output {
                    eprintln!("checking {} {}", labeled.label.job, labeled.label.trial);
                }
                rows.push(super::labeled::check_trial(&labeled, &arms, &out).await);
            }
            let summary = super::labeled::summary(&rows, &arms, &super::labeled::targets());
            let text = serde_json::to_string_pretty(&summary).map_err(|e| e.to_string())?;
            std::fs::create_dir_all(&out)
                .map_err(|e| format!("cannot create {}: {e}", out.display()))?;
            std::fs::write(out.join("summary.json"), text.clone() + "\n")
                .map_err(|e| format!("cannot write summary: {e}"))?;
            if json_output {
                println!("{text}");
            } else {
                for line in super::labeled::lines(&summary) {
                    println!("{line}");
                }
                println!("Wrote {}.", out.join("summary.json").display());
            }
            Ok(0)
        }
        "truth" => {
            truth_command(
                jobs,
                traces,
                out,
                input,
                &truth_options.join(","),
                json_output,
            )
            .await
        }
        _ => Err(USAGE.to_string()),
    }
}

/// `checks truth`. `--rows` arrives as `input`; `--jev` and `--set` as
/// `jev=…,set=…`.
async fn truth_command(
    jobs: Option<String>,
    traces: Option<PathBuf>,
    out: Option<PathBuf>,
    rows_file: Option<PathBuf>,
    options: &str,
    json_output: bool,
) -> Result<i32, String> {
    use super::truth;
    let option = |name: &str| {
        options
            .split(',')
            .find_map(|kv| kv.strip_prefix(&format!("{name}=")).map(str::to_string))
    };
    let jev_word = option("jev").unwrap_or_else(|| "live".to_string());
    let set = option("set").unwrap_or_else(|| "held_out".to_string());
    if !["all", "calibration", "held_out"].contains(&set.as_str()) {
        return Err(format!(
            "--set must be all, calibration, or held_out, not {set}"
        ));
    }
    let out = out
        .or_else(truth::default_dir)
        .ok_or("no --out and no HOME")?;
    std::fs::create_dir_all(&out).map_err(|e| format!("cannot create {}: {e}", out.display()))?;
    let rows = if let Some(path) = rows_file {
        truth::read_rows(&path)?
    } else {
        let jobs = match jobs {
            Some(dir) if dir == "none" => None,
            Some(dir) => Some(PathBuf::from(dir)),
            None => std::env::var_os("HOME")
                .map(|home| PathBuf::from(home).join(".openagents/terminal-bench/jobs")),
        };
        let traces = traces.or_else(|| Some(PathBuf::from("bench/terminal-bench/traces")));
        let mut loaded = truth::scan(jobs.as_deref(), traces.as_deref());
        if let Some(pattern) = option("match") {
            loaded.retain(|trial| trial.row.job.contains(&pattern));
        }
        let recorded_path = out.join(truth::RECORDED_FILE);
        let mut recorded = crate::component::jev::Recorded::load(&recorded_path)?;
        let client = match jev_word.as_str() {
            "live" => {
                let dir = crate::credentials::openagents_dir().ok_or("HOME is not set")?;
                let key = crate::credentials::jev_key(|name| std::env::var(name).ok(), &dir)?;
                Some(crate::credentials::jev_client(&key.secret)?)
            }
            "recorded" => None,
            "off" => {
                recorded = crate::component::jev::Recorded::empty();
                None
            }
            other => return Err(format!("--jev must be live, recorded, or off, not {other}")),
        };
        let (replayed, live, tokens) =
            truth::ask_reports(&mut loaded, &mut recorded, client.as_ref()).await;
        if live > 0 {
            recorded.save(&recorded_path)?;
        }
        if !json_output {
            eprintln!(
                "{} trials · report answers: {replayed} recorded, {live} live ({tokens} input tokens, ${:.4})",
                loaded.len(),
                tokens as f64 * crate::component::jev::USD_PER_MILLION_INPUT / 1e6
            );
        }
        let rows: Vec<truth::Row> = loaded.into_iter().map(|l| l.row).collect();
        truth::write_rows(&out.join("rows.jsonl"), &rows)?;
        rows
    };
    truth::validate_rows(&rows)?;
    let summary = truth::summary(&rows);
    let text = serde_json::to_string_pretty(&summary).map_err(|e| e.to_string())?;
    crate::record::write_atomic(&out.join("summary.json"), format!("{text}\n").as_bytes())?;
    if json_output {
        println!("{text}");
    } else {
        for line in truth::lines(&summary, &set) {
            println!("{line}");
        }
        println!("Wrote {}.", out.join("summary.json").display());
    }
    Ok(0)
}

/// Where `checks recall` writes by default.
#[must_use]
pub fn recall_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .filter(|home| !home.is_empty())
        .map(|home| PathBuf::from(home).join(".openagents/coder-one/checks-recall"))
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
