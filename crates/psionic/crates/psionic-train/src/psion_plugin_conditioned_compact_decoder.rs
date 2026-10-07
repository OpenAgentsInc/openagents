use std::{
    fs,
    path::{Path, PathBuf},
};

use psionic_models::{
    PsionCompactDecoderDescriptor, PsionCompactDecoderError, PsionCompactDecoderSizeAnchor,
    PsionCompactDecoderTokenizerBinding, PsionCompactDecoderTokenizerFamily,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{PsionPluginConditionedSftStageManifest, PSION_PLUGIN_CONDITIONED_SFT_RUN_BUNDLE_REF};

/// Stable schema version for the plugin-conditioned compact-decoder reference config.
pub const PSION_PLUGIN_CONDITIONED_COMPACT_DECODER_REFERENCE_SCHEMA_VERSION: &str =
    "psionic.psion.plugin_conditioned_compact_decoder_reference.v1";
/// Stable committed config ref for the first plugin-conditioned compact-decoder config.
pub const PSION_PLUGIN_CONDITIONED_COMPACT_DECODER_REFERENCE_CONFIG_REF: &str =
    "fixtures/psion/plugins/models/psion_plugin_conditioned_compact_decoder_reference_config_v1.json";
/// Stable committed config ref for the first mixed plugin-conditioned compact-decoder config.
pub const PSION_PLUGIN_CONDITIONED_MIXED_COMPACT_DECODER_REFERENCE_CONFIG_REF: &str =
    "fixtures/psion/plugins/models/psion_plugin_conditioned_mixed_compact_decoder_reference_config_v1.json";
/// Stable lane identifier for the first plugin-conditioned compact-decoder config.
pub const PSION_PLUGIN_CONDITIONED_REFERENCE_LANE_ID: &str =
    "psion_plugin_conditioned_host_native_reference";
/// Stable lane identifier for the first mixed plugin-conditioned compact-decoder config.
pub const PSION_PLUGIN_CONDITIONED_MIXED_REFERENCE_LANE_ID: &str =
    "psion_plugin_conditioned_mixed_reference";
/// Stable lane identifier for the first accelerated host-native plugin-conditioned compact-decoder config.
pub const PSION_PLUGIN_CONDITIONED_HOST_NATIVE_ACCELERATED_LANE_ID: &str =
    "psion_plugin_host_native_accelerated";
/// Stable model id for the first plugin-conditioned compact-decoder reference config.
pub const PSION_PLUGIN_CONDITIONED_REFERENCE_MODEL_ID: &str =
    "psion-plugin-conditioned-compact-decoder-reference-v1";
/// Stable model id for the first mixed plugin-conditioned compact-decoder reference config.
pub const PSION_PLUGIN_CONDITIONED_MIXED_REFERENCE_MODEL_ID: &str =
    "psion-plugin-conditioned-mixed-compact-decoder-reference-v1";
/// Stable model id for the first accelerated host-native plugin-conditioned compact-decoder config.
pub const PSION_PLUGIN_CONDITIONED_HOST_NATIVE_ACCELERATED_MODEL_ID: &str =
    "psion-plugin-conditioned-host-native-accelerated-v1";
/// Stable revision for the first plugin-conditioned compact-decoder reference config.
pub const PSION_PLUGIN_CONDITIONED_REFERENCE_REVISION: &str = "plugin-conditioned-v1";
/// Stable tokenizer id reused for the first plugin-conditioned compact-decoder reference config.
pub const PSION_PLUGIN_CONDITIONED_REFERENCE_TOKENIZER_ID: &str = "psion_sentencepiece_seed";
/// Stable tokenizer version reused for the first plugin-conditioned compact-decoder reference config.
pub const PSION_PLUGIN_CONDITIONED_REFERENCE_TOKENIZER_VERSION: &str = "v1";
/// Stable tokenizer digest reused for the first plugin-conditioned compact-decoder reference config.
pub const PSION_PLUGIN_CONDITIONED_REFERENCE_TOKENIZER_DIGEST: &str =
    "sha256:psion_sentencepiece_seed_tokenizer_digest_v1";
/// Stable special-token digest reused for the first plugin-conditioned compact-decoder reference config.
pub const PSION_PLUGIN_CONDITIONED_REFERENCE_SPECIAL_TOKENS_DIGEST: &str =
    "sha256:psion_sentencepiece_seed_added_tokens_digest_v1";
/// Stable prompt-template digest frozen for the plugin-conditioned lane.
pub const PSION_PLUGIN_CONDITIONED_REFERENCE_TEMPLATE_DIGEST: &str =
    "sha256:psion_plugin_conditioned_prompt_template_digest_v1";
/// Stable vocabulary size reused for the first plugin-conditioned compact-decoder reference config.
pub const PSION_PLUGIN_CONDITIONED_REFERENCE_VOCAB_SIZE: usize = 32_768;
/// Stable context window for the first plugin-conditioned compact-decoder reference config.
pub const PSION_PLUGIN_CONDITIONED_REFERENCE_CONTEXT_TOKENS: usize = 8_192;

#[derive(Clone, Copy)]
struct PsionPluginConditionedReferenceLaneProfile<'a> {
    lane_id: &'a str,
    model_id: &'a str,
    stage_run_bundle_ref: &'a str,
    checkpoint_ref_prefix: &'a str,
    export_directory_name: &'a str,
    checkpoint_detail: &'a str,
}

