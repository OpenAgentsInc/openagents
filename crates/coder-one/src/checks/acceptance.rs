//! The result every acceptance check returns (issue #9656).
//!
//! An acceptance check runs something that doesn't depend on the
//! candidate, such as the task's own command, a provided checker, a
//! reference program, or an oracle written from the task's stated
//! definition, and compares the candidate with it. Each check reports the
//! same [`Acceptance`], so a loop, a metric target (issue #9657), or
//! failure localization (issue #9658) can read any of them without
//! knowing which check ran:
//!
//! - one [`Case`] per thing checked, passed or failed, with the stated
//!   parameter value or input the case covers;
//! - the first failing case, with what was observed against what was
//!   expected ([`FirstFailure`]);
//! - whether the check is trivially passing: whether it also passes on the
//!   untouched workspace or on an empty output ([`Triviality`]);
//! - its authority class ([`Authority`]), the tier issue #9629 defines;
//! - where it came from ([`Provenance`]).
//!
//! [`Acceptance::from_contract`] converts a `checks.contract` report into
//! this shape.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::contract::{Checked, Outcome, Plan};
use crate::accept::authority::Authority;

/// The schema of an [`Acceptance`].
pub const SCHEMA: &str = "openagents.coder-one.acceptance-check.v1";

/// How one case ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    /// The candidate did what the check expects.
    Passed,
    /// The candidate did something else.
    Failed,
    /// The case couldn't run, so it says nothing about the candidate.
    CouldNotRun,
}

impl Verdict {
    /// The verdict as the records spell it.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Verdict::Passed => "passed",
            Verdict::Failed => "failed",
            Verdict::CouldNotRun => "could_not_run",
        }
    }
}

/// What one case covers: a stated parameter and its value, or a stated
/// or boundary input. Every field is optional because a case can cover a
/// command with neither.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Covers {
    /// The stated parameter's name, such as `k` or `threshold`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parameter: Option<String>,
    /// The stated value of that parameter the case runs with.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    /// The input the case runs on: a stated example, a boundary input, or
    /// the command it runs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input: Option<String>,
    /// Where the value or input comes from: `stated` when the task says
    /// it, `boundary` when it's a boundary input extracted from the task,
    /// or `command` when it's the task's own command.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
}

/// One case's result.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Case {
    /// Stable within one check, such as `K3` or `O2`.
    pub id: String,
    pub covers: Covers,
    pub verdict: Verdict,
    /// What the candidate produced, bounded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed: Option<String>,
    /// What the check expected, bounded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected: Option<String>,
    /// Why a case couldn't run, or a bounded account of the difference.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[serde(default)]
    pub milliseconds: u64,
}

/// The first failing case, in case order.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FirstFailure {
    pub case: String,
    pub covers: Covers,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

/// Whether a check passes where it shouldn't. Each field is `None` when
/// the check never ran there.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Triviality {
    /// It passes on the untouched workspace.
    #[serde(default)]
    pub untouched: Option<bool>,
    /// It passes on an empty output.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub empty_output: Option<bool>,
}

impl Triviality {
    /// Whether any run showed the check passing where it shouldn't.
    #[must_use]
    pub fn trivially_passing(self) -> bool {
        self.untouched == Some(true) || self.empty_output == Some(true)
    }
}

/// Where a check came from.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Provenance {
    /// The component that made it: `checks.contract` or `checks.oracle`.
    pub component: String,
    /// How it was obtained: `contract` (the task's stated commands),
    /// `found` (a provided checker, example, or reference program), or
    /// `written` (an oracle written from the task's stated definition).
    pub source: String,
    /// The file, command, or program it runs, when there is one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin: Option<String>,
    /// The digest of the plan or program, so two runs are comparable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub digest: Option<String>,
    /// Anything else the maker recorded: Jev answer keys, the writing
    /// session and its cost.
    #[serde(default, skip_serializing_if = "Value::is_null")]
    pub detail: Value,
}

