//! Psion general instruct SFT lane (psionic#1117).
//!
//! Extends the bounded legal SFT machinery into a general instruct lane:
//! an owned, versioned chat template with generation masking (only assistant
//! spans train), a provenance-disciplined instruct corpus contract shaped
//! like the pretraining data manifests, masked-loss LoRA training over the
//! reused open-adapter execution backend, warmup/cosine learning-rate
//! schedules wired through the existing scheduler bindings, and a
//! serialize/restore checkpoint-resume drill over `FixedBudgetTrainingRun`.
//!
//! External reference (read-only, never vendored):
//! `tetherto/qvac-fabric-llm.cpp` `llama-finetune-lora`, which demonstrates
//! the same feature set (assistant-only masked loss, checkpoint/resume,
//! schedulers) on consumer and mobile GPUs.
//!
//! Claim discipline: this is a bounded fixture-scale Psion statistical lane.
//! It makes no instruct-model capability claim and no real training-run
//! claim.

use std::collections::BTreeSet;

use psionic_core::TensorData;
use psionic_data::{TokenizerDigest, TokenizerFamily};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    FixedBudgetTrainingRun, OPEN_ADAPTER_CUDA_BACKEND_LABEL, OpenAdapterAdmissibleModelFamily,
    OpenAdapterArtifactExportRequest, OpenAdapterExecutionConfig, OpenAdapterHiddenStateSample,
    OpenAdapterLmHeadTarget, OpenAdapterPrecisionPolicy, OpenAdapterReferenceModel,
    OpenAdapterSftError, OpenAdapterTrainingExecutionBackend, OpenAdapterTrainingExecutionError,
    TrainingCoreError, TrainingLoopBudget, TrainingOptimizerConfig,
    TrainingOptimizerResidencyPolicy, TrainingParameterGroupState, TrainingSchedulerBinding,
    TrainingSchedulerConfig, TrainingStepReceipt,
};

/// Schema version for the owned Psion chat template contract.
pub const PSION_CHAT_TEMPLATE_SCHEMA_VERSION: &str = "psion.chat_template.v1";
/// Stable identifier of the owned Psion chat template.
pub const PSION_CHAT_TEMPLATE_ID: &str = "psion_chat_template";
/// Current version of the owned Psion chat template.
pub const PSION_CHAT_TEMPLATE_VERSION: &str = "v1";
/// Schema version for the instruct corpus provenance manifest.
pub const PSION_INSTRUCT_CORPUS_MANIFEST_SCHEMA_VERSION: &str = "psion.instruct_corpus_manifest.v1";
/// Schema version for the instruct SFT lane config.
pub const PSION_INSTRUCT_SFT_LANE_CONFIG_SCHEMA_VERSION: &str = "psion.instruct_sft_lane_config.v1";
/// Schema version for the instruct SFT lane report.
pub const PSION_INSTRUCT_SFT_LANE_REPORT_SCHEMA_VERSION: &str = "psion.instruct_sft_lane_report.v1";
/// Stable lane identifier for the bounded instruct SFT lane.
pub const PSION_INSTRUCT_SFT_LANE_ID: &str = "psion_instruct_sft_v1";
/// Checkpoint family recorded by instruct SFT lane runs.
pub const PSION_INSTRUCT_SFT_CHECKPOINT_FAMILY: &str = "psion.instruct_sft_lora";

/// Pretraining reference learning rate the instruct default sits below.
///
/// The recorded Psion pretraining dense-rank optimizer is
/// `TrainingOptimizerConfig::adamw(0.0003, ...)` in
/// `crates/psionic-train/examples/psion_trusted_cluster_run_fixtures.rs`, and
/// the same `3e-4` appears as `effective_learning_rate` in the dense-rank
/// group telemetry of
/// `fixtures/psion/trusted_cluster/psion_trusted_cluster_run_bundle_v1.json`.
pub const PSION_INSTRUCT_SFT_PRETRAINING_REFERENCE_LEARNING_RATE: f32 = 3.0e-4;
/// Default instruct SFT learning rate: ten times below the recorded
/// pretraining reference learning rate.
pub const PSION_INSTRUCT_SFT_DEFAULT_LEARNING_RATE: f32 = 3.0e-5;
/// Maximum admissible instruct learning rate relative to the pretraining
/// reference. Configs above `reference / 5` are refused.
pub const PSION_INSTRUCT_SFT_MAX_LEARNING_RATE_RATIO: f32 = 0.2;

/// Workspace-relative path of the committed chat template fixture.
pub const PSION_CHAT_TEMPLATE_FIXTURE_PATH: &str =
    "fixtures/psion/instruct/psion_chat_template_v1.json";
/// Workspace-relative path of the committed example instruct corpus.
pub const PSION_INSTRUCT_EXAMPLE_CORPUS_FIXTURE_PATH: &str =
    "fixtures/psion/instruct/psion_instruct_example_corpus_v1.jsonl";
/// Workspace-relative path of the committed corpus provenance manifest.
pub const PSION_INSTRUCT_CORPUS_MANIFEST_FIXTURE_PATH: &str =
    "fixtures/psion/instruct/psion_instruct_corpus_manifest_v1.json";
/// Workspace-relative path of the committed token-level generation-mask fixture.
pub const PSION_INSTRUCT_GENERATION_MASK_FIXTURE_PATH: &str =
    "fixtures/psion/instruct/psion_instruct_generation_mask_fixture_v1.json";
/// Workspace-relative path of the committed lane report fixture.
pub const PSION_INSTRUCT_SFT_LANE_REPORT_FIXTURE_PATH: &str =
    "fixtures/psion/instruct/psion_instruct_sft_lane_report_v1.json";

/// Shared claim boundary for every instruct-lane artifact.
pub const PSION_INSTRUCT_SFT_CLAIM_BOUNDARY: &str = "Bounded fixture-scale Psion statistical \
     lane evidence only: deterministic smoke training over repo-owned example records. No \
     general instruct capability claim and no real training-run claim.";

/// Chat roles admitted by the owned Psion chat template.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionChatRole {
    /// System instruction turn.
    System,
    /// Human/user turn.
    User,
    /// Model/assistant turn. Only assistant spans train.
    Assistant,
}

/// Owned, versioned chat template with generation-masking semantics.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionChatTemplate {
    /// Schema version.
    pub schema_version: String,
    /// Stable template identifier.
    pub template_id: String,
    /// Template version.
    pub template_version: String,
    /// Marker token opening a system turn. Never trains.
    pub system_open: String,
    /// Marker token opening a user turn. Never trains.
    pub user_open: String,
    /// Marker token opening an assistant turn. Never trains.
    pub assistant_open: String,
    /// Marker token closing any turn. Trains only after assistant content.
    pub turn_close: String,
    /// End-of-sequence token emitted after the final assistant turn. Trains.
    pub eos_token: String,
    /// Stable digest pinning this template revision.
    pub template_digest: String,
}

impl PsionChatTemplate {
    /// Returns the canonical v1 Psion chat template.
    #[must_use]
    pub fn v1() -> Self {
        let mut template = Self {
            schema_version: String::from(PSION_CHAT_TEMPLATE_SCHEMA_VERSION),
            template_id: String::from(PSION_CHAT_TEMPLATE_ID),
            template_version: String::from(PSION_CHAT_TEMPLATE_VERSION),
            system_open: String::from("<|psion_system|>"),
            user_open: String::from("<|psion_user|>"),
            assistant_open: String::from("<|psion_assistant|>"),
            turn_close: String::from("<|psion_end|>"),
            eos_token: String::from("<|psion_eos|>"),
            template_digest: String::new(),
        };
        template.template_digest = template.stable_digest();
        template
    }

    /// Returns all marker tokens owned by this template.
    #[must_use]
    pub fn markers(&self) -> [&str; 5] {
        [
            self.system_open.as_str(),
            self.user_open.as_str(),
            self.assistant_open.as_str(),
            self.turn_close.as_str(),
            self.eos_token.as_str(),
        ]
    }

    /// Returns the role-open marker for one role.
    #[must_use]
    pub fn role_open(&self, role: PsionChatRole) -> &str {
        match role {
            PsionChatRole::System => self.system_open.as_str(),
            PsionChatRole::User => self.user_open.as_str(),
            PsionChatRole::Assistant => self.assistant_open.as_str(),
        }
    }

