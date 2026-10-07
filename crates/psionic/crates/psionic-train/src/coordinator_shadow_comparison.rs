//! Coordinator shadow comparison (Khala M6 / TRINITY substrate, roadmap Phase 4).
//!
//! This module lands the **shadow-candidate decision primitive** from
//! `docs/sakana/psionic-coordinator-roadmap.md` Phase 4 and the M6 "Done when"
//! gate (issue #6014): *the learned router beats the heuristic router on
//! cost-per-accepted-outcome in shadow on the in-distribution task set.*
//!
//! The P1–P5 substrate ([`crate::coordinator_evolution`] +
//! [`crate::coordinator_live_training`]) trains a coordinator head and produces
//! per-trajectory outcomes (verdict + spend). What was missing is the typed
//! comparison that turns two streams of outcomes — the **learned** coordinator's
//! and the **heuristic** router's, over the *same* samples — into the single
//! business metric the roadmap names and a promote / hold / rollback
//! recommendation under the existing confidence bands
//! (`docs/COMPILED_AGENT_SHADOW_GOVERNANCE.md`: high ≥ 0.80 / watch ≥ 0.60 /
//! review < 0.60).
//!
//! ## The metric: verified-work-per-sat
//!
//! Per `docs/sakana/coordinator-as-verified-work.md`, the objective is not raw
//! pass rate but **verified-work-per-sat-spent** ("cost-per-accepted-outcome"
//! inverted). For an arm with `v` verified trajectories over a batch that spent
//! `s` sats:
//!
//! ```text
//!   verified_work_per_sat = v / s            (s > 0)
//! ```
//!
//! The offline lane (`s == 0`, no workers move sats) has no sat denominator, so
//! the comparison falls back to the **verified rate** `v / n` and reports the
//! lane explicitly. A real shadow promotion runs on the paid lane where the sat
//! denominator exists.
//!
//! ## What this is and is not
//!
//! - It is the deterministic decision logic that the eventual candidate-artifact
//!   emission (Phase 4 / `CompiledAgentPromotedArtifactContract`) consumes: given
//!   two outcome streams it answers "promote, hold, or rollback, and why".
//! - It is **not** the contract-emission wiring itself. Binding a coordinator
//!   head into `CompiledAgentPromotedArtifactContract` as a typed candidate
//!   entry touches an existing runtime contract and is intentionally deferred to
//!   a reviewed change (see `docs/KHALA_M6_M7_COORDINATOR_PLAN.md`).
//! - It moves no sats, dispatches no work, and starts no training. It is a pure
//!   function over already-collected outcomes.
//!
//! The halt/accept decision is never a head output: each [`ArmOutcome`] verdict
//! is the replay-validator verdict, exactly as the verified-work doc requires.

use serde::{Deserialize, Serialize};

use crate::coordinator_evolution::{TrajectoryOutcome, VerificationVerdict};

// ---------------------------------------------------------------------------
// Confidence bands (mirrors COMPILED_AGENT_SHADOW_GOVERNANCE.md thresholds).
// ---------------------------------------------------------------------------

/// Default high-confidence floor (≥ 0.80) from the compiled-agent shadow
/// governance policy.
pub const DEFAULT_HIGH_CONFIDENCE_MIN: f32 = 0.80;
/// Default watch-band floor (≥ 0.60) from the compiled-agent shadow governance
/// policy. Below this is the review band.
pub const DEFAULT_WATCH_CONFIDENCE_MIN: f32 = 0.60;

/// The confidence band a shadow comparison falls into, matching the
/// compiled-agent governance vocabulary so the coordinator rides the existing
/// promotion machinery without a new band scheme.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShadowConfidenceBand {
    /// At or above the high-confidence floor: eligible for promotion on a clean
    /// win.
    High,
    /// Between the watch floor and the high floor: keep shadowing, do not
    /// promote yet.
    Watch,
    /// Below the watch floor: review; do not promote, candidate is weak.
    Review,
}

