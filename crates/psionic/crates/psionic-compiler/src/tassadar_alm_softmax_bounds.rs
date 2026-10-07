use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Stable report id for the W1.4 softmax approximation bound certificate.
pub const TASSADAR_ALM_SOFTMAX_BOUNDS_REPORT_ID: &str = "tassadar_alm.softmax_bounds.w1_4.v1";
/// Claim boundary for the W1.4 softmax approximation certificate.
pub const TASSADAR_ALM_SOFTMAX_BOUNDS_CLAIM_BOUNDARY: &str = "certifies analytic softmax-to-hardmax probability-mass bounds for bounded \
     ALM keyed reads; it does not change the executor, train a model, or claim \
     f32 serving parity";

/// Input domain for a hardmax-vs-softmax certificate.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TassadarSoftmaxHardmaxBoundInput {
    /// Number of candidate keys in the attention row.
    pub candidate_count: usize,
    /// Minimum logit margin between the winning key and every non-winner.
    pub logit_gap: f64,
    /// Softmax inverse temperature beta.
    pub inverse_temperature: f64,
}

/// Analytic hardmax-vs-softmax certificate.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TassadarSoftmaxHardmaxBound {
    /// Number of candidate keys in the attention row.
    pub candidate_count: usize,
    /// Minimum logit margin between the winning key and every non-winner.
    pub logit_gap: f64,
    /// Softmax inverse temperature beta.
    pub inverse_temperature: f64,
    /// `(n - 1) * exp(-beta * gap)`.
    pub exp_tail_upper_bound: f64,
    /// Upper bound on total probability assigned to all non-winning keys.
    pub nonwinner_mass_upper_bound: f64,
    /// Lower bound on the softmax probability of the hardmax winner.
    pub winner_probability_lower_bound: f64,
    /// L1 distance upper bound between softmax and one-hot hardmax.
    pub l1_distance_upper_bound: f64,
    /// Human-readable formula pinned into the report.
    pub stated_bound: String,
}

/// Certificate specialized to integer-keyed ALM reads with parabolic scores.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TassadarIntegerKeyedReadSoftmaxBound {
    /// Number of candidate keys in the keyed read.
    pub key_count: usize,
    /// Minimum integer distance from the winner key to any other key.
    pub min_integer_key_gap: i64,
    /// Implied logit gap for scores `2*q*k - k^2` at `q == winner_key`.
    pub parabolic_score_gap: f64,
    /// Generic hardmax certificate using the implied score gap.
    pub hardmax_bound: TassadarSoftmaxHardmaxBound,
}

/// Report emitted for C4/W1.4.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TassadarSoftmaxBoundsReport {
    /// Stable report id.
    pub report_id: String,
    /// Claim boundary.
    pub claim_boundary: String,
    /// Bound for a representative bounded ALM attention row.
    pub keyed_read_bound: TassadarIntegerKeyedReadSoftmaxBound,
    /// Stable digest of the report payload.
    pub report_digest: String,
}

impl TassadarSoftmaxBoundsReport {
    /// Returns a stable digest over the report facts, excluding the digest field.
    #[must_use]
    pub fn stable_digest_without_field(&self) -> String {
        let mut clone = self.clone();
        clone.report_digest.clear();
        hex::encode(Sha256::digest(
            serde_json::to_vec(&clone).unwrap_or_default(),
        ))
    }
}

/// Softmax-bound certification failure.
#[derive(Debug, Error, PartialEq)]
pub enum TassadarSoftmaxBoundError {
    /// Candidate count must include one winner and at least one non-winner.
    #[error("candidate_count must be at least 2, got {candidate_count}")]
    CandidateCountTooSmall {
        /// Provided candidate count.
        candidate_count: usize,
    },
    /// Logit gap must be positive and finite.
    #[error("logit_gap must be positive and finite, got {logit_gap}")]
    InvalidLogitGap {
        /// Provided logit gap.
        logit_gap: f64,
    },
    /// Inverse temperature must be positive and finite.
    #[error("inverse_temperature must be positive and finite, got {inverse_temperature}")]
    InvalidInverseTemperature {
        /// Provided inverse temperature.
        inverse_temperature: f64,
    },
    /// Integer key gap must be positive.
    #[error("min_integer_key_gap must be positive, got {min_integer_key_gap}")]
    InvalidIntegerKeyGap {
        /// Provided integer key gap.
        min_integer_key_gap: i64,
    },
}