/// How plugin-conditioned structure is serialized into the decoder token stream.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionPluginConditionedSerializationStrategy {
    /// Serialize plugin-use traces as structured JSON carrying schema ids, tool names, and receipt refs.
    StructuredJsonWithSchemaIdsAndReceiptRefs,
}

/// Context-budget assumptions frozen for the plugin-conditioned reference config.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionPluginConditionedContextAssumptions {
    /// Explicit context window bound into the descriptor.
    pub max_context_tokens: usize,
    /// Max plugin calls per trace admitted by the first stage contract.
    pub max_plugin_calls_per_trace: u32,
    /// Minimum token budget reserved for the user directive.
    pub reserved_directive_tokens: u32,
    /// Minimum token budget reserved for serialized schema and packet structure.
    pub reserved_schema_tokens: u32,
    /// Minimum token budget reserved for receipt refs and refusal anchors.
    pub reserved_receipt_anchor_tokens: u32,
    /// Minimum token budget reserved for the assistant completion.
    pub reserved_completion_tokens: u32,
    /// Short explanation of the context posture.
    pub detail: String,
}

impl PsionPluginConditionedContextAssumptions {
    fn validate_against_stage(
        &self,
        stage_manifest: &PsionPluginConditionedSftStageManifest,
        descriptor: &PsionCompactDecoderDescriptor,
    ) -> Result<(), PsionPluginConditionedCompactDecoderError> {
        ensure_nonempty(
            self.detail.as_str(),
            "plugin_conditioned_context_assumptions.detail",
        )?;
        if self.max_context_tokens != descriptor.config.max_context {
            return Err(PsionPluginConditionedCompactDecoderError::FieldMismatch {
                field: String::from("plugin_conditioned_context_assumptions.max_context_tokens"),
                expected: descriptor.config.max_context.to_string(),
                actual: self.max_context_tokens.to_string(),
            });
        }
        if self.max_plugin_calls_per_trace != stage_manifest.config.max_plugin_calls_per_trace {
            return Err(PsionPluginConditionedCompactDecoderError::FieldMismatch {
                field: String::from(
                    "plugin_conditioned_context_assumptions.max_plugin_calls_per_trace",
                ),
                expected: stage_manifest.config.max_plugin_calls_per_trace.to_string(),
                actual: self.max_plugin_calls_per_trace.to_string(),
            });
        }
        let reserved_total = self
            .reserved_directive_tokens
            .saturating_add(self.reserved_schema_tokens)
            .saturating_add(self.reserved_receipt_anchor_tokens)
            .saturating_add(self.reserved_completion_tokens);
        if reserved_total > self.max_context_tokens as u32 {
            return Err(PsionPluginConditionedCompactDecoderError::FieldMismatch {
                field: String::from("plugin_conditioned_context_assumptions.total_reserved_tokens"),
                expected: format!("at most {}", self.max_context_tokens),
                actual: reserved_total.to_string(),
            });
        }
        Ok(())
    }
}