/// Thresholds for the confidence-band decision. Defaults match the
/// compiled-agent shadow governance policy.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ShadowConfidenceThresholds {
    /// Minimum learned-arm confidence for the [`ShadowConfidenceBand::High`]
    /// band.
    pub high_confidence_min: f32,
    /// Minimum learned-arm confidence for the [`ShadowConfidenceBand::Watch`]
    /// band (below this is review).
    pub watch_confidence_min: f32,
}

impl Default for ShadowConfidenceThresholds {
    fn default() -> Self {
        Self {
            high_confidence_min: DEFAULT_HIGH_CONFIDENCE_MIN,
            watch_confidence_min: DEFAULT_WATCH_CONFIDENCE_MIN,
        }
    }
}

impl ShadowConfidenceThresholds {
    fn band_for(&self, confidence: f32) -> ShadowConfidenceBand {
        if confidence >= self.high_confidence_min {
            ShadowConfidenceBand::High
        } else if confidence >= self.watch_confidence_min {
            ShadowConfidenceBand::Watch
        } else {
            ShadowConfidenceBand::Review
        }
    }
}

// ---------------------------------------------------------------------------
// Per-arm aggregate.
// ---------------------------------------------------------------------------

/// The aggregate statistics for one arm (learned coordinator OR heuristic
/// router) over a shadow batch. Built from a slice of [`TrajectoryOutcome`]s,
/// which already carry the replay-validator verdict and the spend.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArmOutcome {
    /// Number of trajectories in the batch.
    pub trajectories: usize,
    /// Number of `Verified` trajectories (accepted outcomes).
    pub verified: usize,
    /// Total spend across the batch, in the reward adapter's units (sats).
    pub total_cost: f32,
}

impl ArmOutcome {
    /// Aggregates a batch of trajectory outcomes into an arm summary.
    #[must_use]
    pub fn from_outcomes(outcomes: &[TrajectoryOutcome]) -> Self {
        let verified = outcomes
            .iter()
            .filter(|o| o.verdict == VerificationVerdict::Verified)
            .count();
        let total_cost = outcomes.iter().map(|o| o.cost).sum();
        Self {
            trajectories: outcomes.len(),
            verified,
            total_cost,
        }
    }

    /// Verified rate `verified / trajectories` (the offline-lane comparison and
    /// the band confidence). Returns `0.0` for an empty batch.
    #[must_use]
    pub fn verified_rate(&self) -> f32 {
        if self.trajectories == 0 {
            return 0.0;
        }
        self.verified as f32 / self.trajectories as f32
    }

    /// Verified-work-per-sat `verified / total_cost` — the roadmap's business
    /// metric. Returns `None` when no sats were spent (offline lane); callers
    /// fall back to [`verified_rate`](Self::verified_rate) and report the lane.
    #[must_use]
    pub fn verified_work_per_sat(&self) -> Option<f32> {
        if self.total_cost > 0.0 {
            Some(self.verified as f32 / self.total_cost)
        } else {
            None
        }
    }
}

// ---------------------------------------------------------------------------
// Comparison lane + recommendation.
// ---------------------------------------------------------------------------

/// Which metric drove the comparison.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComparisonLane {
    /// Paid lane: both arms moved sats, compared on verified-work-per-sat.
    VerifiedWorkPerSat,
    /// Offline lane: no sats moved, compared on verified rate.
    VerifiedRate,
}

/// The recommendation a shadow comparison produces. This is consumed by the
/// (deferred, reviewed) candidate-artifact emission; it never promotes by
/// itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShadowRecommendation {
    /// Learned arm is a clean win in the high-confidence band: eligible for an
    /// approval-gated `runtime_promotion`.
    PromoteCandidate,
    /// Learned arm is not yet a clean win, or sits in the watch band: keep
    /// shadowing.
    HoldShadow,
    /// Learned arm regressed against the heuristic baseline: roll back to the
    /// heuristic router (the `rollback_artifact_id`).
    Rollback,
}