/// Certifies the bound:
/// nonwinner_mass <= T / (1 + T), winner_probability >= 1 / (1 + T),
/// where T = (n - 1) * exp(-beta * gap).
pub fn certify_tassadar_softmax_hardmax_bound(
    input: TassadarSoftmaxHardmaxBoundInput,
) -> Result<TassadarSoftmaxHardmaxBound, TassadarSoftmaxBoundError> {
    if input.candidate_count < 2 {
        return Err(TassadarSoftmaxBoundError::CandidateCountTooSmall {
            candidate_count: input.candidate_count,
        });
    }
    if !input.logit_gap.is_finite() || input.logit_gap <= 0.0 {
        return Err(TassadarSoftmaxBoundError::InvalidLogitGap {
            logit_gap: input.logit_gap,
        });
    }
    if !input.inverse_temperature.is_finite() || input.inverse_temperature <= 0.0 {
        return Err(TassadarSoftmaxBoundError::InvalidInverseTemperature {
            inverse_temperature: input.inverse_temperature,
        });
    }

    let nonwinner_count = (input.candidate_count - 1) as f64;
    let exp_tail_upper_bound =
        nonwinner_count * (-(input.inverse_temperature * input.logit_gap)).exp();
    let denominator = 1.0 + exp_tail_upper_bound;
    let nonwinner_mass_upper_bound = exp_tail_upper_bound / denominator;
    let winner_probability_lower_bound = 1.0 / denominator;
    let l1_distance_upper_bound = 2.0 * nonwinner_mass_upper_bound;

    Ok(TassadarSoftmaxHardmaxBound {
        candidate_count: input.candidate_count,
        logit_gap: input.logit_gap,
        inverse_temperature: input.inverse_temperature,
        exp_tail_upper_bound,
        nonwinner_mass_upper_bound,
        winner_probability_lower_bound,
        l1_distance_upper_bound,
        stated_bound: String::from(
            "For n candidates with winner margin Delta and inverse temperature beta, \
             total nonwinner softmax mass is <= ((n-1) * exp(-beta*Delta)) / \
             (1 + (n-1) * exp(-beta*Delta)); L1 distance to hardmax is at most twice that mass.",
        ),
    })
}

/// Certifies the ALM keyed-read score family `score(q, k) = 2*q*k - k^2`
/// when the query equals the winning integer key.
pub fn certify_tassadar_integer_keyed_read_softmax_bound(
    key_count: usize,
    min_integer_key_gap: i64,
    inverse_temperature: f64,
) -> Result<TassadarIntegerKeyedReadSoftmaxBound, TassadarSoftmaxBoundError> {
    if min_integer_key_gap <= 0 {
        return Err(TassadarSoftmaxBoundError::InvalidIntegerKeyGap {
            min_integer_key_gap,
        });
    }
    let parabolic_score_gap = (min_integer_key_gap * min_integer_key_gap) as f64;
    let hardmax_bound = certify_tassadar_softmax_hardmax_bound(TassadarSoftmaxHardmaxBoundInput {
        candidate_count: key_count,
        logit_gap: parabolic_score_gap,
        inverse_temperature,
    })?;

    Ok(TassadarIntegerKeyedReadSoftmaxBound {
        key_count,
        min_integer_key_gap,
        parabolic_score_gap,
        hardmax_bound,
    })
}

/// Builds the canonical W1.4 report used by C4.
pub fn build_tassadar_alm_softmax_bounds_report_v1()
-> Result<TassadarSoftmaxBoundsReport, TassadarSoftmaxBoundError> {
    let keyed_read_bound = certify_tassadar_integer_keyed_read_softmax_bound(1_024, 1, 32.0)?;
    let mut report = TassadarSoftmaxBoundsReport {
        report_id: String::from(TASSADAR_ALM_SOFTMAX_BOUNDS_REPORT_ID),
        claim_boundary: String::from(TASSADAR_ALM_SOFTMAX_BOUNDS_CLAIM_BOUNDARY),
        keyed_read_bound,
        report_digest: String::new(),
    };
    report.report_digest = report.stable_digest_without_field();
    Ok(report)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;

    #[test]
    fn hardmax_bound_matches_closed_form() {
        let bound = certify_tassadar_softmax_hardmax_bound(TassadarSoftmaxHardmaxBoundInput {
            candidate_count: 4,
            logit_gap: 2.0,
            inverse_temperature: 3.0,
        })
        .expect("certifies");
        let tail = 3.0 * (-6.0_f64).exp();
        let expected_mass = tail / (1.0 + tail);

        assert!((bound.exp_tail_upper_bound - tail).abs() < 1e-15);
        assert!((bound.nonwinner_mass_upper_bound - expected_mass).abs() < 1e-15);
        assert!((bound.winner_probability_lower_bound - (1.0 - expected_mass)).abs() < 1e-15);
        assert!((bound.l1_distance_upper_bound - (2.0 * expected_mass)).abs() < 1e-15);
    }

    #[test]
    fn integer_keyed_read_report_reaches_w1_4_bound() {
        let report = build_tassadar_alm_softmax_bounds_report_v1().expect("report builds");
        let bound = &report.keyed_read_bound;

        assert_eq!(bound.key_count, 1_024);
        assert_eq!(bound.min_integer_key_gap, 1);
        assert_eq!(bound.parabolic_score_gap, 1.0);
        assert!(bound.hardmax_bound.nonwinner_mass_upper_bound < 1.4e-11);
        assert!(bound.hardmax_bound.l1_distance_upper_bound < 2.7e-11);
        assert_eq!(report.report_digest, report.stable_digest_without_field());
    }

    #[test]
    fn invalid_domains_refuse() {
        assert!(matches!(
            certify_tassadar_softmax_hardmax_bound(TassadarSoftmaxHardmaxBoundInput {
                candidate_count: 1,
                logit_gap: 1.0,
                inverse_temperature: 1.0,
            }),
            Err(TassadarSoftmaxBoundError::CandidateCountTooSmall { .. })
        ));
        assert!(matches!(
            certify_tassadar_integer_keyed_read_softmax_bound(2, 0, 1.0),
            Err(TassadarSoftmaxBoundError::InvalidIntegerKeyGap { .. })
        ));
    }
}