/// Vocabulary and serialization posture frozen for the plugin-conditioned reference config.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionPluginConditionedSerializationContract {
    /// Serialization strategy admitted by the lane.
    pub strategy: PsionPluginConditionedSerializationStrategy,
    /// Whether bespoke plugin-only tokens are admitted.
    pub custom_plugin_tokens_admitted: bool,
    /// Whether schema ids stay serialized verbatim in the text stream.
    pub schema_ids_serialized_verbatim: bool,
    /// Whether tool names stay serialized verbatim in the text stream.
    pub tool_names_serialized_verbatim: bool,
    /// Whether receipt refs stay serialized verbatim in the text stream.
    pub receipt_refs_serialized_verbatim: bool,
    /// Short explanation of the serialization posture.
    pub detail: String,
}

impl PsionPluginConditionedSerializationContract {
    fn validate_against_descriptor(
        &self,
        descriptor: &PsionCompactDecoderDescriptor,
    ) -> Result<(), PsionPluginConditionedCompactDecoderError> {
        ensure_nonempty(
            self.detail.as_str(),
            "plugin_conditioned_serialization_contract.detail",
        )?;
        ensure_bool_false(
            self.custom_plugin_tokens_admitted,
            "plugin_conditioned_serialization_contract.custom_plugin_tokens_admitted",
        )?;
        ensure_bool_true(
            self.schema_ids_serialized_verbatim,
            "plugin_conditioned_serialization_contract.schema_ids_serialized_verbatim",
        )?;
        ensure_bool_true(
            self.tool_names_serialized_verbatim,
            "plugin_conditioned_serialization_contract.tool_names_serialized_verbatim",
        )?;
        ensure_bool_true(
            self.receipt_refs_serialized_verbatim,
            "plugin_conditioned_serialization_contract.receipt_refs_serialized_verbatim",
        )?;
        check_string_match(
            descriptor.tokenizer_binding.tokenizer_id.as_str(),
            PSION_PLUGIN_CONDITIONED_REFERENCE_TOKENIZER_ID,
            "descriptor.tokenizer_binding.tokenizer_id",
        )?;
        check_string_match(
            descriptor.tokenizer_binding.tokenizer_version.as_str(),
            PSION_PLUGIN_CONDITIONED_REFERENCE_TOKENIZER_VERSION,
            "descriptor.tokenizer_binding.tokenizer_version",
        )?;
        check_string_match(
            descriptor.tokenizer_binding.tokenizer_digest.as_str(),
            PSION_PLUGIN_CONDITIONED_REFERENCE_TOKENIZER_DIGEST,
            "descriptor.tokenizer_binding.tokenizer_digest",
        )?;
        if descriptor.tokenizer_binding.vocab_size != PSION_PLUGIN_CONDITIONED_REFERENCE_VOCAB_SIZE
        {
            return Err(PsionPluginConditionedCompactDecoderError::FieldMismatch {
                field: String::from("descriptor.tokenizer_binding.vocab_size"),
                expected: PSION_PLUGIN_CONDITIONED_REFERENCE_VOCAB_SIZE.to_string(),
                actual: descriptor.tokenizer_binding.vocab_size.to_string(),
            });
        }
        check_string_match(
            descriptor
                .tokenizer_binding
                .special_tokens_digest
                .as_deref()
                .unwrap_or_default(),
            PSION_PLUGIN_CONDITIONED_REFERENCE_SPECIAL_TOKENS_DIGEST,
            "descriptor.tokenizer_binding.special_tokens_digest",
        )?;
        check_string_match(
            descriptor
                .tokenizer_binding
                .template_digest
                .as_deref()
                .unwrap_or_default(),
            PSION_PLUGIN_CONDITIONED_REFERENCE_TEMPLATE_DIGEST,
            "descriptor.tokenizer_binding.template_digest",
        )?;
        Ok(())
    }
}

/// Lane-specific checkpoint and export naming frozen for the plugin-conditioned reference config.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionPluginConditionedCheckpointNaming {
    /// Checkpoint family for the lane.
    pub checkpoint_family: String,
    /// Bound stage run-bundle ref.
    pub stage_run_bundle_ref: String,
    /// Bound stage-manifest digest.
    pub stage_manifest_digest: String,
    /// Stable checkpoint-ref prefix for the lane.
    pub checkpoint_ref_prefix: String,
    /// Stable export directory name for the lane.
    pub export_directory_name: String,
    /// Stable descriptor file name.
    pub descriptor_file_name: String,
    /// Stable checkpoint file name.
    pub checkpoint_file_name: String,
    /// Stable export-format identifier.
    pub export_format_id: String,
    /// Short explanation of the naming posture.
    pub detail: String,
}

