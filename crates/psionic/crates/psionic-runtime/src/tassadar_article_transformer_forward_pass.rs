use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    BackendProbeState, BackendToolchainIdentity, ExecutionProofArtifactResidency,
    ExecutionProofAugmentationPosture, ExecutionProofBundle, ExecutionProofBundleKind,
    ExecutionProofBundleStatus, ExecutionProofRuntimeIdentity, RuntimeManifest,
    RuntimeManifestArtifactBinding, RuntimeManifestArtifactKind,
    RuntimeManifestStaticConfigBinding, TrainingCheckpointReference, ValidationMatrixReference,
};

/// Stable claims-profile identifier for the article-Transformer forward-pass lane.
pub const TASSADAR_ARTICLE_TRANSFORMER_FORWARD_PASS_CLAIMS_PROFILE_ID: &str =
    "tassadar.article_transformer.forward_pass.v1";

/// Digest-bound model identity for one canonical article-Transformer run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleTransformerModelArtifactBinding {
    /// Stable model identifier.
    pub model_id: String,
    /// Stable model family label.
    pub model_family: String,
    /// Stable descriptor digest.
    pub descriptor_digest: String,
    /// Stable digest over the bounded trainable parameter surface.
    pub trainable_parameter_digest: String,
    /// Stable model-artifact reference.
    pub artifact_id: String,
    /// Stable parameter-bundle digest surfaced to runtime receipts.
    pub weight_bundle_digest: String,
    /// Stable primary artifact SHA-256 digest.
    pub primary_artifact_sha256: String,
    /// Stable digest over the full model-artifact binding.
    pub artifact_digest: String,
}

impl TassadarArticleTransformerModelArtifactBinding {
    /// Creates one digest-bound model-artifact identity.
    #[must_use]
    pub fn new(
        model_id: impl Into<String>,
        model_family: impl Into<String>,
        descriptor_digest: impl Into<String>,
        trainable_parameter_digest: impl Into<String>,
        artifact_id: impl Into<String>,
        weight_bundle_digest: impl Into<String>,
        primary_artifact_sha256: impl Into<String>,
    ) -> Self {
        let mut binding = Self {
            model_id: model_id.into(),
            model_family: model_family.into(),
            descriptor_digest: descriptor_digest.into(),
            trainable_parameter_digest: trainable_parameter_digest.into(),
            artifact_id: artifact_id.into(),
            weight_bundle_digest: weight_bundle_digest.into(),
            primary_artifact_sha256: primary_artifact_sha256.into(),
            artifact_digest: String::new(),
        };
        binding.artifact_digest = stable_digest(
            b"psionic_tassadar_article_transformer_model_artifact_binding|",
            &(
                binding.model_id.as_str(),
                binding.model_family.as_str(),
                binding.descriptor_digest.as_str(),
                binding.trainable_parameter_digest.as_str(),
                binding.artifact_id.as_str(),
                binding.weight_bundle_digest.as_str(),
                binding.primary_artifact_sha256.as_str(),
            ),
        );
        binding
    }
}

/// Run-config capture for one article-Transformer forward pass.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleTransformerForwardPassRunConfig {
    /// Stable run identifier.
    pub run_id: String,
    /// Stable request identifier.
    pub request_id: String,
    /// Stable product identifier.
    pub product_id: String,
    /// Source index-tensor shape.
    pub source_shape: Vec<usize>,
    /// Source token ids.
    pub source_token_ids: Vec<usize>,
    /// Target index-tensor shape.
    pub target_shape: Vec<usize>,
    /// Target token ids.
    pub target_token_ids: Vec<usize>,
    /// Stable execution-mode label.
    pub execution_mode: String,
    /// Stable environment references resolved for this run.
    pub environment_refs: Vec<String>,
    /// Stable digest over the run-config payload.
    pub run_config_digest: String,
}

