use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{TassadarExecutorDecodeMode, TassadarExecutorSelectionState};

/// Route posture bound into the direct model-weight execution proof.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarDirectModelWeightRoutePosture {
    /// The published route stays direct for the requested decode mode.
    DirectGuaranteed,
    /// The published route may fall back and therefore cannot close the proof.
    FallbackCapable,
}

/// Route binding carried into the direct model-weight execution proof.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarDirectModelWeightRouteBinding {
    /// Stable routed product identifier.
    pub route_product_id: String,
    /// Stable route identifier.
    pub route_id: String,
    /// Stable route descriptor digest.
    pub route_descriptor_digest: String,
    /// Stable route model identifier.
    pub route_model_id: String,
    /// Stable benchmark report backing the route posture.
    pub benchmark_report_ref: String,
    /// Requested decode mode carried by the route binding.
    pub requested_decode_mode: TassadarExecutorDecodeMode,
    /// Direct or fallback-capable route posture.
    pub route_posture: TassadarDirectModelWeightRoutePosture,
    /// Plain-language route boundary.
    pub note: String,
}

/// Input surface validated before a direct model-weight proof receipt is emitted.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarDirectModelWeightExecutionProofInput {
    /// Stable receipt identifier.
    pub receipt_id: String,
    /// Stable benchmark reference.
    pub benchmark_ref: String,
    /// Stable benchmark environment reference.
    pub benchmark_environment_ref: String,
    /// Stable benchmark report reference.
    pub benchmark_report_ref: String,
    /// Stable workload family identifier.
    pub workload_family_id: String,
    /// Stable article case identifier.
    pub article_case_id: String,
    /// Short article case summary.
    pub article_case_summary: String,
    /// Stable executor product identifier.
    pub executor_product_id: String,
    /// Stable executor model identifier.
    pub model_id: String,
    /// Stable model-descriptor digest.
    pub model_descriptor_digest: String,
    /// Stable weight-bundle digest.
    pub model_weight_bundle_digest: String,
    /// Primary external model artifact digest when one exists.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_primary_artifact_digest: Option<String>,
    /// Stable lineage-contract reference for the weight bundle.
    pub model_lineage_contract_ref: String,
    /// Stable digest over the lineage contract bound into the receipt.
    pub model_lineage_contract_digest: String,
    /// Requested decode mode for the execution.
    pub requested_decode_mode: TassadarExecutorDecodeMode,
    /// Effective decode mode observed at runtime.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effective_decode_mode: Option<TassadarExecutorDecodeMode>,
    /// Direct/fallback/refused selection state observed at runtime.
    pub selection_state: TassadarExecutorSelectionState,
    /// Whether fallback was observed anywhere in the session path.
    pub fallback_observed: bool,
    /// Explicit observed external-call count.
    pub external_call_count: u32,
    /// Whether any external-tool marker was observed in runtime identity.
    pub external_tool_surface_observed: bool,
    /// Whether any CPU-result-substitution marker was observed in runtime identity.
    pub cpu_result_substitution_observed: bool,
    /// Runtime backend feature markers preserved for auditability.
    pub compiled_backend_features: Vec<String>,
    /// Stable program artifact digest.
    pub program_artifact_digest: String,
    /// Stable trace artifact digest.
    pub trace_artifact_digest: String,
    /// Stable trace digest.
    pub trace_digest: String,
    /// Stable trace-proof digest.
    pub trace_proof_digest: String,
    /// Stable runtime-manifest identity digest.
    pub runtime_manifest_identity_digest: String,
    /// Stable runtime-manifest digest.
    pub runtime_manifest_digest: String,
    /// Stable proof-bundle request digest.
    pub proof_bundle_request_digest: String,
    /// Stable proof-bundle model id when one exists.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proof_bundle_model_id: Option<String>,
    /// Route binding that proves the published route remained direct.
    pub route_binding: TassadarDirectModelWeightRouteBinding,
}

