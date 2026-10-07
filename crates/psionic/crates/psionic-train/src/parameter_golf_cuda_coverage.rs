use psionic_core::{
    builtin_quantization_capability_semantics_report, PsionicRefusal, PsionicRefusalCode,
    PsionicRefusalScope,
};
use psionic_ir::{builtin_advanced_operator_program_matrix_report, GraphError};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::PARAMETER_GOLF_DISTRIBUTED_8XH100_VERSION;

/// One Parameter Golf CUDA training requirement family.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParameterGolfCudaTrainingFamily {
    /// BF16 or FP32 precision-policy requirements for the public baseline lane.
    Precision,
    /// RoPE plus GQA attention requirements for the decoder core.
    Attention,
    /// RMSNorm execution requirements for the decoder core.
    RmsNorm,
    /// Residual addition and residual-mix requirements for the decoder core.
    Residual,
    /// Optimizer requirements such as Muon on matrix parameters.
    Optimizer,
    /// Post-train export or quantization requirements.
    Quantization,
}

/// Status vocabulary for one Parameter Golf CUDA training requirement.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParameterGolfCudaTrainingCoverageStatus {
    /// The current repo has an early but real answer for this requirement.
    ImplementedEarly,
    /// The current repo has some substrate or semantics, but not full direct closure.
    Partial,
    /// The current repo still lacks a credible implementation answer.
    Planned,
}

/// One machine-readable CUDA training coverage case for Parameter Golf.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterGolfCudaTrainingCoverageCase {
    /// Stable case identifier.
    pub case_id: String,
    /// Requirement family covered by the case.
    pub family: ParameterGolfCudaTrainingFamily,
    /// Current status for this requirement.
    pub status: ParameterGolfCudaTrainingCoverageStatus,
    /// Stable statement of what the public Parameter Golf baseline requires.
    pub required_scope: String,
    /// Stable statement of what the repo currently owns.
    pub current_surface: String,
    /// Honest boundary note when the public CUDA surface remains narrower.
    pub boundary_note: String,
}

/// Aggregate machine-readable CUDA training coverage report for Parameter Golf.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterGolfCudaTrainingCapabilityReport {
    /// Stable schema version.
    pub schema_version: u32,
    /// Stable scope window for the report.
    pub scope_window: String,
    /// Distributed-lane version this report is aligned to.
    pub distributed_lane_version: String,
    /// Stable digest for the advanced-operator matrix reused as evidence.
    pub advanced_operator_matrix_digest: String,
    /// Stable digest for the quantization semantics report reused as evidence.
    pub quantization_semantics_digest: String,
    /// Ordered coverage cases.
    pub cases: Vec<ParameterGolfCudaTrainingCoverageCase>,
    /// Stable blocker case identifiers for full challenge-speed CUDA closure.
    pub blocking_case_ids: Vec<String>,
    /// Stable digest over the report contents.
    pub report_digest: String,
}

impl ParameterGolfCudaTrainingCapabilityReport {
    fn new(
        distributed_lane_version: impl Into<String>,
        advanced_operator_matrix_digest: impl Into<String>,
        quantization_semantics_digest: impl Into<String>,
        cases: Vec<ParameterGolfCudaTrainingCoverageCase>,
    ) -> Self {
        let distributed_lane_version = distributed_lane_version.into();
        let advanced_operator_matrix_digest = advanced_operator_matrix_digest.into();
        let quantization_semantics_digest = quantization_semantics_digest.into();
        let blocking_case_ids = cases
            .iter()
            .filter(|case| case.status != ParameterGolfCudaTrainingCoverageStatus::ImplementedEarly)
            .map(|case| case.case_id.clone())
            .collect::<Vec<_>>();
        let report_digest = stable_digest(
            b"psionic_parameter_golf_cuda_training_capability_report|",
            &(
                &distributed_lane_version,
                &advanced_operator_matrix_digest,
                &quantization_semantics_digest,
                &cases,
                &blocking_case_ids,
            ),
        );
        Self {
            schema_version: 1,
            scope_window: String::from("parameter_golf_cuda_training_v1"),
            distributed_lane_version,
            advanced_operator_matrix_digest,
            quantization_semantics_digest,
            cases,
            blocking_case_ids,
            report_digest,
        }
    }

    /// Returns the current blocker case identifiers.
    #[must_use]
    pub fn challenge_kernel_blockers(&self) -> &[String] {
        self.blocking_case_ids.as_slice()
    }

    /// Returns honest boundary-note lines for the remaining blockers.
    #[must_use]
    pub fn boundary_notes(&self) -> Vec<String> {
        self.cases
            .iter()
            .filter(|case| case.status != ParameterGolfCudaTrainingCoverageStatus::ImplementedEarly)
            .map(|case| format!("{}: {}", case.case_id, case.boundary_note))
            .collect()
    }