    /// Returns a stable digest over the template contract, excluding the
    /// pinned digest field itself.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(b"psion_chat_template|");
        hasher.update(self.schema_version.as_bytes());
        hasher.update(b"|");
        hasher.update(self.template_id.as_bytes());
        hasher.update(b"|");
        hasher.update(self.template_version.as_bytes());
        for marker in self.markers() {
            hasher.update(b"|marker|");
            hasher.update(marker.as_bytes());
        }
        format!("sha256:{:x}", hasher.finalize())
    }

    /// Validates the template contract and digest pin.
    pub fn validate(&self) -> Result<(), PsionInstructSftError> {
        if self.schema_version != PSION_CHAT_TEMPLATE_SCHEMA_VERSION {
            return Err(PsionInstructSftError::InvalidTemplate {
                detail: format!(
                    "schema_version `{}` is not `{PSION_CHAT_TEMPLATE_SCHEMA_VERSION}`",
                    self.schema_version
                ),
            });
        }
        if self.template_id.trim().is_empty() || self.template_version.trim().is_empty() {
            return Err(PsionInstructSftError::InvalidTemplate {
                detail: String::from("template_id and template_version must be non-empty"),
            });
        }
        let mut seen = BTreeSet::new();
        for marker in self.markers() {
            if marker.trim().is_empty() {
                return Err(PsionInstructSftError::InvalidTemplate {
                    detail: String::from("template markers must be non-empty"),
                });
            }
            if marker.chars().any(char::is_whitespace) {
                return Err(PsionInstructSftError::InvalidTemplate {
                    detail: format!("template marker `{marker}` must not contain whitespace"),
                });
            }
            if !seen.insert(marker) {
                return Err(PsionInstructSftError::InvalidTemplate {
                    detail: format!("template marker `{marker}` is duplicated"),
                });
            }
        }
        if self.template_digest != self.stable_digest() {
            return Err(PsionInstructSftError::InvalidTemplate {
                detail: String::from("template_digest does not match the template contents"),
            });
        }
        Ok(())
    }

    /// Renders one conversation into template tokens with a generation mask.
    ///
    /// Masking semantics: role-open markers never train; system and user
    /// content never trains; assistant content tokens train; the turn-close
    /// after assistant content trains; the final end-of-sequence token
    /// trains. A conversation that does not end with an assistant turn is
    /// refused so the trainable end-of-sequence token can never be silently
    /// dropped.
    pub fn render_conversation(
        &self,
        conversation_id: &str,
        messages: &[PsionChatMessage],
    ) -> Result<PsionRenderedConversation, PsionInstructSftError> {
        self.validate()?;
        if conversation_id.trim().is_empty() {
            return Err(PsionInstructSftError::InvalidConversation {
                conversation_id: conversation_id.to_string(),
                detail: String::from("conversation_id must be non-empty"),
            });
        }
        if messages.is_empty() {
            return Err(PsionInstructSftError::InvalidConversation {
                conversation_id: conversation_id.to_string(),
                detail: String::from("conversation must contain at least one message"),
            });
        }
        if !messages
            .iter()
            .any(|message| message.role == PsionChatRole::User)
        {
            return Err(PsionInstructSftError::InvalidConversation {
                conversation_id: conversation_id.to_string(),
                detail: String::from("conversation must contain at least one user turn"),
            });
        }
        if messages
            .last()
            .is_none_or(|message| message.role != PsionChatRole::Assistant)
        {
            return Err(PsionInstructSftError::MissingFinalAssistantTurn {
                conversation_id: conversation_id.to_string(),
            });
        }
        let mut tokens = Vec::new();
        for (index, message) in messages.iter().enumerate() {
            if message.role == PsionChatRole::System && index != 0 {
                return Err(PsionInstructSftError::InvalidConversation {
                    conversation_id: conversation_id.to_string(),
                    detail: format!("system turn at index {index} must be the first message"),
                });
            }
            let content_tokens: Vec<&str> = message.content.split_whitespace().collect();
            if content_tokens.is_empty() {
                return Err(PsionInstructSftError::InvalidConversation {
                    conversation_id: conversation_id.to_string(),
                    detail: format!("message at index {index} has empty content"),
                });
            }
            for marker in self.markers() {
                if message.content.contains(marker) {
                    return Err(PsionInstructSftError::RoleBleed {
                        conversation_id: conversation_id.to_string(),
                        message_index: index,
                        marker: marker.to_string(),
                    });
                }
            }
            let assistant = message.role == PsionChatRole::Assistant;
            tokens.push(PsionTemplateToken {
                text: self.role_open(message.role).to_string(),
                role: message.role,
                segment: PsionTemplateSegment::RoleOpen,
                trains: false,
            });
            for content_token in content_tokens {
                tokens.push(PsionTemplateToken {
                    text: content_token.to_string(),
                    role: message.role,
                    segment: PsionTemplateSegment::Content,
                    trains: assistant,
                });
            }
            tokens.push(PsionTemplateToken {
                text: self.turn_close.clone(),
                role: message.role,
                segment: PsionTemplateSegment::TurnClose,
                trains: assistant,
            });
        }
        tokens.push(PsionTemplateToken {
            text: self.eos_token.clone(),
            role: PsionChatRole::Assistant,
            segment: PsionTemplateSegment::Eos,
            trains: true,
        });

        let loss_mask: Vec<u8> = tokens.iter().map(|token| u8::from(token.trains)).collect();
        let trainable_token_count = loss_mask.iter().filter(|flag| **flag == 1).count();
        let masked_token_count = loss_mask.len() - trainable_token_count;
        let rendered_text = tokens
            .iter()
            .map(|token| token.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        let mut hasher = Sha256::new();
        hasher.update(b"psion_generation_mask|");
        hasher.update(self.template_digest.as_bytes());
        hasher.update(b"|");
        hasher.update(conversation_id.as_bytes());
        for (token, flag) in tokens.iter().zip(loss_mask.iter()) {
            hasher.update(b"|tok|");
            hasher.update(token.text.as_bytes());
            hasher.update(b"|");
            hasher.update([*flag]);
        }
        let mask_digest = format!("sha256:{:x}", hasher.finalize());
        Ok(PsionRenderedConversation {
            conversation_id: conversation_id.to_string(),
            template_digest: self.template_digest.clone(),
            tokens,
            loss_mask,
            trainable_token_count,
            masked_token_count,
            rendered_text,
            mask_digest,
        })
    }
}

/// One chat message inside an instruct record.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionChatMessage {
    /// Message role.
    pub role: PsionChatRole,
    /// Message content text.
    pub content: String,
}

/// Template segment classification for one rendered token.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionTemplateSegment {
    /// Role-open marker token.
    RoleOpen,
    /// Message content token.
    Content,
    /// Turn-close marker token.
    TurnClose,
    /// Final end-of-sequence token.
    Eos,
}

/// One rendered template token with its generation-mask decision.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionTemplateToken {
    /// Token text.
    pub text: String,
    /// Owning role.
    pub role: PsionChatRole,
    /// Segment kind.
    pub segment: PsionTemplateSegment,
    /// Whether this token contributes to the training loss.
    pub trains: bool,
}

/// One conversation rendered through the owned chat template.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionRenderedConversation {
    /// Stable conversation identifier.
    pub conversation_id: String,
    /// Digest of the template used to render.
    pub template_digest: String,
    /// Rendered tokens in order.
    pub tokens: Vec<PsionTemplateToken>,
    /// Loss mask aligned with `tokens` (1 trains, 0 masked).
    pub loss_mask: Vec<u8>,
    /// Number of trainable tokens.
    pub trainable_token_count: usize,
    /// Number of masked tokens.
    pub masked_token_count: usize,
    /// Space-joined rendered text.
    pub rendered_text: String,
    /// Stable digest over tokens and mask.
    pub mask_digest: String,
}

/// One provenance-disciplined instruct corpus record.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionInstructCorpusRecord {
    /// Stable record identifier.
    pub record_id: String,
    /// Source identifier, mirroring the pretraining admission manifests.
    pub source_id: String,
    /// Source family identifier.
    pub source_family_id: String,
    /// Rights posture for the record content.
    pub rights_posture: String,
    /// Canonical reference for the source material.
    pub canonical_reference: String,
    /// SHA-256 digest over the canonical message JSON.
    pub content_digest: String,
    /// Conversation messages.
    pub messages: Vec<PsionChatMessage>,
}

impl PsionInstructCorpusRecord {
    /// Computes the canonical content digest over the message list.
    pub fn compute_content_digest(
        messages: &[PsionChatMessage],
    ) -> Result<String, PsionInstructSftError> {
        let payload =
            serde_json::to_vec(messages).map_err(|error| PsionInstructSftError::Serialization {
                detail: error.to_string(),
            })?;
        let mut hasher = Sha256::new();
        hasher.update(b"psion_instruct_record|");
        hasher.update(payload.as_slice());
        Ok(format!("sha256:{:x}", hasher.finalize()))
    }

    /// Validates provenance fields, the pinned content digest, and that the
    /// record renders cleanly through the provided template.
    pub fn validate(
        &self,
        template: &PsionChatTemplate,
    ) -> Result<PsionRenderedConversation, PsionInstructSftError> {
        for (field, value) in [
            ("record_id", self.record_id.as_str()),
            ("source_id", self.source_id.as_str()),
            ("source_family_id", self.source_family_id.as_str()),
            ("rights_posture", self.rights_posture.as_str()),
            ("canonical_reference", self.canonical_reference.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(PsionInstructSftError::InvalidRecord {
                    record_id: self.record_id.clone(),
                    detail: format!("{field} must be non-empty"),
                });
            }
        }
        let expected = Self::compute_content_digest(&self.messages)?;
        if self.content_digest != expected {
            return Err(PsionInstructSftError::InvalidRecord {
                record_id: self.record_id.clone(),
                detail: String::from("content_digest does not match the message contents"),
            });
        }
        template.render_conversation(self.record_id.as_str(), &self.messages)
    }
}

