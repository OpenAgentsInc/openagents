//! G1 v2 numerical qualification of a served DeepSeek-V4.1 runtime (#12405).
//!
//! The [qualification review](../../../../docs/psionic/deepseek-v41-completion.md)
//! replaces v1's per-element limits on a CPU model with checks on the served
//! runtime itself, against the B200 oracle on a common token prefix:
//!
//! - **Distributions.** Temperature-1 probabilities from a stable log-softmax
//!   in `f64`: total variation, Jensen–Shannon divergence (natural log), the
//!   reference token's log-probability error, and the largest error of
//!   centered logits (each vector minus its mean), which separates a uniform
//!   logit offset from changed probabilities.
//! - **Decisions with margins.** Top-1 tokens, router expert sets, and index
//!   selections use one rule: with a frozen score-error budget `epsilon`, an
//!   oracle boundary gap above `2 * epsilon` requires the same decision; inside
//!   it, at most the policy's number of substitutions, each costing at most
//!   `2 * epsilon` of oracle score. A substitution whose regret or boundary
//!   gap is unknown is unresolved, never passed.
//! - **Coverage.** Missing positions, nonfinite logits, and shape mismatches
//!   fail.
//!
//! A [`Policy`] with an unset limit leaves its check pending. The evaluator
//! never relaxes a limit; the policy file carries its own derivation and hash.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// The policy schema this module reads.
pub const POLICY_SCHEMA: &str = "psionic.deepseek_v41.g1_policy.v2";

/// Why a comparison could not be made.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QualifyError(pub String);

impl std::fmt::Display for QualifyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for QualifyError {}

fn bad(message: impl Into<String>) -> QualifyError {
    QualifyError(message.into())
}

/// Distribution error at one position.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Distribution {
    /// `0.5 * sum(abs(p - q))`.
    pub total_variation: f64,
    /// Jensen–Shannon divergence in nats.
    pub jensen_shannon: f64,
    /// Candidate minus oracle log-probability of the reference token.
    pub reference_logprob_error: f64,
    /// Largest absolute difference of mean-centered logits.
    pub centered_logit_error: f64,
    /// The oracle's and candidate's top-1 tokens.
    pub oracle_top1: u32,
    pub candidate_top1: u32,
    /// Oracle logit of its top-1 minus its second-highest logit.
    pub oracle_top1_gap: f64,
    /// Oracle logit of its top-1 minus the oracle logit of the candidate's top-1.
    pub top1_regret: f64,
}

fn log_softmax(logits: &[f32]) -> Vec<f64> {
    let max = logits
        .iter()
        .fold(f64::NEG_INFINITY, |m, x| m.max(f64::from(*x)));
    let sum: f64 = logits.iter().map(|x| (f64::from(*x) - max).exp()).sum();
    let log_sum = max + sum.ln();
    logits.iter().map(|x| f64::from(*x) - log_sum).collect()
}

fn top2(values: &[f32]) -> (usize, f64, f64) {
    let mut best = (0usize, f64::NEG_INFINITY);
    let mut second = f64::NEG_INFINITY;
    for (i, v) in values.iter().enumerate() {
        let v = f64::from(*v);
        if v > best.1 {
            second = best.1;
            best = (i, v);
        } else if v > second {
            second = v;
        }
    }
    (best.0, best.1, second)
}

/// Compares full-vocabulary logits at one position. `reference_token` is the
/// token the forced sequence continues with.
///
/// # Errors
///
/// Mismatched or empty shapes, nonfinite logits, and an out-of-range token.
pub fn compare_logits(
    oracle: &[f32],
    candidate: &[f32],
    reference_token: u32,
) -> Result<Distribution, QualifyError> {
    if oracle.is_empty() || oracle.len() != candidate.len() {
        return Err(bad(format!(
            "logit shapes differ: oracle {}, candidate {}",
            oracle.len(),
            candidate.len()
        )));
    }
    if oracle.iter().chain(candidate).any(|x| !x.is_finite()) {
        return Err(bad("nonfinite logits"));
    }
    let token = reference_token as usize;
    if token >= oracle.len() {
        return Err(bad("reference token outside the vocabulary"));
    }
    let lp = log_softmax(oracle);
    let lq = log_softmax(candidate);
    let mut tv = 0.0;
    let mut js = 0.0;
    for (a, b) in lp.iter().zip(&lq) {
        let (p, q) = (a.exp(), b.exp());
        tv += (p - q).abs();
        let m = 0.5 * (p + q);
        if m > 0.0 {
            let lm = m.ln();
            if p > 0.0 {
                js += 0.5 * p * (a - lm);
            }
            if q > 0.0 {
                js += 0.5 * q * (b - lm);
            }
        }
    }
    let mean = |v: &[f32]| v.iter().map(|x| f64::from(*x)).sum::<f64>() / v.len() as f64;
    let (mo, mc) = (mean(oracle), mean(candidate));
    let centered = oracle
        .iter()
        .zip(candidate)
        .map(|(o, c)| ((f64::from(*c) - mc) - (f64::from(*o) - mo)).abs())
        .fold(0.0, f64::max);
    let (oracle_top1, best, second) = top2(oracle);
    let (candidate_top1, _, _) = top2(candidate);
    Ok(Distribution {
        total_variation: 0.5 * tv,
        jensen_shannon: js.max(0.0),
        reference_logprob_error: lq[token] - lp[token],
        centered_logit_error: centered,
        oracle_top1: oracle_top1 as u32,
        candidate_top1: candidate_top1 as u32,
        oracle_top1_gap: best - second,
        top1_regret: best - f64::from(oracle[candidate_top1]),
    })
}

