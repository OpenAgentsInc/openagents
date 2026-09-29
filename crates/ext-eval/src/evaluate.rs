//! From a suite and finished runs to a verdict and a report.
//!
//! [`evaluate`] grades every planned run and then [`conclude`]s: it scores
//! the runs, asks the Gym gate `ext-eval-v2` for the verdict, and builds
//! `report.json`, the documents it references, and `report.html`. Grading
//! is the only step that calls a door; a runner that grades runs as they
//! finish calls [`crate::score::grade_all`] itself and then [`conclude`].

use std::path::Path;

use gym::gate::{Gate, Outcome, Rule, Verdict as GateVerdict};
use serde_json::Value;

use crate::artifact::{ArtifactRef, JSON, json_bytes};
use crate::discover::Suite;
use crate::door::Doors;
use crate::record::RunRecord;
use crate::report::{Artifacts, DoorNames, Identity, MAX_INLINE_REPORT, Parts, REPORT_SCHEMA};
use crate::score::{GradedRun, Plan, PlanError, Scores, grade_all};

/// The gate every extension evaluation is judged by.
pub const GATE_ID: &str = "ext-eval-v2";

/// A report's verdict.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    /// The gate kept the extension: **Better**.
    Pass,
    /// The gate rejected it: **Worse**.
    Fail,
    /// Neither: **No clear change**.
    Inconclusive,
}

impl Verdict {
    /// The word a report writes.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::Fail => "fail",
            Self::Inconclusive => "inconclusive",
        }
    }

    /// The words the phone shows.
    #[must_use]
    pub const fn plain(self) -> &'static str {
        match self {
            Self::Pass => "Better",
            Self::Fail => "Worse",
            Self::Inconclusive => "No clear change",
        }
    }

    fn of(gate: GateVerdict) -> Self {
        match gate {
            GateVerdict::Passed => Self::Pass,
            GateVerdict::Failed => Self::Fail,
            GateVerdict::Unverifiable => Self::Inconclusive,
        }
    }
}

