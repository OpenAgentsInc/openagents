//! `report.json`: an `openagents.eval-report.v1` document (NIP-EVAL) with
//! the extension-evaluation profile in `meta.ext_eval`, and the artifacts
//! it references.
//!
//! The report references its suite, partition, runs, and limitations by
//! ArtifactRef, as NIP-EVAL requires, so the engine writes those documents
//! beside it under `artifacts/`. Every reference is to exact bytes this
//! module produced; [`validate`] checks a report against the contract the
//! way a reader would.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Map, Value, json};

use crate::artifact::{ArtifactRef, JSON, MARKDOWN, TOML, json_bytes};
use crate::case::{CASE_SCHEMA, Case};
use crate::discover::Suite;
use crate::record::Arm;
use crate::score::{ArmSummary, CaseArm, GradedRun, Scores};

/// The report schema.
pub const REPORT_SCHEMA: &str = "openagents.eval-report.v1";
/// The suite schema.
pub const SUITE_SCHEMA: &str = "openagents.eval-suite.v1";
/// The profile schema in `meta.ext_eval`.
pub const PROFILE_SCHEMA: &str = "openagents.ext-eval.v1";
/// The runs artifact's schema.
pub const RUNS_SCHEMA: &str = "openagents.ext-eval-runs.v1";
/// One run's grader answers.
pub const GRADES_SCHEMA: &str = "openagents.ext-eval-grades.v1";
/// The partition artifact's schema.
pub const PARTITION_SCHEMA: &str = "openagents.eval-partition.v1";
/// The limitations artifact's schema.
pub const LIMITATIONS_SCHEMA: &str = "openagents.eval-limitations.v1";
/// The workload artifact's schema.
pub const WORKLOAD_SCHEMA: &str = "openagents.ext-eval-workload.v1";
/// The labels artifact's schema.
pub const LABELS_SCHEMA: &str = "openagents.ext-eval-labels.v1";
/// The metrics artifact's schema.
pub const METRICS_SCHEMA: &str = "openagents.ext-eval-metrics.v1";
/// The environment artifact's schema, when the runner gives none.
pub const ENVIRONMENT_SCHEMA: &str = "openagents.ext-eval-environment.v1";
/// An arm's configuration artifact.
pub const CONFIGURATION_SCHEMA: &str = "openagents.ext-eval-configuration.v1";
/// The gate file's schema.
pub const GATE_SCHEMA: &str = "openagents.gym.gate.v2";
/// The largest report a `3189` publication may carry inline.
pub const MAX_INLINE_REPORT: usize = 64 * 1024;

/// One arm's identity, as the runner knows it.
#[derive(Clone, Debug, PartialEq)]
pub struct ArmSetup {
    /// The arm's DefinitionRef: the extension for the subject, the agent
    /// alone for the baseline.
    pub definition: Value,
    /// The run lock the arm held, as an ArtifactRef.
    pub lock: ArtifactRef,
    /// The pinned chat door, by name.
    pub door: String,
    /// The run configuration the runner used (Coder's bounds, grants),
    /// recorded as given.
    pub run: Value,
}

/// Who and what a report is about, beyond the suite and the runs.
#[derive(Clone, Debug, PartialEq)]
pub struct Identity {
    /// The suite author's public key, 64 lowercase hex.
    pub author: String,
    /// The package slug the suite belongs to.
    pub package: String,
    /// The suite component's slug.
    pub component: String,
    /// The evaluator: a signer's public key or a local provenance ID.
    pub evaluator: String,
    /// The subject arm.
    pub subject: ArmSetup,
    /// The baseline arm; required when the plan runs one.
    pub baseline: Option<ArmSetup>,
    /// When the first run started, Unix seconds.
    pub started_at: u64,
    /// When the last run ended, Unix seconds.
    pub ended_at: u64,
    /// For a hosted run, the EventRef of the signed request it served.
    pub requester: Option<Value>,
    /// The EventRef of the NIP-EXT release (`3184`) that published the
    /// suite, when it was published.
    pub suite_release: Option<Value>,
    /// The runner, toolchain, and execution policy; a default names this
    /// crate when absent.
    pub environment: Option<Value>,
    /// Why the suite couldn't finish, when the runner stopped it.
    pub partial: Option<String>,
}

/// The doors a grading pass used, by name.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DoorNames {
    /// The decision door.
    pub decision: Option<String>,
    /// The judge door.
    pub judge: Option<String>,
}