/// Workspace-relative committed artifact reference, mirroring the shape used
/// by the pretraining recipe bundle (`PsionActualPretrainingArtifactRef`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionInstructArtifactRef {
    /// Workspace-relative path to the committed artifact.
    pub path: String,
    /// SHA-256 of the committed artifact contents.
    pub sha256: String,
}

/// Per-record provenance row carried by the corpus manifest.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionInstructCorpusRecordProvenance {
    /// Stable record identifier.
    pub record_id: String,
    /// Source identifier.
    pub source_id: String,
    /// Source family identifier.
    pub source_family_id: String,
    /// Rights posture.
    pub rights_posture: String,
    /// Canonical reference.
    pub canonical_reference: String,
    /// Record content digest.
    pub content_digest: String,
    /// Generation-mask digest from the canonical template render.
    pub mask_digest: String,
    /// Trainable token count under the pinned template.
    pub trainable_token_count: usize,
    /// Masked token count under the pinned template.
    pub masked_token_count: usize,
}

/// Provenance manifest for one versioned instruct corpus.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionInstructCorpusManifest {
    /// Schema version.
    pub schema_version: String,
    /// Stable corpus identifier.
    pub corpus_id: String,
    /// Corpus version.
    pub corpus_version: String,
    /// Dataset identity string, mirroring the pretraining bundles.
    pub dataset_identity: String,
    /// Whether this corpus is a clearly labeled repo-owned example.
    pub example_corpus: bool,
    /// Template identifier the corpus is rendered against.
    pub template_id: String,
    /// Template version.
    pub template_version: String,
    /// Pinned template digest.
    pub template_digest: String,
    /// Committed corpus JSONL artifact reference.
    pub corpus_jsonl: PsionInstructArtifactRef,
    /// Record count.
    pub record_count: usize,
    /// Total trainable tokens across all records.
    pub total_trainable_tokens: u64,
    /// Total masked tokens across all records.
    pub total_masked_tokens: u64,
    /// Per-record provenance rows.
    pub records: Vec<PsionInstructCorpusRecordProvenance>,
    /// Claim boundary for the manifest.
    pub claim_boundary: String,
    /// Stable manifest digest.
    pub manifest_digest: String,
}

impl PsionInstructCorpusManifest {
    /// Computes the stable manifest digest, excluding the pinned digest field.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.manifest_digest.clear();
        stable_json_digest(b"psion_instruct_corpus_manifest|", &clone)
    }

    /// Validates manifest invariants and the pinned digest.
    pub fn validate(&self) -> Result<(), PsionInstructSftError> {
        if self.schema_version != PSION_INSTRUCT_CORPUS_MANIFEST_SCHEMA_VERSION {
            return Err(PsionInstructSftError::InvalidManifest {
                detail: format!(
                    "schema_version `{}` is not `{PSION_INSTRUCT_CORPUS_MANIFEST_SCHEMA_VERSION}`",
                    self.schema_version
                ),
            });
        }
        for (field, value) in [
            ("corpus_id", self.corpus_id.as_str()),
            ("corpus_version", self.corpus_version.as_str()),
            ("dataset_identity", self.dataset_identity.as_str()),
            ("template_id", self.template_id.as_str()),
            ("template_digest", self.template_digest.as_str()),
            ("corpus_jsonl.path", self.corpus_jsonl.path.as_str()),
            ("corpus_jsonl.sha256", self.corpus_jsonl.sha256.as_str()),
            ("claim_boundary", self.claim_boundary.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(PsionInstructSftError::InvalidManifest {
                    detail: format!("{field} must be non-empty"),
                });
            }
        }
        if self.records.is_empty() || self.record_count != self.records.len() {
            return Err(PsionInstructSftError::InvalidManifest {
                detail: String::from("record_count must match a non-empty records list"),
            });
        }
        let mut record_ids = BTreeSet::new();
        let mut trainable_total = 0u64;
        let mut masked_total = 0u64;
        for record in &self.records {
            if !record_ids.insert(record.record_id.as_str()) {
                return Err(PsionInstructSftError::InvalidManifest {
                    detail: format!("duplicate record_id `{}`", record.record_id),
                });
            }
            if !record.content_digest.starts_with("sha256:") {
                return Err(PsionInstructSftError::InvalidManifest {
                    detail: format!(
                        "record `{}` content_digest must be sha256-prefixed",
                        record.record_id
                    ),
                });
            }
            if record.trainable_token_count == 0 {
                return Err(PsionInstructSftError::InvalidManifest {
                    detail: format!(
                        "record `{}` must carry at least one trainable token",
                        record.record_id
                    ),
                });
            }
            trainable_total += record.trainable_token_count as u64;
            masked_total += record.masked_token_count as u64;
        }
        if trainable_total != self.total_trainable_tokens
            || masked_total != self.total_masked_tokens
        {
            return Err(PsionInstructSftError::InvalidManifest {
                detail: String::from("token totals do not match the per-record provenance rows"),
            });
        }
        if self.manifest_digest != self.stable_digest() {
            return Err(PsionInstructSftError::InvalidManifest {
                detail: String::from("manifest_digest does not match the manifest contents"),
            });
        }
        Ok(())
    }
}

/// Builds the provenance manifest for one corpus from its validated records.
pub fn build_psion_instruct_corpus_manifest(
    template: &PsionChatTemplate,
    corpus_id: &str,
    corpus_version: &str,
    example_corpus: bool,
    corpus_jsonl: PsionInstructArtifactRef,
    records: &[PsionInstructCorpusRecord],
) -> Result<PsionInstructCorpusManifest, PsionInstructSftError> {
    if records.is_empty() {
        return Err(PsionInstructSftError::InvalidManifest {
            detail: String::from("corpus must contain at least one record"),
        });
    }
    let mut rows = Vec::new();
    let mut trainable_total = 0u64;
    let mut masked_total = 0u64;
    for record in records {
        let rendered = record.validate(template)?;
        trainable_total += rendered.trainable_token_count as u64;
        masked_total += rendered.masked_token_count as u64;
        rows.push(PsionInstructCorpusRecordProvenance {
            record_id: record.record_id.clone(),
            source_id: record.source_id.clone(),
            source_family_id: record.source_family_id.clone(),
            rights_posture: record.rights_posture.clone(),
            canonical_reference: record.canonical_reference.clone(),
            content_digest: record.content_digest.clone(),
            mask_digest: rendered.mask_digest,
            trainable_token_count: rendered.trainable_token_count,
            masked_token_count: rendered.masked_token_count,
        });
    }
    let mut manifest = PsionInstructCorpusManifest {
        schema_version: String::from(PSION_INSTRUCT_CORPUS_MANIFEST_SCHEMA_VERSION),
        corpus_id: corpus_id.to_string(),
        corpus_version: corpus_version.to_string(),
        dataset_identity: format!("{corpus_id}@{corpus_version}"),
        example_corpus,
        template_id: template.template_id.clone(),
        template_version: template.template_version.clone(),
        template_digest: template.template_digest.clone(),
        corpus_jsonl,
        record_count: rows.len(),
        total_trainable_tokens: trainable_total,
        total_masked_tokens: masked_total,
        records: rows,
        claim_boundary: String::from(PSION_INSTRUCT_SFT_CLAIM_BOUNDARY),
        manifest_digest: String::new(),
    };
    manifest.manifest_digest = manifest.stable_digest();
    manifest.validate()?;
    Ok(manifest)
}

/// Optimizer family admitted by the bounded instruct lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionInstructSftOptimizerKind {
    /// AdamW with the lane's fixed beta/epsilon defaults.
    AdamW,
    /// Plain SGD without momentum.
    Sgd,
}