/// What went wrong concluding an evaluation.
#[derive(Debug, thiserror::Error)]
pub enum EvalError {
    /// The runs don't match the plan.
    #[error(transparent)]
    Plan(#[from] PlanError),
    /// The gate is not an extension evaluation rule.
    #[error("gate {0} does not judge extension evaluations")]
    Gate(String),
    /// The gate file could not be read.
    #[error("the gate could not be loaded: {0}")]
    GateLoad(String),
    /// An identity value that fails the shared contracts.
    #[error("{0}")]
    Identity(String),
}

/// A finished evaluation: the scores, the verdict, and every file the
/// results directory holds.
#[derive(Clone, Debug)]
pub struct Evaluation {
    /// Every planned run, graded.
    pub runs: Vec<GradedRun>,
    /// The scores.
    pub scores: Scores,
    /// The gate's verdict with its criteria and digest.
    pub gate: Outcome,
    /// The report's verdict.
    pub verdict: Verdict,
    /// The evaluator.
    pub evaluator: String,
    /// Why the suite couldn't finish, when it couldn't.
    pub partial: Option<String>,
    /// Explicit limitations, as the report's limitations document lists
    /// them.
    pub limitations: Vec<String>,
    /// Case warnings: graders that can't pass with the operations asked for.
    pub warnings: Vec<String>,
    /// `report.json`.
    pub report: Value,
    /// Its exact bytes.
    pub report_bytes: Vec<u8>,
    /// Their reference.
    pub report_ref: ArtifactRef,
    /// The suite's reference.
    pub suite_ref: ArtifactRef,
    /// The documents the report references, by path under `artifacts/`.
    pub artifacts: Artifacts,
    /// `report.html`.
    pub html: String,
}

/// The suite's own documents, as a run writes them under `artifacts/`:
/// `suite.json` and `cases.json`'s exact bytes, for a suite by `author`
/// published as `<author>:<package>/<component>`. A suite released from
/// these bytes is the suite every later run of the same cases writes,
/// which is what lets a hosted run or a check cite the release before it
/// runs anything.
///
/// # Errors
///
/// Returns [`EvalError::GateLoad`] when the gate does not load.
pub fn suite_documents(
    suite: &Suite,
    author: &str,
    package: &str,
    component: &str,
) -> Result<(Vec<u8>, Vec<u8>), EvalError> {
    suite_documents_under(GATE_ID, suite, author, package, component)
}

/// [`suite_documents`] for a suite judged by the gate `gate_id`
/// (`ext-eval-v2` or `ext-eval-cost-v1`), which its acceptance names.
///
/// # Errors
///
/// Returns [`EvalError::GateLoad`] when the gate does not load.
pub fn suite_documents_under(
    gate_id: &str,
    suite: &Suite,
    author: &str,
    package: &str,
    component: &str,
) -> Result<(Vec<u8>, Vec<u8>), EvalError> {
    let (_, gate_file) = load_gate_named(gate_id)?;
    let placeholder = crate::report::ArmSetup {
        definition: Value::Null,
        lock: ArtifactRef::of(b"", JSON, None),
        door: String::new(),
        run: Value::Null,
    };
    let identity = Identity {
        author: author.to_string(),
        package: package.to_string(),
        component: component.to_string(),
        evaluator: author.to_string(),
        subject: placeholder,
        baseline: None,
        started_at: 0,
        ended_at: 0,
        requester: None,
        suite_release: None,
        environment: None,
        defaults: None,
        partial: None,
    };
    let mut artifacts = Artifacts::new();
    crate::report::suite_artifacts(suite, &identity, &gate_file, &mut artifacts);
    let take = |name: &str| artifacts.get(name).cloned().unwrap_or_default();
    Ok((take("suite.json"), take("cases.json")))
}

/// The time and cost changes, as plain notes beside the verdict: `Faster`,
/// `Slower`, `Cheaper`, or `Costlier`, per run, when both arms measured
/// it and the change clears the spread between repeats. The gate never
/// keeps an extension on these alone (`ext-eval-v2`), so a reader sees
/// them here instead: "Faster: 17.0 s against 31.0 s per run".
#[must_use]
pub fn notes(scores: &Scores) -> Vec<String> {
    let comparison = &scores.comparison;
    if !comparison.baseline_present {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut note = |measure: &gym::gate::ArmMeasure,
                    lower: &str,
                    higher: &str,
                    show: &dyn Fn(f64) -> String| {
        let (Some(with), Some(without)) = (measure.subject, measure.baseline) else {
            return;
        };
        if (with - without).abs() <= measure.spread.unwrap_or(0.0) || with == without {
            return;
        }
        let word = if with < without { lower } else { higher };
        out.push(format!(
            "{word}: {} against {} per run",
            show(with),
            show(without)
        ));
    };
    note(&comparison.seconds, "Faster", "Slower", &|v| {
        format!("{v:.1} s")
    });
    note(&comparison.cost_usd, "Cheaper", "Costlier", &|v| {
        format!("${v:.4}")
    });
    out
}

/// The committed `ext-eval-v2` gate and its file's exact bytes.
///
/// # Errors
///
/// Returns [`EvalError::GateLoad`] when the gate does not load.
pub fn load_gate() -> Result<(Gate, Vec<u8>), EvalError> {
    load_gate_named(GATE_ID)
}

/// A committed extension evaluation gate by ID and its file's exact
/// bytes: `ext-eval-v2` (correctness primary), `ext-eval-cost-v1` (cost
/// primary, correctness held non-inferior), or `ext-eval-v1`.
///
/// # Errors
///
/// Returns [`EvalError::GateLoad`] when the gate does not load, and
/// [`EvalError::Gate`] when it isn't an extension evaluation gate.
pub fn load_gate_named(id: &str) -> Result<(Gate, Vec<u8>), EvalError> {
    let path = gym::gate::gates_dir().join(format!("{id}.json"));
    let bytes = std::fs::read(&path).map_err(|error| EvalError::GateLoad(error.to_string()))?;
    let gate = Gate::load(&path).map_err(|error| EvalError::GateLoad(error.to_string()))?;
    if !matches!(gate.rule, Rule::ExtEval(_)) {
        return Err(EvalError::Gate(gate.id.clone()));
    }
    Ok((gate, bytes))
}

/// Grades every planned run, scores them, and concludes.
///
/// # Errors
///
/// Returns [`EvalError`] for records that don't match the plan, a gate
/// that doesn't judge extension evaluations, or identity values that fail
/// the shared contracts.
pub fn evaluate(
    suite: &Suite,
    plan: &Plan,
    records: Vec<RunRecord>,
    identity: &Identity,
    gate: (&Gate, &[u8]),
    doors: Doors<'_>,
) -> Result<Evaluation, EvalError> {
    let runs = grade_all(suite, plan, records, doors)?;
    let names = DoorNames {
        decision: doors.decision.map(crate::door::DecisionDoor::describe),
        judge: doors.judge.map(crate::door::JudgeDoor::describe),
    };
    conclude(suite, plan, runs, identity, gate, &names)
}

/// Scores graded runs, asks the gate, and builds the report. Pure.
///
/// # Errors
///
/// Returns [`EvalError`] for a gate that doesn't judge extension
/// evaluations, or identity values that fail the shared contracts.
pub fn conclude(
    suite: &Suite,
    plan: &Plan,
    runs: Vec<GradedRun>,
    identity: &Identity,
    (gate, gate_file): (&Gate, &[u8]),
    doors: &DoorNames,
) -> Result<Evaluation, EvalError> {
    if !matches!(gate.rule, Rule::ExtEval(_)) {
        return Err(EvalError::Gate(gate.id.clone()));
    }
    check_identity(identity, plan)?;
    let group = format!(
        "{}:{}/{}",
        identity.author, identity.package, identity.component
    );
    let scores = Scores::compute(suite, plan, &runs, &group);
    let outcome = gate.judge_ext_eval(&scores.comparison);
    let verdict = Verdict::of(outcome.verdict);
    let unfinished = scores.subject.coverage.cancelled
        + scores.subject.coverage.unknown
        + scores
            .baseline
            .as_ref()
            .map_or(0, |b| b.coverage.cancelled + b.coverage.unknown);
    let partial = identity.partial.clone().or_else(|| match unfinished {
        0 => None,
        1 => Some("1 run did not finish".to_string()),
        n => Some(format!("{n} runs did not finish")),
    });
    let limitations = limitations(plan, &scores, identity);
    let warnings = suite
        .cases
        .iter()
        .flat_map(crate::case::Case::warnings)
        .collect();

    let mut artifacts = Artifacts::new();
    let report = crate::report::build(
        &Parts {
            suite,
            identity,
            doors,
            scores: &scores,
            runs: &runs,
            verdict: verdict.word(),
            gate_digest: &outcome.gate_digest,
            gate_file,
            limitations: &limitations,
            partial: partial.as_deref(),
        },
        &mut artifacts,
    );
    let report_bytes = json_bytes(&report);
    let report_ref = ArtifactRef::of(&report_bytes, JSON, Some(REPORT_SCHEMA));
    let suite_ref = report
        .get("suite")
        .cloned()
        .and_then(|value| serde_json::from_value(value).ok())
        .ok_or_else(|| EvalError::Identity("the suite reference did not build".into()))?;
    let mut evaluation = Evaluation {
        runs,
        scores,
        gate: outcome,
        verdict,
        evaluator: identity.evaluator.clone(),
        partial,
        limitations,
        warnings,
        report,
        report_bytes,
        report_ref,
        suite_ref,
        artifacts,
        html: String::new(),
    };
    evaluation.html = crate::html::render(&evaluation);
    Ok(evaluation)
}

fn check_identity(identity: &Identity, plan: &Plan) -> Result<(), EvalError> {
    let hex = |value: &str| {
        value.len() == 64
            && value
                .chars()
                .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c))
    };
    let slug = |value: &str| {
        !value.is_empty()
            && value.len() <= 64
            && value.starts_with(|c: char| c.is_ascii_lowercase() || c.is_ascii_digit())
            && value
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
    };
    let fail = |text: String| Err(EvalError::Identity(text));
    if !hex(&identity.author) {
        return fail("the suite author must be a 64-hex public key".into());
    }
    if !slug(&identity.package) || !slug(&identity.component) {
        return fail("the suite's package and component must be lowercase slugs".into());
    }
    if !hex(&identity.evaluator) {
        return fail("the evaluator must be a 64-hex public key".into());
    }
    if identity.started_at > identity.ended_at {
        return fail("the evaluation ended before it started".into());
    }
    let arms = std::iter::once(("subject", Some(&identity.subject)))
        .chain(std::iter::once(("baseline", identity.baseline.as_ref())));
    for (arm, setup) in arms {
        let Some(setup) = setup else {
            if plan.baseline {
                return fail("the plan runs a baseline arm, and the identity has none".into());
            }
            continue;
        };
        if let Err(error) = nostr::contracts::parse_definition(&setup.definition) {
            return fail(format!(
                "the {arm} definition is not a DefinitionRef: {error}"
            ));
        }
        if let Err(error) = nostr::contracts::parse_artifact(&setup.lock.value()) {
            return fail(format!("the {arm} lock is not an ArtifactRef: {error}"));
        }
    }
    if let Some(requester) = &identity.requester
        && let Err(error) = nostr::eval_ext::event_pointer(
            requester,
            nostr::kinds::CJ_EXECUTION_REQUEST,
            "requester",
        )
    {
        return fail(format!(
            "the requester is not a NIP-CJ execution request EventRef: {error}"
        ));
    }
    if let Some(release) = &identity.suite_release
        && let Err(error) =
            nostr::eval_ext::event_pointer(release, nostr::kinds::EXT_RELEASE, "suite_release")
    {
        return fail(format!(
            "the suite release is not a NIP-EXT release EventRef: {error}"
        ));
    }
    Ok(())
}