/// The structured result of a learned-vs-heuristic shadow comparison.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ShadowComparison {
    /// Aggregate for the learned coordinator arm.
    pub learned: ArmOutcome,
    /// Aggregate for the heuristic router arm.
    pub heuristic: ArmOutcome,
    /// Which metric drove the comparison.
    pub lane: ComparisonLane,
    /// The learned arm's metric value (verified-work-per-sat on the paid lane,
    /// verified rate on the offline lane).
    pub learned_metric: f32,
    /// The heuristic arm's metric value, same units as `learned_metric`.
    pub heuristic_metric: f32,
    /// Whether the learned arm strictly beat the heuristic on the lane metric.
    pub learned_wins: bool,
    /// The confidence band the learned arm's verified rate falls into.
    pub band: ShadowConfidenceBand,
    /// The promote / hold / rollback recommendation.
    pub recommendation: ShadowRecommendation,
    /// Human-readable summary for receipts and logs.
    pub summary: String,
}

impl ShadowComparison {
    /// Compares a learned-coordinator outcome stream against a heuristic-router
    /// outcome stream over the same shadow batch, with the default confidence
    /// thresholds.
    ///
    /// Both streams should cover the same samples (so the comparison is paired);
    /// the function does not enforce sample identity — it compares aggregates —
    /// but callers must build the two streams over the same task set for the
    /// result to be meaningful.
    #[must_use]
    pub fn compare(
        learned_outcomes: &[TrajectoryOutcome],
        heuristic_outcomes: &[TrajectoryOutcome],
    ) -> Self {
        Self::compare_with(
            learned_outcomes,
            heuristic_outcomes,
            ShadowConfidenceThresholds::default(),
        )
    }

    /// Comparison with explicit confidence thresholds.
    #[must_use]
    pub fn compare_with(
        learned_outcomes: &[TrajectoryOutcome],
        heuristic_outcomes: &[TrajectoryOutcome],
        thresholds: ShadowConfidenceThresholds,
    ) -> Self {
        let learned = ArmOutcome::from_outcomes(learned_outcomes);
        let heuristic = ArmOutcome::from_outcomes(heuristic_outcomes);

        // Paid lane iff BOTH arms have a positive sat denominator. If either arm
        // is zero-cost (offline / simulated), fall back to verified rate so the
        // comparison is always defined.
        let (lane, learned_metric, heuristic_metric) = match (
            learned.verified_work_per_sat(),
            heuristic.verified_work_per_sat(),
        ) {
            (Some(l), Some(h)) => (ComparisonLane::VerifiedWorkPerSat, l, h),
            _ => (
                ComparisonLane::VerifiedRate,
                learned.verified_rate(),
                heuristic.verified_rate(),
            ),
        };

        let learned_wins = learned_metric > heuristic_metric;
        let regressed = learned_metric < heuristic_metric;
        let band = thresholds.band_for(learned.verified_rate());

        // Promotion requires BOTH a clean win on the lane metric AND a
        // high-confidence band. A regression recommends rollback. Anything else
        // holds the shadow.
        let recommendation = if regressed {
            ShadowRecommendation::Rollback
        } else if learned_wins && band == ShadowConfidenceBand::High {
            ShadowRecommendation::PromoteCandidate
        } else {
            ShadowRecommendation::HoldShadow
        };

        let lane_label = match lane {
            ComparisonLane::VerifiedWorkPerSat => "verified-work-per-sat",
            ComparisonLane::VerifiedRate => "verified-rate (offline, no sat denominator)",
        };
        let summary = format!(
            "shadow [{lane_label}]: learned {learned_metric:.6} vs heuristic {heuristic_metric:.6} \
             (learned verified {}/{}, band {:?}) -> {:?}",
            learned.verified, learned.trajectories, band, recommendation
        );

        Self {
            learned,
            heuristic,
            lane,
            learned_metric,
            heuristic_metric,
            learned_wins,
            band,
            recommendation,
            summary,
        }
    }