impl PsionPluginConditionedCheckpointNaming {
    fn validate_against_stage(
        &self,
        stage_manifest: &PsionPluginConditionedSftStageManifest,
        descriptor: &PsionCompactDecoderDescriptor,
    ) -> Result<(), PsionPluginConditionedCompactDecoderError> {
        ensure_nonempty(
            self.checkpoint_family.as_str(),
            "plugin_conditioned_checkpoint_naming.checkpoint_family",
        )?;
        ensure_nonempty(
            self.stage_run_bundle_ref.as_str(),
            "plugin_conditioned_checkpoint_naming.stage_run_bundle_ref",
        )?;
        ensure_nonempty(
            self.stage_manifest_digest.as_str(),
            "plugin_conditioned_checkpoint_naming.stage_manifest_digest",
        )?;
        ensure_nonempty(
            self.checkpoint_ref_prefix.as_str(),
            "plugin_conditioned_checkpoint_naming.checkpoint_ref_prefix",
        )?;
        ensure_nonempty(
            self.export_directory_name.as_str(),
            "plugin_conditioned_checkpoint_naming.export_directory_name",
        )?;
        ensure_nonempty(
            self.detail.as_str(),
            "plugin_conditioned_checkpoint_naming.detail",
        )?;
        check_string_match(
            self.checkpoint_family.as_str(),
            stage_manifest.checkpoint_family.as_str(),
            "plugin_conditioned_checkpoint_naming.checkpoint_family",
        )?;
        check_string_match(
            self.stage_manifest_digest.as_str(),
            stage_manifest.manifest_digest.as_str(),
            "plugin_conditioned_checkpoint_naming.stage_manifest_digest",
        )?;
        ensure_nonempty(
            self.stage_run_bundle_ref.as_str(),
            "plugin_conditioned_checkpoint_naming.stage_run_bundle_ref",
        )?;
        check_string_match(
            self.descriptor_file_name.as_str(),
            descriptor.export_contract.descriptor_file_name.as_str(),
            "plugin_conditioned_checkpoint_naming.descriptor_file_name",
        )?;
        check_string_match(
            self.checkpoint_file_name.as_str(),
            descriptor.export_contract.checkpoint_file_name.as_str(),
            "plugin_conditioned_checkpoint_naming.checkpoint_file_name",
        )?;
        check_string_match(
            self.export_format_id.as_str(),
            descriptor.export_contract.export_format_id.as_str(),
            "plugin_conditioned_checkpoint_naming.export_format_id",
        )?;
        Ok(())
    }
}

/// Full reference config for the first plugin-conditioned compact-decoder lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionPluginConditionedCompactDecoderReferenceConfig {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable lane identifier.
    pub lane_id: String,
    /// Bound stage-manifest digest.
    pub stage_manifest_digest: String,
    /// Bound canonical dataset identity.
    pub stage_dataset_identity: String,
    /// Compact-decoder descriptor for the lane.
    pub descriptor: PsionCompactDecoderDescriptor,
    /// Context-budget assumptions.
    pub context_assumptions: PsionPluginConditionedContextAssumptions,
    /// Serialization and vocabulary posture.
    pub serialization_contract: PsionPluginConditionedSerializationContract,
    /// Checkpoint and export naming posture.
    pub checkpoint_naming: PsionPluginConditionedCheckpointNaming,
    /// Short explanation of the config.
    pub summary: String,
    /// Stable digest over the full config.
    pub config_digest: String,
}