/// Documents a report references, keyed by their path under `artifacts/`.
pub type Artifacts = BTreeMap<String, Vec<u8>>;

fn put(artifacts: &mut Artifacts, name: &str, value: &Value, schema: &str) -> ArtifactRef {
    let bytes = json_bytes(value);
    let reference = ArtifactRef::of(&bytes, JSON, Some(schema));
    artifacts.insert(name.to_string(), bytes);
    reference
}

/// The suite document and what it references, written into `artifacts`.
/// Returns the suite's ArtifactRef and the partition's.
pub(crate) fn suite_artifacts(
    suite: &Suite,
    identity: &Identity,
    gate_file: &[u8],
    artifacts: &mut Artifacts,
) -> (ArtifactRef, ArtifactRef) {
    let mut ordered: Vec<&Case> = suite.cases.iter().collect();
    ordered.sort_by(|left, right| left.name.cmp(&right.name));
    let entries: Vec<Value> = ordered.into_iter().map(case_entry).collect();
    // `nostr::eval_ext` checks the manifest and writes its canonical bytes.
    let manifest = nostr::eval_ext::case_manifest(&entries).unwrap_or_default();
    let cases = ArtifactRef::of(&manifest, JSON, Some(CASE_SCHEMA));
    artifacts.insert("cases.json".to_string(), manifest);
    let ids: Vec<&str> = suite.cases.iter().map(|case| case.name.as_str()).collect();
    let partition = put(
        artifacts,
        "partition.json",
        &json!({
            "v": PARTITION_SCHEMA,
            "requires": [],
            "development": ids,
            "held_out": [],
            "excluded": [],
        }),
        PARTITION_SCHEMA,
    );
    let count =
        |kind: crate::case::Kind| suite.cases.iter().filter(|case| case.kind == kind).count();
    let workload = put(
        artifacts,
        "workload.json",
        &json!({
            "v": WORKLOAD_SCHEMA,
            "requires": [],
            "collection": "authored",
            "synthetic": true,
            "task_families": {
                "should-fire": count(crate::case::Kind::ShouldFire),
                "should-not-fire": count(crate::case::Kind::ShouldNotFire),
            },
            // The sampling story: what S was drawn from, the rules that
            // decided which cases were in, the strata, and whether the
            // cases were sampled, enumerated, or written. Every starter
            // case was written by hand, so the method is `constructed`;
            // a suite whose author names no frame claims only the tasks
            // its subject says it helps with.
            "frame": "the case directories the suite's author wrote under its eval directory",
            "inclusion": "every case directory the run kept",
            "sampling": "every case under the eval directory the run kept",
            "exclusions": [],
            "strata": {
                "should-fire": count(crate::case::Kind::ShouldFire),
                "should-not-fire": count(crate::case::Kind::ShouldNotFire),
            },
            "method": "constructed",
        }),
        WORKLOAD_SCHEMA,
    );
    let labels = put(
        artifacts,
        "labels.json",
        &json!({
            "v": LABELS_SCHEMA,
            "requires": [],
            "source": identity.author,
            "rubric": "each case's graders, as listed in cases.json",
            "procedure": "the graders run on every run; structural graders are deterministic, and decision and judge graders ask a door three times",
            "uncertainty": "a suite written by an extension's publisher is the publisher's own claim",
        }),
        LABELS_SCHEMA,
    );
    let metrics = put(
        artifacts,
        "metrics.json",
        &json!({ "v": METRICS_SCHEMA, "requires": [], "metrics": metric_definitions(suite) }),
        METRICS_SCHEMA,
    );
    let environment_value = identity.environment.clone().unwrap_or_else(|| {
        json!({
            "v": ENVIRONMENT_SCHEMA,
            "requires": [],
            "engine": format!("ext-eval {}", env!("CARGO_PKG_VERSION")),
        })
    });
    let environment_schema = environment_value
        .get("v")
        .and_then(Value::as_str)
        .unwrap_or(ENVIRONMENT_SCHEMA)
        .to_string();
    let environment = put(
        artifacts,
        "environment.json",
        &environment_value,
        &environment_schema,
    );
    let gate = ArtifactRef::of(gate_file, JSON, Some(GATE_SCHEMA));
    let mut suite_ref = put(
        artifacts,
        "suite.json",
        &json!({
            "v": SUITE_SCHEMA,
            "requires": [],
            "id": format!("{}:{}/{}", identity.author, identity.package, identity.component),
            "purpose": "operation",
            "workload": workload.value(),
            "cases": cases.value(),
            "partition": partition.value(),
            "labels": labels.value(),
            "metrics": metrics.value(),
            "acceptance": {
                "id": format!("{}:gym-gates/ext-eval-v2", identity.author),
                "artifact": gate.value(),
            },
            "environment": environment.value(),
        }),
        SUITE_SCHEMA,
    );
    suite_ref.event.clone_from(&identity.suite_release);
    (suite_ref, partition)
}

