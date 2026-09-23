//! The labeled fixture set for `verify.support`: (requirement, evidence,
//! outcome) triples.
//!
//! Two sources, kept apart:
//!
//! - **Development**: the eight mini-task candidates (each task's good and
//!   bad script) and the synthetic off-by-one log parser. Their ground
//!   truth is known by construction and by each mini-task's grader. The
//!   cutoffs are fitted here and nowhere else.
//! - **Evaluation**: the nine v3 Luna candidates `verify.checks` recovered
//!   from retained streams, with the verifier's reward. They're only
//!   scored.
//!
//! The evidence in each fixture is frozen: the check ran when the fixture
//! was built, and its observations are stored with scratch paths replaced,
//! so a suite run replays recorded Jev answers without running a scenario.
//!
//! Labels are per requirement. A passing candidate meets every requirement
//! the run judges. A failing candidate's labels come from [`LABELS`]: which
//! requirement its failure breaks, from the verifier's test outcomes or the
//! known-bad source, and, where the verifier said so, which requirement it
//! still meets. Any other requirement of a failing candidate stays
//! unlabeled. Labels are for scoring only; no label, verifier output, or
//! test name enters the evidence.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use super::{Evidence, Params, Skipped, evidence};
use crate::checks::{self, Input, TaskText};
use crate::component::{FIXTURE_SCHEMA, Fixture};
use crate::record::Recorder;

/// Hand labels for failing candidates: fixture, a fragment of the
/// requirement's text, whether it's met, and the basis.
pub const LABELS: &[(&str, &str, bool, &str)] = &[
    (
        "support--v3-luna-log-summary-date-ranges",
        "severity levels to count are exactly",
        false,
        "the verifier's counts test failed; the retained program counts severity words anywhere in a line",
    ),
    (
        "support--v3-luna-log-summary-date-ranges",
        "total count of each severity",
        false,
        "the verifier's counts test failed; the retained program counts severity words anywhere in a line",
    ),
    (
        "support--v3-luna-log-summary-date-ranges-2",
        "severity levels to count are exactly",
        false,
        "the verifier's counts test failed; the retained program assigns events to the wrong severity",
    ),
    (
        "support--v3-luna-log-summary-date-ranges-2",
        "total count of each severity",
        false,
        "the verifier's counts test failed; the retained program assigns events to the wrong severity",
    ),
    (
        "support--v3-luna-log-summary-date-ranges-3",
        "severity levels to count are exactly",
        false,
        "the verifier's counts test failed; the retained program uses the same wrong counting rule",
    ),
    (
        "support--v3-luna-log-summary-date-ranges-3",
        "total count of each severity",
        false,
        "the verifier's counts test failed; the retained program uses the same wrong counting rule",
    ),
    (
        "support--v3-luna-headless-terminal",
        "Supports interactive programs",
        false,
        "the verifier's interactive-command test failed",
    ),
    (
        "support--v3-luna-headless-terminal",
        "control C",
        true,
        "the verifier's control C test passed",
    ),
    (
        "support--v3-luna-cancel-async-tasks-2",
        "cleanup code to still run",
        false,
        "the verifier's cancellation test above the concurrency limit failed",
    ),
    (
        "support--v3-luna-cancel-async-tasks-2",
        "Create a Python function called",
        true,
        "the verifier's concurrency and limit tests passed",
    ),
    (
        "support--minitask-log-severity-bad",
        "Count how many times each severity appears",
        false,
        "the known-bad script counts severity words anywhere in the line, and the grader failed",
    ),
    (
        "support--minitask-log-severity-bad",
        "severity levels to count are exactly",
        false,
        "the known-bad script counts severity words anywhere in the line, and the grader failed",
    ),
    (
        "support--minitask-interactive-terminal-bad",
        "Supports interactive programs",
        false,
        "the known-bad script runs each line with bash -c, and the grader's interactive program failed",
    ),
    (
        "support--minitask-cancel-cleanup-bad",
        "cleanup code to still run",
        false,
        "the known-bad script returns before cleanup finishes, and the grader failed",
    ),
    (
        "support--minitask-git-recovery-bad",
        "Recover the lost commit",
        false,
        "the known-bad script retypes the file instead of recovering the commit, and the grader failed",
    ),
    (
        "support--synthetic-log-off-by-one-window",
        "Last 7 days",
        false,
        "the known-bad parser's window is off by one day",
    ),
    (
        "support--synthetic-log-off-by-one-window",
        "severity levels to count are exactly",
        true,
        "the known-bad parser reads the severity field",
    ),
];