impl PsionPluginConditionedCompactDecoderReferenceConfig {
    /// Writes the config to one JSON file.
    pub fn write_to_path(
        &self,
        output_path: impl AsRef<Path>,
    ) -> Result<(), PsionPluginConditionedCompactDecoderError> {
        let output_path = output_path.as_ref();
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                PsionPluginConditionedCompactDecoderError::CreateDir {
                    path: parent.display().to_string(),
                    error,
                }
            })?;
        }
        let json = serde_json::to_string_pretty(self)?;
        fs::write(output_path, format!("{json}\n")).map_err(|error| {
            PsionPluginConditionedCompactDecoderError::Write {
                path: output_path.display().to_string(),
                error,
            }
        })?;
        Ok(())
    }

    /// Validates the config against the plugin-conditioned stage manifest.
    pub fn validate_against_stage(
        &self,
        stage_manifest: &PsionPluginConditionedSftStageManifest,
    ) -> Result<(), PsionPluginConditionedCompactDecoderError> {
        check_string_match(
            self.schema_version.as_str(),
            PSION_PLUGIN_CONDITIONED_COMPACT_DECODER_REFERENCE_SCHEMA_VERSION,
            "plugin_conditioned_compact_decoder_reference.schema_version",
        )?;
        ensure_nonempty(
            self.lane_id.as_str(),
            "plugin_conditioned_compact_decoder_reference.lane_id",
        )?;
        check_string_match(
            self.stage_manifest_digest.as_str(),
            stage_manifest.manifest_digest.as_str(),
            "plugin_conditioned_compact_decoder_reference.stage_manifest_digest",
        )?;
        check_string_match(
            self.stage_dataset_identity.as_str(),
            stage_manifest
                .dataset_binding
                .stable_dataset_identity
                .as_str(),
            "plugin_conditioned_compact_decoder_reference.stage_dataset_identity",
        )?;
        ensure_nonempty(
            self.summary.as_str(),
            "plugin_conditioned_compact_decoder_reference.summary",
        )?;
        self.descriptor.validate()?;
        ensure_nonempty(
            self.descriptor.model.model_id.as_str(),
            "plugin_conditioned_compact_decoder_reference.descriptor.model.model_id",
        )?;
        check_string_match(
            self.descriptor.model.revision.as_str(),
            PSION_PLUGIN_CONDITIONED_REFERENCE_REVISION,
            "plugin_conditioned_compact_decoder_reference.descriptor.model.revision",
        )?;
        if self.descriptor.size_anchor != PsionCompactDecoderSizeAnchor::Pilot32m {
            return Err(PsionPluginConditionedCompactDecoderError::FieldMismatch {
                field: String::from(
                    "plugin_conditioned_compact_decoder_reference.descriptor.size_anchor",
                ),
                expected: String::from("Pilot32m"),
                actual: format!("{:?}", self.descriptor.size_anchor),
            });
        }
        self.context_assumptions
            .validate_against_stage(stage_manifest, &self.descriptor)?;
        self.serialization_contract
            .validate_against_descriptor(&self.descriptor)?;
        self.checkpoint_naming
            .validate_against_stage(stage_manifest, &self.descriptor)?;
        if self.config_digest != stable_config_digest(self) {
            return Err(PsionPluginConditionedCompactDecoderError::DigestMismatch);
        }
        Ok(())
    }
}

/// Records the first plugin-conditioned compact-decoder reference config.
pub fn record_psion_plugin_conditioned_compact_decoder_reference_config(
    stage_manifest: &PsionPluginConditionedSftStageManifest,
    summary: impl Into<String>,
) -> Result<
    PsionPluginConditionedCompactDecoderReferenceConfig,
    PsionPluginConditionedCompactDecoderError,
> {
    record_psion_plugin_conditioned_compact_decoder_reference_config_for_lane(
        stage_manifest,
        PsionPluginConditionedReferenceLaneProfile {
            lane_id: PSION_PLUGIN_CONDITIONED_REFERENCE_LANE_ID,
            model_id: PSION_PLUGIN_CONDITIONED_REFERENCE_MODEL_ID,
            stage_run_bundle_ref: PSION_PLUGIN_CONDITIONED_SFT_RUN_BUNDLE_REF,
            checkpoint_ref_prefix: "checkpoint://psion/plugin_conditioned_host_native_reference",
            export_directory_name: "psion_plugin_conditioned_compact_decoder_reference",
            checkpoint_detail:
                "Checkpoint and export naming stay on the shared compact-decoder file contract while the checkpoint family and export directory are explicitly bound to the plugin-conditioned host-native lane.",
        },
        summary,
    )
}