impl TassadarArticleTransformerForwardPassRunConfig {
    /// Creates one replay-stable run-config capture.
    #[must_use]
    pub fn new(
        run_id: impl Into<String>,
        request_id: impl Into<String>,
        product_id: impl Into<String>,
        source_shape: Vec<usize>,
        source_token_ids: Vec<usize>,
        target_shape: Vec<usize>,
        target_token_ids: Vec<usize>,
        execution_mode: impl Into<String>,
        mut environment_refs: Vec<String>,
    ) -> Self {
        environment_refs.sort();
        environment_refs.dedup();
        let mut config = Self {
            run_id: run_id.into(),
            request_id: request_id.into(),
            product_id: product_id.into(),
            source_shape,
            source_token_ids,
            target_shape,
            target_token_ids,
            execution_mode: execution_mode.into(),
            environment_refs,
            run_config_digest: String::new(),
        };
        config.run_config_digest = stable_digest(
            b"psionic_tassadar_article_transformer_forward_pass_run_config|",
            &(
                config.run_id.as_str(),
                config.request_id.as_str(),
                config.product_id.as_str(),
                &config.source_shape,
                &config.source_token_ids,
                &config.target_shape,
                &config.target_token_ids,
                config.execution_mode.as_str(),
                &config.environment_refs,
            ),
        );
        config
    }
}

/// One forward-pass-owned attention-trace channel summary.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleTransformerForwardPassChannelTrace {
    /// Stable channel identifier.
    pub channel_id: String,
    /// Stable channel kind such as `encoder_self_attention`.
    pub channel_kind: String,
    /// Dense probability tensor shape.
    pub tensor_shape: Vec<usize>,
    /// Stable digest over the probability payload.
    pub probability_digest: String,
    /// Number of dense elements carried by the channel.
    pub element_count: usize,
}

impl TassadarArticleTransformerForwardPassChannelTrace {
    /// Creates one digest-bound channel summary.
    #[must_use]
    pub fn new(
        channel_id: impl Into<String>,
        channel_kind: impl Into<String>,
        tensor_shape: Vec<usize>,
        probability_digest: impl Into<String>,
        element_count: usize,
    ) -> Self {
        Self {
            channel_id: channel_id.into(),
            channel_kind: channel_kind.into(),
            tensor_shape,
            probability_digest: probability_digest.into(),
            element_count,
        }
    }
}

/// Runtime-visible forward-pass trace artifact for the canonical article stack.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleTransformerForwardPassTraceArtifact {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable artifact identifier.
    pub artifact_id: String,
    /// Stable digest over the full forward output.
    pub forward_output_digest: String,
    /// Stable digest over the encoder hidden state.
    pub encoder_hidden_state_digest: String,
    /// Stable digest over the decoder hidden state.
    pub decoder_hidden_state_digest: String,
    /// Stable digest over the logits tensor.
    pub logits_digest: String,
    /// Greedy decoded token ids emitted from the logits tensor.
    pub predicted_token_ids: Vec<usize>,
    /// Encoder self-attention channel summaries.
    pub encoder_layer_traces: Vec<TassadarArticleTransformerForwardPassChannelTrace>,
    /// Decoder self-attention channel summaries.
    pub decoder_self_attention_traces: Vec<TassadarArticleTransformerForwardPassChannelTrace>,
    /// Decoder cross-attention channel summaries.
    pub decoder_cross_attention_traces: Vec<TassadarArticleTransformerForwardPassChannelTrace>,
    /// Canonical models-module owner reference.
    pub model_module_ref: String,
    /// Canonical transformer-module owner reference.
    pub transformer_module_ref: String,
    /// Stable digest over the trace-owned subset of the artifact.
    pub trace_digest: String,
    /// Stable digest over the full artifact payload.
    pub artifact_digest: String,
}

