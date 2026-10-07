use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Current public record-track baseline submission used for promotion review.
pub const PARAMETER_GOLF_CURRENT_PUBLIC_RECORD_SUBMISSION_ID: &str =
    "2026-03-17_NaiveBaseline";
/// Current public record-track README path used for promotion review.
pub const PARAMETER_GOLF_CURRENT_PUBLIC_RECORD_README_REF: &str =
    "records/track_10min_16mb/2026-03-17_NaiveBaseline/README.md";
/// Current public best record-track score in bits per byte as of 2026-03-18.
pub const PARAMETER_GOLF_CURRENT_PUBLIC_RECORD_SCORE_BPB: f64 = 1.224_365_70;
/// Minimum required SOTA improvement in nats per byte from the public README.
pub const PARAMETER_GOLF_SOTA_MIN_IMPROVEMENT_NATS_PER_BYTE: f64 = 0.005;
/// Maximum p-value admitted when significance evidence is required.
pub const PARAMETER_GOLF_SOTA_REQUIRED_SIGNIFICANCE_P_VALUE: f64 = 0.01;
/// Snapshot date for the current promotion rule surface.
pub const PARAMETER_GOLF_SOTA_RULE_SNAPSHOT_DATE: &str = "2026-03-18";

/// Final maintainer-facing disposition for one submission-promotion receipt.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParameterGolfSubmissionPromotionDisposition {
    /// The supplied evidence supports opening a record-promotion review.
    Promotable,
    /// The supplied evidence does not support record promotion yet.
    Refused,
}

/// Significance posture carried by one promotion receipt.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParameterGolfSubmissionPromotionSignificancePosture {
    /// The current candidate does not require multi-run significance evidence.
    NotRequired,
    /// Significance evidence is required but not yet sufficient.
    RequiredAndMissing,
    /// Significance evidence is present and satisfies the declared bar.
    Satisfied,
    /// The README's systems-only waiver was claimed and supported.
    WaivedSystemsOnly,
}

/// Systems-only waiver posture carried by one promotion receipt.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParameterGolfSubmissionPromotionSystemsOnlyWaiverPosture {
    /// No systems-only waiver is being claimed.
    NotClaimed,
    /// The systems-only waiver is claimed and supported by the supplied evidence.
    ClaimedSupported,
    /// The systems-only waiver is claimed but the supplied evidence is insufficient.
    ClaimedUnsupported,
}

/// Current public best run compared against one promotion candidate.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfSubmissionPromotionCurrentBest {
    /// Public submission identifier.
    pub submission_id: String,
    /// Public README reference for the current best run.
    pub readme_ref: String,
    /// Score in bits per byte.
    pub val_bpb: f64,
    /// Required SOTA delta in nats per byte.
    pub required_delta_nats_per_byte: f64,
    /// Required p-value ceiling when significance evidence is needed.
    pub required_significance_p_value: f64,
}

/// Candidate evidence summarized by the promotion receipt.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfSubmissionPromotionCandidate {
    /// Stable submission identifier.
    pub submission_id: String,
    /// Stable run identifier.
    pub run_id: String,
    /// Candidate benchmark reference.
    pub benchmark_ref: String,
    /// Submission track identifier.
    pub track_id: String,
    /// Whether the candidate is targeting record-track promotion.
    pub record_track_candidate: bool,
    /// Candidate score in bits per byte.
    pub val_bpb: f64,
    /// Whether the README's systems-only waiver is being claimed.
    pub systems_only_waiver_claimed: bool,
    /// Whether the supplied evidence actually supports the systems-only waiver.
    pub systems_only_waiver_supported: bool,
    /// Multi-run significance p-value when one is available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub significance_p_value: Option<f64>,
    /// Evidence refs that justify the significance claim when required.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub significance_evidence_refs: Vec<String>,
    /// Ordered evidence refs that support the promotion review.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence_refs: Vec<String>,
}