/// One acceptance check's result on one candidate.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Acceptance {
    pub schema: String,
    pub task: String,
    /// The candidate: a workspace path, a snapshot, or a label.
    pub candidate: String,
    pub authority: Authority,
    pub provenance: Provenance,
    pub cases: Vec<Case>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_failure: Option<FirstFailure>,
    pub triviality: Triviality,
}

impl Acceptance {
    /// Builds a result from `cases`, finding the first failure.
    #[must_use]
    pub fn new(
        task: &str,
        candidate: &str,
        authority: Authority,
        provenance: Provenance,
        cases: Vec<Case>,
        triviality: Triviality,
    ) -> Acceptance {
        let first_failure = cases
            .iter()
            .find(|c| c.verdict == Verdict::Failed)
            .map(|c| FirstFailure {
                case: c.id.clone(),
                covers: c.covers.clone(),
                observed: c.observed.clone(),
                expected: c.expected.clone(),
                detail: c.detail.clone(),
            });
        Acceptance {
            schema: SCHEMA.to_string(),
            task: task.to_string(),
            candidate: candidate.to_string(),
            authority,
            provenance,
            cases,
            first_failure,
            triviality,
        }
    }

    /// `Some(true)` when at least one case passed and none failed,
    /// `Some(false)` when any failed, and `None` when nothing ran to an
    /// answer.
    #[must_use]
    pub fn passed(&self) -> Option<bool> {
        if self.cases.iter().any(|c| c.verdict == Verdict::Failed) {
            Some(false)
        } else if self.cases.iter().any(|c| c.verdict == Verdict::Passed) {
            Some(true)
        } else {
            None
        }
    }

    /// Passed cases over cases that ran to an answer.
    #[must_use]
    pub fn score(&self) -> Option<f64> {
        let passed = self.count(Verdict::Passed);
        let failed = self.count(Verdict::Failed);
        #[allow(clippy::cast_precision_loss)]
        (passed + failed > 0).then(|| passed as f64 / (passed + failed) as f64)
    }

    /// The number of cases with `verdict`.
    #[must_use]
    pub fn count(&self, verdict: Verdict) -> usize {
        self.cases.iter().filter(|c| c.verdict == verdict).count()
    }

    /// Whether the check passes where it shouldn't.
    #[must_use]
    pub fn trivially_passing(&self) -> bool {
        self.triviality.trivially_passing()
    }

    /// A one-line account for a brief or a log line.
    #[must_use]
    pub fn summary(&self) -> String {
        let head = format!(
            "{} of {} cases passed",
            self.count(Verdict::Passed),
            self.count(Verdict::Passed) + self.count(Verdict::Failed)
        );
        match &self.first_failure {
            Some(f) => format!(
                "{head}; first failure {}{}: expected {}, observed {}",
                f.case,
                covers_phrase(&f.covers),
                f.expected.as_deref().unwrap_or("(not stated)"),
                f.observed.as_deref().unwrap_or("(nothing)")
            ),
            None => head,
        }
    }

    /// Converts a `checks.contract` report: each item that ran to an
    /// answer becomes a case, and each `not_executable` item is left out.
    /// `untouched` is the same plan's results on the untouched workspace,
    /// when they were run.
    #[must_use]
    pub fn from_contract(
        plan: &Plan,
        candidate: &str,
        results: &[Checked],
        untouched: Option<&[Checked]>,
    ) -> Acceptance {
        let cases = results
            .iter()
            .filter(|r| !matches!(r.outcome, Outcome::NotExecutable { .. }))
            .map(|r| contract_case(plan, r))
            .collect();
        let triviality = Triviality {
            untouched: untouched.map(|u| super::contract::call(u) == Some("pass")),
            empty_output: None,
        };
        Acceptance::new(
            &plan.task,
            candidate,
            Authority::ExecutedContract,
            Provenance {
                component: "checks.contract".to_string(),
                source: "contract".to_string(),
                origin: None,
                digest: Some(plan.digest.clone()),
                detail: json!({ "items": plan.checks().count() }),
            },
            cases,
            triviality,
        )
    }
}

fn covers_phrase(covers: &Covers) -> String {
    match (&covers.parameter, &covers.value, &covers.input) {
        (Some(p), Some(v), _) => format!(" ({p} = {v})"),
        (_, _, Some(i)) => format!(" (input {})", super::contract::clip(i, 80)),
        _ => String::new(),
    }
}