impl TassadarArticleTransformerForwardPassTraceArtifact {
    /// Creates one runtime-visible trace artifact from forward-pass digests.
    #[must_use]
    pub fn new(
        artifact_id: impl Into<String>,
        forward_output_digest: impl Into<String>,
        encoder_hidden_state_digest: impl Into<String>,
        decoder_hidden_state_digest: impl Into<String>,
        logits_digest: impl Into<String>,
        predicted_token_ids: Vec<usize>,
        encoder_layer_traces: Vec<TassadarArticleTransformerForwardPassChannelTrace>,
        decoder_self_attention_traces: Vec<TassadarArticleTransformerForwardPassChannelTrace>,
        decoder_cross_attention_traces: Vec<TassadarArticleTransformerForwardPassChannelTrace>,
        model_module_ref: impl Into<String>,
        transformer_module_ref: impl Into<String>,
    ) -> Self {
        let mut artifact = Self {
            schema_version: 1,
            artifact_id: artifact_id.into(),
            forward_output_digest: forward_output_digest.into(),
            encoder_hidden_state_digest: encoder_hidden_state_digest.into(),
            decoder_hidden_state_digest: decoder_hidden_state_digest.into(),
            logits_digest: logits_digest.into(),
            predicted_token_ids,
            encoder_layer_traces,
            decoder_self_attention_traces,
            decoder_cross_attention_traces,
            model_module_ref: model_module_ref.into(),
            transformer_module_ref: transformer_module_ref.into(),
            trace_digest: String::new(),
            artifact_digest: String::new(),
        };
        artifact.trace_digest = stable_digest(
            b"psionic_tassadar_article_transformer_forward_pass_trace_digest|",
            &(
                artifact.forward_output_digest.as_str(),
                artifact.encoder_hidden_state_digest.as_str(),
                artifact.decoder_hidden_state_digest.as_str(),
                artifact.logits_digest.as_str(),
                &artifact.predicted_token_ids,
                &artifact.encoder_layer_traces,
                &artifact.decoder_self_attention_traces,
                &artifact.decoder_cross_attention_traces,
            ),
        );
        artifact.artifact_digest = stable_digest(
            b"psionic_tassadar_article_transformer_forward_pass_trace_artifact|",
            &(
                artifact.schema_version,
                artifact.artifact_id.as_str(),
                artifact.trace_digest.as_str(),
                artifact.model_module_ref.as_str(),
                artifact.transformer_module_ref.as_str(),
            ),
        );
        artifact
    }
}

/// Greedy decode receipt emitted for one forward-pass logits tensor.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleTransformerDecodeReceipt {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable receipt identifier.
    pub receipt_id: String,
    /// Stable decode strategy label.
    pub decode_strategy: String,
    /// Stable digest over the logits tensor used for decode.
    pub logits_digest: String,
    /// Greedy decoded token ids.
    pub predicted_token_ids: Vec<usize>,
    /// Stable digest over the decoded token ids.
    pub decoded_token_digest: String,
    /// Plain-language decode detail.
    pub detail: String,
    /// Stable receipt digest.
    pub receipt_digest: String,
}

impl TassadarArticleTransformerDecodeReceipt {
    /// Creates one greedy decode receipt.
    #[must_use]
    pub fn greedy(
        receipt_id: impl Into<String>,
        logits_digest: impl Into<String>,
        predicted_token_ids: Vec<usize>,
    ) -> Self {
        let mut receipt = Self {
            schema_version: 1,
            receipt_id: receipt_id.into(),
            decode_strategy: String::from("greedy_argmax"),
            logits_digest: logits_digest.into(),
            predicted_token_ids,
            decoded_token_digest: String::new(),
            detail: String::new(),
            receipt_digest: String::new(),
        };
        receipt.decoded_token_digest = stable_digest(
            b"psionic_tassadar_article_transformer_decoded_tokens|",
            &receipt.predicted_token_ids,
        );
        receipt.detail = format!(
            "greedy decode emitted {} token ids from logits digest `{}`",
            receipt.predicted_token_ids.len(),
            receipt.logits_digest
        );
        receipt.receipt_digest = stable_digest(
            b"psionic_tassadar_article_transformer_decode_receipt|",
            &(
                receipt.schema_version,
                receipt.receipt_id.as_str(),
                receipt.decode_strategy.as_str(),
                receipt.logits_digest.as_str(),
                &receipt.predicted_token_ids,
                receipt.decoded_token_digest.as_str(),
            ),
        );
        receipt
    }
}

/// Deterministic replay receipt for one repeated forward pass.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleTransformerReplayReceipt {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable receipt identifier.
    pub receipt_id: String,
    /// Stable digest over the original forward output.
    pub expected_forward_output_digest: String,
    /// Stable digest over the replayed forward output.
    pub replayed_forward_output_digest: String,
    /// Stable digest over the original trace subset.
    pub expected_trace_digest: String,
    /// Stable digest over the replayed trace subset.
    pub replayed_trace_digest: String,
    /// Whether the replay matched exactly.
    pub deterministic_match: bool,
    /// Plain-language replay detail.
    pub detail: String,
    /// Stable receipt digest.
    pub receipt_digest: String,
}