    /// Returns the canonical refusal for full challenge-speed CUDA closure.
    #[must_use]
    pub fn challenge_readiness_refusal(&self) -> Option<PsionicRefusal> {
        if self.blocking_case_ids.is_empty() {
            return None;
        }
        Some(
            PsionicRefusal::new(
                PsionicRefusalCode::UnsupportedBackendCapability,
                PsionicRefusalScope::Runtime,
                format!(
                    "parameter golf CUDA challenge closure still has explicit blockers: {}",
                    self.blocking_case_ids.join(", ")
                ),
            )
            .with_subject(String::from("parameter_golf_cuda_training")),
        )
    }
}

/// Builds the canonical Parameter Golf CUDA training coverage report.
pub fn builtin_parameter_golf_cuda_training_capability_report(
) -> Result<ParameterGolfCudaTrainingCapabilityReport, GraphError> {
    let advanced_operator_report = builtin_advanced_operator_program_matrix_report()?;
    let quantization_report = builtin_quantization_capability_semantics_report();
    Ok(ParameterGolfCudaTrainingCapabilityReport::new(
        PARAMETER_GOLF_DISTRIBUTED_8XH100_VERSION,
        advanced_operator_report.matrix_digest,
        quantization_report.report_digest,
        vec![
            ParameterGolfCudaTrainingCoverageCase {
                case_id: String::from("cuda_bf16_train_graph_and_optimizer_surface"),
                family: ParameterGolfCudaTrainingFamily::Precision,
                status: ParameterGolfCudaTrainingCoverageStatus::ImplementedEarly,
                required_scope: String::from(
                    "the public 8xH100 baseline requires BF16 train-visible parameters and gradients with FP32 optimizer or master-weight posture",
                ),
                current_surface: String::from(
                    "the distributed 8xH100 receipt lane now encodes an explicit BF16-forward or FP32-master precision policy, the public CUDA runtime now owns BF16 dense buffer residency plus bounded BF16xBF16-to-F32 matmul execution on the same row-major surface already used for mixed-precision baseline primitives, and psionic-train now owns one bounded host-orchestrated CUDA BF16 master-weight optimizer step over BF16 train-visible parameter and gradient buffers with FP32 master weights and FP32 optimizer state",
                ),
                boundary_note: String::from(
                    "Do not treat the bounded host-orchestrated BF16 master-weight step as proof that generic BF16 graph execution, fused optimizer kernels, or challenge-speed mixed-precision trainer closure is already done.",
                ),
            },
            ParameterGolfCudaTrainingCoverageCase {
                case_id: String::from("cuda_rope_gqa_decoder_block_backward_runtime"),
                family: ParameterGolfCudaTrainingFamily::Attention,
                status: ParameterGolfCudaTrainingCoverageStatus::ImplementedEarly,
                required_scope: String::from(
                    "the compact decoder baseline requires train-time RoPE plus grouped-query attention over the public 9x512 family",
                ),
                current_surface: String::from(
                    "psionic-ir now accepts grouped-query attention program shapes and lowers bounded reverse-mode decoder-block gradients through dedicated rotary_embedding_backward plus scaled_dot_product_attention_{query,key,value}_backward ops, and the public CUDA execution backend now executes both the bounded dense f32 non-interleaved rotary plus causal grouped-query decoder forward lane and the matching bounded decoder backward graphs through a host-orchestrated dense f32 runtime on CUDA device buffers",
                ),
                boundary_note: String::from(
                    "Do not treat the bounded host-orchestrated CUDA decoder backward path as proof that fused, fully device-resident, or challenge-speed decoder training closure is already done.",
                ),
            },
            ParameterGolfCudaTrainingCoverageCase {
                case_id: String::from("cuda_rms_norm_train_path"),
                family: ParameterGolfCudaTrainingFamily::RmsNorm,
                status: ParameterGolfCudaTrainingCoverageStatus::ImplementedEarly,
                required_scope: String::from(
                    "the compact decoder baseline requires RMSNorm in the train-time forward or backward path",
                ),
                current_surface: String::from(
                    "the public CUDA execution backend now executes bounded dense contiguous f32 RMSNorm forward plus bounded RMSNorm backward graph ops across batched rows, and psionic-ir owns matching dense f32 reference evaluation plus reverse-mode graph semantics for the same train-visible lane",
                ),
                boundary_note: String::from(
                    "Do not treat bounded dense f32 RMSNorm closure as proof that BF16, decoder-block attention, residual-mix, or optimizer closure is already done.",
                ),
            },
            ParameterGolfCudaTrainingCoverageCase {
                case_id: String::from("cuda_residual_mix_train_path"),
                family: ParameterGolfCudaTrainingFamily::Residual,
                status: ParameterGolfCudaTrainingCoverageStatus::ImplementedEarly,
                required_scope: String::from(
                    "the compact decoder baseline requires residual addition plus learned residual-mix control tensors across the train-time path",
                ),
                current_surface: String::from(
                    "the public CUDA runtime now executes one bounded Parameter Golf residual-mix train graph directly through dense contiguous f32 add and mul plus CUDA backward graphs when the residual-control tensors are already materialized to activation shape",
                ),
                boundary_note: String::from(
                    "Do not treat bounded full-shape residual-control execution as proof that generic broadcast, fused decoder, or RoPE/GQA closure is already done.",
                ),
            },
            ParameterGolfCudaTrainingCoverageCase {
                case_id: String::from("cuda_muon_optimizer_path"),
                family: ParameterGolfCudaTrainingFamily::Optimizer,
                status: ParameterGolfCudaTrainingCoverageStatus::ImplementedEarly,
                required_scope: String::from(
                    "the public baseline uses Muon on matrix-shaped transformer parameters under distributed all-reduce",
                ),
                current_surface: String::from(
                    "psionic-train now owns exact CPU Muon reference-step parity, the distributed matrix-update all-reduce contract, and one bounded public CUDA Muon step that offloads the Newton-Schulz BF16 matmul family to the CUDA dense surface while keeping transpose, norm, and scalar orchestration explicit in Rust for the same matrix parameter groups",
                ),
                boundary_note: String::from(
                    "Do not treat the bounded host-orchestrated CUDA Muon step as proof that the full trainer is already fused, fully device-resident, or challenge-speed closed.",
                ),
            },
            ParameterGolfCudaTrainingCoverageCase {
                case_id: String::from("cuda_quantized_export_roundtrip"),
                family: ParameterGolfCudaTrainingFamily::Quantization,
                status: ParameterGolfCudaTrainingCoverageStatus::ImplementedEarly,
                required_scope: String::from(
                    "the public baseline reports post-train int8 plus zlib artifact size and roundtrip validation",
                ),
                current_surface: String::from(
                    "psionic-train now owns raw safetensors plus int8_zlib export or restore for Parameter Golf, and psionic-core now owns a bounded quantization capability semantics report above raw decode",
                ),
                boundary_note: String::from(
                    "This closes the post-train artifact quantization lane only; it does not imply broader train-time QAT or low-precision optimizer closure.",
                ),
            },
        ],
    ))
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

    use psionic_core::builtin_quantization_capability_semantics_report;
    use psionic_ir::builtin_advanced_operator_program_matrix_report;

    use crate::{
        builtin_parameter_golf_cuda_training_capability_report,
        ParameterGolfCudaTrainingCoverageStatus,
    };

    #[test]
    fn parameter_golf_cuda_training_capability_report_tracks_required_families(
    ) -> Result<(), Box<dyn Error>> {
        let report = builtin_parameter_golf_cuda_training_capability_report()?;
        assert_eq!(report.schema_version, 1);
        assert_eq!(report.cases.len(), 6);
        assert_eq!(
            report.advanced_operator_matrix_digest,
            builtin_advanced_operator_program_matrix_report()?.matrix_digest
        );
        assert_eq!(
            report.quantization_semantics_digest,
            builtin_quantization_capability_semantics_report().report_digest
        );
        assert_eq!(
            report.cases.last().expect("quantization case").status,
            ParameterGolfCudaTrainingCoverageStatus::ImplementedEarly
        );
        assert!(report
            .cases
            .iter()
            .all(|case| case.status == ParameterGolfCudaTrainingCoverageStatus::ImplementedEarly));
        assert!(report.blocking_case_ids.is_empty());
        assert!(!report
            .blocking_case_ids
            .iter()
            .any(|case_id| case_id == "cuda_bf16_train_graph_and_optimizer_surface"));
        assert!(!report
            .blocking_case_ids
            .iter()
            .any(|case_id| case_id == "cuda_rope_gqa_decoder_block_backward_runtime"));
        assert!(!report
            .blocking_case_ids
            .iter()
            .any(|case_id| case_id == "cuda_rms_norm_train_path"));
        assert!(!report
            .blocking_case_ids
            .iter()
            .any(|case_id| case_id == "cuda_residual_mix_train_path"));
        assert!(!report
            .blocking_case_ids
            .iter()
            .any(|case_id| case_id == "cuda_muon_optimizer_path"));
        Ok(())
    }

    #[test]
    fn parameter_golf_cuda_training_capability_report_refuses_full_challenge_closure(
    ) -> Result<(), Box<dyn Error>> {
        let report = builtin_parameter_golf_cuda_training_capability_report()?;
        let refusal = report.challenge_readiness_refusal();
        assert!(refusal.is_none());
        Ok(())
    }
}
