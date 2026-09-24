//! Requirement coverage from Coder One's `verify.checks`, per attempt.
//!
//! A check report (`openagents.coder-one.checks.v1`) names the candidate
//! it ran against, each admitted scenario and its verdict with coverage
//! limits, each requirement's state (observed, contradicted, unverifiable,
//! or unobserved), and a diagnostic packet for each failure. Mini-task
//! runs keep theirs in `verification/checks.json`; `coder-one checks
//! recover` writes one per retained Terminal-Bench attempt under
//! `~/.openagents/coder-one/checks/<job>/<trial>/checks.json`. This module
//! reads both and renders them.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

/// The schema a check report carries.
pub const SCHEMA: &str = "openagents.coder-one.checks.v1";

/// The schema of this module's JSON.
pub const VIEW_SCHEMA: &str = "openagents.gym.coder-coverage.v1";

/// Where recovered attempts' checks are written by default.
#[must_use]
pub fn default_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .filter(|home| !home.is_empty())
        .map(|home| PathBuf::from(home).join(".openagents/coder-one/checks"))
}

/// One attempt's check, or why its candidate couldn't be checked.
#[derive(Clone, Debug, PartialEq)]
pub struct Attempt {
    pub job: String,
    pub trial: String,
    pub reward: Option<f64>,
    /// The report, when a check ran.
    pub report: Option<Value>,
    pub unavailable: Option<String>,
}

fn counts(items: &[Value], key: &str) -> BTreeMap<String, usize> {
    let mut out = BTreeMap::new();
    for item in items {
        if let Some(word) = item.get(key).and_then(Value::as_str) {
            *out.entry(word.to_owned()).or_default() += 1;
        }
    }
    out
}

fn array<'a>(report: &'a Value, key: &str) -> &'a [Value] {
    report
        .get(key)
        .and_then(Value::as_array)
        .map_or(&[], Vec::as_slice)
}

/// Counts for a list: scenarios by verdict and requirements by state.
#[must_use]
pub fn summary(report: &Value) -> Value {
    let verdicts = counts(array(report, "verdicts"), "verdict");
    let covered: Vec<Value> = array(report, "coverage")
        .iter()
        .filter(|c| {
            c.get("scenarios")
                .and_then(Value::as_array)
                .is_some_and(|s| !s.is_empty())
        })
        .cloned()
        .collect();
    let states = counts(&covered, "state");
    json!({
        "candidate": report.pointer("/candidate/digest"),
        "scenarios": array(report, "verdicts").len(),
        "passed": verdicts.get("passed").copied().unwrap_or(0),
        "failed": verdicts.get("failed").copied().unwrap_or(0),
        "unavailable": verdicts.get("unavailable").copied().unwrap_or(0) + verdicts.get("inconclusive").copied().unwrap_or(0),
        "observed": states.get("observed").copied().unwrap_or(0),
        "contradicted": states.get("contradicted").copied().unwrap_or(0),
        "unverifiable": states.get("unverifiable").copied().unwrap_or(0),
        "packets": array(report, "packets").len(),
        "support": support_counts(report),
    })
}

/// `verify.support`'s states by word, when it ran.
fn support_counts(report: &Value) -> Value {
    let Some(states) = report.pointer("/support/states").and_then(Value::as_array) else {
        return Value::Null;
    };
    let counts = counts(states, "state");
    json!({
        "judged": states.len(),
        "supported": counts.get("supported").copied().unwrap_or(0),
        "contradicted": counts.get("contradicted").copied().unwrap_or(0),
        "unresolved": counts.get("unresolved").copied().unwrap_or(0),
    })
}

/// A list row's support counts, or nothing when `verify.support` didn't
/// run.
fn support_suffix(counts: &Value) -> String {
    if counts.is_null() {
        return String::new();
    }
    format!(
        " · support {} supported, {} contradicted, {} unresolved",
        counts["supported"], counts["contradicted"], counts["unresolved"]
    )
}

/// The support state of requirement `id`, when `verify.support` judged it.
fn support_state<'a>(report: &'a Value, id: &str) -> Option<&'a Value> {
    report
        .pointer("/support/states")
        .and_then(Value::as_array)?
        .iter()
        .find(|s| s["id"] == id)
}

fn probability(value: Option<&Value>) -> String {
    value
        .and_then(Value::as_f64)
        .map_or("—".to_owned(), |p| format!("{p:.2}"))
}