/// Records the first mixed plugin-conditioned compact-decoder reference config.
pub fn record_psion_plugin_conditioned_mixed_compact_decoder_reference_config(
    stage_manifest: &PsionPluginConditionedSftStageManifest,
    stage_run_bundle_ref: &str,
    summary: impl Into<String>,
) -> Result<
    PsionPluginConditionedCompactDecoderReferenceConfig,
    PsionPluginConditionedCompactDecoderError,
> {
    record_psion_plugin_conditioned_compact_decoder_reference_config_for_lane(
        stage_manifest,
        PsionPluginConditionedReferenceLaneProfile {
            lane_id: PSION_PLUGIN_CONDITIONED_MIXED_REFERENCE_LANE_ID,
            model_id: PSION_PLUGIN_CONDITIONED_MIXED_REFERENCE_MODEL_ID,
            stage_run_bundle_ref,
            checkpoint_ref_prefix: "checkpoint://psion/plugin_conditioned_mixed_reference",
            export_directory_name: "psion_plugin_conditioned_mixed_compact_decoder_reference",
            checkpoint_detail:
                "Checkpoint and export naming stay on the shared compact-decoder file contract while the checkpoint family and export directory are explicitly bound to the first mixed host-native plus guest-artifact lane.",
        },
        summary,
    )
}

/// Records the first accelerated host-native plugin-conditioned compact-decoder config.
pub fn record_psion_plugin_conditioned_host_native_accelerated_compact_decoder_config(
    stage_manifest: &PsionPluginConditionedSftStageManifest,
    stage_run_bundle_ref: &str,
    summary: impl Into<String>,
) -> Result<
    PsionPluginConditionedCompactDecoderReferenceConfig,
    PsionPluginConditionedCompactDecoderError,
> {
    record_psion_plugin_conditioned_compact_decoder_reference_config_for_lane(
        stage_manifest,
        PsionPluginConditionedReferenceLaneProfile {
            lane_id: PSION_PLUGIN_CONDITIONED_HOST_NATIVE_ACCELERATED_LANE_ID,
            model_id: PSION_PLUGIN_CONDITIONED_HOST_NATIVE_ACCELERATED_MODEL_ID,
            stage_run_bundle_ref,
            checkpoint_ref_prefix: "checkpoint://psion/plugin_host_native_accelerated",
            export_directory_name: "psion_plugin_host_native_accelerated",
            checkpoint_detail:
                "Checkpoint and export naming stay on the shared compact-decoder file contract while the checkpoint family and export directory are explicitly bound to the first accelerated host-native plugin-conditioned lane.",
        },
        summary,
    )
}

fn record_psion_plugin_conditioned_compact_decoder_reference_config_for_lane(
    stage_manifest: &PsionPluginConditionedSftStageManifest,
    lane: PsionPluginConditionedReferenceLaneProfile<'_>,
    summary: impl Into<String>,
) -> Result<
    PsionPluginConditionedCompactDecoderReferenceConfig,
    PsionPluginConditionedCompactDecoderError,
