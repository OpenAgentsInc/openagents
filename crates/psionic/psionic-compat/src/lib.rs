//! Machine-readable semantics-claim vocabulary and evidence aggregation.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Human-readable crate ownership summary.
pub const CRATE_ROLE: &str = "compatibility claim vocabulary and semantics evidence aggregation";

/// Stable claim posture for one PyTorch-facing semantics area.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SemanticsClaimPosture {
    /// The area has enough breadth and evidence to make a bounded PyTorch-credible claim.
    PyTorchCredible,
    /// The area has seeded machine-legible evidence, but not enough breadth to claim credibility.
    SeededEvidenceOnly,
    /// The area remains an explicit future compatibility target rather than a current credibility claim.
    PyTorchCompatibleLater,
}

/// One evidence reference that supports the current posture for a claim area.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticsEvidenceRef {
    /// Stable evidence artifact identifier.
    pub artifact_id: String,
    /// Stable digest of the evidence artifact.
    pub artifact_digest: String,
}

impl SemanticsEvidenceRef {
    fn new(artifact_id: impl Into<String>, artifact_digest: impl Into<String>) -> Self {
        Self {
            artifact_id: artifact_id.into(),
            artifact_digest: artifact_digest.into(),
        }
    }
}

/// One machine-readable semantics claim area in the PyTorch-facing vocabulary.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticsClaimArea {
    /// Stable claim-area identifier.
    pub area_id: String,
    /// Current posture for the claim area.
    pub current_posture: SemanticsClaimPosture,
    /// Plain-language scope boundary for the current posture.
    pub bounded_scope: String,
    /// Evidence artifacts that justify the current posture.
    pub evidence_refs: Vec<SemanticsEvidenceRef>,
    /// Concrete blockers that prevent a stronger claim today.
    pub blockers: Vec<String>,
    /// Open issue references that track those blockers when one exists.
    pub blocking_issue_refs: Vec<String>,
}

/// Aggregate machine-readable claim report for Psionic semantics posture.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticsClaimReport {
    /// Stable schema version for the claim report.
    pub schema_version: u32,
    /// Versioned claim vocabulary window.
    pub claim_vocabulary_version: String,
    /// Overall semantics posture for the current report.
    pub overall_posture: SemanticsClaimPosture,
    /// Claim areas carried by the report.
    pub areas: Vec<SemanticsClaimArea>,
    /// Stable digest over the report contents.
    pub report_digest: String,
}

impl SemanticsClaimReport {
    fn new(areas: Vec<SemanticsClaimArea>) -> Self {
        let claim_vocabulary_version = String::from("pytorch_claim_v1");
        let report_digest =
            stable_semantics_claim_report_digest(claim_vocabulary_version.as_str(), &areas);
        Self {
            schema_version: 1,
            claim_vocabulary_version,
            overall_posture: SemanticsClaimPosture::SeededEvidenceOnly,
            areas,
            report_digest,
        }
    }

    /// Returns stable signature lines suitable for fixtures or audits.
    #[must_use]
    pub fn stable_signature_lines(&self) -> Vec<String> {
        let mut lines = vec![
            format!("schema_version={}", self.schema_version),
            format!("claim_vocabulary_version={}", self.claim_vocabulary_version),
            format!("overall_posture={:?}", self.overall_posture),
            format!("report_digest={}", self.report_digest),
        ];
        for area in &self.areas {
            lines.push(format!("{}|{:?}", area.area_id, area.current_posture));
        }
        lines
    }
}

