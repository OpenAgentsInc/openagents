use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tempfile::tempdir;
use thiserror::Error;

use crate::{
    ArtifactWordDecoder, CpuModelTextGenerationService, GenerationOptions, GenerationRequest,
    GenerationResponse, ListModelsObservation, LocalModelCatalog, ModelSummary,
    PsionCapabilityMatrix, PsionCapabilityRegionId, PsionNoImplicitExecutionStatus,
    PsionServedAssumptionKind, PsionServedAssumptionNotice, PsionServedBehaviorVisibility,
    PsionServedContextEnvelopeSurface, PsionServedEvidenceError, PsionServedEvidenceLabel,
    PsionServedLatencyEnvelopeSurface, PsionServedOutputClaimPostureError,
    PsionServedRouteReceipt, PsionServedVisibleClaims, PsionicLocalRuntime,
    ReferenceTextGenerationError, ShowObservation, SmokeEmbeddingsError, SmokeEmbeddingsService,
    record_psion_served_evidence_bundle,
    record_psion_served_output_claim_posture,
};
use psionic_models::ModelLoadError;
use psionic_train::{PsionRouteClass, PsionRouteClassEvaluationReceipt, PsionRouteKind};

pub const PSION_GENERIC_LOAD_AND_GENERATE_SCHEMA_VERSION: &str =
    "psion.generic_load_and_generate.v1";
pub const PSION_GENERIC_LOAD_AND_GENERATE_FIXTURE_PATH: &str =
    "fixtures/psion/serve/psion_generic_load_and_generate_v1.json";
pub const PSION_GENERIC_LOAD_AND_GENERATE_DOC_PATH: &str =
    "docs/PSION_GENERIC_LOAD_AND_GENERATE.md";

const PACKET_ID: &str = "psion_generic_load_and_generate_v1";
const RUNTIME_PATH: &str = "psionic_local_runtime.cpu_model_text_generation_service";
const PROMPT_TEXT: &str = "open agents";
const COLD_REQUEST_ID: &str = "psion-generic-load-generate-cold";
const WARM_REQUEST_ID: &str = "psion-generic-load-generate-warm";
const RUNTIME_BUNDLE_FILE_NAME: &str = "psion_generic_load_and_generate_model.safetensors";