/// Runtime-owned proof receipt for the direct model-weight article claim.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarDirectModelWeightExecutionProofReceipt {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable receipt identifier.
    pub receipt_id: String,
    /// Stable benchmark reference.
    pub benchmark_ref: String,
    /// Stable benchmark environment reference.
    pub benchmark_environment_ref: String,
    /// Stable benchmark report reference.
    pub benchmark_report_ref: String,
    /// Stable workload family identifier.
    pub workload_family_id: String,
    /// Stable article case identifier.
    pub article_case_id: String,
    /// Short article case summary.
    pub article_case_summary: String,
    /// Stable executor product identifier.
    pub executor_product_id: String,
    /// Stable executor model identifier.
    pub model_id: String,
    /// Stable model-descriptor digest.
    pub model_descriptor_digest: String,
    /// Stable weight-bundle digest.
    pub model_weight_bundle_digest: String,
    /// Primary external model artifact digest when one exists.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_primary_artifact_digest: Option<String>,
    /// Stable lineage-contract reference for the weight bundle.
    pub model_lineage_contract_ref: String,
    /// Stable digest over the lineage contract bound into the receipt.
    pub model_lineage_contract_digest: String,
    /// Requested decode mode for the execution.
    pub requested_decode_mode: TassadarExecutorDecodeMode,
    /// Effective decode mode observed at runtime.
    pub effective_decode_mode: TassadarExecutorDecodeMode,
    /// Direct/fallback/refused selection state observed at runtime.
    pub selection_state: TassadarExecutorSelectionState,
    /// Whether fallback was observed anywhere in the session path.
    pub fallback_observed: bool,
    /// Explicit observed external-call count.
    pub external_call_count: u32,
    /// Whether any external-tool marker was observed in runtime identity.
    pub external_tool_surface_observed: bool,
    /// Whether any CPU-result-substitution marker was observed in runtime identity.
    pub cpu_result_substitution_observed: bool,
    /// Runtime backend feature markers preserved for auditability.
    pub compiled_backend_features: Vec<String>,
    /// Stable program artifact digest.
    pub program_artifact_digest: String,
    /// Stable trace artifact digest.
    pub trace_artifact_digest: String,
    /// Stable trace digest.
    pub trace_digest: String,
    /// Stable trace-proof digest.
    pub trace_proof_digest: String,
    /// Stable runtime-manifest identity digest.
    pub runtime_manifest_identity_digest: String,
    /// Stable runtime-manifest digest.
    pub runtime_manifest_digest: String,
    /// Stable proof-bundle request digest.
    pub proof_bundle_request_digest: String,
    /// Stable proof-bundle model id when one exists.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proof_bundle_model_id: Option<String>,
    /// Route binding that proves the published route remained direct.
    pub route_binding: TassadarDirectModelWeightRouteBinding,
    /// Explicit proof boundary.
    pub claim_boundary: String,
    /// Plain-language receipt detail.
    pub detail: String,
    /// Stable receipt digest.
    pub receipt_digest: String,
}