/// Config for one bounded instruct SFT lane run.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionInstructSftLaneConfig {
    /// Schema version.
    pub schema_version: String,
    /// Lane identifier.
    pub lane_id: String,
    /// Stable run identifier.
    pub run_id: String,
    /// Base model identifier for the deterministic smoke path.
    pub base_model: String,
    /// Base model revision.
    pub base_model_revision: String,
    /// Digest of the frozen synthetic base fixture.
    pub base_served_artifact_digest: String,
    /// Tokenizer digest.
    pub tokenizer_digest: String,
    /// Owned chat template, digest-pinned.
    pub template: PsionChatTemplate,
    /// Digest of the corpus provenance manifest feeding this run.
    pub corpus_manifest_digest: String,
    /// Dataset reference recorded in receipts.
    pub dataset_ref: String,
    /// Validator policy reference recorded in receipts.
    pub validator_policy_ref: String,
    /// Adapter identifier.
    pub adapter_id: String,
    /// Adapter revision.
    pub adapter_revision: String,
    /// Synthetic hidden width.
    pub hidden_size: usize,
    /// Synthetic vocabulary width.
    pub vocab_size: usize,
    /// LoRA rank.
    pub lora_rank: usize,
    /// LoRA alpha.
    pub lora_alpha: f32,
    /// Base learning rate. Must sit well below the pretraining reference.
    pub learning_rate: f32,
    /// Recorded pretraining reference learning rate.
    pub pretraining_reference_learning_rate: f32,
    /// Learning-rate schedule reusing the existing scheduler contract.
    pub scheduler: TrainingSchedulerConfig,
    /// Optimizer family.
    pub optimizer: PsionInstructSftOptimizerKind,
    /// Fixed step budget.
    pub max_steps: u64,
    /// Batch size.
    pub batch_size: usize,
    /// Step index after which the resume drill checkpoints and restores.
    pub checkpoint_at_step: u64,
    /// Logical start timestamp.
    pub started_at_ms: u64,
    /// Logical duration per step.
    pub step_duration_ms: u64,
    /// Claim boundary.
    pub claim_boundary: String,
}

impl PsionInstructSftLaneConfig {
    /// Validates the lane config.
    pub fn validate(&self) -> Result<(), PsionInstructSftError> {
        if self.schema_version != PSION_INSTRUCT_SFT_LANE_CONFIG_SCHEMA_VERSION {
            return Err(PsionInstructSftError::InvalidConfig {
                detail: format!(
                    "schema_version `{}` is not `{PSION_INSTRUCT_SFT_LANE_CONFIG_SCHEMA_VERSION}`",
                    self.schema_version
                ),
            });
        }
        for (field, value) in [
            ("lane_id", self.lane_id.as_str()),
            ("run_id", self.run_id.as_str()),
            ("base_model", self.base_model.as_str()),
            ("base_model_revision", self.base_model_revision.as_str()),
            (
                "base_served_artifact_digest",
                self.base_served_artifact_digest.as_str(),
            ),
            ("tokenizer_digest", self.tokenizer_digest.as_str()),
            (
                "corpus_manifest_digest",
                self.corpus_manifest_digest.as_str(),
            ),
            ("dataset_ref", self.dataset_ref.as_str()),
            ("validator_policy_ref", self.validator_policy_ref.as_str()),
            ("adapter_id", self.adapter_id.as_str()),
            ("adapter_revision", self.adapter_revision.as_str()),
            ("claim_boundary", self.claim_boundary.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(PsionInstructSftError::InvalidConfig {
                    detail: format!("{field} must be non-empty"),
                });
            }
        }
        self.template.validate()?;
        if self.hidden_size == 0 || self.vocab_size == 0 {
            return Err(PsionInstructSftError::InvalidConfig {
                detail: String::from("hidden_size and vocab_size must be non-zero"),
            });
        }
        if self.lora_rank == 0 || !self.lora_alpha.is_finite() || self.lora_alpha <= 0.0 {
            return Err(PsionInstructSftError::InvalidConfig {
                detail: String::from("lora_rank and lora_alpha must be positive"),
            });
        }
        if !self.learning_rate.is_finite() || self.learning_rate <= 0.0 {
            return Err(PsionInstructSftError::InvalidConfig {
                detail: String::from("learning_rate must be positive"),
            });
        }
        if !self.pretraining_reference_learning_rate.is_finite()
            || self.pretraining_reference_learning_rate <= 0.0
        {
            return Err(PsionInstructSftError::InvalidConfig {
                detail: String::from("pretraining_reference_learning_rate must be positive"),
            });
        }
        let ratio = self.learning_rate / self.pretraining_reference_learning_rate;
        if ratio > PSION_INSTRUCT_SFT_MAX_LEARNING_RATE_RATIO {
            return Err(PsionInstructSftError::InvalidConfig {
                detail: format!(
                    "learning_rate ratio {ratio:.4} exceeds the instruct ceiling of {PSION_INSTRUCT_SFT_MAX_LEARNING_RATE_RATIO} relative to the pretraining reference",
                ),
            });
        }
        if self.max_steps == 0 || self.batch_size == 0 || self.step_duration_ms == 0 {
            return Err(PsionInstructSftError::InvalidConfig {
                detail: String::from(
                    "max_steps, batch_size, and step_duration_ms must be non-zero",
                ),
            });
        }
        if self.checkpoint_at_step == 0 || self.checkpoint_at_step >= self.max_steps {
            return Err(PsionInstructSftError::InvalidConfig {
                detail: String::from(
                    "checkpoint_at_step must split the run: 0 < checkpoint_at_step < max_steps",
                ),
            });
        }
        Ok(())
    }
}

/// Returns the default bounded instruct SFT lane config.
#[must_use]
pub fn default_psion_instruct_sft_lane_config(
    corpus_manifest_digest: &str,
) -> PsionInstructSftLaneConfig {
    let max_steps = 8;
    PsionInstructSftLaneConfig {
        schema_version: String::from(PSION_INSTRUCT_SFT_LANE_CONFIG_SCHEMA_VERSION),
        lane_id: String::from(PSION_INSTRUCT_SFT_LANE_ID),
        run_id: String::from("psion-instruct-sft-smoke-001"),
        base_model: String::from("psion-instruct-synthetic-base"),
        base_model_revision: String::from("fixture-v1"),
        base_served_artifact_digest: String::from("sha256:psion-instruct-synthetic-base-smoke"),
        tokenizer_digest: String::from("sha256:psion-instruct-template-tokenizer-smoke"),
        template: PsionChatTemplate::v1(),
        corpus_manifest_digest: corpus_manifest_digest.to_string(),
        dataset_ref: String::from("fixture://psion_instruct_example_corpus@v1"),
        validator_policy_ref: String::from("policy://psion-instruct-sft-smoke/v1"),
        adapter_id: String::from("psion-instruct-sft-smoke-adapter"),
        adapter_revision: String::from("rev-001"),
        hidden_size: 16,
        vocab_size: 64,
        lora_rank: 4,
        lora_alpha: 8.0,
        learning_rate: PSION_INSTRUCT_SFT_DEFAULT_LEARNING_RATE,
        pretraining_reference_learning_rate: PSION_INSTRUCT_SFT_PRETRAINING_REFERENCE_LEARNING_RATE,
        scheduler: TrainingSchedulerConfig::CosineAnnealing {
            total_steps: max_steps,
            min_learning_rate: PSION_INSTRUCT_SFT_DEFAULT_LEARNING_RATE / 10.0,
        },
        optimizer: PsionInstructSftOptimizerKind::AdamW,
        max_steps,
        batch_size: 4,
        checkpoint_at_step: 3,
        started_at_ms: 1_780_000_000_000,
        step_duration_ms: 250,
        claim_boundary: String::from(PSION_INSTRUCT_SFT_CLAIM_BOUNDARY),
    }
}

/// Derives masked-loss supervision samples from validated corpus records.
///
/// Only trainable tokens under the generation mask become supervision
/// samples; system and user spans contribute nothing to the loss by
/// construction. Hidden states and target token ids are deterministic
/// functions of the template digest, record id, and token context, keeping
/// the bounded lane replay-safe.
pub fn derive_psion_instruct_supervision_samples(
    template: &PsionChatTemplate,
    records: &[PsionInstructCorpusRecord],
    hidden_size: usize,
    vocab_size: usize,
) -> Result<Vec<OpenAdapterHiddenStateSample>, PsionInstructSftError> {
    if hidden_size == 0 || vocab_size == 0 {
        return Err(PsionInstructSftError::InvalidConfig {
            detail: String::from("hidden_size and vocab_size must be non-zero"),
        });
    }
    let mut samples = Vec::new();
    for record in records {
        let rendered = record.validate(template)?;
        for (index, token) in rendered.tokens.iter().enumerate() {
            if !token.trains {
                continue;
            }
            let context: Vec<&str> = rendered.tokens[..index]
                .iter()
                .map(|context_token| context_token.text.as_str())
                .collect();
            let seed = format!(
                "{}|{}|{}|{}",
                template.template_digest,
                record.record_id,
                index,
                context.join(" "),
            );
            let hidden_state = deterministic_unit_values(seed.as_str(), hidden_size);
            let target_token_id = deterministic_token_id(token.text.as_str(), vocab_size);
            samples.push(OpenAdapterHiddenStateSample::new(
                format!("{}-tok-{index:04}", record.record_id),
                hidden_state,
                target_token_id,
                index.max(1) as u32,
            )?);
        }
    }
    if samples.is_empty() {
        return Err(PsionInstructSftError::InvalidConfig {
            detail: String::from("corpus produced no trainable supervision samples"),
        });
    }
    Ok(samples)
}

/// One per-step scheduled learning-rate row recorded from applied telemetry.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionInstructSftScheduleRow {
    /// One-based global step.
    pub step: u64,
    /// Effective learning rate applied to the LoRA groups at this step.
    pub effective_learning_rate: f32,
}

