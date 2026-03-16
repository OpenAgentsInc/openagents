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
    /// One export/deployment artifact semantics report failed to build.
    #[error(transparent)]
    ExportDeployment(#[from] psionic_compiler::ExportDeploymentArtifactSemanticsError),
}

/// Builds the canonical semantics-claim report for the current Psionic
/// PyTorch-facing posture.
pub fn builtin_semantics_claim_report() -> Result<SemanticsClaimReport, SemanticsClaimError> {
    let operator = psionic_ir::builtin_operator_parity_matrix_report()?;
    let advanced_operator_programs = psionic_ir::builtin_advanced_operator_program_matrix_report()?;
    let extensions = psionic_ir::builtin_extension_contract_semantics_report();
    let program_transforms = psionic_ir::builtin_program_transform_capability_matrix_report();
    let data_ingress = psionic_data::builtin_data_ingress_semantics_report();
    let distributed_data_feed = psionic_data::builtin_distributed_data_feed_semantics_report();
    let tensor_families = psionic_ir::builtin_tensor_family_capability_matrix_report();
    let advanced_dtypes = psionic_core::builtin_advanced_dtype_semantics_report();
    let autocast = psionic_core::builtin_autocast_policy_matrix_report();
    let quantization = psionic_core::builtin_quantization_capability_semantics_report();
    let module = psionic_nn::builtin_module_parity_matrix_report()?;
    let optimizer = psionic_train::builtin_optimizer_parity_matrix_report()?;
    let gradient_scaling = psionic_train::builtin_gradient_scaling_semantics_report();
    let reproducibility = psionic_train::builtin_reproducibility_semantics_report();
    let compiler = psionic_compiler::builtin_compiler_hygiene_parity_matrix_report()?;
    let export_deployment =
        psionic_compiler::builtin_export_deployment_artifact_semantics_report()?;

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
            vec![String::from("#3726")],
        ),
        seeded_area(
            "advanced_operator_family_semantics",
            "Current claim is bounded to reusable linalg gram-matrix, signal Fourier-projection, and rotary-attention residual programs above the compact core plus explicit refusal for distribution and special-function families that still require broader normalization, sampling, or special-function primitives.",
            vec![SemanticsEvidenceRef::new(
                "advanced_operator_program_matrix",
                advanced_operator_programs.matrix_digest,
            )],
            vec![
                String::from("broaden advanced operator-family coverage beyond the current linalg, signal, and attention seed programs"),
                String::from("land concrete distribution and special-function primitives instead of refusal-only family placeholders"),
            ],
            Vec::new(),
        ),
        seeded_area(
            "program_transform_semantics",
            "Current claim is bounded to functionalization, symbolic-rewrite readiness, and export-safe graph handoff over graphs with explicit alias-root and barrier reporting, plus explicit future refusal for `vmap`, `jvp`, and `jacobian`.",
            vec![SemanticsEvidenceRef::new(
                "program_transform_capability_matrix",
                program_transforms.matrix_digest,
            )],
            vec![
                String::from("broaden transform capability beyond functionalization and export-safe graph readiness"),
                String::from("connect export-safe graph capability to deployment artifact contracts"),
            ],
            Vec::new(),
        ),
        seeded_area(
            "export_and_deployment_semantics",
            "Current claim is bounded to export-safe functionalized graph contracts with named entry signatures plus deployment artifact contracts over execution-plan and topology-aware bundles, including explicit refusal for opaque backend-extension graphs and graph-digest mismatches.",
            vec![SemanticsEvidenceRef::new(
                "export_deployment_artifact_semantics",
                export_deployment.report_digest,
            )],
            vec![
                String::from("broaden export and deployment semantics beyond the current execution-plan and topology-aware bundle window"),
                String::from("connect checkpoint migration and broader plugin distribution to the graph-first deployment contracts"),
            ],
            Vec::new(),
        ),
        seeded_area(
            "extension_contract_semantics",
            "Current claim is bounded to typed custom-op, custom-kernel, custom-autograd, backend-plugin, and quantizer-plugin contracts above the existing extensible registry, plus explicit refusal for contracts that bypass declared-output or non-dense quantization-mode requirements.",
            vec![SemanticsEvidenceRef::new(
                "extension_contract_semantics",
                extensions.report_digest,
            )],
            vec![
                String::from("broaden extension semantics beyond the current typed contract bundles"),
                String::from("connect extension contracts to deployment and distribution surfaces"),
            ],
            Vec::new(),
        ),
        seeded_area(
            "data_ingress_semantics",
            "Current claim is bounded to reusable local data-ingress contracts covering map-style and iterable-streaming dataset access, sequential and deterministic-shuffle sampling, batch-sampler policy above packing contracts, and direct-host or pinned-prefetch staging into one target device lane.",
            vec![SemanticsEvidenceRef::new(
                "data_ingress_semantics",
                data_ingress.report_digest,
            )],
            vec![
                String::from("broaden data ingress beyond the current local source, sampler, and staging window"),
            ],
            Vec::new(),
        ),
        seeded_area(
            "distributed_data_feed_semantics",
            "Current claim is bounded to fixed-world-size distributed sampler partitioning over shard-ordered local ingress contracts, with contiguous-block and rank-strided shard assignment, explicit epoch-barrier or fixed-cadence step-barrier worker coordination, runtime-derived per-rank replay generators, and explicit refusal for elastic membership.",
            vec![SemanticsEvidenceRef::new(
                "distributed_data_feed_semantics",
                distributed_data_feed.report_digest,
            )],
            vec![
                String::from("broaden distributed data-feed semantics beyond the current fixed-world-size shard partitioning window"),
                String::from("connect elastic membership and topology revision to replay-safe worker ordering"),
            ],
            Vec::new(),
        ),
        seeded_area(
            "tensor_family_semantics",
            "Current claim is bounded to first-class sparse, nested, masked, and storage-aware tensor-family contracts, their meta-execution and declared-output capability matrix, and explicit refusal proofs for non-dense runtime materialization.",
            vec![SemanticsEvidenceRef::new(
                "tensor_family_capability_matrix",
                tensor_families.matrix_digest,
            )],
            vec![
                String::from("land runtime materialization for non-dense tensor families"),
                String::from(
                    "broaden tensor-family behavior beyond meta, alias, and declared-output contracts",
                ),
                String::from(
                    "connect broader operator-family and export semantics to these non-dense families",
                ),
            ],
            Vec::new(),
        ),
        seeded_area(
            "advanced_dtype_semantics",
            "Current claim is bounded to seeded advanced-dtype promotion, cast, and backend-capability rules over complex, float8, wider integer, and higher-precision real dtypes, plus an explicit bridge back down to the compact runtime-core `DType` subset.",
            vec![SemanticsEvidenceRef::new(
                "advanced_dtype_semantics",
                advanced_dtypes.report_digest,
            )],
            vec![
                String::from("broaden promotion and cast coverage beyond the current seeded matrix"),
                String::from(
                    "connect broader operator-family semantics to the richer dtype vocabulary",
                ),
                String::from("materialize additional dtypes beyond the compact runtime-core subset"),
            ],
            Vec::new(),
        ),
        seeded_area(
            "reproducibility_semantics",
            "Current claim is bounded to framework-wide replay seed discipline, strict trainer and eval runtime contracts, stable local-device and distributed-rank generator derivation, and checkpoint-stable RNG restore across the current runtime and training replay substrate.",
            vec![SemanticsEvidenceRef::new(
                "reproducibility_semantics",
                reproducibility.report_digest,
            )],
            vec![
                String::from("broaden reproducibility coverage beyond the current seeded runtime and replay cases"),
                String::from("connect mixed-precision and distributed data-feed semantics to the replayable RNG contract"),
                String::from("extend checkpointed RNG restore deeper into later train-loop and export surfaces"),
            ],
            Vec::new(),
        ),
        seeded_area(
            "precision_policy_semantics",
            "Current claim is bounded to seeded autocast-style precision-policy rules over backend family, preferred low-precision dtype, operator family, and numerics diagnostics plus train-class gradient scaling, overflow, and underflow handling over the bounded fp16 dynamic-scaling and bf16 no-scaling window.",
            vec![
                SemanticsEvidenceRef::new("autocast_policy_matrix", autocast.report_digest),
                SemanticsEvidenceRef::new(
                    "gradient_scaling_semantics",
                    gradient_scaling.report_digest,
                ),
            ],
            vec![
                String::from("broaden autocast and train-class mixed-precision coverage beyond the current seeded fp16 and bf16 cases"),
                String::from(
                    "connect distributed data-feed semantics and wider operator-family coverage to the mixed-precision surface",
                ),
                String::from("extend backend capability truth beyond the current bounded runtime-vs-meta split"),
            ],
            Vec::new(),
        ),
        seeded_area(
            "quantization_semantics",
            "Current claim is bounded to seeded PTQ, QAT, runtime-execution, compiler-lowering, and export-aware quantization capability semantics above raw file-format decode, plus explicit refusal for unsupported block-quant QAT and broader runtime activation-dtype closure.",
            vec![SemanticsEvidenceRef::new(
                "quantization_capability_semantics",
                quantization.report_digest,
            )],
            vec![
                String::from("broaden quantization coverage beyond the current int8 and ggml_q4_0 seeded cases"),
                String::from("connect quantization capability to extension and plugin contracts"),
                String::from("land deployment-facing export artifacts on top of the export-aware quantization surface"),
            ],
            Vec::new(),
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
                String::from(
                    "add broader mixed-precision and distributed optimizer claim surfaces",
                ),
                String::from("add wider scheduler parity coverage"),
            ],
            vec![String::from("#3734")],
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
                String::from("land export breadth above the current fake/meta slice"),
            ],
            Vec::new(),
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
            Vec::new(),
        ),
        future_area(
            "advanced_tensor_dtype_and_precision",
            "Broader mixed-precision behavior and train-class precision control remain explicit future compatibility targets after landing bounded tensor-family, dtype, reproducibility, autocast, and gradient-scaling seed coverage.",
            vec![
                String::from("broaden mixed-precision and train-class precision control beyond the current seeded fp16/bf16 window"),
            ],
            Vec::new(),
        ),
        future_area(
            "extensions_and_plugins",
            "Broader extension and plugin distribution, loading, and deployment behavior remain future compatibility targets after landing bounded typed contracts.",
            vec![String::from(
                "connect extension contracts to deployment and distribution surfaces",
            )],
            Vec::new(),
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

    use super::{builtin_semantics_claim_report, SemanticsClaimPosture};

    #[test]
    fn semantics_claim_report_marks_seeded_evidence_and_future_compatibility_targets(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let report = builtin_semantics_claim_report()?;
        assert_eq!(report.schema_version, 1);
        assert_eq!(report.claim_vocabulary_version, "pytorch_claim_v1");
        assert_eq!(
            report.overall_posture,
            SemanticsClaimPosture::SeededEvidenceOnly
        );
        assert!(report
            .stable_signature_lines()
            .iter()
            .any(|line| line.starts_with("report_digest=")));

        for area_id in [
            "operator_semantics",
            "advanced_operator_family_semantics",
            "program_transform_semantics",
            "export_and_deployment_semantics",
            "extension_contract_semantics",
            "data_ingress_semantics",
            "distributed_data_feed_semantics",
            "tensor_family_semantics",
            "advanced_dtype_semantics",
            "reproducibility_semantics",
            "precision_policy_semantics",
            "quantization_semantics",
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
            "extensions_and_plugins",
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