fn support_line(state: &Value) -> String {
    format!(
        "         supports {} · contradicts {} · result {} ({}) · candidate {}",
        probability(state.pointer("/judgment/supports")),
        probability(state.pointer("/judgment/contradicts")),
        state["state"].as_str().unwrap_or_default(),
        short(state["why"].as_str().unwrap_or_default(), 70),
        state["candidate"]
            .as_str()
            .map_or(String::new(), |c| c.chars().take(12).collect()),
    )
}

/// A requirement's coverage state in plain words; the record keeps the
/// original word.
fn state_label(state: &str) -> &str {
    match state {
        "unobserved" => "not checked",
        "observed" => "checked",
        other => other,
    }
}

fn short(text: &str, width: usize) -> String {
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.chars().count() <= width {
        flat
    } else {
        format!("{}…", flat.chars().take(width).collect::<String>())
    }
}

/// A report as text: each covered requirement with its scenarios, then
/// each diagnostic packet, then the scenario types that didn't apply.
#[must_use]
pub fn lines(report: &Value) -> Vec<String> {
    let s = summary(report);
    let digest = report
        .pointer("/candidate/digest")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let mut lines = vec![format!(
        "Requirement coverage · candidate {} ({}) · {} scenarios: {} passed, {} failed, {} unavailable",
        &digest[..digest.len().min(12)],
        report
            .pointer("/candidate/origin")
            .and_then(Value::as_str)
            .unwrap_or("unknown"),
        s["scenarios"],
        s["passed"],
        s["failed"],
        s["unavailable"],
    )];
    if let Some(counts) = support_counts(report).as_object() {
        lines.push(format!(
            "verify.support · {} judged: {} supported, {} contradicted, {} unresolved · thresholds: supports at least {}, contradicts at least {}",
            counts["judged"],
            counts["supported"],
            counts["contradicted"],
            counts["unresolved"],
            report
                .pointer("/support/params/supports")
                .unwrap_or(&Value::Null),
            report
                .pointer("/support/params/contradicts")
                .unwrap_or(&Value::Null),
        ));
    }
    for covered in array(report, "coverage") {
        let scenarios = covered
            .get("scenarios")
            .and_then(Value::as_array)
            .map_or(&[][..], Vec::as_slice);
        let id = covered["id"].as_str().unwrap_or_default();
        let support = support_state(report, id);
        if scenarios.is_empty() && support.is_none() {
            continue;
        }
        lines.push(format!(
            "  {:<4} {:<13} {}",
            id,
            state_label(covered["state"].as_str().unwrap_or_default()),
            short(covered["text"].as_str().unwrap_or_default(), 100)
        ));
        if let Some(state) = support {
            lines.push(support_line(state));
        }
        for scenario in scenarios {
            let limits = scenario["coverage"].as_array().map_or(0, Vec::len);
            lines.push(format!(
                "         {:<24} {:<12} {} coverage limits",
                scenario["id"].as_str().unwrap_or_default(),
                scenario["verdict"].as_str().unwrap_or_default(),
                limits
            ));
        }
    }
    let unobserved = array(report, "coverage")
        .iter()
        .filter(|c| {
            c.get("scenarios")
                .and_then(Value::as_array)
                .is_none_or(Vec::is_empty)
                && support_state(report, c["id"].as_str().unwrap_or_default()).is_none()
        })
        .count();
    if unobserved > 0 {
        lines.push(format!(
            "  {unobserved} requirements that no accepted scenario checks"
        ));
    }
    for packet in array(report, "packets") {
        lines.push(format!(
            "  Diagnostic packet · {} · {} · expected: {}",
            packet["requirement"].as_str().unwrap_or_default(),
            packet["scenario"].as_str().unwrap_or_default(),
            short(
                packet
                    .pointer("/expected/statement")
                    .and_then(Value::as_str)
                    .unwrap_or_default(),
                110
            )
        ));
        for hypothesis in packet["hypotheses"].as_array().into_iter().flatten() {
            lines.push(format!(
                "         hypothesis: {}",
                short(hypothesis.as_str().unwrap_or_default(), 110)
            ));
        }
    }
    for ineligible in array(report, "ineligible") {
        lines.push(format!(
            "  Not applicable · {}: {}",
            ineligible["kind"].as_str().unwrap_or_default(),
            short(ineligible["why"].as_str().unwrap_or_default(), 100)
        ));
    }
    lines
}