> {
    let descriptor = reference_descriptor_with_model_id(lane.model_id)?;
    let mut config = PsionPluginConditionedCompactDecoderReferenceConfig {
        schema_version: String::from(
            PSION_PLUGIN_CONDITIONED_COMPACT_DECODER_REFERENCE_SCHEMA_VERSION,
        ),
        lane_id: String::from(lane.lane_id),
        stage_manifest_digest: stage_manifest.manifest_digest.clone(),
        stage_dataset_identity: stage_manifest.dataset_binding.stable_dataset_identity.clone(),
        descriptor: descriptor.clone(),
        context_assumptions: PsionPluginConditionedContextAssumptions {
            max_context_tokens: descriptor.config.max_context,
            max_plugin_calls_per_trace: stage_manifest.config.max_plugin_calls_per_trace,
            reserved_directive_tokens: 1_024,
            reserved_schema_tokens: 2_048,
            reserved_receipt_anchor_tokens: 1_024,
            reserved_completion_tokens: 2_048,
            detail: String::from(
                "The first plugin-conditioned lane reserves explicit budget for directives, structured packet/schema text, receipt anchors, and the assistant completion inside one 8192-token pilot context window.",
            ),
        },
        serialization_contract: PsionPluginConditionedSerializationContract {
            strategy:
                PsionPluginConditionedSerializationStrategy::StructuredJsonWithSchemaIdsAndReceiptRefs,
            custom_plugin_tokens_admitted: false,
            schema_ids_serialized_verbatim: true,
            tool_names_serialized_verbatim: true,
            receipt_refs_serialized_verbatim: true,
            detail: String::from(
                "The first plugin-conditioned compact decoder uses the existing tokenizer and serializes plugin-use supervision as structured JSON with schema ids, tool names, and receipt refs in ordinary token space instead of adding bespoke plugin tokens.",
            ),
        },
        checkpoint_naming: PsionPluginConditionedCheckpointNaming {
            checkpoint_family: stage_manifest.checkpoint_family.clone(),
            stage_run_bundle_ref: String::from(lane.stage_run_bundle_ref),
            stage_manifest_digest: stage_manifest.manifest_digest.clone(),
            checkpoint_ref_prefix: String::from(lane.checkpoint_ref_prefix),
            export_directory_name: String::from(lane.export_directory_name),
            descriptor_file_name: descriptor.export_contract.descriptor_file_name.clone(),
            checkpoint_file_name: descriptor.export_contract.checkpoint_file_name.clone(),
            export_format_id: descriptor.export_contract.export_format_id.clone(),
            detail: String::from(lane.checkpoint_detail),
        },
        summary: summary.into(),
        config_digest: String::new(),
    };
    config.config_digest = stable_config_digest(&config);
    config.validate_against_stage(stage_manifest)?;
    Ok(config)
}

/// Returns the canonical output path for the committed reference config.
#[must_use]
pub fn psion_plugin_conditioned_compact_decoder_reference_config_path() -> PathBuf {
    repo_root().join(PSION_PLUGIN_CONDITIONED_COMPACT_DECODER_REFERENCE_CONFIG_REF)
}

/// Returns the canonical output path for the committed mixed reference config.
#[must_use]
pub fn psion_plugin_conditioned_mixed_compact_decoder_reference_config_path() -> PathBuf {
    repo_root().join(PSION_PLUGIN_CONDITIONED_MIXED_COMPACT_DECODER_REFERENCE_CONFIG_REF)
}

fn reference_descriptor(
) -> Result<PsionCompactDecoderDescriptor, PsionPluginConditionedCompactDecoderError> {
    reference_descriptor_with_model_id(PSION_PLUGIN_CONDITIONED_REFERENCE_MODEL_ID)
}

fn reference_descriptor_with_model_id(
    model_id: &str,
) -> Result<PsionCompactDecoderDescriptor, PsionPluginConditionedCompactDecoderError> {
    let mut descriptor = PsionCompactDecoderDescriptor::new(
        PsionCompactDecoderSizeAnchor::Pilot32m,
        PSION_PLUGIN_CONDITIONED_REFERENCE_REVISION,
        PSION_PLUGIN_CONDITIONED_REFERENCE_CONTEXT_TOKENS,
        PsionCompactDecoderTokenizerBinding {
            tokenizer_id: String::from(PSION_PLUGIN_CONDITIONED_REFERENCE_TOKENIZER_ID),
            tokenizer_version: String::from(PSION_PLUGIN_CONDITIONED_REFERENCE_TOKENIZER_VERSION),
            tokenizer_family: PsionCompactDecoderTokenizerFamily::SentencePiece,
            tokenizer_digest: String::from(PSION_PLUGIN_CONDITIONED_REFERENCE_TOKENIZER_DIGEST),
            vocab_size: PSION_PLUGIN_CONDITIONED_REFERENCE_VOCAB_SIZE,
            special_tokens_digest: Some(String::from(
                PSION_PLUGIN_CONDITIONED_REFERENCE_SPECIAL_TOKENS_DIGEST,
            )),
            template_digest: Some(String::from(
                PSION_PLUGIN_CONDITIONED_REFERENCE_TEMPLATE_DIGEST,
            )),
        },
    )?;
    descriptor.model.model_id = String::from(model_id);
    descriptor.validate()?;
    Ok(descriptor)
}