/// Typed maintainer-facing promotion receipt for one Parameter Golf submission.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfSubmissionPromotionReceipt {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable report identifier.
    pub report_id: String,
    /// Snapshot date for the current README rule surface.
    pub challenge_rule_snapshot_date: String,
    /// README ref that owns the promotion rule.
    pub challenge_readme_ref: String,
    /// Current public best run used for comparison.
    pub current_best: ParameterGolfSubmissionPromotionCurrentBest,
    /// Candidate evidence under review.
    pub candidate: ParameterGolfSubmissionPromotionCandidate,
    /// Positive values mean the candidate beat the current best in bits per byte.
    pub delta_bpb: f64,
    /// Positive values mean the candidate beat the current best in nats per byte.
    pub delta_nats_per_byte: f64,
    /// Whether the candidate cleared the public `0.005` nat improvement bar.
    pub clears_required_nats_delta: bool,
    /// Significance posture for this review.
    pub significance_posture: ParameterGolfSubmissionPromotionSignificancePosture,
    /// Systems-only waiver posture for this review.
    pub systems_only_waiver_posture: ParameterGolfSubmissionPromotionSystemsOnlyWaiverPosture,
    /// Final review disposition.
    pub disposition: ParameterGolfSubmissionPromotionDisposition,
    /// Ordered refusal reasons when promotion is not yet supported.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub refusal_reasons: Vec<String>,
    /// Explicit claim boundary for the receipt.
    pub claim_boundary: String,
    /// Stable digest over the receipt payload.
    pub receipt_digest: String,
}

impl ParameterGolfSubmissionPromotionReceipt {
    /// Returns a stable digest over the receipt payload.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut digestible = self.clone();
        digestible.receipt_digest.clear();
        stable_digest(
            b"psionic_parameter_golf_submission_promotion_receipt|",
            &digestible,
        )
    }

    /// Populates the stable digest field.
    #[must_use]
    pub fn with_stable_digest(mut self) -> Self {
        self.receipt_digest = self.stable_digest();
        self
    }
}