fn case_entry(case: &Case) -> Value {
    let named = |key: &str, name: &str, artifact: &ArtifactRef| json!({ key: name, "artifact": artifact.value() });
    let graders: Vec<Value> = case
        .files
        .graders
        .iter()
        .map(|(file, bytes)| {
            let name = file.strip_suffix(".md").unwrap_or(file);
            named(
                "name",
                name,
                &ArtifactRef::of(bytes, MARKDOWN, Some(CASE_SCHEMA)),
            )
        })
        .collect();
    let fixtures: Vec<Value> = case
        .files
        .fixtures
        .iter()
        .map(|(path, bytes)| {
            named(
                "path",
                path,
                &ArtifactRef::of(bytes, "application/octet-stream", None),
            )
        })
        .collect();
    json!({
        "id": case.name,
        "kind": case.kind.word(),
        "runs": case.runs,
        "prompt": ArtifactRef::of(&case.files.prompt, MARKDOWN, Some(CASE_SCHEMA)).value(),
        "config": case
            .files
            .case_toml
            .as_ref()
            .map(|bytes| ArtifactRef::of(bytes, TOML, Some(CASE_SCHEMA)).value()),
        "graders": graders,
        "fixtures": fixtures,
    })
}

/// Every metric a report may measure, with its definition.
fn metric_definitions(suite: &Suite) -> Vec<Value> {
    let metric = |id: String, unit: &str, direction: &str, population: &str, aggregation: &str| {
        json!({
            "id": id,
            "unit": unit,
            "direction": direction,
            "population": population,
            "aggregation": format!("ext-eval.v1/{aggregation}"),
            "missing": "report_separately",
        })
    };
    let mut out = vec![
        metric(
            "cases_passed".into(),
            "cases",
            "higher",
            "cases whose pass is decided in the arm",
            "count",
        ),
        metric(
            "mean_score".into(),
            "score",
            "higher",
            "cases with a score in the arm",
            "mean",
        ),
        metric(
            "cost_usd".into(),
            "usd",
            "lower",
            "every attempt in the arm",
            "sum",
        ),
        metric(
            "seconds".into(),
            "seconds",
            "lower",
            "every attempt in the arm",
            "sum",
        ),
        metric(
            "change".into(),
            "score",
            "higher",
            "cases scored in both arms",
            "difference_of_means",
        ),
    ];
    for case in &suite.cases {
        let id = &case.name;
        out.push(metric(
            format!("case.{id}.score"),
            "score",
            "higher",
            "scored runs of the case",
            "mean",
        ));
        out.push(metric(
            format!("case.{id}.runs_passed"),
            "runs",
            "higher",
            "planned runs of the case",
            "count",
        ));
        out.push(metric(
            format!("case.{id}.change"),
            "score",
            "higher",
            "the case, when scored in both arms",
            "difference",
        ));
    }
    out
}

/// The runs document and each run's grades, written into `artifacts`.
pub(crate) fn runs_artifact(runs: &[GradedRun], artifacts: &mut Artifacts) -> ArtifactRef {
    let mut entries = Vec::with_capacity(runs.len());
    for run in runs {
        let grades = put(
            artifacts,
            &format!(
                "grades/{}/{}-{}.json",
                run.case,
                run.arm.word(),
                run.attempt
            ),
            &json!({
                "v": GRADES_SCHEMA,
                "requires": [],
                "case": run.case,
                "arm": run.arm.word(),
                "attempt": run.attempt,
                "outcome": run.outcome.coverage_word(),
                "reason": reason(run),
                "score": run.score,
                "passed": run.passed,
                "cost_usd": run.cost_usd,
                "seconds": run.seconds,
                "created_files": run.created_files,
                "graders": run.graders,
            }),
            GRADES_SCHEMA,
        );
        let mut run_artifacts = Vec::new();
        if let Some(trajectory) = &run.trajectory {
            run_artifacts.push(trajectory.value());
        }
        run_artifacts.push(grades.value());
        entries.push(json!({
            "arm": run.arm.word(),
            "case": run.case,
            "attempt": run.attempt,
            "outcome": run.outcome.coverage_word(),
            "receipts": run.receipts.iter().map(ArtifactRef::value).collect::<Vec<_>>(),
            "artifacts": run_artifacts,
        }));
    }
    put(
        artifacts,
        "runs.json",
        &json!({ "v": RUNS_SCHEMA, "requires": [], "runs": entries }),
        RUNS_SCHEMA,
    )
}

