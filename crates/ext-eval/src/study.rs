//! A retained evaluation, reopened and recomputed (#10663).
//!
//! [`reopen`] reads a results directory a run left (`report.json`,
//! `artifacts/`, `runs/`, `suite/`, and `published.json` when it was
//! published) and recomputes what the report claims from every retained
//! attempt: it rebuilds each run from its grades document, scores the
//! runs again with [`Scores::compute_with`], asks the gate the suite names
//! for the verdict, and compares the coverage, the measurements, and the
//! verdict with the report's. Reading runs nothing, calls no door, and
//! publishes nothing.
//!
//! What the reader concludes is one of three things, never more than the
//! files support: the retained attempts reproduce the report
//! ([`Agreement::Agrees`]); they contradict it ([`Agreement::Disputes`]);
//! or something the recomputation needs is missing, including the gate
//! itself on a computer that doesn't hold it ([`Agreement::Unverifiable`]).
//! Only a report that agrees shows its verdict's words; an inconclusive
//! verdict stays "No clear change", and a missing cost or time stays
//! unknown in its arm's count.
//!
//! [`list`] finds the results directories under a working directory, for
//! a page that lists them before one is opened.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::Value;

use crate::case::{Case, LoadOptions, RunFailure};
use crate::discover::Suite;
use crate::evaluate::{Verdict, load_gate_named};
use crate::record::{Arm, RunOutcome, run_path};
use crate::report::{Artifacts, GRADES_SCHEMA, find, measurements, validate, wire_gate};
use crate::score::{CasePlan, CaseScore, Coverage, GradedRun, Scores};

/// The schema of what [`reopen`] answers.
pub const STUDY_SCHEMA: &str = "openagents.ext-eval-study.v1";
/// The schema of what [`list`] answers.
pub const STUDIES_SCHEMA: &str = "openagents.ext-eval-studies.v1";
/// The most files `artifacts/` may hold for a reader to load it.
pub const MAX_FILES: usize = 20_000;
/// The most bytes `artifacts/` may hold for a reader to load it.
pub const MAX_BYTES: u64 = 256 * 1024 * 1024;
/// The most results directories [`list`] answers.
pub const MAX_LISTED: usize = 200;

/// Whether the retained attempts reproduce the report.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Agreement {
    /// Every retained attempt is there, and recomputing them gives the
    /// report's coverage, measurements, and verdict.
    Agrees,
    /// The retained files contradict the report.
    Disputes,
    /// Something the recomputation needs is missing.
    Unverifiable,
}

/// What a retained file's bytes are, against the digest that names them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Kept {
    /// The bytes are there and match.
    Retained,
    /// The file is there and its bytes differ.
    Changed,
    /// The file is not there.
    Missing,
    /// The run wrote none.
    None,
}

/// One arm's identity as the report names it.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Side {
    /// The arm's DefinitionRef: the plugin release for the subject.
    pub definition: Value,
    /// The run lock's ArtifactRef.
    pub lock: Value,
}

/// One retained attempt.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Attempt {
    /// The case.
    pub case: String,
    /// The arm.
    pub arm: Arm,
    /// The attempt, from 1.
    pub attempt: u32,
    /// The NIP-EVAL outcome word.
    pub outcome: String,
    /// The error reason, when it ended with one.
    pub reason: Option<String>,
    /// Its score, when scored.
    pub score: Option<f64>,
    /// Whether it passed, when scored.
    pub passed: Option<bool>,
    /// What it cost; unknown when absent.
    pub cost_usd: Option<f64>,
    /// Wall seconds; unknown when absent.
    pub seconds: Option<f64>,
    /// Its grades document.
    pub grades: Kept,
    /// Its trajectory under `runs/`.
    pub trajectory: Kept,
}