/// A margin-aware decision rule.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DecisionRule {
    /// Score-error budget; `None` leaves the check pending.
    pub epsilon: Option<f64>,
    /// Most substitutions in one decision inside the uncertainty band.
    pub max_substitutions: usize,
    /// Most decisions with any substitution, as a fraction of all decisions
    /// of the kind in one case.
    pub max_substituted_fraction: Option<f64>,
}

/// One selection decision: the oracle's and candidate's chosen IDs, the
/// oracle's boundary gap (k-th minus (k+1)-th score) when known, and oracle
/// scores of the IDs either side chose, when known.
pub struct Selection<'a> {
    pub oracle: &'a [u32],
    pub candidate: &'a [u32],
    pub boundary_gap: Option<f64>,
    pub oracle_scores: Option<&'a BTreeMap<u32, f64>>,
    /// IDs must be below this.
    pub universe: u32,
}

/// What a decision check found.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum Decision {
    Same,
    Substituted { count: usize, regret: f64 },
    Failed { reason: String },
    Unresolved { reason: String },
}

impl DecisionRule {
    /// Applies the rule to one selection.
    #[must_use]
    pub fn check(&self, s: &Selection<'_>) -> Decision {
        let fail = |reason: String| Decision::Failed { reason };
        if s.candidate.len() != s.oracle.len() {
            return fail(format!(
                "{} IDs chosen; the oracle chose {}",
                s.candidate.len(),
                s.oracle.len()
            ));
        }
        let mut seen = std::collections::BTreeSet::new();
        for id in s.candidate {
            if *id >= s.universe || !seen.insert(*id) {
                return fail(format!("invalid or repeated ID {id}"));
            }
        }
        let oracle: std::collections::BTreeSet<u32> = s.oracle.iter().copied().collect();
        let dropped: Vec<u32> = oracle.difference(&seen).copied().collect();
        let added: Vec<u32> = seen.difference(&oracle).copied().collect();
        if dropped.is_empty() {
            return Decision::Same;
        }
        let Some(epsilon) = self.epsilon else {
            return Decision::Unresolved {
                reason: "no score-error budget is frozen".into(),
            };
        };
        let Some(gap) = s.boundary_gap else {
            return Decision::Unresolved {
                reason: "the oracle's boundary gap is unknown".into(),
            };
        };
        if gap > 2.0 * epsilon {
            return fail(format!(
                "the decision changed at a stable boundary (gap {gap:e} > 2 epsilon {:e})",
                2.0 * epsilon
            ));
        }
        if dropped.len() > self.max_substitutions {
            return fail(format!(
                "{} substitutions; at most {} allowed",
                dropped.len(),
                self.max_substitutions
            ));
        }
        let Some(scores) = s.oracle_scores else {
            return Decision::Unresolved {
                reason: "the oracle's scores of the substituted IDs are unknown".into(),
            };
        };
        let sum = |ids: &[u32]| -> Option<f64> { ids.iter().map(|i| scores.get(i).copied()).sum() };
        let (Some(out), Some(inn)) = (sum(&dropped), sum(&added)) else {
            return Decision::Unresolved {
                reason: "the oracle's scores of the substituted IDs are unknown".into(),
            };
        };
        let regret = out - inn;
        if regret > 2.0 * epsilon * dropped.len() as f64 {
            return fail(format!("oracle-score regret {regret:e} exceeds the budget"));
        }
        Decision::Substituted {
            count: dropped.len(),
            regret,
        }
    }
}

/// Per-position and per-case limits on distribution error. `None` leaves a
/// limit pending.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct DistributionLimits {
    pub total_variation: Option<f64>,
    pub jensen_shannon: Option<f64>,
    pub reference_logprob_error: Option<f64>,
    /// Mean total variation over a case's positions.
    pub case_mean_total_variation: Option<f64>,
    /// A tail threshold and the largest fraction of a case's positions allowed
    /// above it.
    pub tail_total_variation: Option<f64>,
    pub tail_fraction: Option<f64>,
}

/// The frozen G1 v2 policy.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Policy {
    pub schema: String,
    pub version: String,
    pub distribution: DistributionLimits,
    pub top1: DecisionRule,
    pub router: DecisionRule,
    pub index: DecisionRule,
}

impl Policy {
    /// Parses a policy file and checks its schema.
    ///
    /// # Errors
    ///
    /// Invalid JSON or another schema.
    pub fn parse(text: &str) -> Result<Self, QualifyError> {
        let policy: Self = serde_json::from_str(text).map_err(|e| bad(format!("policy: {e}")))?;
        if policy.schema != POLICY_SCHEMA {
            return Err(bad(format!(
                "policy schema {} is not {POLICY_SCHEMA}",
                policy.schema
            )));
        }
        Ok(policy)
    }
}

