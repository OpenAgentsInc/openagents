//! The run's outcome and its money, typed so that nothing downstream can
//! read success or a zero cost into a run that did not earn them (#11230,
//! audit RUN-03 and PRODUCT-04).
//!
//! - [`judge`] turns the agent's state and the harness's final required
//!   checks into an [`Outcome`]. Only [`Status::Passed`] allows delivery: a
//!   failed or missing required check, an agent or task error, a run that
//!   declared no required check, and a cancelled run all refuse it, and
//!   `--open-pr` checks [`Outcome::delivers`] before it commits anything.
//! - Checks the agent ran but the harness does not rerun are optional. They
//!   are reported beside the required ones and never decide the outcome.
//! - [`cost`] keeps every cost component's amount, or `null` with the
//!   reason it is unknown. A total exists only when every component is
//!   known; otherwise the known subtotal is reported as such.

use serde_json::{Value, json};

/// Where a run ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    /// Every required final check ran and passed, and the agent ended
    /// without an error. The only status that may deliver.
    Passed,
    /// A required final check failed or has no result, or the agent or
    /// task ended on an error.
    Failed,
    /// The run declared no required check, so nothing proves the change.
    Unchecked,
    /// The person stopped the run.
    Cancelled,
    /// The issue, base commit or worktree could not be prepared.
    SetupFailed,
    /// The decision steps stopped before a briefing existed.
    DecisionFailed,
}

impl Status {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Passed => "passed",
            Self::Failed => "failed",
            Self::Unchecked => "unchecked",
            Self::Cancelled => "cancelled",
            Self::SetupFailed => "setup_failed",
            Self::DecisionFailed => "decision_failed",
        }
    }
}

/// A run's outcome and why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Outcome {
    pub status: Status,
    /// Why the run did not pass; `None` only for [`Status::Passed`].
    pub reason: Option<String>,
}

impl Outcome {
    #[must_use]
    pub fn stopped(status: Status, reason: impl Into<String>) -> Self {
        Self {
            status,
            reason: Some(reason.into()),
        }
    }

    /// Whether this run may be delivered: committed, pushed, or opened as a
    /// pull request.
    #[must_use]
    pub fn delivers(&self) -> bool {
        self.status == Status::Passed
    }

    #[must_use]
    pub fn to_json(&self) -> Value {
        json!({
            "status": self.status.as_str(),
            "reason": self.reason,
            "delivers": self.delivers(),
        })
    }
}

/// The outcome of a run whose agent finished (or was stopped) and whose
/// final checks ran. `required` is every check id the harness must rerun;
/// `checks` the harness's verdicts for them.
#[must_use]
pub fn judge(
    cancelled: bool,
    agent_error: Option<&str>,
    required: &[String],
    checks: &[(String, bool)],
) -> Outcome {
    if cancelled {
        return Outcome::stopped(Status::Cancelled, "The run was stopped.");
    }
    if let Some(error) = agent_error {
        return Outcome::stopped(
            Status::Failed,
            format!("The agent ended on an error: {error}"),
        );
    }
    if required.is_empty() {
        return Outcome::stopped(
            Status::Unchecked,
            "The briefing declared no required check, so nothing proves the change.",
        );
    }
    for id in required {
        match checks.iter().find(|(seen, _)| seen == id) {
            None => {
                return Outcome::stopped(
                    Status::Failed,
                    format!("The required check {id} has no result."),
                );
            }
            Some((_, false)) => {
                return Outcome::stopped(
                    Status::Failed,
                    format!("The required check {id} failed."),
                );
            }
            Some((_, true)) => {}
        }
    }
    Outcome {
        status: Status::Passed,
        reason: None,
    }
}

/// One cost component: its amount, or `null` and why it is unknown.
#[must_use]
pub fn component(usd: Option<f64>, unknown: &str) -> Value {
    match usd {
        Some(usd) => json!({"usd": usd, "unknown_reason": null}),
        None => json!({"usd": null, "unknown_reason": unknown}),
    }
}

/// The run's cost: each component, the known subtotal, and a total only
/// when every component is known.
#[must_use]
pub fn cost(agent: Value, decisions: Value) -> Value {
    let parts = [&agent, &decisions];
    let known: f64 = parts.iter().filter_map(|p| p["usd"].as_f64()).sum();
    let complete = parts.iter().all(|p| p["usd"].is_number());
    json!({
        "denomination": "USD",
        "basis": "reported",
        "components": {"agent": agent, "decisions": decisions},
        "known_subtotal_usd": known,
        "total_usd": if complete { json!(known) } else { Value::Null },
        "complete": complete,
    })
}