/// A run's error reason word, when it ended with one.
#[must_use]
pub fn reason(run: &GradedRun) -> Option<&'static str> {
    match run.outcome {
        crate::record::RunOutcome::Errored(failure) => Some(failure.word()),
        _ => None,
    }
}

fn configuration(
    arm: Arm,
    setup: &ArmSetup,
    doors: &DoorNames,
    artifacts: &mut Artifacts,
) -> Value {
    let reference = put(
        artifacts,
        &format!("{}-configuration.json", arm.word()),
        &json!({
            "v": CONFIGURATION_SCHEMA,
            "requires": [],
            "arm": arm.word(),
            "door": setup.door,
            "decision_door": doors.decision,
            "judge_door": doors.judge,
            "run": setup.run,
        }),
        CONFIGURATION_SCHEMA,
    );
    json!({
        "definition": setup.definition,
        "lock": setup.lock.value(),
        "configuration": reference.value(),
    })
}

/// Everything the report is assembled from.
pub(crate) struct Parts<'a> {
    pub suite: &'a Suite,
    pub identity: &'a Identity,
    pub doors: &'a DoorNames,
    pub scores: &'a Scores,
    pub runs: &'a [GradedRun],
    pub verdict: &'a str,
    pub gate_digest: &'a str,
    pub gate_file: &'a [u8],
    pub limitations: &'a [String],
    pub partial: Option<&'a str>,
}

/// Builds the report and writes every document it references.
pub(crate) fn build(parts: &Parts<'_>, artifacts: &mut Artifacts) -> Value {
    let (suite_ref, partition) =
        suite_artifacts(parts.suite, parts.identity, parts.gate_file, artifacts);
    let runs_ref = runs_artifact(parts.runs, artifacts);
    let limitations = put(
        artifacts,
        "limitations.json",
        &json!({
            "v": LIMITATIONS_SCHEMA,
            "requires": [],
            "limitations": parts.limitations,
            "partial": parts.partial,
        }),
        LIMITATIONS_SCHEMA,
    );
    let subject = configuration(
        Arm::Subject,
        &parts.identity.subject,
        parts.doors,
        artifacts,
    );
    let baseline = match (&parts.scores.baseline, &parts.identity.baseline) {
        (Some(_), Some(setup)) => configuration(Arm::Baseline, setup, parts.doors, artifacts),
        _ => Value::Null,
    };
    let coverage =
        |summary: &ArmSummary| serde_json::to_value(summary.coverage).unwrap_or_default();
    let evidence = vec![runs_ref.value()];
    let measurements = measurements(parts.scores, &evidence);
    let headline = json!({
        "subject_passed": parts.scores.subject.cases_passed,
        "baseline_passed": parts.scores.baseline.as_ref().map(|b| b.cases_passed),
        "total": parts.suite.cases.len(),
    });
    let reliance = reliance(parts);
    json!({
        "v": REPORT_SCHEMA,
        "requires": [],
        "suite": suite_ref.value(),
        "partition": partition.value(),
        "subject": subject,
        "baseline": baseline,
        "evaluator": parts.identity.evaluator,
        "started_at": parts.identity.started_at,
        "ended_at": parts.identity.ended_at,
        "runs": runs_ref.value(),
        "coverage": {
            "subject": coverage(&parts.scores.subject),
            "baseline": parts.scores.baseline.as_ref().map(coverage),
        },
        "measurements": measurements,
        "verdict": parts.verdict,
        "limitations": limitations.value(),
        "meta": {
            "ext_eval": {
                "v": PROFILE_SCHEMA,
                "gate": wire_gate(parts.gate_digest),
                "cases": parts.suite.cases.iter().map(|case| json!({
                    "id": case.name,
                    "kind": case.kind.word(),
                })).collect::<Vec<_>>(),
                "headline": headline,
                "requester": parts.identity.requester,
                "reliance": reliance,
                "identity": "content",
            }
        },
    })
}

