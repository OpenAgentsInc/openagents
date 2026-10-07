use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    tassadar_precision_attention_audit::TASSADAR_PRECISION_ATTENTION_ROBUSTNESS_AUDIT_REPORT_REF,
    ModelDescriptor,
};
use psionic_runtime::{
    build_tassadar_quantization_truth_envelope_runtime_report, TassadarQuantizationBackendFamily,
    TassadarQuantizationNumericRegime, TassadarQuantizationSetting,
    TassadarQuantizationTruthEnvelopeReceipt,
    TASSADAR_QUANTIZATION_TRUTH_ENVELOPE_RUNTIME_REPORT_REF,
};

const TASSADAR_QUANTIZATION_TRUTH_ENVELOPE_PUBLICATION_SCHEMA_VERSION: u16 = 1;

pub const TASSADAR_QUANTIZATION_TRUTH_ENVELOPE_CLAIM_CLASS: &str =
    "execution_truth_served_capability";
pub const TASSADAR_QUANTIZATION_TRUTH_ENVELOPE_EVAL_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_quantization_truth_envelope_eval_report.json";

/// Repo-facing publication status for the quantization truth-envelope lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarQuantizationTruthEnvelopePublicationStatus {
    /// Landed as a repo-backed public surface.
    Implemented,
}

/// Public publication for backend and quantization deployment truth envelopes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarQuantizationTruthEnvelopePublication {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable publication identifier.
    pub publication_id: String,
    /// Repo status vocabulary value.
    pub status: TassadarQuantizationTruthEnvelopePublicationStatus,
    /// Explicit claim class for the lane.
    pub claim_class: String,
    /// Stable model descriptor for the lane.
    pub model: ModelDescriptor,
    /// Ordered backend families covered today.
    pub backend_families: Vec<TassadarQuantizationBackendFamily>,
    /// Ordered numeric regimes covered today.
    pub numeric_regimes: Vec<TassadarQuantizationNumericRegime>,
    /// Ordered quantization families covered today.
    pub quantization_settings: Vec<TassadarQuantizationSetting>,
    /// Ordered deployment envelopes.
    pub envelope_receipts: Vec<TassadarQuantizationTruthEnvelopeReceipt>,
    /// Stable validation refs for the lane.
    pub validation_refs: Vec<String>,
    /// Explicit support boundaries that remain out of scope.
    pub support_boundaries: Vec<String>,
    /// Stable digest over the publication.
    pub publication_digest: String,
}

impl TassadarQuantizationTruthEnvelopePublication {
    fn new() -> Self {
        let runtime_report = build_tassadar_quantization_truth_envelope_runtime_report();
        let mut publication = Self {
            schema_version: TASSADAR_QUANTIZATION_TRUTH_ENVELOPE_PUBLICATION_SCHEMA_VERSION,
            publication_id: String::from(
                "tassadar.quantization_truth_envelope.publication.v1",
            ),
            status: TassadarQuantizationTruthEnvelopePublicationStatus::Implemented,
            claim_class: String::from(TASSADAR_QUANTIZATION_TRUTH_ENVELOPE_CLAIM_CLASS),
            model: ModelDescriptor::new(
                "tassadar-quantization-truth-envelope-v0",
                "tassadar_quantization_truth_envelope",
                "v0",
            ),
            backend_families: vec![
                TassadarQuantizationBackendFamily::CpuReference,
                TassadarQuantizationBackendFamily::MetalServed,
                TassadarQuantizationBackendFamily::CudaServed,
            ],
            numeric_regimes: vec![
                TassadarQuantizationNumericRegime::Fp32Reference,
                TassadarQuantizationNumericRegime::Bf16Served,
                TassadarQuantizationNumericRegime::Fp8Served,
                TassadarQuantizationNumericRegime::Int8Served,
                TassadarQuantizationNumericRegime::Int4Served,
            ],
            quantization_settings: vec![
                TassadarQuantizationSetting::NoneDense,
                TassadarQuantizationSetting::Bf16Cast,
                TassadarQuantizationSetting::Fp8Block,
                TassadarQuantizationSetting::Int8Block,
                TassadarQuantizationSetting::Int4Grouped,
            ],
            envelope_receipts: runtime_report.envelope_receipts,
            validation_refs: vec![
                String::from(TASSADAR_QUANTIZATION_TRUTH_ENVELOPE_RUNTIME_REPORT_REF),
                String::from(TASSADAR_PRECISION_ATTENTION_ROBUSTNESS_AUDIT_REPORT_REF),
                String::from(TASSADAR_QUANTIZATION_TRUTH_ENVELOPE_EVAL_REPORT_REF),
            ],
            support_boundaries: vec![
                String::from(
                    "the publication keeps backend family, numeric regime, quantization family, constrained workloads, and refusal posture explicit instead of assuming backend-invariant exactness for one executor artifact",
                ),
                String::from(
                    "publish_exact and publish_constrained stay separate from refuse_publication; constrained rows do not widen into broad served exactness or backend-agnostic claims",
                ),
                String::from(
                    "the lane is bounded to the current executor workload families and known deployment regimes; it does not claim arbitrary Wasm closure, broad import closure, or deployment-robust exactness under all future quantization/export paths",
                ),
            ],
            publication_digest: String::new(),
        };
        publication.publication_digest = stable_digest(
            b"psionic_tassadar_quantization_truth_envelope_publication|",
            &publication,
        );
        publication
    }
}

/// Returns the canonical public publication for quantization deployment truth envelopes.
#[must_use]
pub fn tassadar_quantization_truth_envelope_publication()
-> TassadarQuantizationTruthEnvelopePublication {
    TassadarQuantizationTruthEnvelopePublication::new()
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        tassadar_quantization_truth_envelope_publication,
        TassadarQuantizationTruthEnvelopePublicationStatus,
    };
    use psionic_runtime::{
        TassadarQuantizationBackendFamily, TassadarQuantizationNumericRegime,
        TassadarQuantizationSetting,
    };

    #[test]
    fn quantization_truth_envelope_publication_is_machine_legible() {
        let publication = tassadar_quantization_truth_envelope_publication();

        assert_eq!(
            publication.status,
            TassadarQuantizationTruthEnvelopePublicationStatus::Implemented
        );
        assert!(publication
            .backend_families
            .contains(&TassadarQuantizationBackendFamily::CudaServed));
        assert!(publication
            .numeric_regimes
            .contains(&TassadarQuantizationNumericRegime::Int4Served));
        assert!(publication
            .quantization_settings
            .contains(&TassadarQuantizationSetting::Fp8Block));
        assert_eq!(publication.envelope_receipts.len(), 5);
        assert!(!publication.publication_digest.is_empty());
    }
}