/// Failure returned while aggregating semantics-claim evidence.
#[derive(Debug, Error)]
pub enum SemanticsClaimError {
    /// One operator-parity artifact failed to build.
    #[error(transparent)]
    Operator(#[from] psionic_ir::GraphError),
    /// One module-parity artifact failed to build.
    #[error(transparent)]
    Module(#[from] psionic_nn::ModuleStateError),
    /// One optimizer-parity artifact failed to build.
    #[error(transparent)]
    Optimizer(#[from] psionic_train::TrainingOptimizerError),
    /// One compiler-hygiene artifact failed to build.
    #[error(transparent)]
    Compiler(#[from] psionic_compiler::CompilerHygieneParityError),
}

/// Builds the canonical semantics-claim report for the current Psionic
/// PyTorch-facing posture.
pub fn builtin_semantics_claim_report() -> Result<SemanticsClaimReport, SemanticsClaimError> {
    let operator = psionic_ir::builtin_operator_parity_matrix_report()?;
    let module = psionic_nn::builtin_module_parity_matrix_report()?;
    let optimizer = psionic_train::builtin_optimizer_parity_matrix_report()?;
    let compiler = psionic_compiler::builtin_compiler_hygiene_parity_matrix_report()?;

    Ok(SemanticsClaimReport::new(vec![
        seeded_area(
            "operator_semantics",
            "Current claim is bounded to the seeded OpInfo-style matrix over add, mul, matmul, reshape, permute, concat, scaled_dot_product_attention, and one explicit backend-capability refusal.",
            vec![SemanticsEvidenceRef::new(
                "operator_parity_matrix",
                operator.matrix_digest,
            )],
            vec![
                String::from("broaden operator-family breadth beyond the current seeded cases"),
                String::from("add wider dtype and device cartesian coverage"),
                String::from(
                    "prove runtime numerical parity beyond meta-execution shape contracts",
                ),
            ],
            vec![
                String::from("#3735"),
                String::from("#3726"),
                String::from("#3725"),
            ],
        ),
        seeded_area(
            "module_and_state_semantics",
            "Current claim is bounded to normalized module-tree and state_dict semantics for linear, batch_norm1d, and one nested transformer-style fixture plus one explicit registration-order refusal proof.",
            vec![SemanticsEvidenceRef::new(
                "module_parity_matrix",
                module.matrix_digest,
            )],
            vec![
                String::from("prove forward numerics parity for standard modules"),
                String::from("support registration-order-preserving serialization where required"),
                String::from("widen module-family breadth beyond the current seeded fixtures"),
            ],
            Vec::new(),
        ),
        seeded_area(
            "optimizer_semantics",
            "Current claim is bounded to single-step optimizer semantics for SGD, Adam, AdamW, LARS, and LAMB plus one explicit state-kind refusal proof.",
            vec![SemanticsEvidenceRef::new(
                "optimizer_parity_matrix",
                optimizer.matrix_digest,
            )],
            vec![
                String::from("broaden parameter-group and multi-step optimizer parity coverage"),
                String::from("add mixed-precision and distributed optimizer claim surfaces"),
                String::from("add wider scheduler parity coverage"),
            ],
            vec![
                String::from("#3728"),
                String::from("#3729"),
                String::from("#3734"),
            ],
        ),
        seeded_area(
            "compiler_hygiene_and_fake_tensor",
            "Current claim is bounded to seeded fake-tensor graph-vs-plan parity, non-dense meta-tensor contracts, cache-temperature hygiene, alias-aware memory planning, and one explicit symbolic-shape refusal proof.",
            vec![SemanticsEvidenceRef::new(
                "compiler_hygiene_parity_matrix",
                compiler.matrix_digest,
            )],
            vec![
                String::from("land symbolic-shape environments and guard simplification"),
                String::from("land transform and export breadth above the current fake/meta slice"),
            ],
            vec![String::from("#3731"), String::from("#3736")],
        ),
        future_area(
            "checkpoint_and_model_io_interop",
            "Portable model-IO and compatibility contracts exist, but PyTorch-credible checkpoint migration breadth is not claimed yet.",
            vec![
                String::from(
                    "broaden state-dict and checkpoint migration beyond the current bounded formats",
                ),
                String::from(
                    "prove practical migration behavior over dense and sharded checkpoint families",
                ),
            ],
            vec![String::from("#3736")],
        ),
        future_area(
            "advanced_tensor_dtype_and_precision",
            "Advanced tensor families, dtype breadth, autocast policy, and gradient-scaling semantics remain explicit future compatibility targets.",
            vec![
                String::from("land sparse, nested, masked, and storage-aware tensor semantics"),
                String::from("land advanced dtype and promotion rules"),
                String::from("land autocast and gradient-scaling systems"),
            ],
            vec![
                String::from("#3725"),
                String::from("#3726"),
                String::from("#3728"),
                String::from("#3729"),
            ],
        ),
        future_area(
            "quantization_and_export",
            "Quantization, export-safe graphs, and deployment-facing compatibility remain future compatibility targets rather than current credibility claims.",
            vec![
                String::from(
                    "land quantization as a library capability family above file-format decode",
                ),
                String::from("land export-safe graph and deployment artifact contracts"),
            ],
            vec![String::from("#3730"), String::from("#3736")],
        ),
        future_area(
            "data_and_distributed_training_semantics",
            "Dataset, sampler, host-device staging, and distributed input-order semantics remain future compatibility targets rather than current credibility claims.",
            vec![
                String::from("land dataset, iterable-streaming, sampler, and staging abstractions"),
                String::from("land distributed and sharded data-feed semantics"),
            ],
            vec![String::from("#3733"), String::from("#3734")],
        ),
        future_area(
            "extensions_and_plugins",
            "User-facing extension and plugin contracts remain future compatibility targets rather than current credibility claims.",
            vec![String::from(
                "publish custom-op, custom-kernel, custom-autograd, backend-plugin, and quantizer-plugin contracts",
            )],
            vec![String::from("#3732")],
        ),
        future_area(
            "advanced_operator_families",
            "Advanced linalg, signal, distributions, special-function, and attention-family semantics remain future compatibility targets rather than current credibility claims.",
            vec![String::from(
                "land advanced operator-family programs beyond the current seeded subset",
            )],
            vec![String::from("#3735")],
        ),
    ]))
}

fn seeded_area(
    area_id: &str,
    bounded_scope: &str,
    evidence_refs: Vec<SemanticsEvidenceRef>,
    blockers: Vec<String>,
    blocking_issue_refs: Vec<String>,
) -> SemanticsClaimArea {
    SemanticsClaimArea {
        area_id: String::from(area_id),
        current_posture: SemanticsClaimPosture::SeededEvidenceOnly,
        bounded_scope: String::from(bounded_scope),
        evidence_refs,
        blockers,
        blocking_issue_refs,
    }
}

fn future_area(
    area_id: &str,
    bounded_scope: &str,
    blockers: Vec<String>,
    blocking_issue_refs: Vec<String>,
) -> SemanticsClaimArea {
    SemanticsClaimArea {
        area_id: String::from(area_id),
        current_posture: SemanticsClaimPosture::PyTorchCompatibleLater,
        bounded_scope: String::from(bounded_scope),
        evidence_refs: Vec::new(),
        blockers,
        blocking_issue_refs,
    }
}

fn stable_semantics_claim_report_digest(
    claim_vocabulary_version: &str,
    areas: &[SemanticsClaimArea],
) -> String {
    let mut lines = vec![format!(
        "claim_vocabulary_version={claim_vocabulary_version}"
    )];
    for area in areas {
        lines.push(format!("{}|{:?}", area.area_id, area.current_posture));
        lines.push(format!("scope={}", area.bounded_scope));
        for evidence in &area.evidence_refs {
            lines.push(format!(
                "evidence={}|{}",
                evidence.artifact_id, evidence.artifact_digest
            ));
        }
        for blocker in &area.blockers {
            lines.push(format!("blocker={blocker}"));
        }
        for issue in &area.blocking_issue_refs {
            lines.push(format!("issue={issue}"));
        }
    }
    lines.sort();
    let mut hasher = Sha256::new();
    for line in lines {
        hasher.update(line.as_bytes());
        hasher.update(b"\n");
    }
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::{SemanticsClaimPosture, builtin_semantics_claim_report};

    #[test]
    fn semantics_claim_report_marks_seeded_evidence_and_future_compatibility_targets()
    -> Result<(), Box<dyn std::error::Error>> {
        let report = builtin_semantics_claim_report()?;
        assert_eq!(report.schema_version, 1);
        assert_eq!(report.claim_vocabulary_version, "pytorch_claim_v1");
        assert_eq!(
            report.overall_posture,
            SemanticsClaimPosture::SeededEvidenceOnly
        );
        assert!(
            report
                .stable_signature_lines()
                .iter()
                .any(|line| line.starts_with("report_digest="))
        );

        for area_id in [
            "operator_semantics",
            "module_and_state_semantics",
            "optimizer_semantics",
            "compiler_hygiene_and_fake_tensor",
        ] {
            let area = report
                .areas
                .iter()
                .find(|area| area.area_id == area_id)
                .expect("missing seeded-evidence area");
            assert_eq!(
                area.current_posture,
                SemanticsClaimPosture::SeededEvidenceOnly
            );
            assert!(!area.evidence_refs.is_empty());
        }

        for area_id in [
            "checkpoint_and_model_io_interop",
            "advanced_tensor_dtype_and_precision",
            "quantization_and_export",
            "data_and_distributed_training_semantics",
            "extensions_and_plugins",
            "advanced_operator_families",
        ] {
            let area = report
                .areas
                .iter()
                .find(|area| area.area_id == area_id)
                .expect("missing future compatibility area");
            assert_eq!(
                area.current_posture,
                SemanticsClaimPosture::PyTorchCompatibleLater
            );
            assert!(area.evidence_refs.is_empty());
            assert!(!area.blockers.is_empty());
        }

        Ok(())
    }
}