/// Builds one maintainer-facing promotion receipt from explicit candidate evidence.
#[must_use]
pub fn build_parameter_golf_submission_promotion_receipt(
    candidate: ParameterGolfSubmissionPromotionCandidate,
) -> ParameterGolfSubmissionPromotionReceipt {
    let current_best = ParameterGolfSubmissionPromotionCurrentBest {
        submission_id: String::from(PARAMETER_GOLF_CURRENT_PUBLIC_RECORD_SUBMISSION_ID),
        readme_ref: String::from(PARAMETER_GOLF_CURRENT_PUBLIC_RECORD_README_REF),
        val_bpb: PARAMETER_GOLF_CURRENT_PUBLIC_RECORD_SCORE_BPB,
        required_delta_nats_per_byte: PARAMETER_GOLF_SOTA_MIN_IMPROVEMENT_NATS_PER_BYTE,
        required_significance_p_value: PARAMETER_GOLF_SOTA_REQUIRED_SIGNIFICANCE_P_VALUE,
    };
    let delta_bpb = current_best.val_bpb - candidate.val_bpb;
    let delta_nats_per_byte = delta_bpb * std::f64::consts::LN_2;
    let clears_required_nats_delta =
        delta_nats_per_byte >= PARAMETER_GOLF_SOTA_MIN_IMPROVEMENT_NATS_PER_BYTE;

    let systems_only_waiver_posture = match (
        candidate.systems_only_waiver_claimed,
        candidate.systems_only_waiver_supported,
    ) {
        (false, _) => ParameterGolfSubmissionPromotionSystemsOnlyWaiverPosture::NotClaimed,
        (true, true) => ParameterGolfSubmissionPromotionSystemsOnlyWaiverPosture::ClaimedSupported,
        (true, false) => {
            ParameterGolfSubmissionPromotionSystemsOnlyWaiverPosture::ClaimedUnsupported
        }
    };
    let significance_posture = if matches!(
        systems_only_waiver_posture,
        ParameterGolfSubmissionPromotionSystemsOnlyWaiverPosture::ClaimedSupported
    ) {
        ParameterGolfSubmissionPromotionSignificancePosture::WaivedSystemsOnly
    } else if clears_required_nats_delta {
        match candidate.significance_p_value {
            Some(p_value)
                if p_value <= PARAMETER_GOLF_SOTA_REQUIRED_SIGNIFICANCE_P_VALUE
                    && !candidate.significance_evidence_refs.is_empty() =>
            {
                ParameterGolfSubmissionPromotionSignificancePosture::Satisfied
            }
            _ => ParameterGolfSubmissionPromotionSignificancePosture::RequiredAndMissing,
        }
    } else {
        ParameterGolfSubmissionPromotionSignificancePosture::NotRequired
    };

    let mut refusal_reasons = Vec::new();
    if !candidate.record_track_candidate {
        refusal_reasons.push(String::from(
            "candidate is not targeting the record track, so record promotion review is out of scope",
        ));
    }
    if matches!(
        systems_only_waiver_posture,
        ParameterGolfSubmissionPromotionSystemsOnlyWaiverPosture::ClaimedUnsupported
    ) {
        refusal_reasons.push(String::from(
            "systems-only waiver was claimed but the supplied evidence does not justify treating the candidate as ML-equivalent",
        ));
    }
    if !clears_required_nats_delta
        && !matches!(
            systems_only_waiver_posture,
            ParameterGolfSubmissionPromotionSystemsOnlyWaiverPosture::ClaimedSupported
        )
    {
        refusal_reasons.push(format!(
            "candidate improves the current best by only {:.8} nats per byte, below the required {:.3} nat bar",
            delta_nats_per_byte,
            PARAMETER_GOLF_SOTA_MIN_IMPROVEMENT_NATS_PER_BYTE
        ));
    }
    if matches!(
        significance_posture,
        ParameterGolfSubmissionPromotionSignificancePosture::RequiredAndMissing
    ) {
        refusal_reasons.push(format!(
            "candidate cleared the README delta bar but does not yet carry significance evidence with p <= {:.2}",
            PARAMETER_GOLF_SOTA_REQUIRED_SIGNIFICANCE_P_VALUE
        ));
    }
    if candidate.evidence_refs.is_empty() {
        refusal_reasons.push(String::from(
            "promotion review requires explicit evidence refs instead of README prose alone",
        ));
    }

    ParameterGolfSubmissionPromotionReceipt {
        schema_version: 1,
        report_id: String::from("parameter_golf.submission_promotion_receipt.v1"),
        challenge_rule_snapshot_date: String::from(PARAMETER_GOLF_SOTA_RULE_SNAPSHOT_DATE),
        challenge_readme_ref: String::from("README.md"),
        current_best,
        candidate,
        delta_bpb,
        delta_nats_per_byte,
        clears_required_nats_delta,
        significance_posture,
        systems_only_waiver_posture,
        disposition: if refusal_reasons.is_empty() {
            ParameterGolfSubmissionPromotionDisposition::Promotable
        } else {
            ParameterGolfSubmissionPromotionDisposition::Refused
        },
        refusal_reasons,
        claim_boundary: String::from(
            "This receipt makes the public README promotion gate machine-readable. It does not create score evidence by itself; it only records whether the supplied record-track delta, significance, waiver, and supporting artifacts justify a promotion claim.",
        ),
        receipt_digest: String::new(),
    }
    .with_stable_digest()
}