impl TassadarArticleTransformerReplayReceipt {
    /// Creates one deterministic replay receipt.
    #[must_use]
    pub fn new(
        receipt_id: impl Into<String>,
        expected_forward_output_digest: impl Into<String>,
        replayed_forward_output_digest: impl Into<String>,
        expected_trace_digest: impl Into<String>,
        replayed_trace_digest: impl Into<String>,
    ) -> Self {
        let expected_forward_output_digest = expected_forward_output_digest.into();
        let replayed_forward_output_digest = replayed_forward_output_digest.into();
        let expected_trace_digest = expected_trace_digest.into();
        let replayed_trace_digest = replayed_trace_digest.into();
        let deterministic_match = expected_forward_output_digest == replayed_forward_output_digest
            && expected_trace_digest == replayed_trace_digest;
        let mut receipt = Self {
            schema_version: 1,
            receipt_id: receipt_id.into(),
            expected_forward_output_digest,
            replayed_forward_output_digest,
            expected_trace_digest,
            replayed_trace_digest,
            deterministic_match,
            detail: String::new(),
            receipt_digest: String::new(),
        };
        receipt.detail = format!(
            "deterministic replay matched={}",
            receipt.deterministic_match
        );
        receipt.receipt_digest = stable_digest(
            b"psionic_tassadar_article_transformer_replay_receipt|",
            &(
                receipt.schema_version,
                receipt.receipt_id.as_str(),
                receipt.expected_forward_output_digest.as_str(),
                receipt.replayed_forward_output_digest.as_str(),
                receipt.expected_trace_digest.as_str(),
                receipt.replayed_trace_digest.as_str(),
                receipt.deterministic_match,
            ),
        );
        receipt
    }
}

/// Optional checkpoint lineage threaded into one runtime forward-pass bundle.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleTransformerCheckpointLineage {
    /// Runtime-visible checkpoint identity.
    pub checkpoint: TrainingCheckpointReference,
    /// Parent checkpoint reference when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_checkpoint_ref: Option<String>,
    /// Parent manifest digest when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_manifest_digest: Option<String>,
}

/// Canonical runtime evidence bundle for one article-Transformer forward pass.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleTransformerForwardPassEvidenceBundle {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable bundle identifier.
    pub bundle_id: String,
    /// Tied TAS requirement id.
    pub tied_requirement_id: String,
    /// Canonical models-module owner reference.
    pub model_module_ref: String,
    /// Canonical transformer-module owner reference.
    pub transformer_module_ref: String,
    /// Canonical runtime-module owner reference.
    pub runtime_module_ref: String,
    /// Model artifact identity.
    pub model_artifact: TassadarArticleTransformerModelArtifactBinding,
    /// Captured run config.
    pub run_config: TassadarArticleTransformerForwardPassRunConfig,
    /// Optional checkpoint lineage carried into the run.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checkpoint_lineage: Option<TassadarArticleTransformerCheckpointLineage>,
    /// Runtime-visible forward-pass trace artifact.
    pub trace_artifact: TassadarArticleTransformerForwardPassTraceArtifact,
    /// Decode receipt.
    pub decode_receipt: TassadarArticleTransformerDecodeReceipt,
    /// Deterministic replay receipt.
    pub replay_receipt: TassadarArticleTransformerReplayReceipt,
    /// Digest-bound runtime manifest for the run.
    pub runtime_manifest: RuntimeManifest,
    /// Canonical Psionic proof bundle for the run.
    pub proof_bundle: ExecutionProofBundle,
    /// Explicit claim boundary.
    pub claim_boundary: String,
    /// Stable bundle digest.
    pub bundle_digest: String,
}

/// Returns the current validation reference for the article-Transformer forward-pass lane.
#[must_use]
pub fn tassadar_article_transformer_forward_pass_validation_reference() -> ValidationMatrixReference
{
    ValidationMatrixReference::not_yet_validated("tassadar.article_transformer.forward_pass.phase2")
}