/// A requirement's label.
#[must_use]
pub fn labels_for(
    fixture: &str,
    candidate_met: Option<bool>,
    basis: &str,
    evidence: &[Evidence],
) -> Vec<Value> {
    evidence
        .iter()
        .filter_map(|e| {
            if candidate_met == Some(true) {
                return Some(json!({ "requirement": e.requirement.id, "met": true, "basis": basis }));
            }
            LABELS
                .iter()
                .find(|(name, fragment, _, _)| *name == fixture && e.requirement.text.contains(fragment))
                .map(|(_, _, met, why)| json!({ "requirement": e.requirement.id, "met": met, "basis": why }))
        })
        .collect()
}

fn scratch(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "coder-one-support-fixtures-{label}-{}-{}",
        std::process::id(),
        atif::now_ms()
    ))
}

/// The retained broad "done" answer of a trial: the `done` Noul of its
/// `jev_close` decision.
#[must_use]
pub fn retained_done(traces: &Path, job: &str, trial: &str) -> Option<f64> {
    let path = traces.join(job).join(format!("{trial}.json"));
    let value: Value = serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()?;
    for step in value["steps"].as_array()? {
        for call in step["tool_calls"].as_array().into_iter().flatten() {
            if call["function_name"] != "jev_close" {
                continue;
            }
            for result in step
                .pointer("/observation/results")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                let answers: Value =
                    serde_json::from_str(result["content"].as_str().unwrap_or_default()).ok()?;
                if let Some(p) = answers.pointer("/done/noul").and_then(Value::as_f64) {
                    return Some(p);
                }
            }
        }
    }
    None
}

#[allow(clippy::too_many_arguments)]
fn fixture(
    name: &str,
    source: Value,
    input: &Input,
    report: &checks::Report,
    outcome: (Option<bool>, &str),
    baseline: Value,
    split: &str,
    params: Params,
) -> Fixture {
    let (evidence, skipped): (Vec<Evidence>, Vec<Skipped>) =
        evidence(&input.candidate, report, params);
    let labels = labels_for(name, outcome.0, outcome.1, &evidence);
    Fixture {
        schema: FIXTURE_SCHEMA.to_string(),
        component: "verify.support".to_string(),
        source,
        input: json!({
            "task": input.task,
            "candidate": { "label": input.candidate.label, "origin": input.candidate.origin, "digest": input.candidate.digest() },
            "evidence": evidence,
            "skipped": skipped,
            "labels": labels,
            "outcome": { "met": outcome.0, "basis": outcome.1 },
            "baseline": baseline,
            "split": split,
        }),
        retained: Value::Null,
    }
}