fn limitations(plan: &Plan, scores: &Scores, identity: &Identity) -> Vec<String> {
    let mut out = vec![
        "A suite written by an extension's author describes intended behavior; it is not \
         independent evidence."
            .to_string(),
        "Trajectories stay private; the report carries their digests.".to_string(),
        "Metric aggregations are named ext-eval.v1 operations, not DefinitionRefs.".to_string(),
    ];
    if !plan.baseline {
        out.push("No baseline arm ran, so the report claims no change.".into());
    }
    let excluded: Vec<&str> = scores
        .cases
        .iter()
        .filter(|case| case.subject_only)
        .map(|case| case.id.as_str())
        .collect();
    if !excluded.is_empty() {
        out.push(format!(
            "Scored with the extension only and excluded from the change: {}.",
            excluded.join(", ")
        ));
    }
    if let Some(runs) = plan.runs {
        out.push(format!(
            "Every case ran {runs} times per arm, overriding the cases' runs."
        ));
    }
    if identity
        .baseline
        .as_ref()
        .is_some_and(|baseline| baseline.door != identity.subject.door)
    {
        out.push("The subject and baseline arms used different doors.".into());
    }
    out
}

impl Evaluation {
    /// Writes `report.json`, `report.html`, and `artifacts/` into `dir`.
    /// The runner writes `runs/` beside them.
    ///
    /// # Errors
    ///
    /// Returns the I/O error of the first file that could not be written.
    pub fn write(&self, dir: &Path) -> std::io::Result<()> {
        std::fs::create_dir_all(dir)?;
        std::fs::write(dir.join("report.json"), &self.report_bytes)?;
        std::fs::write(dir.join("report.html"), &self.html)?;
        for (name, bytes) in &self.artifacts {
            let path = dir.join("artifacts").join(name);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(path, bytes)?;
        }
        Ok(())
    }

    /// The command's exit code: `0` Better or a clean single-arm run, `1`
    /// Worse or inconclusive, `2` partial.
    #[must_use]
    pub fn exit_code(&self) -> i32 {
        if self.partial.is_some() {
            return 2;
        }
        let single_arm = self.scores.baseline.is_none();
        match self.verdict {
            Verdict::Pass => 0,
            Verdict::Inconclusive if single_arm => 0,
            Verdict::Fail | Verdict::Inconclusive => 1,
        }
    }

    /// Whether the report fits inline in a `3189` publication (64 KiB).
    #[must_use]
    pub fn fits_inline(&self) -> bool {
        self.report_bytes.len() <= MAX_INLINE_REPORT
    }
}