/// Every attempt's check under `dir`, keyed by job and trial.
#[must_use]
pub fn load_dir(dir: &Path) -> BTreeMap<(String, String), Attempt> {
    let mut out = BTreeMap::new();
    for job in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let job_path = job.path();
        if !job_path.is_dir() {
            continue;
        }
        for trial in std::fs::read_dir(&job_path).into_iter().flatten().flatten() {
            let path = trial.path().join("checks.json");
            let Ok(value) = std::fs::read_to_string(&path)
                .map_err(|_| ())
                .and_then(|text| serde_json::from_str::<Value>(&text).map_err(|_| ()))
            else {
                continue;
            };
            let job_name = job_path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            let trial_name = trial.file_name().to_string_lossy().into_owned();
            // `verify.support`'s states, written beside the check.
            let support = std::fs::read_to_string(trial.path().join("support.json"))
                .ok()
                .and_then(|text| serde_json::from_str::<Value>(&text).ok());
            let report = value
                .get("report")
                .filter(|r| r["schema"] == SCHEMA)
                .cloned()
                .map(|mut report| {
                    if let Some(support) = support {
                        report["support"] = support;
                    }
                    report
                });
            out.insert(
                (job_name.clone(), trial_name.clone()),
                Attempt {
                    job: job_name,
                    trial: trial_name,
                    reward: value.pointer("/attempt/reward").and_then(Value::as_f64),
                    unavailable: value
                        .get("unavailable")
                        .and_then(Value::as_str)
                        .map(str::to_owned),
                    report,
                },
            );
        }
    }
    out
}

impl Attempt {
    fn row(&self) -> String {
        let (verdicts, states) = self.report.as_ref().map_or_else(
            || {
                (
                    format!(
                        "unavailable: {}",
                        short(self.unavailable.as_deref().unwrap_or("no check"), 60)
                    ),
                    String::new(),
                )
            },
            |report| {
                let s = summary(report);
                (
                    format!(
                        "{} passed · {} failed · {} unavailable",
                        s["passed"], s["failed"], s["unavailable"]
                    ),
                    format!(
                        "{} observed · {} contradicted · {} unverifiable{}",
                        s["observed"],
                        s["contradicted"],
                        s["unverifiable"],
                        support_suffix(&s["support"])
                    ),
                )
            },
        );
        format!(
            "{:<70} {:>6}  {:<38} {}",
            short(&format!("{} / {}", self.job, self.trial), 70),
            self.reward.map_or("—".to_owned(), |r| format!("{r:.1}")),
            verdicts,
            states
        )
    }

    fn to_json(&self, detail: bool) -> Value {
        json!({
            "job": self.job,
            "trial": self.trial,
            "reward": self.reward,
            "unavailable": self.unavailable,
            "summary": self.report.as_ref().map(summary),
            "report": if detail { self.report.clone().unwrap_or(Value::Null) } else { Value::Null },
        })
    }
}

const HELP: &str = "\
gym coder coverage [--dir PATH] [--minitasks-dir PATH] [--attempt JOB/TRIAL | --run ID] [--json]

Lists requirement coverage from Coder One's verify.checks, per attempt: the
recovered Terminal-Bench attempts `coder-one checks recover` wrote under
~/.openagents/coder-one/checks, and mini-task runs that ran checks. Each row
shows the verifier reward, scenario verdicts, and requirement states, and, where
verify.support judged them, how many requirements it supports, contradicts, or
leaves unresolved. --attempt or --run shows one report: each requirement's
scenarios, verdicts, and coverage limits beside Jev's supports and contradicts
judgments and the state they establish, then the diagnostic packets. Write
recovered attempts' support states with `coder-one support evaluate --write-checks`.";