impl TassadarDirectModelWeightExecutionProofReceipt {
    /// Validates one proof input and emits a direct model-weight execution receipt.
    pub fn new(
        input: TassadarDirectModelWeightExecutionProofInput,
    ) -> Result<Self, TassadarDirectModelWeightExecutionProofError> {
        let effective_decode_mode = input.effective_decode_mode.ok_or(
            TassadarDirectModelWeightExecutionProofError::MissingEffectiveDecodeMode {
                receipt_id: input.receipt_id.clone(),
            },
        )?;
        if input.model_lineage_contract_ref.trim().is_empty() {
            return Err(
                TassadarDirectModelWeightExecutionProofError::MissingModelLineageContractRef {
                    receipt_id: input.receipt_id,
                },
            );
        }
        if input.model_lineage_contract_digest.trim().is_empty() {
            return Err(
                TassadarDirectModelWeightExecutionProofError::MissingModelLineageContractDigest {
                    receipt_id: input.receipt_id,
                },
            );
        }
        if input.selection_state != TassadarExecutorSelectionState::Direct {
            return Err(
                TassadarDirectModelWeightExecutionProofError::SelectionNotDirect {
                    receipt_id: input.receipt_id,
                    selection_state: input.selection_state,
                },
            );
        }
        if input.fallback_observed {
            return Err(
                TassadarDirectModelWeightExecutionProofError::FallbackObserved {
                    receipt_id: input.receipt_id,
                },
            );
        }
        if input.external_call_count > 0 {
            return Err(
                TassadarDirectModelWeightExecutionProofError::ExternalCallsObserved {
                    receipt_id: input.receipt_id,
                    external_call_count: input.external_call_count,
                },
            );
        }
        if input.external_tool_surface_observed {
            return Err(
                TassadarDirectModelWeightExecutionProofError::ExternalToolSurfaceObserved {
                    receipt_id: input.receipt_id,
                },
            );
        }
        if input.cpu_result_substitution_observed {
            return Err(
                TassadarDirectModelWeightExecutionProofError::CpuResultSubstitutionObserved {
                    receipt_id: input.receipt_id,
                },
            );
        }
        if input.route_binding.route_posture
            != TassadarDirectModelWeightRoutePosture::DirectGuaranteed
        {
            return Err(
                TassadarDirectModelWeightExecutionProofError::RouteNotDirectGuaranteed {
                    receipt_id: input.receipt_id,
                    route_descriptor_digest: input.route_binding.route_descriptor_digest,
                    route_posture: input.route_binding.route_posture,
                },
            );
        }
        if input.route_binding.requested_decode_mode != input.requested_decode_mode {
            return Err(
                TassadarDirectModelWeightExecutionProofError::RouteDecodeModeMismatch {
                    receipt_id: input.receipt_id,
                    route_requested_decode_mode: input.route_binding.requested_decode_mode,
                    execution_requested_decode_mode: input.requested_decode_mode,
                },
            );
        }
        if effective_decode_mode != input.requested_decode_mode {
            return Err(
                TassadarDirectModelWeightExecutionProofError::DecodeModeDrift {
                    receipt_id: input.receipt_id,
                    requested_decode_mode: input.requested_decode_mode,
                    effective_decode_mode,
                },
            );
        }
        if input.route_binding.route_model_id != input.model_id {
            return Err(
                TassadarDirectModelWeightExecutionProofError::RouteModelMismatch {
                    receipt_id: input.receipt_id,
                    route_model_id: input.route_binding.route_model_id,
                    execution_model_id: input.model_id,
                },
            );
        }
        if let Some(proof_bundle_model_id) = input.proof_bundle_model_id.as_ref() {
            if proof_bundle_model_id != &input.model_id {
                return Err(
                    TassadarDirectModelWeightExecutionProofError::ProofBundleModelMismatch {
                        receipt_id: input.receipt_id,
                        proof_bundle_model_id: proof_bundle_model_id.clone(),
                        execution_model_id: input.model_id,
                    },
                );
            }
        }

        let mut receipt = Self {
            schema_version: 2,
            receipt_id: input.receipt_id,
            benchmark_ref: input.benchmark_ref,
            benchmark_environment_ref: input.benchmark_environment_ref,
            benchmark_report_ref: input.benchmark_report_ref,
            workload_family_id: input.workload_family_id,
            article_case_id: input.article_case_id,
            article_case_summary: input.article_case_summary,
            executor_product_id: input.executor_product_id,
            model_id: input.model_id,
            model_descriptor_digest: input.model_descriptor_digest,
            model_weight_bundle_digest: input.model_weight_bundle_digest,
            model_primary_artifact_digest: input.model_primary_artifact_digest,
            model_lineage_contract_ref: input.model_lineage_contract_ref,
            model_lineage_contract_digest: input.model_lineage_contract_digest,
            requested_decode_mode: input.requested_decode_mode,
            effective_decode_mode,
            selection_state: input.selection_state,
            fallback_observed: input.fallback_observed,
            external_call_count: input.external_call_count,
            external_tool_surface_observed: input.external_tool_surface_observed,
            cpu_result_substitution_observed: input.cpu_result_substitution_observed,
            compiled_backend_features: input.compiled_backend_features,
            program_artifact_digest: input.program_artifact_digest,
            trace_artifact_digest: input.trace_artifact_digest,
            trace_digest: input.trace_digest,
            trace_proof_digest: input.trace_proof_digest,
            runtime_manifest_identity_digest: input.runtime_manifest_identity_digest,
            runtime_manifest_digest: input.runtime_manifest_digest,
            proof_bundle_request_digest: input.proof_bundle_request_digest,
            proof_bundle_model_id: input.proof_bundle_model_id,
            route_binding: input.route_binding,
            claim_boundary: String::from(
                "this receipt closes direct model-weight execution only for the committed article workload and model pairing named here. It proves explicit no-fallback, zero-external-call, zero-tool-surface posture on the direct executor lane with bound route, trace, proof, runtime-manifest lineage, and explicit weight-lineage contract binding, but it does not imply future routes or undeclared workloads inherit the same proof without this receipt family",
            ),
            detail: String::new(),
            receipt_digest: String::new(),
        };
        receipt.detail = format!(
            "direct model-weight execution proof `{}` binds case `{}` to model `{}` on requested/effective decode `{}` with route `{}`, lineage `{}`, and zero external calls",
            receipt.receipt_id,
            receipt.article_case_id,
            receipt.model_id,
            receipt.requested_decode_mode.as_str(),
            receipt.route_binding.route_descriptor_digest,
            receipt.model_lineage_contract_ref,
        );
        receipt.receipt_digest = stable_digest(
            b"psionic_tassadar_direct_model_weight_execution_proof_receipt|",
            &receipt,
        );
        Ok(receipt)
    }
}