/// One arm's totals. A cost or time is a sum only when every attempt's is
/// known; otherwise it is absent and `*_unknown` counts the attempts
/// without one.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Totals {
    /// What the arm's runs cost.
    pub cost_usd: Option<f64>,
    /// Attempts whose cost is unknown.
    pub cost_unknown: usize,
    /// The arm's wall seconds.
    pub seconds: Option<f64>,
    /// Attempts whose time is unknown.
    pub seconds_unknown: usize,
    /// Cases whose pass is decided, passed, and unknown.
    pub cases_scored: usize,
    /// Cases that passed.
    pub cases_passed: usize,
    /// Cases whose pass is unknown.
    pub cases_unknown: usize,
}

/// A retained evaluation, reopened.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Study {
    /// [`STUDY_SCHEMA`].
    pub v: &'static str,
    /// The results directory.
    pub dir: String,
    /// `report.json`'s digest.
    pub report: Option<String>,
    /// The suite's qualified ID.
    pub suite: Option<String>,
    /// The suite's ArtifactRef, with its release EventRef when published.
    pub suite_ref: Value,
    /// The gate the suite names, by ID.
    pub gate: Option<String>,
    /// The gate's digest as the report records it.
    pub gate_digest: Option<String>,
    /// The subject arm: the plugin release.
    pub subject: Option<Side>,
    /// The baseline arm, when one ran.
    pub baseline: Option<Side>,
    /// The evaluator.
    pub evaluator: Option<String>,
    /// When the first run started, Unix seconds.
    pub started_at: Option<u64>,
    /// When the last run ended, Unix seconds.
    pub ended_at: Option<u64>,
    /// The `coder-defaults` release a marginal run held in both arms.
    pub defaults: Option<Value>,
    /// The cases the partition puts in development and held out.
    pub development: usize,
    /// Held-out (confirmation) cases.
    pub held_out: usize,
    /// The report's verdict word.
    pub reported: Option<String>,
    /// The verdict the retained attempts give, when the gate is here.
    pub recomputed: Option<String>,
    /// Whether they agree.
    pub agreement: Agreement,
    /// The words to show: the verdict's when they agree, otherwise
    /// `Disputed` or `Unverified`.
    pub shown: String,
    /// Recomputed coverage.
    pub coverage: BTreeMap<&'static str, Coverage>,
    /// Recomputed totals.
    pub totals: BTreeMap<&'static str, Totals>,
    /// Recomputed per-case scores, in suite order.
    pub cases: Vec<CaseScore>,
    /// Every retained attempt, as the runs document lists them.
    pub attempts: Vec<Attempt>,
    /// Why the suite didn't finish, when it didn't.
    pub partial: Option<String>,
    /// The report's limitations.
    pub limitations: Vec<String>,
    /// The result publication's event ID, when this directory was published.
    pub published: Option<String>,
    /// Where it was published.
    pub relay: Option<String>,
    /// What contradicts the report.
    pub problems: Vec<String>,
    /// What the recomputation needed and didn't find.
    pub missing: Vec<String>,
}

impl Study {
    fn empty(dir: &Path) -> Self {
        Self {
            v: STUDY_SCHEMA,
            dir: dir.display().to_string(),
            report: None,
            suite: None,
            suite_ref: Value::Null,
            gate: None,
            gate_digest: None,
            subject: None,
            baseline: None,
            evaluator: None,
            started_at: None,
            ended_at: None,
            defaults: None,
            development: 0,
            held_out: 0,
            reported: None,
            recomputed: None,
            agreement: Agreement::Unverifiable,
            shown: "Unverified".into(),
            coverage: BTreeMap::new(),
            totals: BTreeMap::new(),
            cases: Vec::new(),
            attempts: Vec::new(),
            partial: None,
            limitations: Vec::new(),
            published: None,
            relay: None,
            problems: Vec::new(),
            missing: Vec::new(),
        }
    }