/// Checkpoint/resume drill outcome for one lane run.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionInstructSftResumeDrillReport {
    /// Step count completed before the checkpoint was taken.
    pub checkpoint_at_step: u64,
    /// Stable digest over the serialized run state checkpoint.
    pub checkpoint_state_digest: String,
    /// Steps completed after restoring from the checkpoint.
    pub resumed_steps: u64,
    /// Whether the resumed run reproduced the uninterrupted parameters
    /// bit-exactly.
    pub resume_bit_exact: bool,
    /// Digest over the uninterrupted run's final LoRA parameters.
    pub uninterrupted_final_parameter_digest: String,
    /// Digest over the resumed run's final LoRA parameters.
    pub resumed_final_parameter_digest: String,
    /// Whether post-resume step receipt digests match the uninterrupted run.
    pub post_resume_receipt_digests_match: bool,
}

/// Machine-readable report for one bounded instruct SFT lane run.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionInstructSftLaneReport {
    /// Schema version.
    pub schema_version: String,
    /// Lane identifier.
    pub lane_id: String,
    /// Run identifier.
    pub run_id: String,
    /// Pinned template digest.
    pub template_digest: String,
    /// Corpus provenance manifest digest.
    pub corpus_manifest_digest: String,
    /// Base learning rate.
    pub learning_rate: f32,
    /// Recorded pretraining reference learning rate.
    pub pretraining_reference_learning_rate: f32,
    /// Ratio of instruct to pretraining learning rate.
    pub learning_rate_ratio: f32,
    /// Scheduler family label.
    pub scheduler_kind: String,
    /// Optimizer family.
    pub optimizer: PsionInstructSftOptimizerKind,
    /// Masked-loss supervision sample count (trainable tokens only).
    pub sample_count: usize,
    /// Trainable token count across the corpus.
    pub trainable_token_count: u64,
    /// Masked token count across the corpus.
    pub masked_token_count: u64,
    /// Completed steps.
    pub completed_steps: u64,
    /// First-step loss (first batch, before any update touched it).
    pub initial_loss: f32,
    /// Final-step loss (last scheduled batch).
    pub final_loss: f32,
    /// Loss recomputed on the first batch after the full bounded run.
    pub final_first_batch_loss: f32,
    /// Whether the first batch's loss improved across the bounded run.
    pub loss_improved: bool,
    /// Applied per-step learning-rate trajectory from group telemetry.
    pub learning_rate_schedule: Vec<PsionInstructSftScheduleRow>,
    /// Step receipt digests.
    pub step_receipt_digests: Vec<String>,
    /// Exported adapter artifact digest.
    pub adapter_artifact_digest: String,
    /// Exported adapter identity digest.
    pub adapter_identity_digest: String,
    /// Checkpoint/resume drill outcome.
    pub resume_drill: PsionInstructSftResumeDrillReport,
    /// Claim boundary.
    pub claim_boundary: String,
    /// Stable report digest.
    pub report_digest: String,
}

impl PsionInstructSftLaneReport {
    /// Computes the stable report digest, excluding the pinned digest field.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.report_digest.clear();
        stable_json_digest(b"psion_instruct_sft_lane_report|", &clone)
    }
}