/// Builds every fixture: the evaluation set from the `checks--v3-luna-*`
/// fixtures under `components` and their retained trials under `traces`,
/// and the development set from mini-task runs and the synthetic
/// off-by-one parser. Writes each as `<out>/<name>/verify.support.json`
/// and returns the names. Scenarios need `python3`.
///
/// # Errors
///
/// Returns a message when a fixture can't be read or written.
pub async fn build(components: &Path, traces: &Path, out: &Path) -> Result<Vec<String>, String> {
    let params = Params::default();
    let mut names = Vec::new();
    let mut write = |fixture: Fixture, name: &str| -> Result<(), String> {
        let dir = out.join(name);
        std::fs::create_dir_all(&dir)
            .map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
        fixture.save(&dir)?;
        names.push(name.to_string());
        Ok(())
    };

    // Evaluation: recovered v3 candidates.
    for dir in crate::component::fixtures_for(components, "verify.checks") {
        let base = dir
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let Some(rest) = base.strip_prefix("checks--v3-") else {
            continue;
        };
        let checks_fixture = Fixture::load(&dir, "verify.checks")?;
        let input: Input = serde_json::from_value(checks_fixture.input["input"].clone())
            .map_err(|e| format!("{base}: {e}"))?;
        let reward = checks_fixture.input["reward"].as_f64();
        let job = checks_fixture.source["job"].as_str().unwrap_or_default();
        let trial = checks_fixture.source["trial"].as_str().unwrap_or_default();
        let report = checks::check(&input, &Recorder::default(), &scratch(rest)).await;
        let name = format!("support--v3-{rest}");
        let met = reward.map(|r| r >= 1.0);
        let basis = match met {
            Some(true) => "the verifier passed every test",
            Some(false) => "the verifier failed the trial",
            None => "no reward",
        };
        let done = retained_done(traces, job, trial);
        write(
            fixture(
                &name,
                json!({
                    "kind": "retained",
                    "job": job,
                    "trial": trial,
                    "note": "candidate and observed samples from the verify.checks fixture; evidence from a check run when the fixture was built; baseline from the trial's retained jev_close answer",
                }),
                &input,
                &report,
                (met, basis),
                json!({ "done": done, "source": format!("{job}/{trial}.json jev_close") }),
                "evaluation",
                params,
            ),
            &name,
        )?;
    }

    // Development: mini-task candidates.
    let runs = scratch("minitasks");
    for task in crate::minitask::CATALOG {
        for (variant, script) in crate::minitask::scripts(task) {
            let ran = crate::minitask::run::run(crate::minitask::run::Options {
                task: *task,
                executor: crate::minitask::run::ExecutorChoice::Scripted {
                    variant: variant.to_string(),
                    script: None,
                },
                out: runs.clone(),
                jev: None,
                speed: 0.0,
                deadline: std::time::Duration::from_secs(60),
                controls: crate::session::Controls::default(),
                checks: true,
                brief: None,
                monitor: None,
                repair: None,
            })
            .await?;
            let work = ran.dir.join("work");
            let input = checks::workspace_input(task, &work);
            let report: checks::Report = serde_json::from_str(
                &std::fs::read_to_string(ran.dir.join(checks::COVERAGE_FILE))
                    .map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;
            let claim = script
                .events
                .iter()
                .rev()
                .find_map(|t| match &t.act {
                    crate::scripted::Act::Claim { text } => Some(text.clone()),
                    _ => None,
                })
                .unwrap_or_default();
            let changes = super::scrub(&crate::delegate::changes(&work, None));
            let met = ran.grade.reward().map(|r| r >= 1.0);
            let name = format!("support--minitask-{}-{variant}", task.id);
            let criteria = crate::requirements::mechanical(task.instruction).criteria(12);
            write(
                fixture(
                    &name,
                    json!({
                        "kind": "synthetic",
                        "task": task.id,
                        "script": variant,
                        "note": "a scripted mini-task episode with verify.checks; the grader's verdict is the outcome",
                    }),
                    &input,
                    &report,
                    (
                        met,
                        &format!("the mini-task grader said {}", ran.grade.verdict),
                    ),
                    json!({ "done": null, "request": { "report": claim, "changes": changes, "criteria": criteria }, "source": "the closing check the episode asks, over the script's last claim and the workspace's changes" }),
                    "development",
                    params,
                ),
                &name,
            )?;
        }
    }
    let _ = std::fs::remove_dir_all(&runs);

    // Development: the synthetic off-by-one parser, which no mini-task
    // script writes.
    let case = checks::synthetic::case("log-off-by-one-window")
        .ok_or("no synthetic case log-off-by-one-window")?;
    let report = checks::check(&case.input, &Recorder::default(), &scratch("off-by-one")).await;
    let name = "support--synthetic-log-off-by-one-window";
    write(
        fixture(
            name,
            json!({ "kind": "synthetic", "case": case.name, "note": "a known-bad synthetic candidate; it has no report, so it has no done baseline" }),
            &case.input,
            &report,
            (Some(false), "a known-bad synthetic candidate"),
            json!({ "done": null, "source": "none: the candidate has no report" }),
            "development",
            params,
        ),
        name,
    )?;
    Ok(names)
}

/// The task text as a fixture stores it.
#[must_use]
pub fn task_of(fixture: &Fixture) -> Option<TaskText> {
    serde_json::from_value(fixture.input["task"].clone()).ok()
}