/// Validation failure for the direct model-weight execution receipt.
#[derive(Clone, Debug, Error, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TassadarDirectModelWeightExecutionProofError {
    #[error("direct model-weight proof `{receipt_id}` was missing an effective decode mode")]
    MissingEffectiveDecodeMode { receipt_id: String },
    #[error("direct model-weight proof `{receipt_id}` was missing a model-lineage contract ref")]
    MissingModelLineageContractRef { receipt_id: String },
    #[error(
        "direct model-weight proof `{receipt_id}` was missing a model-lineage contract digest"
    )]
    MissingModelLineageContractDigest { receipt_id: String },
    #[error(
        "direct model-weight proof `{receipt_id}` observed selection state `{selection_state:?}` instead of `direct`"
    )]
    SelectionNotDirect {
        receipt_id: String,
        selection_state: TassadarExecutorSelectionState,
    },
    #[error(
        "direct model-weight proof `{receipt_id}` observed fallback on the article session path"
    )]
    FallbackObserved { receipt_id: String },
    #[error(
        "direct model-weight proof `{receipt_id}` observed {external_call_count} external calls"
    )]
    ExternalCallsObserved {
        receipt_id: String,
        external_call_count: u32,
    },
    #[error("direct model-weight proof `{receipt_id}` observed external-tool surface markers")]
    ExternalToolSurfaceObserved { receipt_id: String },
    #[error("direct model-weight proof `{receipt_id}` observed CPU result substitution markers")]
    CpuResultSubstitutionObserved { receipt_id: String },
    #[error(
        "direct model-weight proof `{receipt_id}` bound route `{route_descriptor_digest}` as `{route_posture:?}` instead of `direct_guaranteed`"
    )]
    RouteNotDirectGuaranteed {
        receipt_id: String,
        route_descriptor_digest: String,
        route_posture: TassadarDirectModelWeightRoutePosture,
    },
    #[error(
        "direct model-weight proof `{receipt_id}` saw route decode `{route_requested_decode_mode:?}` but execution requested `{execution_requested_decode_mode:?}`"
    )]
    RouteDecodeModeMismatch {
        receipt_id: String,
        route_requested_decode_mode: TassadarExecutorDecodeMode,
        execution_requested_decode_mode: TassadarExecutorDecodeMode,
    },
    #[error(
        "direct model-weight proof `{receipt_id}` drifted from requested decode `{requested_decode_mode:?}` to effective `{effective_decode_mode:?}`"
    )]
    DecodeModeDrift {
        receipt_id: String,
        requested_decode_mode: TassadarExecutorDecodeMode,
        effective_decode_mode: TassadarExecutorDecodeMode,
    },
    #[error(
        "direct model-weight proof `{receipt_id}` bound route model `{route_model_id}` but execution used `{execution_model_id}`"
    )]
    RouteModelMismatch {
        receipt_id: String,
        route_model_id: String,
        execution_model_id: String,
    },
    #[error(
        "direct model-weight proof `{receipt_id}` bound proof-bundle model `{proof_bundle_model_id}` but execution used `{execution_model_id}`"
    )]
    ProofBundleModelMismatch {
        receipt_id: String,
        proof_bundle_model_id: String,
        execution_model_id: String,
    },
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
        TassadarDirectModelWeightExecutionProofError, TassadarDirectModelWeightExecutionProofInput,
        TassadarDirectModelWeightExecutionProofReceipt, TassadarDirectModelWeightRouteBinding,
        TassadarDirectModelWeightRoutePosture,
    };
    use crate::{TassadarExecutorDecodeMode, TassadarExecutorSelectionState};

    fn input() -> TassadarDirectModelWeightExecutionProofInput {
        TassadarDirectModelWeightExecutionProofInput {
            receipt_id: String::from("direct-proof.long_loop_kernel"),
            benchmark_ref: String::from(
                "fixtures/tassadar/reports/tassadar_article_class_benchmark_report.json",
            ),
            benchmark_environment_ref: String::from(
                "fixtures/tassadar/environments/tassadar_article_class_environment.json",
            ),
            benchmark_report_ref: String::from(
                "fixtures/tassadar/reports/tassadar_article_class_benchmark_report.json",
            ),
            workload_family_id: String::from("long_loop_kernel"),
            article_case_id: String::from("long_loop_kernel"),
            article_case_summary: String::from("Long-loop article kernel"),
            executor_product_id: String::from("psionic.executor_trace"),
            model_id: String::from("tassadar.article_i32_compute.v1"),
            model_descriptor_digest: String::from("model-descriptor-digest"),
            model_weight_bundle_digest: String::from("weight-bundle-digest"),
            model_primary_artifact_digest: Some(String::from("weight-artifact-digest")),
            model_lineage_contract_ref: String::from(
                "fixtures/tassadar/models/tassadar_article_transformer_trace_bound_trained_v0_lineage_contract.json",
            ),
            model_lineage_contract_digest: String::from("lineage-contract-digest"),
            requested_decode_mode: TassadarExecutorDecodeMode::ReferenceLinear,
            effective_decode_mode: Some(TassadarExecutorDecodeMode::ReferenceLinear),
            selection_state: TassadarExecutorSelectionState::Direct,
            fallback_observed: false,
            external_call_count: 0,
            external_tool_surface_observed: false,
            cpu_result_substitution_observed: false,
            compiled_backend_features: vec![
                String::from("tassadar_executor"),
                String::from("tassadar.decode.reference_linear.v1"),
            ],
            program_artifact_digest: String::from("program-artifact-digest"),
            trace_artifact_digest: String::from("trace-artifact-digest"),
            trace_digest: String::from("trace-digest"),
            trace_proof_digest: String::from("trace-proof-digest"),
            runtime_manifest_identity_digest: String::from("runtime-manifest-identity-digest"),
            runtime_manifest_digest: String::from("runtime-manifest-digest"),
            proof_bundle_request_digest: String::from("proof-bundle-request-digest"),
            proof_bundle_model_id: Some(String::from("tassadar.article_i32_compute.v1")),
            route_binding: TassadarDirectModelWeightRouteBinding {
                route_product_id: String::from("psionic.planner_executor_route"),
                route_id: String::from("route.article"),
                route_descriptor_digest: String::from("route-descriptor-digest"),
                route_model_id: String::from("tassadar.article_i32_compute.v1"),
                benchmark_report_ref: String::from(
                    "fixtures/tassadar/reports/tassadar_article_class_benchmark_report.json",
                ),
                requested_decode_mode: TassadarExecutorDecodeMode::ReferenceLinear,
                route_posture: TassadarDirectModelWeightRoutePosture::DirectGuaranteed,
                note: String::from("direct dense floor"),
            },
        }
    }

    #[test]
    fn direct_model_weight_execution_proof_receipt_is_machine_legible() {
        let receipt = TassadarDirectModelWeightExecutionProofReceipt::new(input())
            .expect("receipt should validate");

        assert_eq!(receipt.schema_version, 2);
        assert_eq!(receipt.external_call_count, 0);
        assert!(!receipt.external_tool_surface_observed);
        assert!(!receipt.cpu_result_substitution_observed);
        assert_eq!(
            receipt.route_binding.route_posture,
            TassadarDirectModelWeightRoutePosture::DirectGuaranteed
        );
        assert!(!receipt.receipt_digest.is_empty());
    }

    #[test]
    fn direct_model_weight_execution_proof_receipt_rejects_fallback_or_external_surfaces() {
        let mut missing_lineage_ref = input();
        missing_lineage_ref.model_lineage_contract_ref.clear();
        assert_eq!(
            TassadarDirectModelWeightExecutionProofReceipt::new(missing_lineage_ref).unwrap_err(),
            TassadarDirectModelWeightExecutionProofError::MissingModelLineageContractRef {
                receipt_id: String::from("direct-proof.long_loop_kernel"),
            }
        );

        let mut missing_lineage_digest = input();
        missing_lineage_digest.model_lineage_contract_digest.clear();
        assert_eq!(
            TassadarDirectModelWeightExecutionProofReceipt::new(missing_lineage_digest)
                .unwrap_err(),
            TassadarDirectModelWeightExecutionProofError::MissingModelLineageContractDigest {
                receipt_id: String::from("direct-proof.long_loop_kernel"),
            }
        );

        let mut fallback = input();
        fallback.fallback_observed = true;
        assert_eq!(
            TassadarDirectModelWeightExecutionProofReceipt::new(fallback).unwrap_err(),
            TassadarDirectModelWeightExecutionProofError::FallbackObserved {
                receipt_id: String::from("direct-proof.long_loop_kernel"),
            }
        );

        let mut external = input();
        external.external_call_count = 1;
        assert_eq!(
            TassadarDirectModelWeightExecutionProofReceipt::new(external).unwrap_err(),
            TassadarDirectModelWeightExecutionProofError::ExternalCallsObserved {
                receipt_id: String::from("direct-proof.long_loop_kernel"),
                external_call_count: 1,
            }
        );

        let mut route = input();
        route.route_binding.route_posture = TassadarDirectModelWeightRoutePosture::FallbackCapable;
        assert_eq!(
            TassadarDirectModelWeightExecutionProofReceipt::new(route).unwrap_err(),
            TassadarDirectModelWeightExecutionProofError::RouteNotDirectGuaranteed {
                receipt_id: String::from("direct-proof.long_loop_kernel"),
                route_descriptor_digest: String::from("route-descriptor-digest"),
                route_posture: TassadarDirectModelWeightRoutePosture::FallbackCapable,
            }
        );
    }
}