/// Errors produced by the instruct SFT lane.
#[derive(Debug, Error)]
pub enum PsionInstructSftError {
    /// Template contract violation.
    #[error("invalid psion chat template: {detail}")]
    InvalidTemplate {
        /// Failure detail.
        detail: String,
    },
    /// Conversation shape violation.
    #[error("invalid conversation `{conversation_id}`: {detail}")]
    InvalidConversation {
        /// Conversation identifier.
        conversation_id: String,
        /// Failure detail.
        detail: String,
    },
    /// Conversation does not end with an assistant turn, so the trainable
    /// end-of-sequence token would be silently dropped.
    #[error(
        "conversation `{conversation_id}` does not end with an assistant turn; refusing to render without a trainable end-of-sequence token"
    )]
    MissingFinalAssistantTurn {
        /// Conversation identifier.
        conversation_id: String,
    },
    /// Message content contains a template marker, which would corrupt the
    /// generation mask across role boundaries.
    #[error(
        "conversation `{conversation_id}` message {message_index} contains template marker `{marker}`; refusing role bleed into the generation mask"
    )]
    RoleBleed {
        /// Conversation identifier.
        conversation_id: String,
        /// Offending message index.
        message_index: usize,
        /// Marker found inside content.
        marker: String,
    },
    /// Corpus record violation.
    #[error("invalid instruct corpus record `{record_id}`: {detail}")]
    InvalidRecord {
        /// Record identifier.
        record_id: String,
        /// Failure detail.
        detail: String,
    },
    /// Corpus manifest violation.
    #[error("invalid instruct corpus manifest: {detail}")]
    InvalidManifest {
        /// Failure detail.
        detail: String,
    },
    /// Lane config violation.
    #[error("invalid instruct sft lane config: {detail}")]
    InvalidConfig {
        /// Failure detail.
        detail: String,
    },
    /// JSON serialization failure.
    #[error("instruct sft serialization failed: {detail}")]
    Serialization {
        /// Failure detail.
        detail: String,
    },
    /// Core training loop failure.
    #[error(transparent)]
    TrainingCore(#[from] TrainingCoreError),
    /// Open adapter execution failure.
    #[error(transparent)]
    Execution(#[from] OpenAdapterTrainingExecutionError),
    /// Open adapter SFT export failure.
    #[error(transparent)]
    SftExport(#[from] OpenAdapterSftError),
}

/// Runs the bounded instruct SFT lane: masked-loss LoRA training with the
/// scheduler wired into the reused parameter-group bindings, adapter export,
/// and a serialize/restore checkpoint-resume drill.
pub fn run_psion_instruct_sft_lane(
    config: &PsionInstructSftLaneConfig,
    records: &[PsionInstructCorpusRecord],
) -> Result<PsionInstructSftLaneReport, PsionInstructSftError> {
    config.validate()?;
    let samples = derive_psion_instruct_supervision_samples(
        &config.template,
        records,
        config.hidden_size,
        config.vocab_size,
    )?;
    let sample_count = samples.len();
    let mut trainable_total = 0u64;
    let mut masked_total = 0u64;
    for record in records {
        let rendered = record.validate(&config.template)?;
        trainable_total += rendered.trainable_token_count as u64;
        masked_total += rendered.masked_token_count as u64;
    }

    let backend = build_instruct_backend(config, samples)?;

    // Uninterrupted run with the scheduler wired into every LoRA group.
    let mut run = initialize_scheduled_run(&backend, config)?;
    let receipts = apply_steps(&backend, config, &mut run, 0, config.max_steps)?;
    let final_parameter_digest = run_parameter_digest(&backend, &run)?;
    let exported = backend.export_run_artifact(
        &run,
        &OpenAdapterArtifactExportRequest::new(
            config.dataset_ref.clone(),
            config.validator_policy_ref.clone(),
            config.adapter_id.clone(),
            config.adapter_revision.clone(),
        ),
    )?;

    // Checkpoint/resume drill: train to the checkpoint, serialize the full
    // run state (parameters, optimizer moments, scheduler state, completed
    // steps), restore it, finish the budget, and require bit-exact agreement
    // with the uninterrupted run.
    let mut segment_run = initialize_scheduled_run(&backend, config)?;
    apply_steps(
        &backend,
        config,
        &mut segment_run,
        0,
        config.checkpoint_at_step,
    )?;
    let checkpoint_bytes =
        serde_json::to_vec(&segment_run).map_err(|error| PsionInstructSftError::Serialization {
            detail: error.to_string(),
        })?;
    let checkpoint_state_digest = format!("sha256:{:x}", {
        let mut hasher = Sha256::new();
        hasher.update(b"psion_instruct_sft_checkpoint|");
        hasher.update(checkpoint_bytes.as_slice());
        hasher.finalize()
    });
    drop(segment_run);
    let mut resumed_run: FixedBudgetTrainingRun =
        serde_json::from_slice(checkpoint_bytes.as_slice()).map_err(|error| {
            PsionInstructSftError::Serialization {
                detail: error.to_string(),
            }
        })?;
    let resumed_receipts = apply_steps(
        &backend,
        config,
        &mut resumed_run,
        config.checkpoint_at_step,
        config.max_steps,
    )?;
    let resumed_parameter_digest = run_parameter_digest(&backend, &resumed_run)?;
    let resume_bit_exact = run_parameters_bit_equal(&backend, &run, &resumed_run)?;
    let post_resume_receipt_digests_match = receipts[config.checkpoint_at_step as usize..]
        .iter()
        .map(|receipt| receipt.receipt_digest.as_str())
        .eq(resumed_receipts
            .iter()
            .map(|receipt| receipt.receipt_digest.as_str()));

    let initial_loss = receipts
        .first()
        .map(|receipt| receipt.loss)
        .unwrap_or_default();
    let final_loss = receipts
        .last()
        .map(|receipt| receipt.loss)
        .unwrap_or_default();
    // Compare like with like: recompute the first batch's loss against the
    // trained parameters instead of comparing losses across different batches.
    let final_first_batch_loss = backend.produce_gradient_batch(&run, 0)?.training_batch.loss;
    let learning_rate_schedule = receipts
        .iter()
        .map(|receipt| PsionInstructSftScheduleRow {
            step: receipt.schedule.global_step,
            effective_learning_rate: receipt
                .group_telemetry
                .first()
                .map(|telemetry| telemetry.effective_learning_rate)
                .unwrap_or_default(),
        })
        .collect();

    let mut report = PsionInstructSftLaneReport {
        schema_version: String::from(PSION_INSTRUCT_SFT_LANE_REPORT_SCHEMA_VERSION),
        lane_id: config.lane_id.clone(),
        run_id: config.run_id.clone(),
        template_digest: config.template.template_digest.clone(),
        corpus_manifest_digest: config.corpus_manifest_digest.clone(),
        learning_rate: config.learning_rate,
        pretraining_reference_learning_rate: config.pretraining_reference_learning_rate,
        learning_rate_ratio: config.learning_rate / config.pretraining_reference_learning_rate,
        scheduler_kind: scheduler_kind_label(&config.scheduler),
        optimizer: config.optimizer,
        sample_count,
        trainable_token_count: trainable_total,
        masked_token_count: masked_total,
        completed_steps: run.completed_steps(),
        initial_loss,
        final_loss,
        final_first_batch_loss,
        loss_improved: final_first_batch_loss < initial_loss,
        learning_rate_schedule,
        step_receipt_digests: receipts
            .iter()
            .map(|receipt| receipt.receipt_digest.clone())
            .collect(),
        adapter_artifact_digest: exported.adapter_artifact_digest,
        adapter_identity_digest: exported.adapter_identity_digest,
        resume_drill: PsionInstructSftResumeDrillReport {
            checkpoint_at_step: config.checkpoint_at_step,
            checkpoint_state_digest,
            resumed_steps: resumed_receipts.len() as u64,
            resume_bit_exact,
            uninterrupted_final_parameter_digest: final_parameter_digest,
            resumed_final_parameter_digest: resumed_parameter_digest,
            post_resume_receipt_digests_match,
        },
        claim_boundary: config.claim_boundary.clone(),
        report_digest: String::new(),
    };
    report.report_digest = report.stable_digest();
    Ok(report)
}

fn build_instruct_backend(
    config: &PsionInstructSftLaneConfig,
    samples: Vec<OpenAdapterHiddenStateSample>,
) -> Result<OpenAdapterTrainingExecutionBackend, PsionInstructSftError> {
    let optimizer = match config.optimizer {
        PsionInstructSftOptimizerKind::AdamW => {
            TrainingOptimizerConfig::adamw(config.learning_rate, 0.9, 0.99, 1e-8)
        }
        PsionInstructSftOptimizerKind::Sgd => TrainingOptimizerConfig::sgd(config.learning_rate),
    };
    Ok(OpenAdapterTrainingExecutionBackend::new(
        OpenAdapterExecutionConfig {
            run_id: config.run_id.clone(),
            checkpoint_family: String::from(PSION_INSTRUCT_SFT_CHECKPOINT_FAMILY),
            execution_backend_label: String::from(OPEN_ADAPTER_CUDA_BACKEND_LABEL),
            admissible_model_family: OpenAdapterAdmissibleModelFamily::GptOssDecoderLmHeadLora,
            budget: TrainingLoopBudget::new(config.max_steps, 1, 1)?,
            batch_size: config.batch_size,
            precision_policy: OpenAdapterPrecisionPolicy::F32Reference,
            model: OpenAdapterReferenceModel {
                base_model_id: config.base_model.clone(),
                base_model_revision: config.base_model_revision.clone(),
                base_served_artifact_digest: config.base_served_artifact_digest.clone(),
                tokenizer: TokenizerDigest::new(
                    TokenizerFamily::BytePairEncoding,
                    config.tokenizer_digest.clone(),
                    config.vocab_size as u32,
                )
                .with_template_digest(config.template.template_digest.as_str()),
                hidden_size: config.hidden_size,
                vocab_size: config.vocab_size,
                target: OpenAdapterLmHeadTarget {
                    target_id: String::from("lm_head.weight"),
                    lora_rank: config.lora_rank,
                    lora_alpha: config.lora_alpha,
                    optimizer,
                    optimizer_residency_policy: TrainingOptimizerResidencyPolicy::host_only(),
                },
            },
        },
        samples,
    )?)
}

fn initialize_scheduled_run(
    backend: &OpenAdapterTrainingExecutionBackend,
    config: &PsionInstructSftLaneConfig,
) -> Result<FixedBudgetTrainingRun, PsionInstructSftError> {
    let groups: Vec<TrainingParameterGroupState> = backend
        .initial_training_groups()?
        .into_iter()
        .map(|mut group| {
            group.scheduler = Some(TrainingSchedulerBinding::new(config.scheduler.clone()));
            group
        })
        .collect();
    Ok(FixedBudgetTrainingRun::new(
        config.run_id.clone(),
        String::from(PSION_INSTRUCT_SFT_CHECKPOINT_FAMILY),
        TrainingLoopBudget::new(config.max_steps, 1, 1)?,
        groups,
    )?)
}

fn apply_steps(
    backend: &OpenAdapterTrainingExecutionBackend,
    config: &PsionInstructSftLaneConfig,
    run: &mut FixedBudgetTrainingRun,
    from_step: u64,
    to_step: u64,
) -> Result<Vec<TrainingStepReceipt>, PsionInstructSftError> {
    let mut receipts = Vec::new();
    for step_index in from_step..to_step {
        let batch_index = step_index as usize % backend.batches().len().max(1);
        let started_at_ms = config.started_at_ms + step_index * config.step_duration_ms;
        let finished_at_ms = started_at_ms + config.step_duration_ms;
        let (step_input, _) =
            backend.produce_step_input(run, batch_index, started_at_ms, finished_at_ms)?;
        receipts.push(run.apply_step(step_input)?);
    }
    Ok(receipts)
}

fn run_lora_group_values<'run>(
    backend: &OpenAdapterTrainingExecutionBackend,
    run: &'run FixedBudgetTrainingRun,
) -> Result<Vec<&'run [f32]>, PsionInstructSftError> {
    let target = &backend.config().model.target;
    let mut values = Vec::new();
    for group_id in [target.lora_a_group_id(), target.lora_b_group_id()] {
        let group = run.parameter_group(group_id.as_str()).ok_or_else(|| {
            PsionInstructSftError::InvalidConfig {
                detail: format!("run is missing LoRA group `{group_id}`"),
            }
        })?;
        match &group.parameter.data {
            TensorData::F32(group_values) => values.push(group_values.as_slice()),
            _ => {
                return Err(PsionInstructSftError::InvalidConfig {
                    detail: format!("LoRA group `{group_id}` is not an f32 reference tensor"),
                });
            }
        }
    }
    Ok(values)
}

fn run_parameter_digest(
    backend: &OpenAdapterTrainingExecutionBackend,
    run: &FixedBudgetTrainingRun,
) -> Result<String, PsionInstructSftError> {
    let mut hasher = Sha256::new();
    hasher.update(b"psion_instruct_sft_parameters|");
    for group_values in run_lora_group_values(backend, run)? {
        hasher.update(b"|group|");
        for value in group_values {
            hasher.update(value.to_bits().to_le_bytes());
        }
    }
    Ok(format!("sha256:{:x}", hasher.finalize()))
}

fn run_parameters_bit_equal(
    backend: &OpenAdapterTrainingExecutionBackend,
    left: &FixedBudgetTrainingRun,
    right: &FixedBudgetTrainingRun,
) -> Result<bool, PsionInstructSftError> {
    let left_values = run_lora_group_values(backend, left)?;
    let right_values = run_lora_group_values(backend, right)?;
    Ok(left_values.len() == right_values.len()
        && left_values
            .iter()
            .zip(right_values.iter())
            .all(|(left_group, right_group)| {
                left_group.len() == right_group.len()
                    && left_group
                        .iter()
                        .zip(right_group.iter())
                        .all(|(left_value, right_value)| {
                            left_value.to_bits() == right_value.to_bits()
                        })
            }))
}

fn scheduler_kind_label(config: &TrainingSchedulerConfig) -> String {
    String::from(match config {
        TrainingSchedulerConfig::Constant => "constant",
        TrainingSchedulerConfig::StepLr { .. } => "step_lr",
        TrainingSchedulerConfig::LinearWarmup { .. } => "linear_warmup",
        TrainingSchedulerConfig::InverseSquareRootWarmup { .. } => "inverse_square_root_warmup",
        TrainingSchedulerConfig::CosineAnnealing { .. } => "cosine_annealing",
    })
}

