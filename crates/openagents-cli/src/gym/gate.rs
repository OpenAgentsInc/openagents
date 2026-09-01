//! The bench gate: per-suite floors a complete scored run is judged against,
//! and the third verdict that keeps an unmeasurable criterion from passing by
//! default.
//!
//! This is the Rust port of the deleted
//! `packages/coder-effectiveness/src/thresholds.ts`, with the floors moved
//! out of a sibling thresholds file and into the suite manifest's `gate`
//! block, so a suite and the floor it is judged against travel in one file
//! and cannot be paired wrongly. The suite's content digest deliberately does
//! NOT cover the gate — the digest pins what was run, and tightening a floor
//! must not make historical runs read as drifted — so each evaluated row pins
//! the gate it was scored against by its own digest instead.
//!
//! A criterion is `passed`, `failed`, or `unverifiable`, and `failed` beats
//! `unverifiable` beats `passed`: a measured breach is a breach whatever else
//! could not be measured. `unverifiable` is the point of the third verdict.
//! If a gate declares a cost ceiling and the run happened on an unpriced
//! lane, the honest answer is not "under budget" — nothing was measured — and
//! reporting a pass would make the gate quietest exactly when the lane is
//! least accountable.

use crate::errors::CliError;
use crate::gym::results::ScoreReport;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// The floors a suite manifest declares. `openagents.effectiveness_suite.v1`
/// carries this as an optional `gate` object; a suite without one is scored
/// but not judged, and its rows record `gateStatus: null`.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GateSpec {
    /// Rationale prose carried in the manifest. Excluded from the gate
    /// digest: commentary on a floor is not the floor.
    #[serde(default, rename = "$comment", skip_serializing)]
    pub comment: Option<serde_json::Value>,
    /// Fewest verifier-judged trials for the run to be worth scoring at all.
    pub min_graded_trials: u64,
    /// Lowest acceptable accepted-over-graded rate, 0..1.
    pub min_success_rate: f64,
    /// Most of the run that may go ungraded before the run is unreadable, 0..1.
    pub max_ungraded_ratio: f64,
    /// Optional dollar ceiling on cost per accepted outcome.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_cost_per_accepted_outcome_usd: Option<f64>,
    /// Score the cost ceiling against rates the catalog marks provisional.
    /// False, a placeholder-priced run leaves the cost criterion
    /// unverifiable rather than passing it.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub accept_placeholder_rates: bool,
}

impl GateSpec {
    /// Reject values outside their ranges, naming the suite that declared them.
    pub fn validate(&self, suite_id: &str) -> Result<(), CliError> {
        let fraction = |name: &str, value: f64| -> Result<(), CliError> {
            if (0.0..=1.0).contains(&value) {
                Ok(())
            } else {
                Err(CliError::Configuration(format!(
                    "suite {suite_id} gate: {name} must be between 0 and 1, got {value}"
                )))
            }
        };
        fraction("minSuccessRate", self.min_success_rate)?;
        fraction("maxUngradedRatio", self.max_ungraded_ratio)?;
        if let Some(ceiling) = self.max_cost_per_accepted_outcome_usd
            && !(ceiling > 0.0)
        {
            return Err(CliError::Configuration(format!(
                "suite {suite_id} gate: maxCostPerAcceptedOutcomeUsd must be greater than zero, got {ceiling}"
            )));
        }
        Ok(())
    }

    /// `gate:<sha256>` over the canonical serialization of the floors. Rows
    /// record it, so a later reader knows which version of the gate a verdict
    /// was scored against even after the manifest's gate has been retuned.
    pub fn digest(&self) -> String {
        let source = serde_json::to_string(self).unwrap_or_default();
        let hash = Sha256::digest(source.as_bytes());
        let mut out = String::with_capacity(69);
        out.push_str("gate:");
        for b in hash.iter() {
            use std::fmt::Write;
            let _ = write!(out, "{:02x}", b);
        }
        out
    }
}

/// One floor's verdict, with the measurement it was passed or failed on.
#[derive(Debug, Clone, Serialize)]
pub struct GateCriterion {
    pub name: String,
    /// `passed`, `failed`, or `unverifiable`.
    pub verdict: &'static str,
    pub detail: String,
}

/// The gate's verdict over a complete scored run.
#[derive(Debug, Clone, Serialize)]
pub struct GateOutcome {
    /// The suite whose gate was evaluated — the row's `thresholdsId`,
    /// matching the id the retired TypeScript thresholds files carried.
    pub thresholds_id: String,
    /// The evaluated gate's content digest — the row's `gateDigest`.
    pub gate_digest: String,
    /// `failed` beats `unverifiable` beats `passed`.
    pub status: &'static str,
    pub criteria: Vec<GateCriterion>,
}