fn stable_config_digest(config: &PsionPluginConditionedCompactDecoderReferenceConfig) -> String {
    let mut canonical = config.clone();
    canonical.config_digest.clear();
    let encoded =
        serde_json::to_vec(&canonical).expect("plugin-conditioned config should serialize");
    let mut hasher = Sha256::new();
    hasher.update(b"psion_plugin_conditioned_compact_decoder_reference|");
    hasher.update(&encoded);
    hex::encode(hasher.finalize())
}

fn ensure_nonempty(
    value: &str,
    field: &str,
) -> Result<(), PsionPluginConditionedCompactDecoderError> {
    if value.trim().is_empty() {
        return Err(PsionPluginConditionedCompactDecoderError::MissingField {
            field: String::from(field),
        });
    }
    Ok(())
}

fn ensure_bool_true(
    value: bool,
    field: &str,
) -> Result<(), PsionPluginConditionedCompactDecoderError> {
    if !value {
        return Err(PsionPluginConditionedCompactDecoderError::FieldMismatch {
            field: String::from(field),
            expected: String::from("true"),
            actual: String::from("false"),
        });
    }
    Ok(())
}

fn ensure_bool_false(
    value: bool,
    field: &str,
) -> Result<(), PsionPluginConditionedCompactDecoderError> {
    if value {
        return Err(PsionPluginConditionedCompactDecoderError::FieldMismatch {
            field: String::from(field),
            expected: String::from("false"),
            actual: String::from("true"),
        });
    }
    Ok(())
}

fn check_string_match(
    actual: &str,
    expected: &str,
    field: &str,
) -> Result<(), PsionPluginConditionedCompactDecoderError> {
    if actual != expected {
        return Err(PsionPluginConditionedCompactDecoderError::FieldMismatch {
            field: String::from(field),
            expected: String::from(expected),
            actual: String::from(actual),
        });
    }
    Ok(())
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace layout should keep `crates/` under the repo root")
        .parent()
        .expect("workspace layout should keep `crates/` two levels below the repo root")
        .to_path_buf()
}

/// Error returned by the plugin-conditioned compact-decoder reference config.
#[derive(Debug, Error)]
pub enum PsionPluginConditionedCompactDecoderError {
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("missing field `{field}`")]
    MissingField { field: String },
    #[error("field `{field}` expected `{expected}`, found `{actual}`")]
    FieldMismatch {
        field: String,
        expected: String,
        actual: String,
    },
    #[error("config digest drifted from the canonical contents")]
    DigestMismatch,
    #[error(transparent)]
    Descriptor(#[from] PsionCompactDecoderError),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[cfg(test)]
mod tests {
    use super::record_psion_plugin_conditioned_compact_decoder_reference_config;
    use crate::PsionPluginConditionedCompactDecoderReferenceConfig;

    fn stage_manifest_fixture() -> crate::PsionPluginConditionedSftStageManifest {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/plugins/training/psion_plugin_conditioned_sft_v1/psion_plugin_conditioned_sft_stage_manifest.json"
        ))
        .expect("plugin-conditioned stage manifest fixture should parse")
    }

    #[test]
    fn plugin_conditioned_compact_decoder_reference_validates(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let stage_manifest = stage_manifest_fixture();
        let config = record_psion_plugin_conditioned_compact_decoder_reference_config(
            &stage_manifest,
            "The first plugin-conditioned compact-decoder config freezes one pilot-sized descriptor, one no-custom-token JSON serialization posture, and lane-bound checkpoint naming.",
        )?;
        config.validate_against_stage(&stage_manifest)?;
        Ok(())
    }

    #[test]
    fn plugin_conditioned_compact_decoder_rejects_custom_plugin_tokens(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let stage_manifest = stage_manifest_fixture();
        let mut config: PsionPluginConditionedCompactDecoderReferenceConfig =
            record_psion_plugin_conditioned_compact_decoder_reference_config(
                &stage_manifest,
                "summary",
            )?;
        config.serialization_contract.custom_plugin_tokens_admitted = true;
        config.config_digest = super::stable_config_digest(&config);
        let error = config
            .validate_against_stage(&stage_manifest)
            .expect_err("custom plugin tokens should be rejected");
        assert!(error.to_string().contains("custom_plugin_tokens_admitted"));
        Ok(())
    }
}