/// What the run relied on beyond the extension it measured, as
/// `meta.ext_eval.reliance` (NIP-EVAL): the hosted runner for a hosted
/// run, the machine as a digest of its name (never the name), the pinned
/// chat door, the decision door the selector used, and this crate as the
/// agent harness and grader implementation. A reader compares it with a
/// rerun's to see what the two shared. The model behind the door and its
/// version are what the door reports, which this runner doesn't read yet,
/// so `model` is unknown rather than guessed.
fn reliance(parts: &Parts<'_>) -> Value {
    let engine = format!("ext-eval@{}", env!("CARGO_PKG_VERSION"));
    let host = std::env::var("HOSTNAME")
        .ok()
        .filter(|name| !name.is_empty())
        .map(|name| nostr::contracts::digest_bytes(name.as_bytes()));
    json!({
        "runner": parts
            .identity
            .requester
            .as_ref()
            .map(|_| parts.identity.evaluator.clone()),
        "host": host,
        "door": parts.identity.subject.door,
        "model": Value::Null,
        "agent": engine,
        "selector": parts.doors.decision,
        "graders": engine,
    })
}

fn measurement(
    arm: &str,
    metric: String,
    value: Option<f64>,
    denominator: usize,
    unknown: usize,
    evidence: &[Value],
) -> Value {
    json!({
        "arm": arm,
        "metric": metric,
        "value": value.filter(|value| value.is_finite()),
        "denominator": denominator,
        "unknown_count": unknown,
        "uncertainty": null,
        "evidence": evidence,
    })
}

#[allow(clippy::cast_precision_loss)]
fn measurements(scores: &Scores, evidence: &[Value]) -> Vec<Value> {
    let mut out = Vec::new();
    let arms = [
        (Arm::Subject, Some(&scores.subject)),
        (Arm::Baseline, scores.baseline.as_ref()),
    ];
    for (arm, summary) in arms {
        let Some(summary) = summary else { continue };
        let word = arm.word();
        let planned = summary.coverage.planned as usize;
        let with_score = scores
            .cases
            .iter()
            .filter_map(|case| side(case, arm))
            .filter(|case| case.score.is_some())
            .count();
        let attempted = summary.coverage.attempted as usize;
        out.push(measurement(
            word,
            "cases_passed".into(),
            Some(summary.cases_passed as f64),
            summary.cases_scored,
            summary.cases_unknown,
            evidence,
        ));
        out.push(measurement(
            word,
            "mean_score".into(),
            summary.mean_score,
            with_score,
            planned - with_score,
            evidence,
        ));
        out.push(measurement(
            word,
            "cost_usd".into(),
            summary.cost_usd,
            attempted,
            summary.cost_unknown,
            evidence,
        ));
        out.push(measurement(
            word,
            "seconds".into(),
            summary.seconds,
            attempted,
            summary.seconds_unknown,
            evidence,
        ));
        for case in &scores.cases {
            let Some(side) = side(case, arm) else {
                continue;
            };
            let planned = side.planned as usize;
            let scored = side.scored as usize;
            out.push(measurement(
                word,
                format!("case.{}.score", case.id),
                side.score,
                scored,
                planned - scored,
                evidence,
            ));
            out.push(measurement(
                word,
                format!("case.{}.runs_passed", case.id),
                Some(f64::from(side.runs_passed)),
                planned,
                planned - scored,
                evidence,
            ));
        }
    }
    if scores.baseline.is_some() {
        let baseline_cases = scores.cases.iter().filter(|case| case.baseline.is_some());
        let compared = scores.cases.iter().filter(|case| case.compared).count();
        let uncompared = baseline_cases.filter(|case| !case.compared).count();
        out.push(measurement(
            "comparison",
            "change".into(),
            scores.change,
            compared,
            uncompared,
            evidence,
        ));
        for case in scores.cases.iter().filter(|case| case.baseline.is_some()) {
            out.push(measurement(
                "comparison",
                format!("case.{}.change", case.id),
                case.change,
                usize::from(case.compared),
                usize::from(!case.compared),
                evidence,
            ));
        }
    }
    out
}