fn deterministic_unit_values(seed: &str, len: usize) -> Vec<f32> {
    let mut values = Vec::with_capacity(len);
    let mut counter = 0u64;
    while values.len() < len {
        let mut hasher = Sha256::new();
        hasher.update(b"psion_instruct_hidden|");
        hasher.update(seed.as_bytes());
        hasher.update(counter.to_le_bytes());
        let digest = hasher.finalize();
        for chunk in digest.chunks_exact(4) {
            if values.len() == len {
                break;
            }
            let raw = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
            // Map to [-1, 1] deterministically.
            values.push(((raw as f64 / u32::MAX as f64) * 2.0 - 1.0) as f32);
        }
        counter += 1;
    }
    values
}

fn deterministic_token_id(token_text: &str, vocab_size: usize) -> u32 {
    let mut hasher = Sha256::new();
    hasher.update(b"psion_instruct_token|");
    hasher.update(token_text.as_bytes());
    let digest = hasher.finalize();
    let raw = u32::from_le_bytes([digest[0], digest[1], digest[2], digest[3]]);
    raw % (vocab_size as u32)
}

fn stable_json_digest<T: Serialize>(domain: &[u8], value: &T) -> String {
    let payload = serde_json::to_vec(value).unwrap_or_default();
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update(payload.as_slice());
    format!("sha256:{:x}", hasher.finalize())
}