/// One position's comparison against the policy.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PositionResult {
    pub label: String,
    pub metrics: Distribution,
    pub top1: Decision,
    /// Limits this position exceeded.
    pub failures: Vec<String>,
    /// Limits not yet frozen.
    pub pending: Vec<String>,
}

/// Checks one position against the policy's per-position limits and top-1 rule.
#[must_use]
pub fn judge_position(policy: &Policy, label: &str, metrics: Distribution) -> PositionResult {
    let mut failures = Vec::new();
    let mut pending = Vec::new();
    let l = policy.distribution;
    for (name, limit, value) in [
        (
            "total_variation",
            l.total_variation,
            metrics.total_variation,
        ),
        ("jensen_shannon", l.jensen_shannon, metrics.jensen_shannon),
        (
            "reference_logprob_error",
            l.reference_logprob_error,
            metrics.reference_logprob_error.abs(),
        ),
    ] {
        match limit {
            None => pending.push(name.to_string()),
            Some(limit) if !(value <= limit) => {
                failures.push(format!("{name} {value:e} > {limit:e}"));
            }
            Some(_) => {}
        }
    }
    let scores: BTreeMap<u32, f64> = [
        (metrics.oracle_top1, 0.0),
        (metrics.candidate_top1, -metrics.top1_regret),
    ]
    .into_iter()
    .collect();
    let top1 = policy.top1.check(&Selection {
        oracle: &[metrics.oracle_top1],
        candidate: &[metrics.candidate_top1],
        boundary_gap: Some(metrics.oracle_top1_gap),
        oracle_scores: Some(&scores),
        universe: u32::MAX,
    });
    match &top1 {
        Decision::Failed { reason } => failures.push(format!("top-1: {reason}")),
        Decision::Unresolved { reason } => pending.push(format!("top-1: {reason}")),
        _ => {}
    }
    PositionResult {
        label: label.to_string(),
        metrics,
        top1,
        failures,
        pending,
    }
}

/// A case's aggregate verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CaseResult {
    pub case: String,
    pub mode: String,
    pub positions: Vec<PositionResult>,
    pub missing: Vec<String>,
    pub mean_total_variation: f64,
    pub max_total_variation: f64,
    pub tail_fraction: Option<f64>,
    pub substituted_top1: usize,
    pub failures: Vec<String>,
    pub pending: Vec<String>,
    pub passed: bool,
}

/// Aggregates one case and mode. `missing` lists required positions that
/// produced no comparison; any missing position fails the case.
#[must_use]
pub fn judge_case(
    policy: &Policy,
    case: &str,
    mode: &str,
    positions: Vec<PositionResult>,
    missing: Vec<String>,
) -> CaseResult {
    let mut failures: Vec<String> = positions
        .iter()
        .flat_map(|p| p.failures.iter().map(move |f| format!("{}: {f}", p.label)))
        .collect();
    let mut pending: Vec<String> = positions
        .iter()
        .flat_map(|p| p.pending.iter().cloned())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    for m in &missing {
        failures.push(format!("{m}: missing"));
    }
    let n = positions.len().max(1) as f64;
    let tvs: Vec<f64> = positions
        .iter()
        .map(|p| p.metrics.total_variation)
        .collect();
    let mean = tvs.iter().sum::<f64>() / n;
    let max = tvs.iter().copied().fold(0.0, f64::max);
    let l = policy.distribution;
    match l.case_mean_total_variation {
        None => pending.push("case_mean_total_variation".into()),
        Some(limit) if !(mean <= limit) => {
            failures.push(format!("mean total variation {mean:e} > {limit:e}"));
        }
        Some(_) => {}
    }
    let tail_fraction = l
        .tail_total_variation
        .map(|t| tvs.iter().filter(|v| **v > t).count() as f64 / n);
    match (tail_fraction, l.tail_fraction) {
        (Some(f), Some(limit)) if !(f <= limit) => {
            failures.push(format!("tail fraction {f} > {limit}"));
        }
        (Some(_), Some(_)) => {}
        _ => pending.push("tail_total_variation".into()),
    }
    let substituted = positions
        .iter()
        .filter(|p| matches!(p.top1, Decision::Substituted { .. }))
        .count();
    match policy.top1.max_substituted_fraction {
        None => pending.push("top1.max_substituted_fraction".into()),
        Some(limit) if substituted as f64 / n > limit => {
            failures.push(format!("{substituted} top-1 substitutions exceed {limit}"));
        }
        Some(_) => {}
    }
    if positions.is_empty() {
        failures.push("no positions compared".into());
    }
    let passed = failures.is_empty() && pending.is_empty();
    CaseResult {
        case: case.to_string(),
        mode: mode.to_string(),
        positions,
        missing,
        mean_total_variation: mean,
        max_total_variation: max,
        tail_fraction,
        substituted_top1: substituted,
        failures,
        pending,
        passed,
    }
}

#[cfg(test)]
#[path = "deepseek_v41_qualify_tests.rs"]
mod tests;