fn contract_case(plan: &Plan, r: &Checked) -> Case {
    let item = plan.items.iter().find(|i| i.id == r.id);
    let covers = Covers {
        parameter: None,
        value: None,
        input: item.and_then(|i| i.command.clone().or_else(|| i.path.clone())),
        from: Some("command".to_string()),
    };
    let expected = item
        .and_then(|i| i.expect.as_ref())
        .map(|e| super::contract::clip(&serde_json::to_string(e).unwrap_or_default(), 300));
    let (verdict, observed, detail) = match &r.outcome {
        Outcome::Matched { observed } => (Verdict::Passed, Some(observed.clone()), None),
        Outcome::Differed { diff, .. } => (Verdict::Failed, None, Some(diff.clone())),
        Outcome::CouldNotRun { why } | Outcome::NotExecutable { why } => {
            (Verdict::CouldNotRun, None, Some(why.clone()))
        }
    };
    Case {
        id: r.id.clone(),
        covers,
        verdict,
        observed,
        expected,
        detail,
        milliseconds: r.milliseconds,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checks::contract::{Exit, Expect, Item, Kind};

    fn item(id: &str) -> Item {
        Item {
            id: id.to_string(),
            kind: Kind::Command,
            source: "instruction".to_string(),
            span: "run it".to_string(),
            command: Some(format!("./run {id}")),
            path: None,
            expect: Some(Expect::Exit { exit: Exit::Zero }),
            wall_sec: None,
            stated_bound: false,
            not_executable: None,
            decided_by: None,
        }
    }

    #[test]
    fn a_contract_report_converts_with_its_first_failure() {
        let plan = Plan::seal("t", "/app", "do it", vec![item("K1"), item("K2")], vec![]);
        let results = vec![
            Checked {
                id: "K1".to_string(),
                kind: Kind::Command,
                outcome: Outcome::Matched {
                    observed: "exit 0".to_string(),
                },
                milliseconds: 3,
            },
            Checked {
                id: "K2".to_string(),
                kind: Kind::Command,
                outcome: Outcome::Differed {
                    diff: "expected exit 0, observed exit 1".to_string(),
                    similarity: None,
                },
                milliseconds: 4,
            },
        ];
        let untouched = vec![Checked {
            id: "K1".to_string(),
            kind: Kind::Command,
            outcome: Outcome::Differed {
                diff: "x".to_string(),
                similarity: None,
            },
            milliseconds: 1,
        }];
        let a = Acceptance::from_contract(&plan, "c1", &results, Some(&untouched));
        assert_eq!(a.passed(), Some(false));
        assert_eq!(a.score(), Some(0.5));
        assert_eq!(a.authority, Authority::ExecutedContract);
        assert_eq!(
            a.first_failure.as_ref().map(|f| f.case.as_str()),
            Some("K2")
        );
        assert_eq!(
            a.first_failure
                .as_ref()
                .and_then(|f| f.covers.input.as_deref()),
            Some("./run K2")
        );
        assert!(!a.trivially_passing());
        let back: Acceptance = serde_json::from_value(json!(a)).unwrap();
        assert_eq!(back, a);
    }

    #[test]
    fn passing_on_the_untouched_workspace_is_trivial() {
        let t = Triviality {
            untouched: Some(true),
            empty_output: None,
        };
        assert!(t.trivially_passing());
        assert!(!Triviality::default().trivially_passing());
    }

    #[test]
    fn nothing_ran_is_no_answer() {
        let a = Acceptance::new(
            "t",
            "c",
            Authority::IndependentlySupported,
            Provenance::default(),
            vec![Case {
                id: "O1".to_string(),
                covers: Covers::default(),
                verdict: Verdict::CouldNotRun,
                observed: None,
                expected: None,
                detail: Some("no python".to_string()),
                milliseconds: 0,
            }],
            Triviality::default(),
        );
        assert_eq!(a.passed(), None);
        assert!(a.first_failure.is_none());
    }
}