impl GateOutcome {
    /// The criteria that kept the gate from passing.
    pub fn breaches(&self) -> impl Iterator<Item = &GateCriterion> {
        self.criteria.iter().filter(|c| c.verdict != "passed")
    }
}

/// Score a complete run's report against a suite's floors. Pure.
///
/// Completeness is the caller's precondition: the scorer only evaluates the
/// gate for a run that covered every task its score-tier suite pins, because
/// floors declared over a suite mean nothing over a different set of work.
/// (The retired TypeScript gate carried that rule as an unverifiable
/// `run_tier=score` criterion; here the coverage check runs first, so an
/// incomplete run has no gate verdict at all rather than a decorated one.)
pub fn evaluate_gate(report: &ScoreReport, suite_id: &str, spec: &GateSpec) -> GateOutcome {
    let mut criteria = vec![
        graded_trials_criterion(report, spec),
        success_rate_criterion(report, spec),
        ungraded_ratio_criterion(report, spec),
    ];
    if let Some(cost) = cost_criterion(report, spec) {
        criteria.push(cost);
    }
    let status = if criteria.iter().any(|c| c.verdict == "failed") {
        "failed"
    } else if criteria.iter().any(|c| c.verdict == "unverifiable") {
        "unverifiable"
    } else {
        "passed"
    };
    GateOutcome {
        thresholds_id: suite_id.to_string(),
        gate_digest: spec.digest(),
        status,
        criteria,
    }
}

fn graded_trials_criterion(report: &ScoreReport, spec: &GateSpec) -> GateCriterion {
    GateCriterion {
        name: format!("graded_trials>={}", spec.min_graded_trials),
        verdict: if report.graded >= spec.min_graded_trials {
            "passed"
        } else {
            "failed"
        },
        detail: format!(
            "{} of {} trials were graded",
            report.graded, report.trials_total
        ),
    }
}

fn success_rate_criterion(report: &ScoreReport, spec: &GateSpec) -> GateCriterion {
    let name = format!("success_rate>={:.3}", spec.min_success_rate);
    match report.success_rate {
        None => GateCriterion {
            name,
            verdict: "unverifiable",
            detail: "no verifier ran, so the run has no success rate rather than a zero one".into(),
        },
        Some(rate) => GateCriterion {
            name,
            verdict: if rate >= spec.min_success_rate {
                "passed"
            } else {
                "failed"
            },
            detail: format!(
                "success rate {rate:.3} over {} graded trials",
                report.graded
            ),
        },
    }
}

fn ungraded_ratio_criterion(report: &ScoreReport, spec: &GateSpec) -> GateCriterion {
    GateCriterion {
        name: format!("ungraded_ratio<={:.3}", spec.max_ungraded_ratio),
        verdict: if report.ungraded_ratio <= spec.max_ungraded_ratio {
            "passed"
        } else {
            "failed"
        },
        detail: format!(
            "{} of {} trials went ungraded ({:.3})",
            report.ungraded, report.trials_total, report.ungraded_ratio
        ),
    }
}

/// `None` when the gate declares no cost ceiling.
fn cost_criterion(report: &ScoreReport, spec: &GateSpec) -> Option<GateCriterion> {
    let ceiling = spec.max_cost_per_accepted_outcome_usd?;
    let name = format!("cost_per_accepted_outcome<=${ceiling:.4}");
    let Some(usd) = report.cost_per_accepted_outcome_usd else {
        return Some(GateCriterion {
            name,
            verdict: "unverifiable",
            detail: format!(
                "cost per accepted outcome is unknown ({})",
                report.cost_disposition
            ),
        });
    };
    if report.rate_basis.as_deref() == Some("operator_placeholder")
        && !spec.accept_placeholder_rates
    {
        return Some(GateCriterion {
            name,
            verdict: "unverifiable",
            detail: format!(
                "cost per accepted outcome is ${usd:.4} but every rate behind it is an operator placeholder; set acceptPlaceholderRates to score against provisional rates"
            ),
        });
    }
    Some(GateCriterion {
        name,
        verdict: if usd <= ceiling { "passed" } else { "failed" },
        detail: format!("cost per accepted outcome ${usd:.4} against a ${ceiling:.4} ceiling"),
    })
}