fn side(case: &crate::score::CaseScore, arm: Arm) -> Option<&CaseArm> {
    match arm {
        Arm::Subject => Some(&case.subject),
        Arm::Baseline => case.baseline.as_ref(),
    }
}

/// The report's top-level keys, a closed set.
pub const REPORT_KEYS: [&str; 16] = [
    "v",
    "requires",
    "suite",
    "partition",
    "subject",
    "baseline",
    "evaluator",
    "started_at",
    "ended_at",
    "runs",
    "coverage",
    "measurements",
    "verdict",
    "limitations",
    "meta",
    "optimization",
];

/// Checks a report against the NIP-EVAL report contract and the extension
/// evaluation profile, the way a reader would before trusting it.
///
/// `artifacts`, when given, are the documents the report references; the
/// check then also confirms each referenced document's bytes, that every
/// measured metric is one the suite defines, and that the runs document
/// lists exactly the attempts the coverage counts.
///
/// # Errors
///
/// Returns every problem found, one line each.
pub fn validate(report: &Value, artifacts: Option<&Artifacts>) -> Result<(), Vec<String>> {
    let mut problems = Vec::new();
    let mut problem = |text: String| problems.push(text);
    let Some(object) = report.as_object() else {
        return Err(vec!["the report is not a JSON object".into()]);
    };
    // The wire contract as `crates/nostr` reads it, then the checks that
    // need the referenced documents.
    let bytes = serde_json::to_vec(report).unwrap_or_default();
    if let Err(error) = nostr::eval_ext::parse_report(&bytes) {
        problem(format!("NIP-EVAL ext-eval profile: {error}"));
    }
    for key in object.keys() {
        if !REPORT_KEYS.contains(&key.as_str()) {
            problem(format!("unknown report key `{key}`"));
        }
    }
    for key in REPORT_KEYS.iter().filter(|key| **key != "optimization") {
        if !object.contains_key(*key) {
            problem(format!("the report has no `{key}`"));
        }
    }
    if object.get("v").and_then(Value::as_str) != Some(REPORT_SCHEMA) {
        problem(format!("`v` is not {REPORT_SCHEMA}"));
    }
    if !object.get("requires").is_some_and(Value::is_array) {
        problem("`requires` is not an array".into());
    }
    for key in ["suite", "partition", "runs", "limitations"] {
        if let Some(value) = object.get(key)
            && let Err(error) = nostr::contracts::parse_artifact(value)
        {
            problem(format!("`{key}` is not an ArtifactRef: {error}"));
        }
    }
    let baseline_present = !object.get("baseline").is_none_or(Value::is_null);
    for arm in ["subject", "baseline"] {
        match object.get(arm) {
            Some(Value::Null) if arm == "baseline" => {}
            Some(Value::Object(side)) => {
                let keys: BTreeSet<&str> = side.keys().map(String::as_str).collect();
                if keys != BTreeSet::from(["definition", "lock", "configuration"]) {
                    problem(format!(
                        "`{arm}` must be {{definition, lock, configuration}}"
                    ));
                }
                if let Some(definition) = side.get("definition")
                    && let Err(error) = nostr::contracts::parse_definition(definition)
                {
                    problem(format!(
                        "`{arm}.definition` is not a DefinitionRef: {error}"
                    ));
                }
                for key in ["lock", "configuration"] {
                    if let Some(value) = side.get(key)
                        && let Err(error) = nostr::contracts::parse_artifact(value)
                    {
                        problem(format!("`{arm}.{key}` is not an ArtifactRef: {error}"));
                    }
                }
            }
            _ => problem(format!("`{arm}` is not a subject object")),
        }
    }
    if !object
        .get("evaluator")
        .and_then(Value::as_str)
        .is_some_and(|evaluator| !evaluator.trim().is_empty())
    {
        problem("`evaluator` is empty".into());
    }
    match (
        object.get("started_at").and_then(Value::as_u64),
        object.get("ended_at").and_then(Value::as_u64),
    ) {
        (Some(start), Some(end)) if start <= end => {}
        (Some(_), Some(_)) => problem("`started_at` is after `ended_at`".into()),
        _ => problem("`started_at` and `ended_at` must be Unix seconds".into()),
    }
    let mut attempted = BTreeMap::new();
    match object.get("coverage").and_then(Value::as_object) {
        Some(coverage) => {
            for arm in ["subject", "baseline"] {
                match coverage.get(arm) {
                    Some(Value::Null) | None if arm == "baseline" && !baseline_present => {}
                    Some(Value::Object(counts)) if arm == "subject" || baseline_present => {
                        check_coverage(arm, counts, &mut problem);
                        if let Some(count) = counts.get("attempted").and_then(Value::as_u64) {
                            attempted.insert(arm, count);
                        }
                    }
                    _ => problem(format!(
                        "`coverage.{arm}` must be counts when the arm ran and null otherwise"
                    )),
                }
            }
        }
        None => problem("`coverage` is not an object".into()),
    }
    let metric_ids = artifacts.and_then(|artifacts| suite_metrics(report, artifacts));
    match object.get("measurements").and_then(Value::as_array) {
        Some(entries) => {
            for (index, entry) in entries.iter().enumerate() {
                check_measurement(
                    index,
                    entry,
                    baseline_present,
                    metric_ids.as_ref(),
                    &mut problem,
                );
            }
        }
        None => problem("`measurements` is not an array".into()),
    }
    match object.get("verdict").and_then(Value::as_str) {
        Some("pass" | "fail") if !baseline_present => {
            problem(
                "a report without a baseline can't claim a change; its verdict is inconclusive"
                    .into(),
            );
        }
        Some("pass" | "fail" | "inconclusive") => {}
        _ => problem("`verdict` must be pass, fail, or inconclusive".into()),
    }
    check_profile(object, &mut problem);
    if let Some(artifacts) = artifacts {
        check_artifacts(report, artifacts, &attempted, &mut problem);
    }
    if problems.is_empty() {
        Ok(())
    } else {
        Err(problems)
    }
}