fn stable_digest<T>(prefix: &[u8], value: &T) -> String
where
    T: Serialize,
{
    let encoded = match serde_json::to_vec(value) {
        Ok(encoded) => encoded,
        Err(error) => error.to_string().into_bytes(),
    };
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(encoded);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use super::{
        PARAMETER_GOLF_CURRENT_PUBLIC_RECORD_SCORE_BPB,
        ParameterGolfSubmissionPromotionDisposition,
        ParameterGolfSubmissionPromotionSignificancePosture,
        ParameterGolfSubmissionPromotionSystemsOnlyWaiverPosture,
        build_parameter_golf_submission_promotion_receipt,
    };

    use super::ParameterGolfSubmissionPromotionCandidate;

    #[test]
    fn parameter_golf_submission_promotion_receipt_refuses_non_record_candidate() {
        let receipt = build_parameter_golf_submission_promotion_receipt(
            ParameterGolfSubmissionPromotionCandidate {
                submission_id: String::from("candidate"),
                run_id: String::from("run"),
                benchmark_ref: String::from("benchmark://openagents/psionic/parameter_golf/local"),
                track_id: String::from("non-record-unlimited-compute-16mb"),
                record_track_candidate: false,
                val_bpb: PARAMETER_GOLF_CURRENT_PUBLIC_RECORD_SCORE_BPB - 0.1,
                systems_only_waiver_claimed: false,
                systems_only_waiver_supported: false,
                significance_p_value: Some(0.001),
                significance_evidence_refs: vec![String::from("significance.json")],
                evidence_refs: vec![String::from("replay.json")],
            },
        );
        assert_eq!(
            receipt.disposition,
            ParameterGolfSubmissionPromotionDisposition::Refused
        );
        assert!(receipt
            .refusal_reasons
            .iter()
            .any(|reason| reason.contains("not targeting the record track")));
    }

    #[test]
    fn parameter_golf_submission_promotion_receipt_waives_significance_for_supported_systems_only_claim(
    ) {
        let receipt = build_parameter_golf_submission_promotion_receipt(
            ParameterGolfSubmissionPromotionCandidate {
                submission_id: String::from("candidate"),
                run_id: String::from("run"),
                benchmark_ref: String::from("benchmark://openagents/psionic/parameter_golf/distributed_8xh100"),
                track_id: String::from("record_10min_16mb"),
                record_track_candidate: true,
                val_bpb: PARAMETER_GOLF_CURRENT_PUBLIC_RECORD_SCORE_BPB,
                systems_only_waiver_claimed: true,
                systems_only_waiver_supported: true,
                significance_p_value: None,
                significance_evidence_refs: Vec::new(),
                evidence_refs: vec![String::from("distributed_receipt.json")],
            },
        );
        assert_eq!(
            receipt.systems_only_waiver_posture,
            ParameterGolfSubmissionPromotionSystemsOnlyWaiverPosture::ClaimedSupported
        );
        assert_eq!(
            receipt.significance_posture,
            ParameterGolfSubmissionPromotionSignificancePosture::WaivedSystemsOnly
        );
        assert_eq!(
            receipt.disposition,
            ParameterGolfSubmissionPromotionDisposition::Promotable
        );
    }

    #[test]
    fn parameter_golf_submission_promotion_receipt_round_trips() -> Result<(), Box<dyn Error>> {
        let receipt = build_parameter_golf_submission_promotion_receipt(
            ParameterGolfSubmissionPromotionCandidate {
                submission_id: String::from("candidate"),
                run_id: String::from("run"),
                benchmark_ref: String::from("benchmark://openagents/psionic/parameter_golf/distributed_8xh100"),
                track_id: String::from("record_10min_16mb"),
                record_track_candidate: true,
                val_bpb: 1.20,
                systems_only_waiver_claimed: false,
                systems_only_waiver_supported: false,
                significance_p_value: Some(0.001),
                significance_evidence_refs: vec![String::from("significance.json")],
                evidence_refs: vec![String::from("replay.json")],
            },
        );
        let encoded = serde_json::to_vec(&receipt)?;
        let decoded: super::ParameterGolfSubmissionPromotionReceipt =
            serde_json::from_slice(&encoded)?;
        assert_eq!(decoded, receipt);
        Ok(())
    }
}