    /// Whether the comparison recommends an approval-gated promotion. This is a
    /// *recommendation only*; the actual promotion is the approval-gated
    /// `runtime_promotion` governance step, never automatic.
    #[must_use]
    pub fn recommends_promotion(&self) -> bool {
        self.recommendation == ShadowRecommendation::PromoteCandidate
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coordinator_evolution::{TrajectoryOutcome, VerificationVerdict};

    fn verified_at(cost: f32) -> TrajectoryOutcome {
        TrajectoryOutcome {
            verdict: VerificationVerdict::Verified,
            cost,
        }
    }
    fn rejected_at(cost: f32) -> TrajectoryOutcome {
        TrajectoryOutcome {
            verdict: VerificationVerdict::Rejected,
            cost,
        }
    }

    #[test]
    fn arm_aggregates_verified_and_cost() {
        let arm = ArmOutcome::from_outcomes(&[
            verified_at(10.0),
            rejected_at(5.0),
            verified_at(10.0),
            verified_at(10.0),
        ]);
        assert_eq!(arm.trajectories, 4);
        assert_eq!(arm.verified, 3);
        assert!((arm.total_cost - 35.0).abs() < 1e-6);
        assert!((arm.verified_rate() - 0.75).abs() < 1e-6);
        // 3 verified / 35 sats.
        assert!((arm.verified_work_per_sat().unwrap() - (3.0 / 35.0)).abs() < 1e-6);
    }

    #[test]
    fn offline_arm_has_no_sat_denominator() {
        let arm = ArmOutcome::from_outcomes(&[verified_at(0.0), rejected_at(0.0)]);
        assert_eq!(arm.verified_work_per_sat(), None);
        assert!((arm.verified_rate() - 0.5).abs() < 1e-6);
    }

    #[test]
    fn empty_arm_is_zero_rate_no_metric() {
        let arm = ArmOutcome::from_outcomes(&[]);
        assert_eq!(arm.trajectories, 0);
        assert_eq!(arm.verified_rate(), 0.0);
        assert_eq!(arm.verified_work_per_sat(), None);
    }

    #[test]
    fn paid_lane_promotes_a_high_confidence_clean_win() {
        // Learned: 9/10 verified, cheap (10 sats each => 100 sats).
        let mut learned = vec![verified_at(10.0); 9];
        learned.push(rejected_at(10.0));
        // Heuristic: 8/10 verified, pricier (20 sats each => 200 sats).
        let mut heuristic = vec![verified_at(20.0); 8];
        heuristic.push(rejected_at(20.0));
        heuristic.push(rejected_at(20.0));

        let cmp = ShadowComparison::compare(&learned, &heuristic);
        assert_eq!(cmp.lane, ComparisonLane::VerifiedWorkPerSat);
        // learned 9/100 = 0.09 ; heuristic 8/200 = 0.04.
        assert!(cmp.learned_wins);
        assert_eq!(cmp.band, ShadowConfidenceBand::High); // 0.9 >= 0.80
        assert_eq!(cmp.recommendation, ShadowRecommendation::PromoteCandidate);
        assert!(cmp.recommends_promotion());
    }

    #[test]
    fn clean_win_in_watch_band_holds_not_promotes() {
        // Learned wins on per-sat but verified rate (0.7) is only in the watch
        // band, so it must HOLD, not promote.
        let mut learned = vec![verified_at(10.0); 7];
        learned.extend(vec![rejected_at(10.0); 3]);
        let mut heuristic = vec![verified_at(40.0); 6];
        heuristic.extend(vec![rejected_at(40.0); 4]);

        let cmp = ShadowComparison::compare(&learned, &heuristic);
        // learned 7/100=0.07 ; heuristic 6/400=0.015 -> learned wins on per-sat.
        assert!(cmp.learned_wins);
        assert_eq!(cmp.band, ShadowConfidenceBand::Watch); // 0.70 in [0.60,0.80)
        assert_eq!(cmp.recommendation, ShadowRecommendation::HoldShadow);
        assert!(!cmp.recommends_promotion());
    }

    #[test]
    fn regression_recommends_rollback() {
        // Learned worse than heuristic on per-sat -> rollback to heuristic.
        let learned = vec![verified_at(50.0), rejected_at(50.0)]; // 1/100 = 0.01
        let heuristic = vec![verified_at(10.0), verified_at(10.0)]; // 2/20 = 0.10
        let cmp = ShadowComparison::compare(&learned, &heuristic);
        assert_eq!(cmp.lane, ComparisonLane::VerifiedWorkPerSat);
        assert!(!cmp.learned_wins);
        assert_eq!(cmp.recommendation, ShadowRecommendation::Rollback);
    }

    #[test]
    fn offline_lane_falls_back_to_verified_rate() {
        // No sats moved in either arm (offline / simulated). Compare on rate.
        let learned = vec![
            verified_at(0.0),
            verified_at(0.0),
            verified_at(0.0),
            rejected_at(0.0),
        ];
        let heuristic = vec![
            verified_at(0.0),
            rejected_at(0.0),
            rejected_at(0.0),
            rejected_at(0.0),
        ];
        let cmp = ShadowComparison::compare(&learned, &heuristic);
        assert_eq!(cmp.lane, ComparisonLane::VerifiedRate);
        // learned 0.75 vs heuristic 0.25.
        assert!(cmp.learned_wins);
        assert_eq!(cmp.band, ShadowConfidenceBand::Watch); // 0.75 in [0.60,0.80)
        assert_eq!(cmp.recommendation, ShadowRecommendation::HoldShadow);
    }

    #[test]
    fn mixed_lane_one_arm_offline_falls_back_to_rate() {
        // Learned paid, heuristic offline -> no common per-sat denominator,
        // fall back to verified rate so the comparison is still defined.
        let learned = vec![verified_at(10.0), verified_at(10.0)];
        let heuristic = vec![verified_at(0.0), rejected_at(0.0)];
        let cmp = ShadowComparison::compare(&learned, &heuristic);
        assert_eq!(cmp.lane, ComparisonLane::VerifiedRate);
        assert!((cmp.learned_metric - 1.0).abs() < 1e-6);
        assert!((cmp.heuristic_metric - 0.5).abs() < 1e-6);
        assert!(cmp.learned_wins);
    }

    #[test]
    fn tie_holds_shadow() {
        // Identical arms: no win, no regression -> hold.
        let learned = vec![verified_at(10.0), rejected_at(10.0)];
        let heuristic = vec![verified_at(10.0), rejected_at(10.0)];
        let cmp = ShadowComparison::compare(&learned, &heuristic);
        assert!(!cmp.learned_wins);
        assert_eq!(cmp.recommendation, ShadowRecommendation::HoldShadow);
    }

    #[test]
    fn high_band_win_but_not_promote_when_metric_only_ties() {
        // Same per-sat metric, both arms high verified rate -> tie, hold even
        // though band is High (promotion needs a strict win).
        let learned = vec![verified_at(10.0); 9];
        let mut learned = learned;
        learned.push(rejected_at(10.0));
        let heuristic = {
            let mut h = vec![verified_at(10.0); 9];
            h.push(rejected_at(10.0));
            h
        };
        let cmp = ShadowComparison::compare(&learned, &heuristic);
        assert_eq!(cmp.band, ShadowConfidenceBand::High);
        assert!(!cmp.learned_wins);
        assert_eq!(cmp.recommendation, ShadowRecommendation::HoldShadow);
    }
}