fn check_coverage(arm: &str, counts: &Map<String, Value>, problem: &mut impl FnMut(String)) {
    const KEYS: [&str; 8] = [
        "planned",
        "attempted",
        "completed",
        "refused",
        "failed",
        "cancelled",
        "unknown",
        "excluded",
    ];
    let keys: BTreeSet<&str> = counts.keys().map(String::as_str).collect();
    if keys != KEYS.iter().copied().collect() {
        problem(format!(
            "`coverage.{arm}` must have exactly {}",
            KEYS.join(", ")
        ));
        return;
    }
    let get = |key: &str| counts.get(key).and_then(Value::as_u64);
    if KEYS.iter().any(|key| get(key).is_none()) {
        problem(format!(
            "`coverage.{arm}` counts must be nonnegative integers"
        ));
        return;
    }
    let terminal: u64 = ["completed", "refused", "failed", "cancelled", "unknown"]
        .iter()
        .filter_map(|key| get(key))
        .sum();
    if Some(terminal) != get("attempted") {
        problem(format!(
            "`coverage.{arm}`: completed, refused, failed, cancelled, and unknown sum to \
             {terminal}, not the {} attempted",
            get("attempted").unwrap_or_default()
        ));
    }
}

fn check_measurement(
    index: usize,
    entry: &Value,
    baseline_present: bool,
    metric_ids: Option<&BTreeSet<String>>,
    problem: &mut impl FnMut(String),
) {
    let at = format!("measurements[{index}]");
    let Some(entry) = entry.as_object() else {
        problem(format!("{at} is not an object"));
        return;
    };
    let keys: BTreeSet<&str> = entry.keys().map(String::as_str).collect();
    let expected = BTreeSet::from([
        "arm",
        "metric",
        "value",
        "denominator",
        "unknown_count",
        "uncertainty",
        "evidence",
    ]);
    if keys != expected {
        problem(format!(
            "{at} must have exactly arm, metric, value, denominator, unknown_count, uncertainty, \
             and evidence"
        ));
        return;
    }
    match entry.get("arm").and_then(Value::as_str) {
        Some("subject" | "comparison") => {}
        Some("baseline") if baseline_present => {}
        Some("baseline") => problem(format!("{at} measures a baseline the report doesn't have")),
        _ => problem(format!("{at}.arm must be subject, baseline, or comparison")),
    }
    match entry.get("metric").and_then(Value::as_str) {
        Some(metric) if metric_ids.is_none_or(|ids| ids.contains(metric)) => {}
        Some(metric) => problem(format!("{at}.metric `{metric}` is not a suite metric")),
        None => problem(format!("{at}.metric must be a string")),
    }
    match entry.get("value") {
        Some(Value::Null) => {}
        Some(Value::Number(number)) if number.as_f64().is_some_and(f64::is_finite) => {}
        _ => problem(format!("{at}.value must be a finite number or null")),
    }
    for key in ["denominator", "unknown_count"] {
        if entry.get(key).and_then(Value::as_u64).is_none() {
            problem(format!("{at}.{key} must be a nonnegative integer"));
        }
    }
    match entry.get("uncertainty") {
        Some(Value::Null) => {}
        Some(value) if nostr::contracts::parse_artifact(value).is_ok() => {}
        _ => problem(format!("{at}.uncertainty must be an ArtifactRef or null")),
    }
    match entry.get("evidence").and_then(Value::as_array) {
        Some(items) => {
            if items
                .iter()
                .any(|item| nostr::contracts::parse_artifact(item).is_err())
            {
                problem(format!(
                    "{at}.evidence holds something that is not an ArtifactRef"
                ));
            }
        }
        None => problem(format!("{at}.evidence must be an array")),
    }
}