/// Builds runtime-manifest and proof-bundle evidence for one article-Transformer forward pass.
#[must_use]
pub fn build_tassadar_article_transformer_forward_pass_evidence_bundle(
    bundle_id: impl Into<String>,
    model_module_ref: impl Into<String>,
    transformer_module_ref: impl Into<String>,
    runtime_module_ref: impl Into<String>,
    model_artifact: TassadarArticleTransformerModelArtifactBinding,
    run_config: TassadarArticleTransformerForwardPassRunConfig,
    checkpoint_lineage: Option<TassadarArticleTransformerCheckpointLineage>,
    trace_artifact: TassadarArticleTransformerForwardPassTraceArtifact,
    decode_receipt: TassadarArticleTransformerDecodeReceipt,
    replay_receipt: TassadarArticleTransformerReplayReceipt,
) -> TassadarArticleTransformerForwardPassEvidenceBundle {
    let bundle_id = bundle_id.into();
    let model_module_ref = model_module_ref.into();
    let transformer_module_ref = transformer_module_ref.into();
    let runtime_module_ref = runtime_module_ref.into();
    let validation = tassadar_article_transformer_forward_pass_validation_reference();
    let runtime_identity = ExecutionProofRuntimeIdentity::new(
        "cpu",
        BackendToolchainIdentity::new(
            "cpu",
            "psionic-transformer",
            vec![
                String::from("tassadar_article_transformer"),
                String::from("forward_pass"),
                String::from("reference_linear"),
            ],
        )
        .with_probe(BackendProbeState::CompiledOnly, Vec::new()),
    );
    let mut runtime_manifest = RuntimeManifest::new(
        format!(
            "tassadar-article-transformer-runtime-manifest-{}",
            run_config.request_id
        ),
        runtime_identity.clone(),
    )
    .with_validation(validation.clone())
    .with_claims_profile_id(TASSADAR_ARTICLE_TRANSFORMER_FORWARD_PASS_CLAIMS_PROFILE_ID)
    .with_artifact_binding(RuntimeManifestArtifactBinding::new(
        RuntimeManifestArtifactKind::ModelDescriptor,
        model_artifact.model_id.clone(),
        model_artifact.descriptor_digest.clone(),
    ))
    .with_artifact_binding(RuntimeManifestArtifactBinding::new(
        RuntimeManifestArtifactKind::PolicyWeights,
        model_artifact.artifact_id.clone(),
        model_artifact.weight_bundle_digest.clone(),
    ))
    .with_artifact_binding(RuntimeManifestArtifactBinding::new(
        RuntimeManifestArtifactKind::ExecutionTrace,
        trace_artifact.artifact_id.clone(),
        trace_artifact.artifact_digest.clone(),
    ))
    .with_static_config_binding(RuntimeManifestStaticConfigBinding::new(
        "tassadar.article_transformer.model_artifact_digest",
        model_artifact.artifact_digest.clone(),
    ))
    .with_static_config_binding(RuntimeManifestStaticConfigBinding::new(
        "tassadar.article_transformer.parameter_digest",
        model_artifact.trainable_parameter_digest.clone(),
    ))
    .with_static_config_binding(RuntimeManifestStaticConfigBinding::new(
        "tassadar.article_transformer.run_config_digest",
        run_config.run_config_digest.clone(),
    ))
    .with_static_config_binding(RuntimeManifestStaticConfigBinding::new(
        "tassadar.article_transformer.execution_mode",
        stable_bytes_digest(run_config.execution_mode.as_bytes()),
    ))
    .with_static_config_binding(RuntimeManifestStaticConfigBinding::new(
        "tassadar.article_transformer.trace_digest",
        trace_artifact.trace_digest.clone(),
    ))
    .with_static_config_binding(RuntimeManifestStaticConfigBinding::new(
        "tassadar.article_transformer.decode_strategy",
        stable_bytes_digest(decode_receipt.decode_strategy.as_bytes()),
    ))
    .with_static_config_binding(RuntimeManifestStaticConfigBinding::new(
        "tassadar.article_transformer.replay_receipt_digest",
        replay_receipt.receipt_digest.clone(),
    ))
    .with_static_config_binding(RuntimeManifestStaticConfigBinding::new(
        "tassadar.article_transformer.model_module_ref",
        stable_bytes_digest(model_module_ref.as_bytes()),
    ))
    .with_static_config_binding(RuntimeManifestStaticConfigBinding::new(
        "tassadar.article_transformer.transformer_module_ref",
        stable_bytes_digest(transformer_module_ref.as_bytes()),
    ))
    .with_static_config_binding(RuntimeManifestStaticConfigBinding::new(
        "tassadar.article_transformer.runtime_module_ref",
        stable_bytes_digest(runtime_module_ref.as_bytes()),
    ));
    for environment_ref in &run_config.environment_refs {
        runtime_manifest = runtime_manifest.with_environment_ref(environment_ref.clone());
    }
    if let Some(checkpoint_lineage) = &checkpoint_lineage {
        runtime_manifest = runtime_manifest
            .with_artifact_binding(RuntimeManifestArtifactBinding::new(
                RuntimeManifestArtifactKind::Checkpoint,
                checkpoint_lineage
                    .checkpoint
                    .checkpoint_ref
                    .clone()
                    .unwrap_or_else(|| checkpoint_lineage.checkpoint.stream_id.clone()),
                checkpoint_lineage.checkpoint.object_digest.clone(),
            ))
            .with_static_config_binding(RuntimeManifestStaticConfigBinding::new(
                "tassadar.article_transformer.checkpoint_manifest_digest",
                checkpoint_lineage.checkpoint.manifest_digest.clone(),
            ))
            .with_static_config_binding(RuntimeManifestStaticConfigBinding::new(
                "tassadar.article_transformer.checkpoint_family",
                stable_bytes_digest(checkpoint_lineage.checkpoint.checkpoint_family.as_bytes()),
            ));
        if let Some(parent_checkpoint_ref) = &checkpoint_lineage.parent_checkpoint_ref {
            runtime_manifest = runtime_manifest.with_static_config_binding(
                RuntimeManifestStaticConfigBinding::new(
                    "tassadar.article_transformer.parent_checkpoint_ref",
                    stable_bytes_digest(parent_checkpoint_ref.as_bytes()),
                ),
            );
        }
        if let Some(parent_manifest_digest) = &checkpoint_lineage.parent_manifest_digest {
            runtime_manifest = runtime_manifest.with_static_config_binding(
                RuntimeManifestStaticConfigBinding::new(
                    "tassadar.article_transformer.parent_manifest_digest",
                    parent_manifest_digest.clone(),
                ),
            );
        }
    }

    let mut proof_bundle = ExecutionProofBundle::new(
        ExecutionProofBundleKind::Local,
        if replay_receipt.deterministic_match {
            ExecutionProofBundleStatus::Succeeded
        } else {
            ExecutionProofBundleStatus::Failed
        },
        run_config.request_id.clone(),
        run_config.run_config_digest.clone(),
        run_config.product_id.clone(),
        runtime_identity,
    )
    .with_model_id(model_artifact.model_id.clone())
    .with_validation(validation)
    .with_activation_fingerprint_posture(ExecutionProofAugmentationPosture::Unavailable);
    let mut input_artifact_digests = vec![model_artifact.artifact_digest.clone()];
    if let Some(checkpoint_lineage) = &checkpoint_lineage {
        input_artifact_digests.push(checkpoint_lineage.checkpoint.object_digest.clone());
    }
    proof_bundle.artifact_residency = Some(ExecutionProofArtifactResidency {
        served_artifact_digest: None,
        weight_bundle_digest: Some(model_artifact.weight_bundle_digest.clone()),
        cluster_artifact_residency_digest: None,
        sharded_model_manifest_digest: None,
        input_artifact_digests,
        output_artifact_digests: vec![
            trace_artifact.artifact_digest.clone(),
            decode_receipt.receipt_digest.clone(),
            replay_receipt.receipt_digest.clone(),
        ],
        stdout_sha256: None,
        stderr_sha256: None,
    });
    if !replay_receipt.deterministic_match {
        proof_bundle = proof_bundle.with_failure_reason(String::from(
            "article_transformer_forward_pass_replay_mismatch",
        ));
    }

    let mut bundle = TassadarArticleTransformerForwardPassEvidenceBundle {
        schema_version: 1,
        bundle_id,
        tied_requirement_id: String::from("TAS-165"),
        model_module_ref,
        transformer_module_ref,
        runtime_module_ref,
        model_artifact,
        run_config,
        checkpoint_lineage,
        trace_artifact,
        decode_receipt,
        replay_receipt,
        runtime_manifest,
        proof_bundle,
        claim_boundary: String::from(
            "this runtime bundle covers one canonical article-Transformer forward-pass lane with model identity, run-config capture, attention-trace summaries, greedy decode receipt, deterministic replay receipt, and optional checkpoint lineage bound into the Psionic runtime-manifest and proof-bundle substrate. It does not claim final article trace-vocabulary closure, benchmark parity, fast-route promotion, or final article-equivalence green status.",
        ),
        bundle_digest: String::new(),
    };
    bundle.bundle_digest = stable_digest(
        b"psionic_tassadar_article_transformer_forward_pass_evidence_bundle|",
        &bundle,
    );
    bundle
}