/// Returns the repo-owned example instruct corpus records.
///
/// Every record is clearly labeled example material authored inside this
/// repository: `source_family_id` is `psion_instruct_example_seed` and the
/// rights posture is `repo_owned_example_text`.
#[must_use]
pub fn psion_instruct_example_corpus_records() -> Vec<PsionInstructCorpusRecord> {
    let conversations: Vec<(&str, Vec<PsionChatMessage>)> = vec![
        (
            "psion_instruct_example_0001",
            vec![
                PsionChatMessage {
                    role: PsionChatRole::System,
                    content: String::from(
                        "You are a careful assistant that answers briefly and honestly.",
                    ),
                },
                PsionChatMessage {
                    role: PsionChatRole::User,
                    content: String::from("What does a learning-rate warmup do?"),
                },
                PsionChatMessage {
                    role: PsionChatRole::Assistant,
                    content: String::from(
                        "Warmup ramps the learning rate up from a small value so early \
                         optimizer steps stay stable before the schedule decays it.",
                    ),
                },
            ],
        ),
        (
            "psion_instruct_example_0002",
            vec![
                PsionChatMessage {
                    role: PsionChatRole::User,
                    content: String::from("Name one reason to mask user tokens from the loss."),
                },
                PsionChatMessage {
                    role: PsionChatRole::Assistant,
                    content: String::from(
                        "Training only on assistant tokens stops the model from learning to \
                         imitate user phrasing instead of producing answers.",
                    ),
                },
            ],
        ),
        (
            "psion_instruct_example_0003",
            vec![
                PsionChatMessage {
                    role: PsionChatRole::User,
                    content: String::from("Why checkpoint a fine-tune run?"),
                },
                PsionChatMessage {
                    role: PsionChatRole::Assistant,
                    content: String::from(
                        "A checkpoint lets an interrupted run resume from saved optimizer and \
                         parameter state instead of restarting from scratch.",
                    ),
                },
                PsionChatMessage {
                    role: PsionChatRole::User,
                    content: String::from("And what must the checkpoint include?"),
                },
                PsionChatMessage {
                    role: PsionChatRole::Assistant,
                    content: String::from(
                        "Parameters, optimizer moments, scheduler state, and the completed \
                         step count.",
                    ),
                },
            ],
        ),
        (
            "psion_instruct_example_0004",
            vec![
                PsionChatMessage {
                    role: PsionChatRole::System,
                    content: String::from("Answer in one sentence."),
                },
                PsionChatMessage {
                    role: PsionChatRole::User,
                    content: String::from("What is a LoRA adapter?"),
                },
                PsionChatMessage {
                    role: PsionChatRole::Assistant,
                    content: String::from(
                        "A LoRA adapter is a small pair of low-rank matrices trained on top \
                         of frozen base weights.",
                    ),
                },
            ],
        ),
    ];
    conversations
        .into_iter()
        .map(|(record_id, messages)| {
            let content_digest = PsionInstructCorpusRecord::compute_content_digest(&messages)
                .unwrap_or_default();
            PsionInstructCorpusRecord {
                record_id: String::from(record_id),
                source_id: format!("{record_id}_source"),
                source_family_id: String::from("psion_instruct_example_seed"),
                rights_posture: String::from("repo_owned_example_text"),
                canonical_reference: format!(
                    "repo://fixtures/psion/instruct/psion_instruct_example_corpus_v1.jsonl#{record_id}"
                ),
                content_digest,
                messages,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn template() -> PsionChatTemplate {
        PsionChatTemplate::v1()
    }

    fn simple_conversation() -> Vec<PsionChatMessage> {
        vec![
            PsionChatMessage {
                role: PsionChatRole::System,
                content: String::from("Be brief."),
            },
            PsionChatMessage {
                role: PsionChatRole::User,
                content: String::from("Say hello world"),
            },
            PsionChatMessage {
                role: PsionChatRole::Assistant,
                content: String::from("hello world"),
            },
        ]
    }

    #[test]
    fn template_v1_digest_is_pinned_and_validates() {
        let template = template();
        template.validate().expect("v1 template must validate");
        assert_eq!(template.template_digest, template.stable_digest());

        let mut tampered = template.clone();
        tampered.eos_token = String::from("<|other_eos|>");
        assert!(matches!(
            tampered.validate(),
            Err(PsionInstructSftError::InvalidTemplate { .. })
        ));
    }

    #[test]
    fn template_rejects_duplicate_and_whitespace_markers() {
        let mut duplicate = template();
        duplicate.user_open = duplicate.assistant_open.clone();
        duplicate.template_digest = duplicate.stable_digest();
        assert!(matches!(
            duplicate.validate(),
            Err(PsionInstructSftError::InvalidTemplate { .. })
        ));

        let mut spaced = template();
        spaced.turn_close = String::from("<| end |>");
        spaced.template_digest = spaced.stable_digest();
        assert!(matches!(
            spaced.validate(),
            Err(PsionInstructSftError::InvalidTemplate { .. })
        ));
    }

    #[test]
    fn render_pins_exact_token_level_generation_mask() {
        let rendered = template()
            .render_conversation("conv-mask", &simple_conversation())
            .expect("render must succeed");
        let texts: Vec<&str> = rendered
            .tokens
            .iter()
            .map(|token| token.text.as_str())
            .collect();
        assert_eq!(
            texts,
            vec![
                "<|psion_system|>",
                "Be",
                "brief.",
                "<|psion_end|>",
                "<|psion_user|>",
                "Say",
                "hello",
                "world",
                "<|psion_end|>",
                "<|psion_assistant|>",
                "hello",
                "world",
                "<|psion_end|>",
                "<|psion_eos|>",
            ]
        );
        assert_eq!(
            rendered.loss_mask,
            vec![0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1]
        );
        assert_eq!(rendered.trainable_token_count, 4);
        assert_eq!(rendered.masked_token_count, 10);
    }

    #[test]
    fn render_refuses_missing_final_assistant_turn() {
        let mut messages = simple_conversation();
        messages.push(PsionChatMessage {
            role: PsionChatRole::User,
            content: String::from("And again?"),
        });
        assert!(matches!(
            template().render_conversation("conv-no-eos", &messages),
            Err(PsionInstructSftError::MissingFinalAssistantTurn { .. })
        ));
    }

    #[test]
    fn render_always_ends_with_trainable_eos() {
        let rendered = template()
            .render_conversation("conv-eos", &simple_conversation())
            .expect("render must succeed");
        let last = rendered.tokens.last().expect("tokens must be non-empty");
        assert_eq!(last.segment, PsionTemplateSegment::Eos);
        assert!(last.trains);
        assert_eq!(*rendered.loss_mask.last().expect("mask non-empty"), 1);
    }

    #[test]
    fn render_refuses_role_bleed_in_any_role() {
        let mut user_bleed = simple_conversation();
        user_bleed[1].content = String::from("Say <|psion_assistant|> hello");
        assert!(matches!(
            template().render_conversation("conv-bleed-user", &user_bleed),
            Err(PsionInstructSftError::RoleBleed {
                message_index: 1,
                ..
            })
        ));

        let mut assistant_bleed = simple_conversation();
        assistant_bleed[2].content = String::from("hello <|psion_eos|> world");
        assert!(matches!(
            template().render_conversation("conv-bleed-assistant", &assistant_bleed),
            Err(PsionInstructSftError::RoleBleed {
                message_index: 2,
                ..
            })
        ));
    }

    #[test]
    fn mask_boundaries_have_no_off_by_one() {
        // Multi-turn conversation exercising both assistant span boundaries.
        let messages = vec![
            PsionChatMessage {
                role: PsionChatRole::User,
                content: String::from("first question"),
            },
            PsionChatMessage {
                role: PsionChatRole::Assistant,
                content: String::from("first answer"),
            },
            PsionChatMessage {
                role: PsionChatRole::User,
                content: String::from("second question"),
            },
            PsionChatMessage {
                role: PsionChatRole::Assistant,
                content: String::from("second answer"),
            },
        ];
        let rendered = template()
            .render_conversation("conv-boundary", &messages)
            .expect("render must succeed");
        for (index, token) in rendered.tokens.iter().enumerate() {
            let expected = match (token.role, token.segment) {
                // Assistant role-open markers never train: span starts at content.
                (PsionChatRole::Assistant, PsionTemplateSegment::RoleOpen) => false,
                (PsionChatRole::Assistant, _) => true,
                _ => false,
            };
            assert_eq!(
                token.trains, expected,
                "token {index} `{}` ({:?}/{:?}) has wrong mask",
                token.text, token.role, token.segment
            );
        }
        // The token immediately before the first assistant content token is
        // the assistant role-open marker and must be masked; the token
        // immediately after the assistant turn-close is the next user
        // role-open marker and must be masked.
        let first_assistant_content = rendered
            .tokens
            .iter()
            .position(|token| {
                token.role == PsionChatRole::Assistant
                    && token.segment == PsionTemplateSegment::Content
            })
            .expect("assistant content exists");
        assert_eq!(rendered.loss_mask[first_assistant_content - 1], 0);
        assert_eq!(rendered.loss_mask[first_assistant_content], 1);
        let first_assistant_close = rendered
            .tokens
            .iter()
            .position(|token| {
                token.role == PsionChatRole::Assistant
                    && token.segment == PsionTemplateSegment::TurnClose
            })
            .expect("assistant turn close exists");
        assert_eq!(rendered.loss_mask[first_assistant_close], 1);
        assert_eq!(rendered.loss_mask[first_assistant_close + 1], 0);
    }

    #[test]
    fn masked_loss_samples_cover_exactly_the_trainable_tokens() {
        let records = psion_instruct_example_corpus_records();
        let samples = derive_psion_instruct_supervision_samples(&template(), &records, 16, 64)
            .expect("samples must derive");
        let mut expected_total = 0usize;
        for record in &records {
            let rendered = record.validate(&template()).expect("record renders");
            expected_total += rendered.trainable_token_count;
            for (index, token) in rendered.tokens.iter().enumerate() {
                let sample_id = format!("{}-tok-{index:04}", record.record_id);
                let present = samples.iter().any(|sample| sample.sample_id == sample_id);
                assert_eq!(
                    present, token.trains,
                    "sample presence must match the generation mask for `{sample_id}`"
                );
            }
        }
        assert_eq!(samples.len(), expected_total);
    }

    #[test]
    fn corpus_record_rejects_tampered_content_digest() {
        let mut record = psion_instruct_example_corpus_records().remove(0);
        record.content_digest = String::from("sha256:tampered");
        assert!(matches!(
            record.validate(&template()),
            Err(PsionInstructSftError::InvalidRecord { .. })
        ));
    }

    #[test]
    fn corpus_manifest_validates_and_rejects_drift() {
        let records = psion_instruct_example_corpus_records();
        let manifest = build_psion_instruct_corpus_manifest(
            &template(),
            "psion_instruct_corpus_example",
            "v1",
            true,
            PsionInstructArtifactRef {
                path: String::from(PSION_INSTRUCT_EXAMPLE_CORPUS_FIXTURE_PATH),
                sha256: String::from("0".repeat(64)),
            },
            &records,
        )
        .expect("manifest must build");
        manifest.validate().expect("manifest must validate");
        assert!(manifest.example_corpus);

        let mut duplicated = manifest.clone();
        duplicated.records.push(duplicated.records[0].clone());
        duplicated.record_count = duplicated.records.len();
        duplicated.manifest_digest = duplicated.stable_digest();
        assert!(matches!(
            duplicated.validate(),
            Err(PsionInstructSftError::InvalidManifest { .. })
        ));

        let mut drifted = manifest.clone();
        drifted.total_trainable_tokens += 1;
        drifted.manifest_digest = drifted.stable_digest();
        assert!(matches!(
            drifted.validate(),
            Err(PsionInstructSftError::InvalidManifest { .. })
        ));
    }

    #[test]
    fn default_config_sits_ten_times_below_the_pretraining_reference() {
        let config = default_psion_instruct_sft_lane_config("sha256:test-manifest");
        config.validate().expect("default config must validate");
        let ratio = config.learning_rate / config.pretraining_reference_learning_rate;
        assert!((ratio - 0.1).abs() < 1e-6);
    }

    #[test]
    fn config_refuses_pretraining_scale_learning_rate() {
        let mut config = default_psion_instruct_sft_lane_config("sha256:test-manifest");
        config.learning_rate = config.pretraining_reference_learning_rate;
        assert!(matches!(
            config.validate(),
            Err(PsionInstructSftError::InvalidConfig { .. })
        ));
    }

    #[test]
    fn lane_run_applies_cosine_schedule_and_improves_loss() {
        let records = psion_instruct_example_corpus_records();
        let config = default_psion_instruct_sft_lane_config("sha256:test-manifest");
        let report = run_psion_instruct_sft_lane(&config, &records).expect("lane must run");
        assert_eq!(report.completed_steps, config.max_steps);
        assert!(report.loss_improved);
        assert_eq!(report.scheduler_kind, "cosine_annealing");
        let schedule = &report.learning_rate_schedule;
        assert_eq!(schedule.len(), config.max_steps as usize);
        // Cosine annealing starts at the base learning rate and ends at the
        // configured minimum.
        assert!((schedule[0].effective_learning_rate - config.learning_rate).abs() < 1e-9);
        let TrainingSchedulerConfig::CosineAnnealing {
            min_learning_rate, ..
        } = config.scheduler
        else {
            panic!("default scheduler must be cosine annealing");
        };
        let last = schedule.last().expect("schedule non-empty");
        assert!((last.effective_learning_rate - min_learning_rate).abs() < 1e-9);
        // Monotone non-increasing decay across the bounded run.
        for window in schedule.windows(2) {
            assert!(window[1].effective_learning_rate <= window[0].effective_learning_rate);
        }
        assert_eq!(report.report_digest, report.stable_digest());
    }

    #[test]
    fn lane_run_applies_linear_warmup_schedule() {
        let records = psion_instruct_example_corpus_records();
        let mut config = default_psion_instruct_sft_lane_config("sha256:test-manifest");
        config.scheduler = TrainingSchedulerConfig::LinearWarmup {
            warmup_steps: 4,
            start_factor: 0.25,
        };
        let report = run_psion_instruct_sft_lane(&config, &records).expect("lane must run");
        let schedule = &report.learning_rate_schedule;
        assert!(schedule[0].effective_learning_rate < config.learning_rate);
        // Warmup ramps monotonically up to the base learning rate.
        for window in schedule.windows(2) {
            assert!(window[1].effective_learning_rate >= window[0].effective_learning_rate);
        }
        let last = schedule.last().expect("schedule non-empty");
        assert!((last.effective_learning_rate - config.learning_rate).abs() < 1e-9);
    }

    #[test]
    fn checkpoint_resume_drill_is_bit_exact_for_adamw_and_sgd() {
        let records = psion_instruct_example_corpus_records();
        for optimizer in [
            PsionInstructSftOptimizerKind::AdamW,
            PsionInstructSftOptimizerKind::Sgd,
        ] {
            let mut config = default_psion_instruct_sft_lane_config("sha256:test-manifest");
            config.optimizer = optimizer;
            let report = run_psion_instruct_sft_lane(&config, &records).expect("lane must run");
            assert!(
                report.resume_drill.resume_bit_exact,
                "{optimizer:?} resume must be bit exact"
            );
            assert!(report.resume_drill.post_resume_receipt_digests_match);
            assert_eq!(
                report.resume_drill.uninterrupted_final_parameter_digest,
                report.resume_drill.resumed_final_parameter_digest
            );
            assert_eq!(
                report.resume_drill.resumed_steps,
                config.max_steps - config.checkpoint_at_step
            );
            assert!(
                report
                    .resume_drill
                    .checkpoint_state_digest
                    .starts_with("sha256:")
            );
        }
    }

    #[test]
    fn lane_report_is_deterministic_across_runs() {
        let records = psion_instruct_example_corpus_records();
        let config = default_psion_instruct_sft_lane_config("sha256:test-manifest");
        let first = run_psion_instruct_sft_lane(&config, &records).expect("first run");
        let second = run_psion_instruct_sft_lane(&config, &records).expect("second run");
        assert_eq!(first, second);
    }

    #[test]
    fn example_corpus_is_labeled_and_provenance_complete() {
        for record in psion_instruct_example_corpus_records() {
            assert_eq!(record.source_family_id, "psion_instruct_example_seed");
            assert_eq!(record.rights_posture, "repo_owned_example_text");
            assert!(record.canonical_reference.starts_with("repo://"));
            record.validate(&template()).expect("record must validate");
        }
    }
}