fn check_profile(object: &Map<String, Value>, problem: &mut impl FnMut(String)) {
    match object.get("meta").and_then(|meta| meta.get("ext_eval")) {
        Some(profile) => {
            if let Err(error) = nostr::eval_ext::parse_profile(profile) {
                problem(format!("`meta.ext_eval`: {error}"));
            }
        }
        None => problem("`meta.ext_eval` is missing".into()),
    }
}

/// The gate digest as `meta.ext_eval.gate` carries it: the Gym's
/// `gate:<sha256>` rule digest, written as a `sha256:` Digest.
#[must_use]
pub fn wire_gate(digest: &str) -> String {
    format!("sha256:{}", digest.strip_prefix("gate:").unwrap_or(digest))
}

/// The metric ids the report's suite defines, when its documents are here.
fn suite_metrics(report: &Value, artifacts: &Artifacts) -> Option<BTreeSet<String>> {
    let suite = find(artifacts, report.get("suite")?)?;
    let metrics = find(artifacts, suite.get("metrics")?)?;
    Some(
        metrics
            .get("metrics")?
            .as_array()?
            .iter()
            .filter_map(|metric| metric.get("id").and_then(Value::as_str).map(str::to_string))
            .collect(),
    )
}

/// The document `reference` names, when its exact bytes are among the
/// artifacts.
fn find(artifacts: &Artifacts, reference: &Value) -> Option<Value> {
    let parsed = nostr::contracts::parse_artifact(reference).ok()?;
    artifacts
        .values()
        .find(|bytes| nostr::contracts::check_artifact_bytes(&parsed, bytes).is_ok())
        .and_then(|bytes| serde_json::from_slice(bytes).ok())
}

fn check_artifacts(
    report: &Value,
    artifacts: &Artifacts,
    attempted: &BTreeMap<&str, u64>,
    problem: &mut impl FnMut(String),
) {
    for key in ["suite", "partition", "runs", "limitations"] {
        if let Some(reference) = report.get(key)
            && find(artifacts, reference).is_none()
        {
            problem(format!(
                "no artifact holds the exact bytes `{key}` references"
            ));
        }
    }
    if let Some(runs) = report
        .get("runs")
        .and_then(|reference| find(artifacts, reference))
    {
        let entries = runs
            .get("runs")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        for arm in ["subject", "baseline"] {
            let listed = entries
                .iter()
                .filter(|entry| entry.get("arm").and_then(Value::as_str) == Some(arm))
                .count() as u64;
            if attempted.get(arm).copied().unwrap_or(0) != listed {
                problem(format!(
                    "the runs document lists {listed} {arm} attempts, and coverage counts {}",
                    attempted.get(arm).copied().unwrap_or(0)
                ));
            }
        }
        let mut seen = BTreeSet::new();
        for entry in &entries {
            let key = (
                entry
                    .get("arm")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                entry
                    .get("case")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                entry
                    .get("attempt")
                    .and_then(Value::as_u64)
                    .unwrap_or_default(),
            );
            if !seen.insert(key.clone()) {
                problem(format!(
                    "the runs document counts {} attempt {} of `{}` twice",
                    key.0, key.2, key.1
                ));
            }
        }
    }
}