fn stable_bytes_digest(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_tassadar_article_transformer_bytes|");
    hasher.update(bytes);
    hex::encode(hasher.finalize())
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
        build_tassadar_article_transformer_forward_pass_evidence_bundle,
        TassadarArticleTransformerCheckpointLineage, TassadarArticleTransformerDecodeReceipt,
        TassadarArticleTransformerForwardPassChannelTrace,
        TassadarArticleTransformerForwardPassRunConfig,
        TassadarArticleTransformerForwardPassTraceArtifact,
        TassadarArticleTransformerModelArtifactBinding, TassadarArticleTransformerReplayReceipt,
    };
    use crate::{
        ExecutionProofBundleStatus, RuntimeManifestArtifactKind, TrainingCheckpointReference,
    };

    #[test]
    fn forward_pass_evidence_bundle_binds_receipts_and_checkpoint_lineage() {
        let model_artifact = TassadarArticleTransformerModelArtifactBinding::new(
            "tassadar-article-transformer-paper-faithful-v0",
            "tassadar_article_transformer",
            "descriptor-digest",
            "parameter-digest",
            "tassadar://article_transformer/weights/tassadar-article-transformer-paper-faithful-v0/weight-bundle-digest",
            "weight-bundle-digest",
            "artifact-sha256",
        );
        let run_config = TassadarArticleTransformerForwardPassRunConfig::new(
            "run-1",
            "request-1",
            "psionic.article_transformer.forward_pass",
            vec![1, 2],
            vec![0, 1],
            vec![1, 3],
            vec![0, 1, 2],
            "eval",
            vec![String::from("fixtures://tassadar/article_transformer")],
        );
        let channel = TassadarArticleTransformerForwardPassChannelTrace::new(
            "encoder_layer_0.self_attention",
            "encoder_self_attention",
            vec![1, 2, 2, 2],
            "probability-digest",
            8,
        );
        let trace_artifact = TassadarArticleTransformerForwardPassTraceArtifact::new(
            "tassadar://article_transformer/trace/request-1/trace-digest",
            "forward-output-digest",
            "encoder-hidden-digest",
            "decoder-hidden-digest",
            "logits-digest",
            vec![3, 4, 5],
            vec![channel.clone()],
            vec![channel.clone()],
            vec![channel],
            "crates/psionic-models/src/tassadar_article_transformer.rs",
            "crates/psionic-transformer/src/encoder_decoder.rs",
        );
        let decode_receipt = TassadarArticleTransformerDecodeReceipt::greedy(
            "decode-receipt-1",
            trace_artifact.logits_digest.clone(),
            trace_artifact.predicted_token_ids.clone(),
        );
        let replay_receipt = TassadarArticleTransformerReplayReceipt::new(
            "replay-receipt-1",
            trace_artifact.forward_output_digest.clone(),
            trace_artifact.forward_output_digest.clone(),
            trace_artifact.trace_digest.clone(),
            trace_artifact.trace_digest.clone(),
        );
        let checkpoint_lineage = TassadarArticleTransformerCheckpointLineage {
            checkpoint: TrainingCheckpointReference::new(
                "train.tassadar.article_transformer",
                "checkpoint-stream",
                "checkpoint-manifest-digest",
                "checkpoint-object-digest",
                "psionic.local.cpu_reference",
                0,
                "cluster.local.cpu_reference",
                "topology.cpu_reference",
                42,
            )
            .with_checkpoint_ref("checkpoint-ref")
            .with_step(7)
            .with_durable_at_ms(42),
            parent_checkpoint_ref: Some(String::from("parent-checkpoint-ref")),
            parent_manifest_digest: Some(String::from("parent-manifest-digest")),
        };

        let bundle = build_tassadar_article_transformer_forward_pass_evidence_bundle(
            "bundle-1",
            "crates/psionic-models/src/tassadar_article_transformer.rs",
            "crates/psionic-transformer/src/encoder_decoder.rs",
            "crates/psionic-runtime/src/tassadar_article_transformer_forward_pass.rs",
            model_artifact,
            run_config,
            Some(checkpoint_lineage),
            trace_artifact,
            decode_receipt,
            replay_receipt,
        );

        assert_eq!(
            bundle.proof_bundle.status,
            ExecutionProofBundleStatus::Succeeded
        );
        assert!(bundle
            .runtime_manifest
            .artifact_bindings
            .iter()
            .any(
                |binding| binding.kind == RuntimeManifestArtifactKind::Checkpoint
                    && binding.reference == "checkpoint-ref"
                    && binding.digest == "checkpoint-object-digest"
            ));
        assert!(bundle
            .runtime_manifest
            .static_config_bindings
            .iter()
            .any(
                |binding| binding.key == "tassadar.article_transformer.parent_manifest_digest"
                    && binding.value_digest == "parent-manifest-digest"
            ));
    }

    #[test]
    fn replay_mismatch_marks_proof_bundle_failed() {
        let model_artifact = TassadarArticleTransformerModelArtifactBinding::new(
            "tassadar-article-transformer-paper-faithful-v0",
            "tassadar_article_transformer",
            "descriptor-digest",
            "parameter-digest",
            "tassadar://article_transformer/weights/tassadar-article-transformer-paper-faithful-v0/weight-bundle-digest",
            "weight-bundle-digest",
            "artifact-sha256",
        );
        let run_config = TassadarArticleTransformerForwardPassRunConfig::new(
            "run-2",
            "request-2",
            "psionic.article_transformer.forward_pass",
            vec![1, 1],
            vec![0],
            vec![1, 1],
            vec![0],
            "eval",
            Vec::new(),
        );
        let trace_artifact = TassadarArticleTransformerForwardPassTraceArtifact::new(
            "trace-2",
            "forward-output-digest",
            "encoder-hidden-digest",
            "decoder-hidden-digest",
            "logits-digest",
            vec![1],
            Vec::new(),
            Vec::new(),
            Vec::new(),
            "crates/psionic-models/src/tassadar_article_transformer.rs",
            "crates/psionic-transformer/src/encoder_decoder.rs",
        );
        let decode_receipt = TassadarArticleTransformerDecodeReceipt::greedy(
            "decode-receipt-2",
            "logits-digest",
            vec![1],
        );
        let replay_receipt = TassadarArticleTransformerReplayReceipt::new(
            "replay-receipt-2",
            "forward-output-digest",
            "different-forward-output-digest",
            "trace-digest",
            "different-trace-digest",
        );

        let bundle = build_tassadar_article_transformer_forward_pass_evidence_bundle(
            "bundle-2",
            "crates/psionic-models/src/tassadar_article_transformer.rs",
            "crates/psionic-transformer/src/encoder_decoder.rs",
            "crates/psionic-runtime/src/tassadar_article_transformer_forward_pass.rs",
            model_artifact,
            run_config,
            None,
            trace_artifact,
            decode_receipt,
            replay_receipt,
        );

        assert_eq!(
            bundle.proof_bundle.status,
            ExecutionProofBundleStatus::Failed
        );
        assert_eq!(
            bundle.proof_bundle.failure_reason.as_deref(),
            Some("article_transformer_forward_pass_replay_mismatch")
        );
    }
}