/// `gym coder coverage …`.
///
/// # Errors
///
/// Returns a message for an unknown option or an attempt that isn't there.
pub fn command(args: &[String], out: &mut impl std::io::Write) -> Result<i32, String> {
    let mut dir = default_dir();
    let mut minitasks = crate::coder_minitasks::default_runs_dir();
    let mut attempt = None;
    let mut run = None;
    let mut json_output = false;
    let mut index = 0;
    while index < args.len() {
        let argument = args[index].as_str();
        match argument {
            "help" | "--help" | "-h" => {
                writeln!(out, "{HELP}").map_err(|e| e.to_string())?;
                return Ok(0);
            }
            "--json" => json_output = true,
            "--dir" | "--minitasks-dir" | "--attempt" | "--run" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| format!("{argument} needs a value"))?
                    .clone();
                match argument {
                    "--dir" => dir = Some(value.into()),
                    "--minitasks-dir" => minitasks = Some(value.into()),
                    "--attempt" => attempt = Some(value),
                    _ => run = Some(value),
                }
                index += 1;
            }
            other => return Err(format!("unknown option {other}\n\n{HELP}")),
        }
        index += 1;
    }
    let attempts = dir.as_deref().map(load_dir).unwrap_or_default();
    let (runs, _) = minitasks
        .as_deref()
        .map(crate::coder_minitasks::load)
        .unwrap_or_default();
    let runs: Vec<_> = runs.into_iter().filter(|r| r.coverage.is_some()).collect();
    let write = |out: &mut dyn std::io::Write, value: &Value| -> Result<(), String> {
        serde_json::to_writer_pretty(&mut *out, value).map_err(|e| e.to_string())?;
        writeln!(out).map_err(|e| e.to_string())
    };
    if let Some(query) = &attempt {
        let found = attempts
            .values()
            .find(|a| format!("{}/{}", a.job, a.trial) == *query || a.trial == *query)
            .ok_or_else(|| format!("no check for attempt {query}"))?;
        if json_output {
            write(
                out,
                &json!({ "schema": VIEW_SCHEMA, "attempt": found.to_json(true) }),
            )?;
        } else {
            writeln!(
                out,
                "{} / {} · reward {}",
                found.job,
                found.trial,
                found.reward.map_or("—".to_owned(), |r| r.to_string())
            )
            .map_err(|e| e.to_string())?;
            match &found.report {
                Some(report) => {
                    for line in lines(report) {
                        writeln!(out, "{line}").map_err(|e| e.to_string())?;
                    }
                }
                None => writeln!(
                    out,
                    "Unavailable: {}",
                    found.unavailable.as_deref().unwrap_or("no check")
                )
                .map_err(|e| e.to_string())?,
            }
        }
        return Ok(0);
    }
    if let Some(query) = &run {
        let found = runs
            .iter()
            .find(|r| r.id == *query || (query == "latest"))
            .ok_or_else(|| format!("no mini-task run {query} with checks"))?;
        let report = found.coverage.clone().unwrap_or(Value::Null);
        if json_output {
            write(
                out,
                &json!({ "schema": VIEW_SCHEMA, "run": found.id, "report": report }),
            )?;
        } else {
            writeln!(
                out,
                "mini-task {} · {} · grade {}",
                found.task, found.id, found.verdict
            )
            .map_err(|e| e.to_string())?;
            for line in lines(&report) {
                writeln!(out, "{line}").map_err(|e| e.to_string())?;
            }
        }
        return Ok(0);
    }
    if json_output {
        write(
            out,
            &json!({
                "schema": VIEW_SCHEMA,
                "dir": dir.as_ref().map(|d| d.display().to_string()),
                "attempts": attempts.values().map(|a| a.to_json(false)).collect::<Vec<_>>(),
                "minitask_runs": runs.iter().map(|r| json!({ "id": r.id, "task": r.task, "grade": r.verdict, "summary": r.coverage.as_ref().map(summary) })).collect::<Vec<_>>(),
            }),
        )?;
    } else {
        writeln!(
            out,
            "Requirement coverage from verify.checks · {} attempts · {} mini-task runs",
            attempts.len(),
            runs.len()
        )
        .map_err(|e| e.to_string())?;
        writeln!(
            out,
            "{:<70} {:>6}  {:<38} requirements",
            "attempt", "reward", "scenarios"
        )
        .map_err(|e| e.to_string())?;
        for a in attempts.values() {
            writeln!(out, "{}", a.row()).map_err(|e| e.to_string())?;
        }
        for r in &runs {
            let s = r.coverage.as_ref().map(summary).unwrap_or(Value::Null);
            writeln!(
                out,
                "{:<70} {:>6}  {} passed · {} failed · {} unavailable{}",
                short(&format!("mini-task {} / {}", r.task, r.id), 70),
                r.reward.map_or("—".to_owned(), |x| format!("{x:.1}")),
                s["passed"],
                s["failed"],
                s["unavailable"],
                support_suffix(&s["support"])
            )
            .map_err(|e| e.to_string())?;
        }
    }
    Ok(0)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn report() -> Value {
        json!({
            "schema": SCHEMA,
            "candidate": { "digest": "abcdef0123456789", "origin": "stream", "label": "t" },
            "verdicts": [
                { "scenario": "data.message-severity", "verdict": "failed", "coverage": ["one format"] },
                { "scenario": "data.date-boundaries", "verdict": "passed", "coverage": [] },
            ],
            "coverage": [
                { "id": "R1", "text": "Each log line contains an event with a severity level.", "kind": "behavior", "state": "contradicted",
                  "scenarios": [{ "id": "data.message-severity", "verdict": "failed", "coverage": ["one format"] }] },
                { "id": "R2", "text": "Write a CSV.", "kind": "deliverable", "state": "unobserved", "scenarios": [] },
            ],
            "packets": [{ "requirement": "R1", "scenario": "data.message-severity",
                "expected": { "statement": "Counts stay the same.", "derivation": "d" },
                "hypotheses": ["The count reads words anywhere in the line."] }],
            "ineligible": [{ "kind": "interactive", "why": "no interactive behavior" }],
        })
    }

    #[test]
    fn a_report_renders_requirements_scenarios_and_packets() {
        let text = lines(&report()).join("\n");
        assert!(text.contains("1 passed, 1 failed"), "{text}");
        assert!(text.contains("R1   contradicted"), "{text}");
        assert!(text.contains("data.message-severity"), "{text}");
        assert!(text.contains("Diagnostic packet · R1"), "{text}");
        assert!(text.contains("hypothesis: The count reads"), "{text}");
        assert!(
            text.contains("1 requirements that no accepted scenario checks"),
            "{text}"
        );
        assert_eq!(summary(&report())["contradicted"], json!(1));
    }

    #[test]
    fn recovered_attempts_list_and_show_one() {
        let dir = std::env::temp_dir().join(format!("gym-coverage-{}", std::process::id()));
        let trial = dir.join("job-a").join("trial-1");
        std::fs::create_dir_all(&trial).unwrap();
        std::fs::write(
            trial.join("checks.json"),
            json!({ "attempt": { "job": "job-a", "trial": "trial-1", "reward": 0.0 }, "report": report() }).to_string(),
        )
        .unwrap();
        let attempts = load_dir(&dir);
        assert_eq!(attempts.len(), 1);
        let mut out = Vec::new();
        command(
            &[
                "--dir".into(),
                dir.display().to_string(),
                "--minitasks-dir".into(),
                "/nonexistent".into(),
            ],
            &mut out,
        )
        .unwrap();
        let text = String::from_utf8(out).unwrap();
        assert!(text.contains("job-a / trial-1"), "{text}");
        assert!(text.contains("1 failed"), "{text}");
        let mut out = Vec::new();
        command(
            &[
                "--dir".into(),
                dir.display().to_string(),
                "--attempt".into(),
                "job-a/trial-1".into(),
                "--json".into(),
            ],
            &mut out,
        )
        .unwrap();
        let value: Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(value["attempt"]["summary"]["failed"], json!(1));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn support_states_show_beside_the_scenarios() {
        let dir = std::env::temp_dir().join(format!("gym-coverage-support-{}", std::process::id()));
        let trial = dir.join("job-a").join("trial-1");
        std::fs::create_dir_all(&trial).unwrap();
        std::fs::write(
            trial.join("checks.json"),
            json!({ "attempt": { "job": "job-a", "trial": "trial-1", "reward": 0.0 }, "report": report() }).to_string(),
        )
        .unwrap();
        std::fs::write(
            trial.join("support.json"),
            json!({
                "schema": "openagents.coder-one.support.v1",
                "params": { "supports": 0.5, "contradicts": 0.3 },
                "candidate": "abcdef0123456789",
                "states": [
                    { "id": "R1", "state": "contradicted", "why": "the evidence contradicts it", "candidate": "abcdef0123456789",
                      "judgment": { "supports": 0.21, "contradicts": 0.88 } },
                    { "id": "R2", "state": "unresolved", "why": "insufficient evidence", "candidate": "abcdef0123456789",
                      "judgment": { "supports": 0.1, "contradicts": 0.1 } },
                ],
            })
            .to_string(),
        )
        .unwrap();
        let attempts = load_dir(&dir);
        let report = attempts.values().next().unwrap().report.clone().unwrap();
        let text = lines(&report).join("\n");
        assert!(
            text.contains("2 judged: 0 supported, 1 contradicted, 1 unresolved"),
            "{text}"
        );
        assert!(
            text.contains("supports 0.21 · contradicts 0.88 · result contradicted"),
            "{text}"
        );
        // R2 has no scenario, but its support state still shows.
        assert!(text.contains("R2   not checked"), "{text}");
        assert!(!text.contains("no accepted scenario checks"), "{text}");
        assert_eq!(summary(&report)["support"]["contradicted"], json!(1));
        let _ = std::fs::remove_dir_all(dir);
    }
}