    fn conclude(mut self) -> Self {
        self.agreement = if !self.problems.is_empty() {
            Agreement::Disputes
        } else if !self.missing.is_empty() || self.recomputed.is_none() {
            Agreement::Unverifiable
        } else {
            Agreement::Agrees
        };
        self.shown = match self.agreement {
            Agreement::Agrees => self
                .recomputed
                .as_deref()
                .and_then(verdict_of)
                .map_or("Unverified", Verdict::plain)
                .to_string(),
            Agreement::Disputes => "Disputed".into(),
            Agreement::Unverifiable => "Unverified".into(),
        };
        self
    }
}

fn verdict_of(word: &str) -> Option<Verdict> {
    match word {
        "pass" => Some(Verdict::Pass),
        "fail" => Some(Verdict::Fail),
        "inconclusive" => Some(Verdict::Inconclusive),
        _ => None,
    }
}

fn failure_of(word: &str) -> Option<RunFailure> {
    [
        RunFailure::Timeout,
        RunFailure::Refused,
        RunFailure::CostCeiling,
        RunFailure::AuthFailed,
        RunFailure::EnvVarRejected,
        RunFailure::UnconfinedHost,
    ]
    .into_iter()
    .find(|failure| failure.word() == word)
}

fn arm_of(word: &str) -> Option<Arm> {
    match word {
        "subject" => Some(Arm::Subject),
        "baseline" => Some(Arm::Baseline),
        _ => None,
    }
}

/// Every file under `artifacts/`, by its path there, within the bounds.
fn load_artifacts(dir: &Path) -> Result<Artifacts, String> {
    let root = dir.join("artifacts");
    let mut out = Artifacts::new();
    let mut total = 0u64;
    let mut stack = vec![root.clone()];
    while let Some(at) = stack.pop() {
        let entries =
            std::fs::read_dir(&at).map_err(|error| format!("{}: {error}", at.display()))?;
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_dir() {
                stack.push(path);
            } else if kind.is_file() {
                total += entry.metadata().map_or(0, |meta| meta.len());
                if out.len() >= MAX_FILES || total > MAX_BYTES {
                    return Err("artifacts/ is larger than a reader loads".into());
                }
                let bytes =
                    std::fs::read(&path).map_err(|error| format!("{}: {error}", path.display()))?;
                let name = path
                    .strip_prefix(&root)
                    .map(|rest| rest.to_string_lossy().replace('\\', "/"))
                    .unwrap_or_default();
                out.insert(name, bytes);
            }
        }
    }
    Ok(out)
}

fn kept(bytes: Option<&[u8]>, reference: &Value) -> Kept {
    let Ok(parsed) = nostr::contracts::parse_artifact(reference) else {
        return Kept::Changed;
    };
    match bytes {
        None => Kept::Missing,
        Some(bytes) if nostr::contracts::check_artifact_bytes(&parsed, bytes).is_ok() => {
            Kept::Retained
        }
        Some(_) => Kept::Changed,
    }
}

/// Numbers within a relative billionth, everything else exactly.
fn same(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::Number(a), Value::Number(b)) => match (a.as_f64(), b.as_f64()) {
            (Some(a), Some(b)) => (a - b).abs() <= 1e-9 * a.abs().max(b.abs()).max(1.0),
            _ => a == b,
        },
        (Value::Array(a), Value::Array(b)) => {
            a.len() == b.len() && a.iter().zip(b).all(|(a, b)| same(a, b))
        }
        (Value::Object(a), Value::Object(b)) => {
            a.len() == b.len()
                && a.iter()
                    .all(|(key, value)| b.get(key).is_some_and(|other| same(value, other)))
        }
        _ => left == right,
    }
}