#[derive(Debug, Error)]
pub enum PsionGenericLoadAndGenerateError {
    #[error("failed to create temp runtime bundle directory: {0}")]
    TempDir(#[from] std::io::Error),
    #[error("failed to load or run the artifact-backed generation service: {0}")]
    Generation(#[from] ReferenceTextGenerationError),
    #[error("failed to create or reload the local decoder checkpoint artifact: {0}")]
    ModelLoad(#[from] ModelLoadError),
    #[error("failed to initialize smoke embeddings service for the stable runtime path: {0}")]
    SmokeEmbeddings(#[from] SmokeEmbeddingsError),
    #[error("failed to parse a committed Psion fixture: {0}")]
    Json(#[from] serde_json::Error),
    #[error("missing generation provenance on the generic load-and-generate response")]
    MissingGenerationProvenance,
    #[error(transparent)]
    ServedEvidence(#[from] PsionServedEvidenceError),
    #[error(transparent)]
    ServedOutputClaimPosture(#[from] PsionServedOutputClaimPostureError),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionGenericLoadAndGeneratePacket {
    pub schema_version: String,
    pub packet_id: String,
    pub runtime_path: String,
    pub list_models: ListModelsObservation,
    pub show_model: ShowObservation,
    pub prompt_text: String,
    pub encoded_prompt_tokens: Vec<u32>,
    pub cold_response: GenerationResponse,
    pub warm_response: GenerationResponse,
    pub detail: String,
    pub packet_digest: String,
}

impl PsionGenericLoadAndGeneratePacket {
    #[must_use]
    pub fn stable_digest(&self) -> String {
        stable_digest(b"psion_generic_load_and_generate_packet|", self)
    }
}

#[derive(Clone, Debug)]
struct SingleModelCatalog {
    list_models: ListModelsObservation,
    show_model: ShowObservation,
}

impl SingleModelCatalog {
    fn from_descriptor(descriptor: &crate::DecoderModelDescriptor) -> Self {
        Self {
            list_models: ListModelsObservation::new(vec![ModelSummary::from_decoder_descriptor(
                descriptor.model.model_id.clone(),
                descriptor,
            )]),
            show_model: ShowObservation::from_decoder_descriptor(
                descriptor.model.model_id.clone(),
                descriptor,
            ),
        }
    }
}

impl LocalModelCatalog for SingleModelCatalog {
    fn list_models(&self) -> ListModelsObservation {
        self.list_models.clone()
    }

    fn show_model(&self, _model: &str) -> ShowObservation {
        self.show_model.clone()
    }
}

pub fn builtin_psion_generic_load_and_generate_packet()
-> Result<PsionGenericLoadAndGeneratePacket, PsionGenericLoadAndGenerateError> {
    let temp = tempdir()?;
    let artifact_path = temp.path().join(RUNTIME_BUNDLE_FILE_NAME);
    ArtifactWordDecoder::write_default_safetensors_artifact(&artifact_path)?;

    let model = ArtifactWordDecoder::from_safetensors_artifact(&artifact_path)?;
    let model_descriptor = model.descriptor().clone();
    let encoded_prompt_tokens = model
        .tokenizer()
        .encode_with_special_tokens(PROMPT_TEXT, true, false)
        .as_slice()
        .iter()
        .map(|token| token.as_u32())
        .collect::<Vec<_>>();

    let generation = CpuModelTextGenerationService::from_safetensors_artifact(&artifact_path)?;
    let catalog = SingleModelCatalog::from_descriptor(&model_descriptor);
    let embeddings = SmokeEmbeddingsService::new()?;
    let mut runtime = PsionicLocalRuntime::new(catalog.clone(), generation, embeddings);

    let mut cold_response = runtime.generate(&GenerationRequest::new_text(
        COLD_REQUEST_ID,
        model_descriptor.clone(),
        None,
        PROMPT_TEXT,
        GenerationOptions::greedy(4),
    ))?;
    attach_generic_psion_provenance(&mut cold_response)?;

    let mut warm_response = runtime.generate(&GenerationRequest::new_text(
        WARM_REQUEST_ID,
        model_descriptor,
        None,
        PROMPT_TEXT,
        GenerationOptions::greedy(4),
    ))?;
    attach_generic_psion_provenance(&mut warm_response)?;

    let mut packet = PsionGenericLoadAndGeneratePacket {
        schema_version: String::from(PSION_GENERIC_LOAD_AND_GENERATE_SCHEMA_VERSION),
        packet_id: String::from(PACKET_ID),
        runtime_path: String::from(RUNTIME_PATH),
        list_models: catalog.list_models(),
        show_model: catalog.show_model("ignored"),
        prompt_text: String::from(PROMPT_TEXT),
        encoded_prompt_tokens,
        cold_response,
        warm_response,
        detail: String::from(
            "The first generic Psion load-and-generate packet closes the operator-facing gap through the existing artifact-backed decoder runtime: one local safetensors checkpoint is loaded, one prompt is encoded, two deterministic generations run through the stable runtime path, and the learned-lane evidence plus claim posture are attached directly to response provenance without widening into executor claims.",
        ),
        packet_digest: String::new(),
    };
    packet.packet_digest = packet.stable_digest();
    Ok(packet)
}

fn attach_generic_psion_provenance(
    response: &mut GenerationResponse,
) -> Result<(), PsionGenericLoadAndGenerateError> {
    let capability_matrix: PsionCapabilityMatrix = serde_json::from_str(include_str!(
        "../../../fixtures/psion/capability/psion_capability_matrix_v1.json"
    ))?;
    let route_receipt: PsionRouteClassEvaluationReceipt = serde_json::from_str(include_str!(
        "../../../fixtures/psion/route/psion_route_class_evaluation_receipt_v1.json"
    ))?;
    let evidence_bundle = record_psion_served_evidence_bundle(
        "psion-generic-load-and-generate-direct-learned-v1",
        capability_matrix.matrix_id.clone(),
        capability_matrix.matrix_version.clone(),
        Some(PsionServedRouteReceipt {
            route_kind: PsionRouteKind::DirectModelAnswer,
            route_class: PsionRouteClass::AnswerWithUncertainty,
            capability_region_id: PsionCapabilityRegionId::BoundedTechnicalReasoningShortContext,
            route_boundary_ref: String::from(
                "capability_matrix.supported.bounded_technical_reasoning_short_context",
            ),
            route_calibration_receipt_id: capability_matrix
                .acceptance_basis
                .route_calibration_receipt_ref
                .clone(),
            route_class_evaluation_receipt_id: route_receipt.receipt_id.clone(),
            route_class_evaluation_receipt_digest: route_receipt.receipt_digest.clone(),
            detail: String::from(
                "The generic Psion load-and-generate path stays in the bounded learned lane and makes that route explicit instead of implying exact execution or hidden tools.",
            ),
        }),
        None,
        PsionNoImplicitExecutionStatus {
            execution_only_via_explicit_surface: true,
            executor_surface_invoked: false,
            explicit_executor_artifact: None,
            detail: String::from(
                "No executor surface was invoked on this runtime path, so the served output must remain a learned judgment only.",
            ),
        },
        vec![PsionServedEvidenceLabel::LearnedJudgment {
            uncertainty_disclosed: true,
            detail: String::from(
                "This example proves train-to-serve closure on the generic learned lane, but it still discloses uncertainty and does not imply exactness, source quotation, or executor-backed truth.",
            ),
        }],
        "Direct learned-lane evidence for the first generic Psion load-and-generate runtime packet.",
    )?;
    let claim_posture = record_psion_served_output_claim_posture(
        "psion-generic-load-and-generate-claim-posture-v1",
        &capability_matrix,
        &evidence_bundle,
        PsionServedVisibleClaims {
            learned_judgment_visible: true,
            source_grounding_visible: false,
            executor_backing_visible: false,
            benchmark_backing_visible: false,
            verification_visible: false,
        },
        vec![
            PsionServedAssumptionNotice {
                assumption_id: String::from("bounded_generic_decoder_runtime"),
                kind: PsionServedAssumptionKind::InterpretationBoundary,
                required_for_interpretation: true,
                detail: String::from(
                    "This runtime packet proves the generic learned-lane load-and-generate seam on the current artifact-backed decoder substrate; it does not by itself prove full pilot-anchor checkpoint parity or executor replacement truth.",
                ),
            },
            PsionServedAssumptionNotice {
                assumption_id: String::from("no_hidden_execution_surface"),
                kind: PsionServedAssumptionKind::EnvironmentBoundary,
                required_for_interpretation: true,
                detail: String::from(
                    "The served output assumes no hidden tool, executor, or fresh external state beyond the loaded checkpoint artifact and the attached learned-lane provenance.",
                ),
            },
        ],
        PsionServedBehaviorVisibility::Route {
            route_kind: PsionRouteKind::DirectModelAnswer,
            route_class: PsionRouteClass::AnswerWithUncertainty,
            detail: String::from(
                "The response remains visibly direct-model output inside the bounded learned lane.",
            ),
        },
        PsionServedContextEnvelopeSurface {
            supported_prompt_tokens: capability_matrix.context_envelope.supported_prompt_tokens,
            supported_completion_tokens: capability_matrix.context_envelope.supported_completion_tokens,
            route_required_above_prompt_tokens: capability_matrix
                .context_envelope
                .route_required_above_prompt_tokens,
            hard_refusal_above_prompt_tokens: capability_matrix
                .context_envelope
                .hard_refusal_above_prompt_tokens,
            prompt_tokens_observed: response.usage.input_tokens as u32,
            detail: capability_matrix.context_envelope.detail.clone(),
        },
        PsionServedLatencyEnvelopeSurface {
            p50_first_token_latency_ms: capability_matrix
                .latency_envelope
                .p50_first_token_latency_ms,
            p95_first_token_latency_ms: capability_matrix
                .latency_envelope
                .p95_first_token_latency_ms,
            p95_end_to_end_latency_ms: capability_matrix
                .latency_envelope
                .p95_end_to_end_latency_ms,
            detail: capability_matrix.latency_envelope.detail.clone(),
        },
        "Generic learned-lane claim posture for the first Psion load-and-generate packet.",
    )?;

    let provenance = response
        .provenance
        .take()
        .ok_or(PsionGenericLoadAndGenerateError::MissingGenerationProvenance)?
        .with_psion_served_evidence(evidence_bundle)
        .with_psion_served_output_claim_posture(claim_posture);
    response.provenance = Some(provenance);
    Ok(())
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut canonical = serde_json::to_value(value).expect("generic packet should serialize");
    if let Some(object) = canonical.as_object_mut() {
        object.insert(String::from("packet_digest"), serde_json::Value::String(String::new()));
    }
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(
        serde_json::to_vec(&canonical).expect("generic packet canonical form should serialize"),
    );
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        PSION_GENERIC_LOAD_AND_GENERATE_FIXTURE_PATH, builtin_psion_generic_load_and_generate_packet,
    };
    use crate::{GenerationLoadState, PsionGenericLoadAndGeneratePacket};

    #[test]
    fn generic_load_and_generate_fixture_matches_builtin_truth()
    -> Result<(), Box<dyn std::error::Error>> {
        let expected = builtin_psion_generic_load_and_generate_packet()?;
        let committed: PsionGenericLoadAndGeneratePacket = serde_json::from_str(include_str!(
            "../../../fixtures/psion/serve/psion_generic_load_and_generate_v1.json"
        ))?;
        assert_eq!(
            committed, expected,
            "fixture {} drifted from builtin output",
            PSION_GENERIC_LOAD_AND_GENERATE_FIXTURE_PATH
        );
        assert_eq!(
            committed
                .cold_response
                .provenance
                .as_ref()
                .map(|value| value.load_state),
            Some(GenerationLoadState::Cold)
        );
        assert_eq!(
            committed
                .warm_response
                .provenance
                .as_ref()
                .map(|value| value.load_state),
            Some(GenerationLoadState::Warm)
        );
        assert!(
            committed
                .cold_response
                .provenance
                .as_ref()
                .and_then(|value| value.psion_served_evidence.as_ref())
                .is_some()
        );
        assert!(
            committed
                .cold_response
                .provenance
                .as_ref()
                .and_then(|value| value.psion_served_output_claim_posture.as_ref())
                .is_some()
        );
        Ok(())
    }
}
