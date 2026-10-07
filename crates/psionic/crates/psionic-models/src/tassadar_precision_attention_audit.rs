use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::ModelDescriptor;

const TASSADAR_PRECISION_ATTENTION_AUDIT_PUBLICATION_SCHEMA_VERSION: u16 = 1;

pub const TASSADAR_PRECISION_ATTENTION_AUDIT_CLAIM_CLASS: &str =
    "research_only_execution_truth";
pub const TASSADAR_PRECISION_ATTENTION_RUNTIME_AUDIT_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_precision_attention_runtime_audit.json";
pub const TASSADAR_PRECISION_ATTENTION_ROBUSTNESS_AUDIT_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_precision_attention_robustness_audit.json";
pub const TASSADAR_PRECISION_ATTENTION_ROBUSTNESS_SUMMARY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_precision_attention_robustness_summary.json";

/// Repo-facing publication status for the precision/attention audit lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPrecisionAttentionAuditPublicationStatus {
    /// Landed as a repo-backed public research surface.
    Implemented,
}

/// Numeric regime audited against the current executor workloads.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarNumericRegime {
    /// Reference floating-point regime used as the exact baseline.
    Fp32Reference,
    /// Lower-precision floating-point regime commonly used in served execution.
    Fp16Served,
    /// Int8-style served quantization regime without explicit extra noise.
    Int8Served,
    /// Int8-style served quantization regime under an added deterministic noise budget.
    Int8ServedWithNoise,
}

impl TassadarNumericRegime {
    /// Returns the stable regime label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Fp32Reference => "fp32_reference",
            Self::Fp16Served => "fp16_served",
            Self::Int8Served => "int8_served",
            Self::Int8ServedWithNoise => "int8_served_with_noise",
        }
    }
}

/// Attention semantics family audited against the current executor workloads.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarAttentionSemanticsFamily {
    /// Hard-selection reference semantics.
    HardSelectionReference,
    /// Sparse validated attention semantics aligned with the current SparseTopK row.
    SparseValidated,
    /// Soft proxy semantics aligned with the current chunked/Reformer-style proxy row.
    SoftProxy,
}

impl TassadarAttentionSemanticsFamily {
    /// Returns the stable family label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::HardSelectionReference => "hard_selection_reference",
            Self::SparseValidated => "sparse_validated",
            Self::SoftProxy => "soft_proxy",
        }
    }
}

/// Drift classification used by the finite-precision and attention-semantics audit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarRobustnessDriftClass {
    /// The audited regime stays exact on the declared workload.
    Exact,
    /// The audited regime stays bounded but loses exact equivalence.
    ApproximateBounded,
    /// The audited regime should refuse rather than silently degrade.
    Refused,
}

/// Public publication for the finite-precision and attention-semantics audit lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPrecisionAttentionAuditPublication {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable publication identifier.
    pub publication_id: String,
    /// Repo status vocabulary value.
    pub status: TassadarPrecisionAttentionAuditPublicationStatus,
    /// Explicit claim class for the lane.
    pub claim_class: String,
    /// Stable model descriptor for the lane.
    pub model: ModelDescriptor,
    /// Ordered workload families audited today.
    pub workload_families: Vec<String>,
    /// Ordered numeric regimes audited today.
    pub numeric_regimes: Vec<TassadarNumericRegime>,
    /// Ordered attention families audited today.
    pub attention_families: Vec<TassadarAttentionSemanticsFamily>,
    /// Stable target surfaces implementing the lane.
    pub target_surfaces: Vec<String>,
    /// Stable validation refs for the lane.
    pub validation_refs: Vec<String>,
    /// Explicit support boundaries that remain out of scope.
    pub support_boundaries: Vec<String>,
    /// Stable digest over the publication.
    pub publication_digest: String,
}

impl TassadarPrecisionAttentionAuditPublication {
    fn new() -> Self {
        let mut publication = Self {
            schema_version: TASSADAR_PRECISION_ATTENTION_AUDIT_PUBLICATION_SCHEMA_VERSION,
            publication_id: String::from("tassadar.precision_attention_audit.publication.v1"),
            status: TassadarPrecisionAttentionAuditPublicationStatus::Implemented,
            claim_class: String::from(TASSADAR_PRECISION_ATTENTION_AUDIT_CLAIM_CLASS),
            model: ModelDescriptor::new(
                "tassadar-precision-attention-audit-v0",
                "tassadar_precision_attention_audit",
                "v0",
            ),
            workload_families: vec![
                String::from("micro_wasm_kernel"),
                String::from("branch_heavy_kernel"),
                String::from("memory_heavy_kernel"),
                String::from("long_loop_kernel"),
                String::from("sudoku_class"),
                String::from("hungarian_matching"),
            ],
            numeric_regimes: vec![
                TassadarNumericRegime::Fp32Reference,
                TassadarNumericRegime::Fp16Served,
                TassadarNumericRegime::Int8Served,
                TassadarNumericRegime::Int8ServedWithNoise,
            ],
            attention_families: vec![
                TassadarAttentionSemanticsFamily::HardSelectionReference,
                TassadarAttentionSemanticsFamily::SparseValidated,
                TassadarAttentionSemanticsFamily::SoftProxy,
            ],
            target_surfaces: vec![
                String::from("crates/psionic-models"),
                String::from("crates/psionic-runtime"),
                String::from("crates/psionic-eval"),
                String::from("crates/psionic-research"),
            ],
            validation_refs: vec![
                String::from(TASSADAR_PRECISION_ATTENTION_RUNTIME_AUDIT_REPORT_REF),
                String::from(TASSADAR_PRECISION_ATTENTION_ROBUSTNESS_AUDIT_REPORT_REF),
                String::from(TASSADAR_PRECISION_ATTENTION_ROBUSTNESS_SUMMARY_REPORT_REF),
            ],
            support_boundaries: vec![
                String::from(
                    "the lane is a research-only robustness audit over declared numeric and attention regimes; it does not prove that theoretical universality survives finite-precision deployment or proxy attention substitutions",
                ),
                String::from(
                    "exact, approximate_bounded, and refused remain separate drift classes; approximate results here do not widen served exactness or collapse refusal boundaries into one generic score",
                ),
                String::from(
                    "the current audit is bounded to the published Tassadar workload families and proxy attention baselines; it does not yet imply arbitrary Wasm closure, broad learned exactness, or deployment-wide backend invariance",
                ),
            ],
            publication_digest: String::new(),
        };
        publication.publication_digest = stable_digest(
            b"psionic_tassadar_precision_attention_audit_publication|",
            &publication,
        );
        publication
    }
}

/// Returns the canonical public publication for the precision/attention audit lane.
#[must_use]
pub fn tassadar_precision_attention_audit_publication() -> TassadarPrecisionAttentionAuditPublication
{
    TassadarPrecisionAttentionAuditPublication::new()
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
        tassadar_precision_attention_audit_publication,
        TassadarAttentionSemanticsFamily, TassadarNumericRegime,
        TassadarPrecisionAttentionAuditPublicationStatus,
    };

    #[test]
    fn precision_attention_audit_publication_is_machine_legible() {
        let publication = tassadar_precision_attention_audit_publication();

        assert_eq!(
            publication.status,
            TassadarPrecisionAttentionAuditPublicationStatus::Implemented
        );
        assert!(publication
            .numeric_regimes
            .contains(&TassadarNumericRegime::Int8ServedWithNoise));
        assert!(publication
            .attention_families
            .contains(&TassadarAttentionSemanticsFamily::SoftProxy));
        assert_eq!(publication.workload_families.len(), 6);
        assert!(!publication.publication_digest.is_empty());
    }
}