/// Reopens the results directory `dir` and recomputes its report from
/// every retained attempt. Reading only; nothing runs or publishes.
///
/// # Errors
///
/// Returns why `dir` holds no extension evaluation report at all. A report
/// that is there but can't be checked is a [`Study`] that says so.
pub fn reopen(dir: &Path) -> Result<Study, String> {
    let report_path = dir.join("report.json");
    let report_bytes = std::fs::read(&report_path)
        .map_err(|error| format!("{}: {error}", report_path.display()))?;
    let report: Value = serde_json::from_slice(&report_bytes)
        .map_err(|_| format!("{} is not JSON", report_path.display()))?;
    if report.get("v").and_then(Value::as_str) != Some(crate::report::REPORT_SCHEMA) {
        return Err(format!(
            "{} is not an extension evaluation report",
            report_path.display()
        ));
    }
    let mut study = Study::empty(dir);
    study.report = Some(nostr::contracts::digest_bytes(&report_bytes));
    study.suite_ref = report["suite"].clone();
    study.evaluator = report["evaluator"].as_str().map(str::to_string);
    study.started_at = report["started_at"].as_u64();
    study.ended_at = report["ended_at"].as_u64();
    study.reported = report["verdict"].as_str().map(str::to_string);
    study.gate_digest = report["meta"]["ext_eval"]["gate"]
        .as_str()
        .map(str::to_string);
    study.defaults = report["meta"]["ext_eval"]
        .get("defaults")
        .filter(|value| !value.is_null())
        .cloned();
    let side = |arm: &str| {
        report
            .get(arm)
            .filter(|value| value.is_object())
            .map(|value| Side {
                definition: value["definition"].clone(),
                lock: value["lock"].clone(),
            })
    };
    study.subject = side("subject");
    study.baseline = side("baseline");
    if let Ok(bytes) = std::fs::read(dir.join("published.json"))
        && let Ok(record) = serde_json::from_slice::<Value>(&bytes)
    {
        study.published = record["result"].as_str().map(str::to_string);
        study.relay = record["relay"].as_str().map(str::to_string);
    }

    let artifacts = match load_artifacts(dir) {
        Ok(artifacts) => artifacts,
        Err(why) => {
            study.missing.push(why);
            if let Err(problems) = validate(&report, None) {
                study.problems.extend(problems);
            }
            return Ok(study.conclude());
        }
    };
    if let Err(problems) = validate(&report, Some(&artifacts)) {
        for problem in problems {
            if problem.starts_with("no artifact holds") {
                study.missing.push(problem);
            } else {
                study.problems.push(problem);
            }
        }
    }
    if let Some(limitations) = find(&artifacts, &report["limitations"]) {
        study.limitations = limitations["limitations"]
            .as_array()
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default();
        study.partial = limitations["partial"].as_str().map(str::to_string);
    }
    if let Some(partition) = find(&artifacts, &report["partition"]) {
        let count = |key: &str| partition[key].as_array().map_or(0, Vec::len);
        study.development = count("development");
        study.held_out = count("held_out");
    }
    let suite_doc = find(&artifacts, &report["suite"]);
    study.suite = suite_doc
        .as_ref()
        .and_then(|suite| suite["id"].as_str().map(str::to_string));
    study.gate = suite_doc.as_ref().and_then(|suite| {
        suite["acceptance"]["id"]
            .as_str()
            .and_then(|id| id.rsplit('/').next())
            .map(str::to_string)
    });

    // The suite's cases, from `suite/`, checked against the manifest the
    // suite document digests.
    let suite = match Suite::load(&dir.join("suite"), LoadOptions::default()) {
        Ok(suite) => Some(suite),
        Err(error) => {
            study
                .missing
                .push(format!("the suite's cases don't load: {error}"));
            None
        }
    };
    if let (Some(suite), Some(doc)) = (&suite, &suite_doc) {
        let manifest = crate::report::case_manifest(suite);
        if kept(Some(&manifest), &doc["cases"]) != Kept::Retained {
            study
                .problems
                .push("the suite's case files are not the ones the report's suite digests".into());
        }
    }

    // Every retained attempt, rebuilt from its grades document.
    let Some(runs_doc) = find(&artifacts, &report["runs"]) else {
        study
            .missing
            .push("the runs document is not retained".into());
        return Ok(study.conclude());
    };
    let entries = runs_doc["runs"].as_array().cloned().unwrap_or_default();
    let mut graded = Vec::with_capacity(entries.len());
    for entry in &entries {
        let (Some(case), Some(arm), Some(attempt)) = (
            entry["case"].as_str(),
            entry["arm"].as_str().and_then(arm_of),
            entry["attempt"]
                .as_u64()
                .and_then(|attempt| u32::try_from(attempt).ok()),
        ) else {
            study
                .problems
                .push("the runs document holds an entry that names no attempt".into());
            continue;
        };
        let refs = entry["artifacts"].as_array().cloned().unwrap_or_default();
        let grades_ref = refs
            .iter()
            .find(|reference| reference["schema"].as_str() == Some(GRADES_SCHEMA))
            .cloned()
            .unwrap_or(Value::Null);
        let trajectory_ref = refs
            .iter()
            .find(|reference| reference["schema"].as_str() != Some(GRADES_SCHEMA))
            .cloned();
        let grades_path = format!("grades/{case}/{}-{attempt}.json", arm.word());
        let grades = find(&artifacts, &grades_ref);
        let grades_kept = if grades.is_some() {
            Kept::Retained
        } else {
            kept(artifacts.get(&grades_path).map(Vec::as_slice), &grades_ref)
        };
        let trajectory = match &trajectory_ref {
            None => Kept::None,
            Some(reference) => {
                let path = dir
                    .join(run_path(case, arm, attempt))
                    .join("trajectory.json");
                kept(std::fs::read(path).ok().as_deref(), reference)
            }
        };
        let outcome_word = entry["outcome"].as_str().unwrap_or_default().to_string();
        let mut attempt_row = Attempt {
            case: case.to_string(),
            arm,
            attempt,
            outcome: outcome_word.clone(),
            reason: None,
            score: None,
            passed: None,
            cost_usd: None,
            seconds: None,
            grades: grades_kept,
            trajectory,
        };
        match grades_kept {
            Kept::Changed => study.problems.push(format!(
                "the grades of {} attempt {attempt} of `{case}` are not the bytes the runs \
                 document names",
                arm.word()
            )),
            Kept::Missing | Kept::None => study.missing.push(format!(
                "the grades of {} attempt {attempt} of `{case}` are not retained",
                arm.word()
            )),
            Kept::Retained => {}
        }
        if trajectory == Kept::Changed {
            study.problems.push(format!(
                "the trajectory of {} attempt {attempt} of `{case}` is not the one the runs \
                 document names",
                arm.word()
            ));
        }
        if let Some(grades) = grades {
            let reason = grades["reason"].as_str().map(str::to_string);
            let outcome = match (grades["outcome"].as_str(), reason.as_deref()) {
                (Some("completed"), _) => Some(RunOutcome::Completed),
                (Some("cancelled"), _) => Some(RunOutcome::Cancelled),
                (Some("unknown"), _) => Some(RunOutcome::Unknown),
                (Some("refused" | "failed"), Some(reason)) => {
                    failure_of(reason).map(RunOutcome::Errored)
                }
                _ => None,
            };
            let Some(outcome) = outcome.filter(|outcome| outcome.coverage_word() == outcome_word)
            else {
                study.problems.push(format!(
                    "the outcome of {} attempt {attempt} of `{case}` differs between the runs \
                     document and its grades",
                    arm.word()
                ));
                continue;
            };
            attempt_row.reason = reason;
            attempt_row.score = grades["score"].as_f64();
            attempt_row.passed = grades["passed"].as_bool();
            attempt_row.cost_usd = grades["cost_usd"].as_f64();
            attempt_row.seconds = grades["seconds"].as_f64();
            graded.push(GradedRun {
                case: case.to_string(),
                arm,
                attempt,
                outcome,
                graders: Vec::new(),
                score: attempt_row.score,
                passed: attempt_row.passed,
                cost_usd: attempt_row.cost_usd,
                seconds: attempt_row.seconds,
                trajectory: None,
                created_files: Vec::new(),
                changed_files: Vec::new(),
                receipts: Vec::new(),
            });
        }
        study.attempts.push(attempt_row);
    }
    let Some(suite) = suite else {
        return Ok(study.conclude());
    };

    // The plan, case by case, from the attempts the runs document lists:
    // each arm that ran a case ran attempts 1 to N, N being the case's runs
    // or one override for every case.
    let baseline_present = study.baseline.is_some();
    let mut listed: BTreeMap<(&str, Arm), BTreeSet<u32>> = BTreeMap::new();
    for attempt in &study.attempts {
        listed
            .entry((attempt.case.as_str(), attempt.arm))
            .or_default()
            .insert(attempt.attempt);
    }
    let mut plans: BTreeMap<String, CasePlan> = BTreeMap::new();
    let mut overrides = BTreeSet::new();
    let names: BTreeSet<&str> = suite.cases.iter().map(|case| case.name.as_str()).collect();
    for attempt in &study.attempts {
        if !names.contains(attempt.case.as_str()) {
            study.problems.push(format!(
                "the runs document lists `{}`, which the suite does not have",
                attempt.case
            ));
        }
    }
    for case in &suite.cases {
        let subject = listed.get(&(case.name.as_str(), Arm::Subject));
        let baseline = listed.get(&(case.name.as_str(), Arm::Baseline));
        let planned = subject.map_or(0, |attempts| attempts.len() as u32);
        let contiguous = |attempts: Option<&BTreeSet<u32>>| {
            attempts.is_none_or(|attempts| attempts.iter().copied().eq(1..=planned))
        };
        if planned == 0 || !contiguous(subject) || !contiguous(baseline) {
            study.problems.push(format!(
                "the runs document does not list attempts 1 to N of `{}` in each arm that ran it",
                case.name
            ));
        }
        if baseline.is_some() && !baseline_present {
            study.problems.push(format!(
                "`{}` lists baseline attempts in a report without a baseline",
                case.name
            ));
        }
        if planned != case.runs {
            overrides.insert(planned);
        }
        plans.insert(
            case.name.clone(),
            CasePlan {
                planned,
                baseline_runs: baseline_present && baseline.is_some(),
                subject_only: baseline_present && baseline.is_none(),
            },
        );
    }
    let uniform = suite
        .cases
        .iter()
        .map(|case| plans.get(&case.name).map_or(0, |plan| plan.planned))
        .collect::<BTreeSet<_>>()
        .len()
        <= 1;
    if !overrides.is_empty() && !uniform {
        study
            .problems
            .push("the cases' attempts follow neither their own runs nor one override".into());
    }

    // The gate's group is the suite's `<author>:<package>/<component>`.
    let group = study.suite.clone().unwrap_or_default();
    let fallback = CasePlan {
        planned: 0,
        baseline_runs: false,
        subject_only: false,
    };
    let plan_of = |case: &Case| plans.get(&case.name).copied().unwrap_or(fallback);
    let scores = Scores::compute_with(&suite, baseline_present, &plan_of, &graded, &group);

    let mut arms = vec![("subject", &scores.subject)];
    if let Some(baseline) = &scores.baseline {
        arms.push(("baseline", baseline));
    }
    for (word, summary) in arms {
        study.coverage.insert(word, summary.coverage);
        study.totals.insert(
            word,
            Totals {
                cost_usd: summary.cost_usd,
                cost_unknown: summary.cost_unknown,
                seconds: summary.seconds,
                seconds_unknown: summary.seconds_unknown,
                cases_scored: summary.cases_scored,
                cases_passed: summary.cases_passed,
                cases_unknown: summary.cases_unknown,
            },
        );
        let recorded = &report["coverage"][word];
        let ours = serde_json::to_value(summary.coverage).unwrap_or_default();
        if study.missing.is_empty() && !same(recorded, &ours) {
            study.problems.push(format!(
                "the {word} arm's coverage does not recompute from the retained attempts"
            ));
        }
    }
    let ours = Value::from(measurements(&scores, &[report["runs"].clone()]));
    if study.missing.is_empty() && !same(&report["measurements"], &ours) {
        study
            .problems
            .push("the measurements do not recompute from the retained attempts".into());
    }
    study.cases.clone_from(&scores.cases);

    // The verdict, from the gate the suite names, when this computer holds
    // exactly that gate.
    match (&study.gate, &suite_doc) {
        (Some(id), Some(doc)) => match load_gate_named(id) {
            Ok((gate, bytes))
                if kept(Some(&bytes), &doc["acceptance"]["artifact"]) == Kept::Retained =>
            {
                let outcome = gate.judge_ext_eval(&scores.comparison);
                if study.gate_digest.as_deref() != Some(wire_gate(&outcome.gate_digest).as_str()) {
                    study
                        .problems
                        .push("the report names another digest for its gate".into());
                }
                let verdict = Verdict::of(outcome.verdict);
                study.recomputed = Some(verdict.word().to_string());
                if study.missing.is_empty() && study.reported.as_deref() != Some(verdict.word()) {
                    study.problems.push(format!(
                        "the retained attempts give {}, and the report says {}",
                        verdict.word(),
                        study.reported.as_deref().unwrap_or("nothing")
                    ));
                }
            }
            Ok(_) => study.missing.push(format!(
                "this computer's {id} gate is not the version the suite names"
            )),
            Err(error) => study
                .missing
                .push(format!("the {id} gate is not on this computer: {error}")),
        },
        _ => study.missing.push("the suite names no gate".into()),
    }
    Ok(study.conclude())
}

/// One results directory, as [`list`] finds it.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Listed {
    /// The results directory.
    pub dir: String,
    /// The suite's qualified ID, from the report's profile.
    pub subject: Option<String>,
    /// The report's verdict word, unchecked.
    pub reported: Option<String>,
    /// When the last run ended, Unix seconds.
    pub ended_at: Option<u64>,
}

/// The results directories under `root`: `results/*`, `evals/results/*`,
/// and `*/evals/results/*`, newest first, at most [`MAX_LISTED`]. Listing
/// reads each `report.json` and checks nothing; [`reopen`] checks one.
#[must_use]
pub fn list(root: &Path) -> Vec<Listed> {
    let mut bases = vec![root.join("results"), root.join("evals/results")];
    if let Ok(entries) = std::fs::read_dir(root) {
        let mut more: Vec<PathBuf> = entries
            .flatten()
            .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
            .map(|entry| entry.path().join("evals/results"))
            .collect();
        more.sort();
        bases.extend(more);
    }
    let mut out = Vec::new();
    for base in bases {
        let Ok(entries) = std::fs::read_dir(&base) else {
            continue;
        };
        for entry in entries.flatten() {
            let dir = entry.path();
            let Ok(bytes) = std::fs::read(dir.join("report.json")) else {
                continue;
            };
            let Ok(report) = serde_json::from_slice::<Value>(&bytes) else {
                continue;
            };
            if report.get("v").and_then(Value::as_str) != Some(crate::report::REPORT_SCHEMA) {
                continue;
            }
            let subject = report["subject"]["definition"]["id"]
                .as_str()
                .map(str::to_string);
            out.push(Listed {
                dir: dir.display().to_string(),
                subject,
                reported: report["verdict"].as_str().map(str::to_string),
                ended_at: report["ended_at"].as_u64(),
            });
            if out.len() >= MAX_LISTED * 4 {
                break;
            }
        }
    }
    out.sort_by(|left, right| {
        right
            .ended_at
            .cmp(&left.ended_at)
            .then_with(|| left.dir.cmp(&right.dir))
    });
    out.truncate(MAX_LISTED);
    out
}
