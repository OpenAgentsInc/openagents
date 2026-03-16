use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    DatasetContractError, DatasetPackingPlan, DatasetPackingPolicy, DatasetRecordEncoding,
    DatasetSequenceDescriptor, TokenizerDigest, TokenizerFamily,
};

/// Stable ABI version for Apple-adapter dataset contracts in `psionic-data`.
pub const APPLE_ADAPTER_DATASET_ABI_VERSION: &str = "psionic.apple_adapter_dataset.v1";
/// Canonical default instruction for datasets that rely on Apple's implicit helper prompt.
pub const APPLE_ADAPTER_DEFAULT_INSTRUCTION: &str =
    "A conversation between a user and a helpful assistant.";

const APPLE_ADAPTER_PACKING_SHARD_KEY: &str = "apple_adapter_dataset";
const APPLE_ADAPTER_COMPAT_TOKENIZER_VERSION: &str = "openagents.apple.compat_tokenizer.v1";
const APPLE_ADAPTER_PROMPT_SHAPING_VERSION: &str = "openagents.apple.prompt_shaping.v1";
const APPLE_ADAPTER_SPECIAL_TOKENS_VERSION: &str = "openagents.apple.special_tokens.v1";

/// Repo-owned runtime profile used to derive Apple adapter compatibility anchors.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppleAdapterRuntimeCompatibilityProfile {
    /// Stable runtime model identifier.
    pub model_id: String,
    /// Stable runtime use-case label.
    pub use_case: String,
    /// Stable runtime guardrail label.
    pub guardrails: String,
    /// Optional explicit Apple runtime compatibility anchor when the bridge or
    /// experiment program can provide a real base-model signature.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub explicit_base_model_signature: Option<String>,
    /// Optional locale carried into prompt shaping.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locale: Option<String>,
    /// Optional dataset-wide default instruction used when samples omit a system turn.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_instruction: Option<String>,
    /// Optional bridge version recorded for operator receipts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bridge_version: Option<String>,
    /// Optional bridge platform recorded for operator receipts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bridge_platform: Option<String>,
}

impl AppleAdapterRuntimeCompatibilityProfile {
    /// Creates a runtime compatibility profile from the live Apple runtime configuration.
    #[must_use]
    pub fn new(
        model_id: impl Into<String>,
        use_case: impl Into<String>,
        guardrails: impl Into<String>,
    ) -> Self {
        Self {
            model_id: model_id.into(),
            use_case: use_case.into(),
            guardrails: guardrails.into(),
            explicit_base_model_signature: None,
            locale: None,
            default_instruction: None,
            bridge_version: None,
            bridge_platform: None,
        }
    }

    /// Attaches an explicit base-model compatibility anchor.
    #[must_use]
    pub fn with_base_model_signature(mut self, base_model_signature: impl Into<String>) -> Self {
        self.explicit_base_model_signature = Some(base_model_signature.into());
        self
    }

    /// Attaches a locale tag used by the repo-owned prompt shaper.
    #[must_use]
    pub fn with_locale(mut self, locale: impl Into<String>) -> Self {
        self.locale = Some(locale.into());
        self
    }

    /// Attaches a default instruction for datasets that need one.
    #[must_use]
    pub fn with_default_instruction(mut self, default_instruction: impl Into<String>) -> Self {
        self.default_instruction = Some(default_instruction.into());
        self
    }

    /// Attaches the bridge version that produced this profile.
    #[must_use]
    pub fn with_bridge_version(mut self, bridge_version: impl Into<String>) -> Self {
        self.bridge_version = Some(bridge_version.into());
        self
    }

    /// Attaches the bridge platform that produced this profile.
    #[must_use]
    pub fn with_bridge_platform(mut self, bridge_platform: impl Into<String>) -> Self {
        self.bridge_platform = Some(bridge_platform.into());
        self
    }

    /// Returns the stable base-model compatibility anchor for this runtime configuration.
    #[must_use]
    pub fn base_model_signature(&self) -> String {
        if let Some(signature) = self
            .explicit_base_model_signature
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return signature.to_string();
        }
        truncated_hex_digest(
            b"psionic_apple_adapter_base_model_signature|",
            &[
                self.model_id.as_str(),
                self.use_case.as_str(),
                self.guardrails.as_str(),
            ],
            40,
        )
    }

    /// Returns the stable prompt-shaping digest for this runtime profile.
    #[must_use]
    pub fn prompt_shaping_digest(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(b"psionic_apple_adapter_prompt_shaping|");
        hasher.update(APPLE_ADAPTER_PROMPT_SHAPING_VERSION.as_bytes());
        hasher.update(b"|");
        hasher.update(self.base_model_signature().as_bytes());
        hasher.update(b"|");
        hasher.update(self.model_id.as_bytes());
        hasher.update(b"|");
        hasher.update(self.use_case.as_bytes());
        hasher.update(b"|");
        hasher.update(self.guardrails.as_bytes());
        if let Some(locale) = &self.locale {
            hasher.update(b"|locale|");
            hasher.update(locale.as_bytes());
        }
        if let Some(default_instruction) = &self.default_instruction {
            hasher.update(b"|default_instruction|");
            hasher.update(default_instruction.as_bytes());
        }
        hasher.update(b"|roles|system,user,assistant|attachments|tools,response_format");
        hex::encode(hasher.finalize())
    }

    /// Returns the stable tokenizer digest for this runtime profile.
    #[must_use]
    pub fn tokenizer_digest(&self) -> TokenizerDigest {
        let mut hasher = Sha256::new();
        hasher.update(b"psionic_apple_adapter_tokenizer|");
        hasher.update(APPLE_ADAPTER_COMPAT_TOKENIZER_VERSION.as_bytes());
        hasher.update(b"|");
        hasher.update(self.base_model_signature().as_bytes());
        hasher.update(b"|");
        hasher.update(self.model_id.as_bytes());
        hasher.update(b"|");
        hasher.update(self.use_case.as_bytes());
        hasher.update(b"|");
        hasher.update(self.guardrails.as_bytes());
        if let Some(locale) = &self.locale {
            hasher.update(b"|locale|");
            hasher.update(locale.as_bytes());
        }
        if let Some(default_instruction) = &self.default_instruction {
            hasher.update(b"|default_instruction|");
            hasher.update(default_instruction.as_bytes());
        }
        TokenizerDigest::new(
            TokenizerFamily::Custom,
            hex::encode(hasher.finalize()),
            65_536,
        )
        .with_special_tokens_digest(stable_hex_digest(
            b"psionic_apple_adapter_special_tokens|",
            &[
                APPLE_ADAPTER_SPECIAL_TOKENS_VERSION,
                "role_boundary",
                "system",
                "user",
                "assistant",
                "tools",
                "response_schema",
                "locale",
                "default_instruction",
            ],
        ))
        .with_template_digest(self.prompt_shaping_digest())
    }

    /// Builds dataset metadata derived from this runtime profile.
    #[must_use]
    pub fn dataset_metadata(&self) -> AppleAdapterDatasetMetadata {
        let mut metadata =
            AppleAdapterDatasetMetadata::new(self.tokenizer_digest(), self.prompt_shaping_digest());
        if let Some(default_instruction) = &self.default_instruction {
            metadata = metadata.with_default_instruction(default_instruction.clone());
        }
        if let Some(locale) = &self.locale {
            metadata = metadata.with_locale(locale.clone());
        }
        metadata
    }
}

/// Typed Apple adapter message roles admitted by the repo-owned dataset spec.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppleAdapterMessageRole {
    /// Optional first system instruction.
    System,
    /// User prompt or request.
    User,
    /// Assistant completion target.
    Assistant,
}

impl AppleAdapterMessageRole {
    /// Stable string label used in digests and errors.
    #[must_use]
    pub const fn label(&self) -> &'static str {
        match self {
            Self::System => "system",
            Self::User => "user",
            Self::Assistant => "assistant",
        }
    }
}

impl fmt::Display for AppleAdapterMessageRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// High-level Apple adapter workload family derived from one sample.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppleAdapterSampleKind {
    /// Plain chat-style supervised fine-tuning.
    SupervisedFineTune,
    /// Schema-free JSON-style guided generation.
    SchemaFreeGuidedGeneration,
    /// Guided generation with an explicit JSON schema.
    GuidedGenerationWithSchema,
    /// Tool-aware training sample.
    ToolCalling,
}

/// Repo-owned lineage metadata for one imported Apple adapter dataset.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppleAdapterDatasetMetadata {
    /// Tokenizer lineage bound to the dataset.
    pub tokenizer: TokenizerDigest,
    /// Digest over prompt shaping, default instruction, and related formatting behavior.
    pub prompt_shaping_digest: String,
    /// Optional default instruction captured for later training or eval reuse.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_instruction: Option<String>,
    /// Optional locale or regional behavior tag when prompt shaping depends on locale.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locale: Option<String>,
}

impl AppleAdapterDatasetMetadata {
    /// Creates the Apple dataset metadata contract.
    #[must_use]
    pub fn new(tokenizer: TokenizerDigest, prompt_shaping_digest: impl Into<String>) -> Self {
        Self {
            tokenizer,
            prompt_shaping_digest: prompt_shaping_digest.into(),
            default_instruction: None,
            locale: None,
        }
    }

    /// Attaches a captured default instruction.
    #[must_use]
    pub fn with_default_instruction(mut self, default_instruction: impl Into<String>) -> Self {
        self.default_instruction = Some(default_instruction.into());
        self
    }

    /// Attaches a locale tag.
    #[must_use]
    pub fn with_locale(mut self, locale: impl Into<String>) -> Self {
        self.locale = Some(locale.into());
        self
    }

    /// Returns a stable digest over the lineage metadata.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(b"psionic_apple_adapter_dataset_metadata|");
        hasher.update(self.tokenizer.stable_digest().as_bytes());
        hasher.update(b"|");
        hasher.update(self.prompt_shaping_digest.as_bytes());
        if let Some(default_instruction) = &self.default_instruction {
            hasher.update(b"|instruction|");
            hasher.update(default_instruction.as_bytes());
        }
        if let Some(locale) = &self.locale {
            hasher.update(b"|locale|");
            hasher.update(locale.as_bytes());
        }
        hex::encode(hasher.finalize())
    }

    /// Validates the lineage metadata.
    pub fn validate(&self) -> Result<(), AppleAdapterDatasetError> {
        if self.tokenizer.tokenizer_digest.trim().is_empty() {
            return Err(AppleAdapterDatasetError::MissingTokenizerDigest);
        }
        if self.tokenizer.vocab_size == 0 {
            return Err(AppleAdapterDatasetError::InvalidTokenizerVocabSize);
        }
        if self.prompt_shaping_digest.trim().is_empty() {
            return Err(AppleAdapterDatasetError::MissingPromptShapingDigest);
        }
        Ok(())
    }
}

/// Typed JSON-schema attachment frozen by the Apple dataset spec.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AppleAdapterJsonSchemaDefinition {
    /// Stable schema name.
    pub name: String,
    /// Raw JSON schema payload.
    pub schema: Value,
}

impl AppleAdapterJsonSchemaDefinition {
    /// Returns a stable digest over the schema attachment.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(b"psionic_apple_adapter_json_schema|");
        hasher.update(self.name.as_bytes());
        hasher.update(b"|");
        hasher.update(canonical_json(self.schema.clone()).as_bytes());
        hex::encode(hasher.finalize())
    }
}

/// Typed guided-generation response format admitted by the Apple dataset spec.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AppleAdapterResponseFormat {
    /// Fully specified JSON schema contract.
    pub json_schema: AppleAdapterJsonSchemaDefinition,
}

/// Tool kinds admitted by the Apple dataset contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppleAdapterToolType {
    /// Function-style tool definition.
    Function,
}

/// Tool function contract frozen from the Apple toolkit shape.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AppleAdapterToolFunctionDefinition {
    /// Stable tool name.
    pub name: String,
    /// Optional tool description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Raw JSON-schema-like function argument contract.
    pub arguments: Value,
}

/// Typed Apple tool definition admitted by the dataset contract.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AppleAdapterToolDefinition {
    /// Tool family.
    pub tool_type: AppleAdapterToolType,
    /// Function contract.
    pub function: AppleAdapterToolFunctionDefinition,
}

impl AppleAdapterToolDefinition {
    /// Returns a stable digest over the tool contract.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(b"psionic_apple_adapter_tool|");
        hasher.update(match self.tool_type {
            AppleAdapterToolType::Function => b"function".as_slice(),
        });
        hasher.update(b"|");
        hasher.update(self.function.name.as_bytes());
        if let Some(description) = &self.function.description {
            hasher.update(b"|description|");
            hasher.update(description.as_bytes());
        }
        hasher.update(b"|arguments|");
        hasher.update(canonical_json(self.function.arguments.clone()).as_bytes());
        hex::encode(hasher.finalize())
    }
}

/// One validated Apple adapter training message.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AppleAdapterMessage {
    /// Message role.
    pub role: AppleAdapterMessageRole,
    /// UTF-8 message content.
    pub content: String,
    /// Optional guided-generation schema attachment for user turns.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response_format: Option<AppleAdapterResponseFormat>,
    /// Optional tool definitions carried by the first system message.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<AppleAdapterToolDefinition>,
}

/// One validated and augmented Apple adapter sample.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AppleAdapterTrainingSample {
    /// Stable sample identifier local to the imported dataset.
    pub sample_id: String,
    /// Physical source line number inside the JSONL file.
    pub source_line_number: usize,
    /// Stable digest over the normalized sample contract.
    pub stable_digest: String,
    /// Derived high-level sample family.
    pub sample_kind: AppleAdapterSampleKind,
    /// Validated message list.
    pub messages: Vec<AppleAdapterMessage>,
    /// Augmented tool inventory extracted from the first system message.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<AppleAdapterToolDefinition>,
    /// Augmented guided-generation schema extracted from the user turn.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response_format: Option<AppleAdapterResponseFormat>,
    /// Parsed structured assistant output when the assistant target is JSON.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub structured_assistant_output: Option<Value>,
}

/// Full imported Apple adapter dataset contract backed by validated typed rows.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AppleAdapterDatasetContract {
    /// Stable Apple dataset ABI version.
    pub abi_version: String,
    /// Fixed record encoding for the imported dataset.
    pub record_encoding: DatasetRecordEncoding,
    /// Captured tokenizer and prompt-shaping lineage.
    pub metadata: AppleAdapterDatasetMetadata,
    /// Imported dataset rows.
    pub samples: Vec<AppleAdapterTrainingSample>,
}

impl AppleAdapterDatasetContract {
    /// Imports and validates a UTF-8 Apple-adapter JSONL payload.
    pub fn from_jsonl_str(
        input: &str,
        metadata: AppleAdapterDatasetMetadata,
    ) -> Result<Self, AppleAdapterDatasetError> {
        metadata.validate()?;
        let mut samples = Vec::new();
        for (line_offset, line) in input.lines().enumerate() {
            let line_number = line_offset + 1;
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let raw_value = serde_json::from_str::<Value>(trimmed).map_err(|err| {
                AppleAdapterDatasetError::JsonLineParse {
                    line_number,
                    message: err.to_string(),
                }
            })?;
            if !raw_value.is_array() {
                return Err(AppleAdapterDatasetError::SampleNotMessageArray { line_number });
            }
            let raw_messages = serde_json::from_value::<Vec<RawAppleAdapterMessage>>(raw_value)
                .map_err(|err| AppleAdapterDatasetError::JsonLineParse {
                    line_number,
                    message: err.to_string(),
                })?;
            samples.push(parse_sample(raw_messages, line_number)?);
        }

        if samples.is_empty() {
            return Err(AppleAdapterDatasetError::DatasetHasNoSamples);
        }

        let contract = Self {
            abi_version: String::from(APPLE_ADAPTER_DATASET_ABI_VERSION),
            record_encoding: DatasetRecordEncoding::JsonlConversation,
            metadata,
            samples,
        };
        contract.validate()?;
        Ok(contract)
    }

    /// Returns a stable digest over the full imported dataset contract.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(b"psionic_apple_adapter_dataset_contract|");
        hasher.update(self.abi_version.as_bytes());
        hasher.update(b"|");
        hasher.update(dataset_record_encoding_label(self.record_encoding));
        hasher.update(b"|");
        hasher.update(self.metadata.stable_digest().as_bytes());
        for sample in &self.samples {
            hasher.update(b"|sample|");
            hasher.update(sample.stable_digest.as_bytes());
        }
        hex::encode(hasher.finalize())
    }

    /// Validates the imported dataset contract.
    pub fn validate(&self) -> Result<(), AppleAdapterDatasetError> {
        if self.abi_version != APPLE_ADAPTER_DATASET_ABI_VERSION {
            return Err(AppleAdapterDatasetError::UnsupportedAbiVersion {
                abi_version: self.abi_version.clone(),
            });
        }
        self.metadata.validate()?;
        if self.samples.is_empty() {
            return Err(AppleAdapterDatasetError::DatasetHasNoSamples);
        }
        let mut sample_ids = BTreeSet::new();
        for sample in &self.samples {
            if sample.sample_id.trim().is_empty() {
                return Err(AppleAdapterDatasetError::MissingSampleId);
            }
            if !sample_ids.insert(sample.sample_id.clone()) {
                return Err(AppleAdapterDatasetError::DuplicateSampleId {
                    sample_id: sample.sample_id.clone(),
                });
            }
        }
        Ok(())
    }

    /// Plans deterministic packing for imported Apple samples using explicit token captures.
    pub fn plan_packing(
        &self,
        captures: &[AppleAdapterSampleTokenCapture],
        policy: &DatasetPackingPolicy,
    ) -> Result<DatasetPackingPlan, AppleAdapterDatasetError> {
        self.validate()?;
        let mut capture_by_id = BTreeMap::new();
        for capture in captures {
            if !capture_by_id
                .insert(capture.sample_id.clone(), capture.clone())
                .is_none()
            {
                return Err(AppleAdapterDatasetError::DuplicateTokenCapture {
                    sample_id: capture.sample_id.clone(),
                });
            }
        }

        for sample_id in capture_by_id.keys() {
            if !self
                .samples
                .iter()
                .any(|sample| sample.sample_id == *sample_id)
            {
                return Err(AppleAdapterDatasetError::UnknownTokenCapture {
                    sample_id: sample_id.clone(),
                });
            }
        }

        let mut sequences = Vec::with_capacity(self.samples.len());
        for (sequence_index, sample) in self.samples.iter().enumerate() {
            let Some(capture) = capture_by_id.get(sample.sample_id.as_str()) else {
                return Err(AppleAdapterDatasetError::MissingTokenCapture {
                    sample_id: sample.sample_id.clone(),
                });
            };
            if capture.tokenizer_digest != self.metadata.tokenizer.tokenizer_digest {
                return Err(AppleAdapterDatasetError::TokenizerDrift {
                    sample_id: sample.sample_id.clone(),
                    expected: self.metadata.tokenizer.tokenizer_digest.clone(),
                    actual: capture.tokenizer_digest.clone(),
                });
            }
            if capture.prompt_shaping_digest != self.metadata.prompt_shaping_digest {
                return Err(AppleAdapterDatasetError::PromptShapingDrift {
                    sample_id: sample.sample_id.clone(),
                    expected: self.metadata.prompt_shaping_digest.clone(),
                    actual: capture.prompt_shaping_digest.clone(),
                });
            }
            if capture.prompt_tokens == 0 {
                return Err(AppleAdapterDatasetError::InvalidPromptTokenCount {
                    sample_id: sample.sample_id.clone(),
                });
            }
            if capture.completion_tokens == 0 {
                return Err(AppleAdapterDatasetError::InvalidCompletionTokenCount {
                    sample_id: sample.sample_id.clone(),
                });
            }
            if !sample.tools.is_empty() && capture.tool_tokens == 0 {
                return Err(AppleAdapterDatasetError::MissingToolTokenCapture {
                    sample_id: sample.sample_id.clone(),
                });
            }
            if sample.response_format.is_some() && capture.response_schema_tokens == 0 {
                return Err(
                    AppleAdapterDatasetError::MissingResponseSchemaTokenCapture {
                        sample_id: sample.sample_id.clone(),
                    },
                );
            }
            sequences.push(DatasetSequenceDescriptor::new(
                sample.sample_id.clone(),
                APPLE_ADAPTER_PACKING_SHARD_KEY,
                sequence_index as u64,
                capture.total_tokens(),
            ));
        }
        policy.plan(sequences.as_slice()).map_err(Into::into)
    }

    /// Derives deterministic token captures from the repo-owned Apple preprocessing path.
    pub fn derive_token_captures(
        &self,
    ) -> Result<Vec<AppleAdapterSampleTokenCapture>, AppleAdapterDatasetError> {
        self.validate()?;
        Ok(self
            .samples
            .iter()
            .map(|sample| {
                let final_assistant_index = sample.messages.len().saturating_sub(1);
                let prompt_tokens = count_dataset_context_tokens(&self.metadata).saturating_add(
                    sample.messages[..final_assistant_index]
                        .iter()
                        .map(count_message_tokens)
                        .sum(),
                );
                let completion_tokens =
                    count_message_tokens(&sample.messages[final_assistant_index]);
                let mut capture = AppleAdapterSampleTokenCapture::new(
                    sample.sample_id.clone(),
                    self.metadata.tokenizer.tokenizer_digest.clone(),
                    self.metadata.prompt_shaping_digest.clone(),
                    prompt_tokens,
                    completion_tokens,
                );
                if !sample.tools.is_empty() {
                    capture = capture.with_tool_tokens(count_tool_tokens(sample.tools.as_slice()));
                }
                if let Some(response_format) = sample.response_format.as_ref() {
                    capture = capture
                        .with_response_schema_tokens(count_response_schema_tokens(response_format));
                }
                capture
            })
            .collect())
    }
}

/// Explicit token and lineage capture used for deterministic Apple packing.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppleAdapterSampleTokenCapture {
    /// Sample id from the imported dataset contract.
    pub sample_id: String,
    /// Tokenizer digest used during token counting.
    pub tokenizer_digest: String,
    /// Prompt-shaping digest used during token counting.
    pub prompt_shaping_digest: String,
    /// Prompt-side token count, excluding attached tool or schema tokens.
    pub prompt_tokens: u32,
    /// Completion-side token count.
    pub completion_tokens: u32,
    /// Tool-schema token count attached to the prompt.
    pub tool_tokens: u32,
    /// Guided-generation schema token count attached to the prompt.
    pub response_schema_tokens: u32,
}

impl AppleAdapterSampleTokenCapture {
    /// Creates one token-capture record for packing and lineage checks.
    #[must_use]
    pub fn new(
        sample_id: impl Into<String>,
        tokenizer_digest: impl Into<String>,
        prompt_shaping_digest: impl Into<String>,
        prompt_tokens: u32,
        completion_tokens: u32,
    ) -> Self {
        Self {
            sample_id: sample_id.into(),
            tokenizer_digest: tokenizer_digest.into(),
            prompt_shaping_digest: prompt_shaping_digest.into(),
            prompt_tokens,
            completion_tokens,
            tool_tokens: 0,
            response_schema_tokens: 0,
        }
    }

    /// Attaches explicit tool-schema token count.
    #[must_use]
    pub const fn with_tool_tokens(mut self, tool_tokens: u32) -> Self {
        self.tool_tokens = tool_tokens;
        self
    }

    /// Attaches explicit guided-generation schema token count.
    #[must_use]
    pub const fn with_response_schema_tokens(mut self, response_schema_tokens: u32) -> Self {
        self.response_schema_tokens = response_schema_tokens;
        self
    }

    /// Returns the total packed token count for the sample.
    #[must_use]
    pub const fn total_tokens(&self) -> u32 {
        self.prompt_tokens
            .saturating_add(self.completion_tokens)
            .saturating_add(self.tool_tokens)
            .saturating_add(self.response_schema_tokens)
    }
}

/// Apple dataset import, validation, or packing error.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum AppleAdapterDatasetError {
    /// Unsupported Apple dataset ABI version.
    #[error("unsupported Apple adapter dataset ABI version `{abi_version}`")]
    UnsupportedAbiVersion {
        /// Observed ABI version.
        abi_version: String,
    },
    /// The imported contract has no samples.
    #[error("Apple adapter dataset must contain at least one sample")]
    DatasetHasNoSamples,
    /// Missing tokenizer digest in lineage metadata.
    #[error("Apple adapter dataset metadata is missing `tokenizer.tokenizer_digest`")]
    MissingTokenizerDigest,
    /// Invalid tokenizer vocab size in lineage metadata.
    #[error("Apple adapter dataset metadata requires `tokenizer.vocab_size > 0`")]
    InvalidTokenizerVocabSize,
    /// Missing prompt-shaping digest in lineage metadata.
    #[error("Apple adapter dataset metadata is missing `prompt_shaping_digest`")]
    MissingPromptShapingDigest,
    /// One source line was not valid JSON.
    #[error("Apple adapter dataset line {line_number} is not valid JSON: {message}")]
    JsonLineParse {
        /// Physical line number.
        line_number: usize,
        /// Parser error summary.
        message: String,
    },
    /// One sample was not an array of messages.
    #[error("Apple adapter dataset line {line_number} must be one JSON array of messages")]
    SampleNotMessageArray {
        /// Physical line number.
        line_number: usize,
    },
    /// One sample had zero messages.
    #[error("Apple adapter dataset line {line_number} must contain at least one message")]
    SampleHasNoMessages {
        /// Physical line number.
        line_number: usize,
    },
    /// One message omitted its role.
    #[error("Apple adapter dataset line {line_number} message {message_index} is missing `role`")]
    MessageMissingRole {
        /// Physical line number.
        line_number: usize,
        /// Zero-based message index.
        message_index: usize,
    },
    /// One message used an unsupported role string.
    #[error(
        "Apple adapter dataset line {line_number} message {message_index} used unsupported role `{role}`"
    )]
    UnsupportedMessageRole {
        /// Physical line number.
        line_number: usize,
        /// Zero-based message index.
        message_index: usize,
        /// Observed role string.
        role: String,
    },
    /// One message omitted content.
    #[error(
        "Apple adapter dataset line {line_number} message {message_index} is missing `content`"
    )]
    MessageMissingContent {
        /// Physical line number.
        line_number: usize,
        /// Zero-based message index.
        message_index: usize,
    },
    /// One message used empty content.
    #[error(
        "Apple adapter dataset line {line_number} message {message_index} must not use empty `content`"
    )]
    EmptyMessageContent {
        /// Physical line number.
        line_number: usize,
        /// Zero-based message index.
        message_index: usize,
    },
    /// A system message appeared after index zero.
    #[error(
        "Apple adapter dataset line {line_number} message {message_index} uses `system` after index 0"
    )]
    SystemMessageNotFirst {
        /// Physical line number.
        line_number: usize,
        /// Zero-based message index.
        message_index: usize,
    },
    /// One sample carried multiple system messages.
    #[error(
        "Apple adapter dataset line {line_number} message {message_index} repeats the `system` role"
    )]
    DuplicateSystemMessage {
        /// Physical line number.
        line_number: usize,
        /// Zero-based message index.
        message_index: usize,
    },
    /// Consecutive identical roles are not admitted.
    #[error(
        "Apple adapter dataset line {line_number} message {message_index} repeats consecutive role `{role}`"
    )]
    ConsecutiveDuplicateRole {
        /// Physical line number.
        line_number: usize,
        /// Zero-based message index.
        message_index: usize,
        /// Repeated role.
        role: AppleAdapterMessageRole,
    },
    /// An assistant message appeared before the first user or did not directly follow a user.
    #[error(
        "Apple adapter dataset line {line_number} message {message_index} uses `assistant` before or without a preceding `user`"
    )]
    AssistantBeforeUser {
        /// Physical line number.
        line_number: usize,
        /// Zero-based message index.
        message_index: usize,
    },
    /// The final message was not an assistant completion.
    #[error("Apple adapter dataset line {line_number} must end with `assistant`, found `{role}`")]
    FinalMessageNotAssistant {
        /// Physical line number.
        line_number: usize,
        /// Observed final role.
        role: AppleAdapterMessageRole,
    },
    /// The sample did not contain a valid user/assistant pair.
    #[error(
        "Apple adapter dataset line {line_number} must contain at least one user/assistant pair"
    )]
    MissingUserAssistantPair {
        /// Physical line number.
        line_number: usize,
    },
    /// Tools were attached to a non-system message.
    #[error(
        "Apple adapter dataset line {line_number} message {message_index} may only attach `tools` to the first system message"
    )]
    ToolsOnNonSystemMessage {
        /// Physical line number.
        line_number: usize,
        /// Zero-based message index.
        message_index: usize,
    },
    /// One tool omitted its type.
    #[error(
        "Apple adapter dataset line {line_number} message {message_index} tool {tool_index} is missing `type`"
    )]
    MissingToolType {
        /// Physical line number.
        line_number: usize,
        /// Zero-based message index.
        message_index: usize,
        /// Zero-based tool index.
        tool_index: usize,
    },
    /// One tool used an unsupported type.
    #[error(
        "Apple adapter dataset line {line_number} message {message_index} tool {tool_index} used unsupported type `{tool_type}`"
    )]
    UnsupportedToolType {
        /// Physical line number.
        line_number: usize,
        /// Zero-based message index.
        message_index: usize,
        /// Zero-based tool index.
        tool_index: usize,
        /// Observed type string.
        tool_type: String,
    },
    /// One tool omitted its function payload.
    #[error(
        "Apple adapter dataset line {line_number} message {message_index} tool {tool_index} is missing `function`"
    )]
    MissingToolFunction {
        /// Physical line number.
        line_number: usize,
        /// Zero-based message index.
        message_index: usize,
        /// Zero-based tool index.
        tool_index: usize,
    },
    /// One tool omitted its function name.
    #[error(
        "Apple adapter dataset line {line_number} message {message_index} tool {tool_index} is missing `function.name`"
    )]
    MissingToolFunctionName {
        /// Physical line number.
        line_number: usize,
        /// Zero-based message index.
        message_index: usize,
        /// Zero-based tool index.
        tool_index: usize,
    },
    /// One tool omitted its argument schema.
    #[error(
        "Apple adapter dataset line {line_number} message {message_index} tool {tool_index} is missing `function.arguments`"
    )]
    MissingToolArguments {
        /// Physical line number.
        line_number: usize,
        /// Zero-based message index.
        message_index: usize,
        /// Zero-based tool index.
        tool_index: usize,
    },
    /// One tool used an invalid argument schema.
    #[error(
        "Apple adapter dataset line {line_number} message {message_index} tool {tool_index} must use an object schema in `function.arguments`"
    )]
    InvalidToolArgumentsSchema {
        /// Physical line number.
        line_number: usize,
        /// Zero-based message index.
        message_index: usize,
        /// Zero-based tool index.
        tool_index: usize,
    },
    /// A response format was attached to a non-user message.
    #[error(
        "Apple adapter dataset line {line_number} message {message_index} may only attach `response_format` to `user` messages"
    )]
    ResponseFormatOnNonUserMessage {
        /// Physical line number.
        line_number: usize,
        /// Zero-based message index.
        message_index: usize,
    },
    /// One message omitted `response_format.type`.
    #[error(
        "Apple adapter dataset line {line_number} message {message_index} is missing `response_format.type`"
    )]
    MissingResponseFormatType {
        /// Physical line number.
        line_number: usize,
        /// Zero-based message index.
        message_index: usize,
    },
    /// One message used an unsupported response-format type.
    #[error(
        "Apple adapter dataset line {line_number} message {message_index} used unsupported `response_format.type` `{format_type}`"
    )]
    UnsupportedResponseFormatType {
        /// Physical line number.
        line_number: usize,
        /// Zero-based message index.
        message_index: usize,
        /// Observed type string.
        format_type: String,
    },
    /// One response format omitted `json_schema`.
    #[error(
        "Apple adapter dataset line {line_number} message {message_index} is missing `response_format.json_schema`"
    )]
    MissingResponseSchemaDefinition {
        /// Physical line number.
        line_number: usize,
        /// Zero-based message index.
        message_index: usize,
    },
    /// One response format omitted `json_schema.name`.
    #[error(
        "Apple adapter dataset line {line_number} message {message_index} is missing `response_format.json_schema.name`"
    )]
    MissingResponseSchemaName {
        /// Physical line number.
        line_number: usize,
        /// Zero-based message index.
        message_index: usize,
    },
    /// One response format omitted `json_schema.schema`.
    #[error(
        "Apple adapter dataset line {line_number} message {message_index} is missing `response_format.json_schema.schema`"
    )]
    MissingResponseSchema {
        /// Physical line number.
        line_number: usize,
        /// Zero-based message index.
        message_index: usize,
    },
    /// One sample attached more than one response format.
    #[error("Apple adapter dataset line {line_number} attaches more than one `response_format`")]
    MultipleResponseFormats {
        /// Physical line number.
        line_number: usize,
    },
    /// One structured assistant payload was not valid JSON.
    #[error(
        "Apple adapter dataset line {line_number} message {message_index} must use valid JSON assistant content for structured output: {message}"
    )]
    StructuredAssistantContentInvalidJson {
        /// Physical line number.
        line_number: usize,
        /// Zero-based message index.
        message_index: usize,
        /// Parser error summary.
        message: String,
    },
    /// One structured assistant payload was valid JSON but not an object.
    #[error(
        "Apple adapter dataset line {line_number} message {message_index} must use a JSON object for structured output"
    )]
    StructuredAssistantContentMustBeObject {
        /// Physical line number.
        line_number: usize,
        /// Zero-based message index.
        message_index: usize,
    },
    /// One imported sample omitted its synthetic id.
    #[error("Apple adapter dataset imported a sample without a `sample_id`")]
    MissingSampleId,
    /// Duplicate sample ids are not admitted.
    #[error("Apple adapter dataset repeated sample id `{sample_id}`")]
    DuplicateSampleId {
        /// Repeated sample id.
        sample_id: String,
    },
    /// A token capture was missing for one sample.
    #[error("Apple adapter packing is missing token capture for sample `{sample_id}`")]
    MissingTokenCapture {
        /// Sample id.
        sample_id: String,
    },
    /// The caller supplied duplicate token captures.
    #[error("Apple adapter packing repeated token capture for sample `{sample_id}`")]
    DuplicateTokenCapture {
        /// Sample id.
        sample_id: String,
    },
    /// The caller supplied a token capture for an unknown sample.
    #[error("Apple adapter packing received token capture for unknown sample `{sample_id}`")]
    UnknownTokenCapture {
        /// Sample id.
        sample_id: String,
    },
    /// Prompt token count must be greater than zero.
    #[error("Apple adapter sample `{sample_id}` requires `prompt_tokens > 0`")]
    InvalidPromptTokenCount {
        /// Sample id.
        sample_id: String,
    },
    /// Completion token count must be greater than zero.
    #[error("Apple adapter sample `{sample_id}` requires `completion_tokens > 0`")]
    InvalidCompletionTokenCount {
        /// Sample id.
        sample_id: String,
    },
    /// Tool-using samples must include tool token capture.
    #[error(
        "Apple adapter sample `{sample_id}` uses tools and requires explicit `tool_tokens` capture"
    )]
    MissingToolTokenCapture {
        /// Sample id.
        sample_id: String,
    },
    /// Schema-guided samples must include response-schema token capture.
    #[error(
        "Apple adapter sample `{sample_id}` uses `response_format` and requires explicit `response_schema_tokens` capture"
    )]
    MissingResponseSchemaTokenCapture {
        /// Sample id.
        sample_id: String,
    },
    /// Tokenizer lineage drifted between dataset import and token capture.
    #[error(
        "Apple adapter sample `{sample_id}` expected tokenizer digest `{expected}` but packing used `{actual}`"
    )]
    TokenizerDrift {
        /// Sample id.
        sample_id: String,
        /// Dataset tokenizer digest.
        expected: String,
        /// Capture tokenizer digest.
        actual: String,
    },
    /// Prompt shaping drifted between dataset import and token capture.
    #[error(
        "Apple adapter sample `{sample_id}` expected prompt shaping digest `{expected}` but packing used `{actual}`"
    )]
    PromptShapingDrift {
        /// Sample id.
        sample_id: String,
        /// Dataset prompt-shaping digest.
        expected: String,
        /// Capture prompt-shaping digest.
        actual: String,
    },
    /// Generic dataset packing refused the derived sequences.
    #[error(transparent)]
    PackingContract(#[from] DatasetContractError),
}

#[derive(Clone, Debug, Deserialize)]
struct RawAppleAdapterMessage {
    role: Option<String>,
    content: Option<String>,
    #[serde(default)]
    response_format: Option<RawAppleAdapterResponseFormat>,
    #[serde(default)]
    tools: Option<Vec<RawAppleAdapterToolDefinition>>,
}

#[derive(Clone, Debug, Deserialize)]
struct RawAppleAdapterResponseFormat {
    #[serde(rename = "type")]
    format_type: Option<String>,
    #[serde(default)]
    json_schema: Option<RawAppleAdapterJsonSchemaDefinition>,
}

#[derive(Clone, Debug, Deserialize)]
struct RawAppleAdapterJsonSchemaDefinition {
    name: Option<String>,
    schema: Option<Value>,
}

#[derive(Clone, Debug, Deserialize)]
struct RawAppleAdapterToolDefinition {
    #[serde(rename = "type")]
    tool_type: Option<String>,
    #[serde(default)]
    function: Option<RawAppleAdapterToolFunctionDefinition>,
}

#[derive(Clone, Debug, Deserialize)]
struct RawAppleAdapterToolFunctionDefinition {
    name: Option<String>,
    #[serde(default)]
    description: Option<String>,
    arguments: Option<Value>,
}

fn parse_sample(
    raw_messages: Vec<RawAppleAdapterMessage>,
    line_number: usize,
) -> Result<AppleAdapterTrainingSample, AppleAdapterDatasetError> {
    if raw_messages.is_empty() {
        return Err(AppleAdapterDatasetError::SampleHasNoMessages { line_number });
    }

    let mut messages = Vec::with_capacity(raw_messages.len());
    let mut saw_user = false;
    let mut saw_user_assistant_pair = false;
    let mut system_seen = false;
    let mut previous_role = None;
    let mut derived_tools = Vec::new();
    let mut derived_response_format = None;

    for (message_index, raw_message) in raw_messages.into_iter().enumerate() {
        let role = parse_role(raw_message.role, line_number, message_index)?;
        let content = parse_content(raw_message.content, line_number, message_index)?;

        if role == AppleAdapterMessageRole::System {
            if message_index != 0 {
                return Err(AppleAdapterDatasetError::SystemMessageNotFirst {
                    line_number,
                    message_index,
                });
            }
            if system_seen {
                return Err(AppleAdapterDatasetError::DuplicateSystemMessage {
                    line_number,
                    message_index,
                });
            }
            system_seen = true;
        }

        if previous_role == Some(role) {
            return Err(AppleAdapterDatasetError::ConsecutiveDuplicateRole {
                line_number,
                message_index,
                role,
            });
        }

        let tools = if let Some(raw_tools) = raw_message.tools {
            if role != AppleAdapterMessageRole::System || message_index != 0 {
                return Err(AppleAdapterDatasetError::ToolsOnNonSystemMessage {
                    line_number,
                    message_index,
                });
            }
            let parsed_tools = parse_tools(raw_tools, line_number, message_index)?;
            derived_tools = parsed_tools.clone();
            parsed_tools
        } else {
            Vec::new()
        };

        let response_format = if let Some(raw_response_format) = raw_message.response_format {
            if role != AppleAdapterMessageRole::User {
                return Err(AppleAdapterDatasetError::ResponseFormatOnNonUserMessage {
                    line_number,
                    message_index,
                });
            }
            if derived_response_format.is_some() {
                return Err(AppleAdapterDatasetError::MultipleResponseFormats { line_number });
            }
            let parsed_response_format =
                parse_response_format(raw_response_format, line_number, message_index)?;
            derived_response_format = Some(parsed_response_format.clone());
            Some(parsed_response_format)
        } else {
            None
        };

        if role == AppleAdapterMessageRole::Assistant {
            if !saw_user || previous_role != Some(AppleAdapterMessageRole::User) {
                return Err(AppleAdapterDatasetError::AssistantBeforeUser {
                    line_number,
                    message_index,
                });
            }
            saw_user_assistant_pair = true;
        }
        if role == AppleAdapterMessageRole::User {
            saw_user = true;
        }

        previous_role = Some(role);
        messages.push(AppleAdapterMessage {
            role,
            content,
            response_format,
            tools,
        });
    }

    let Some(final_message) = messages.last() else {
        return Err(AppleAdapterDatasetError::SampleHasNoMessages { line_number });
    };
    if final_message.role != AppleAdapterMessageRole::Assistant {
        return Err(AppleAdapterDatasetError::FinalMessageNotAssistant {
            line_number,
            role: final_message.role,
        });
    }
    if !saw_user_assistant_pair {
        return Err(AppleAdapterDatasetError::MissingUserAssistantPair { line_number });
    }

    let structured_assistant_output = parse_structured_assistant_output(
        final_message.content.as_str(),
        line_number,
        messages.len() - 1,
        derived_response_format.is_some(),
    )?;
    let sample_kind = derive_sample_kind(
        !derived_tools.is_empty(),
        derived_response_format.is_some(),
        structured_assistant_output.is_some(),
    );
    let stable_digest = stable_sample_digest(
        sample_kind,
        messages.as_slice(),
        derived_tools.as_slice(),
        derived_response_format.as_ref(),
        structured_assistant_output.as_ref(),
    );

    Ok(AppleAdapterTrainingSample {
        sample_id: format!("sample-{line_number:06}"),
        source_line_number: line_number,
        stable_digest,
        sample_kind,
        messages,
        tools: derived_tools,
        response_format: derived_response_format,
        structured_assistant_output,
    })
}

fn parse_role(
    raw_role: Option<String>,
    line_number: usize,
    message_index: usize,
) -> Result<AppleAdapterMessageRole, AppleAdapterDatasetError> {
    let Some(role) = raw_role else {
        return Err(AppleAdapterDatasetError::MessageMissingRole {
            line_number,
            message_index,
        });
    };
    match role.as_str() {
        "system" => Ok(AppleAdapterMessageRole::System),
        "user" => Ok(AppleAdapterMessageRole::User),
        "assistant" => Ok(AppleAdapterMessageRole::Assistant),
        _ => Err(AppleAdapterDatasetError::UnsupportedMessageRole {
            line_number,
            message_index,
            role,
        }),
    }
}

fn parse_content(
    raw_content: Option<String>,
    line_number: usize,
    message_index: usize,
) -> Result<String, AppleAdapterDatasetError> {
    let Some(content) = raw_content else {
        return Err(AppleAdapterDatasetError::MessageMissingContent {
            line_number,
            message_index,
        });
    };
    if content.trim().is_empty() {
        return Err(AppleAdapterDatasetError::EmptyMessageContent {
            line_number,
            message_index,
        });
    }
    Ok(content)
}

fn parse_tools(
    raw_tools: Vec<RawAppleAdapterToolDefinition>,
    line_number: usize,
    message_index: usize,
) -> Result<Vec<AppleAdapterToolDefinition>, AppleAdapterDatasetError> {
    let mut tools = Vec::with_capacity(raw_tools.len());
    for (tool_index, raw_tool) in raw_tools.into_iter().enumerate() {
        let Some(tool_type) = raw_tool.tool_type else {
            return Err(AppleAdapterDatasetError::MissingToolType {
                line_number,
                message_index,
                tool_index,
            });
        };
        if tool_type != "function" {
            return Err(AppleAdapterDatasetError::UnsupportedToolType {
                line_number,
                message_index,
                tool_index,
                tool_type,
            });
        }
        let Some(function) = raw_tool.function else {
            return Err(AppleAdapterDatasetError::MissingToolFunction {
                line_number,
                message_index,
                tool_index,
            });
        };
        let Some(name) = function.name else {
            return Err(AppleAdapterDatasetError::MissingToolFunctionName {
                line_number,
                message_index,
                tool_index,
            });
        };
        if name.trim().is_empty() {
            return Err(AppleAdapterDatasetError::MissingToolFunctionName {
                line_number,
                message_index,
                tool_index,
            });
        }
        let Some(arguments) = function.arguments else {
            return Err(AppleAdapterDatasetError::MissingToolArguments {
                line_number,
                message_index,
                tool_index,
            });
        };
        validate_tool_arguments_schema(&arguments, line_number, message_index, tool_index)?;
        tools.push(AppleAdapterToolDefinition {
            tool_type: AppleAdapterToolType::Function,
            function: AppleAdapterToolFunctionDefinition {
                name,
                description: function.description,
                arguments,
            },
        });
    }
    Ok(tools)
}

fn validate_tool_arguments_schema(
    arguments: &Value,
    line_number: usize,
    message_index: usize,
    tool_index: usize,
) -> Result<(), AppleAdapterDatasetError> {
    let Some(arguments_object) = arguments.as_object() else {
        return Err(AppleAdapterDatasetError::InvalidToolArgumentsSchema {
            line_number,
            message_index,
            tool_index,
        });
    };
    if arguments_object.get("type").and_then(Value::as_str) != Some("object") {
        return Err(AppleAdapterDatasetError::InvalidToolArgumentsSchema {
            line_number,
            message_index,
            tool_index,
        });
    }
    Ok(())
}

fn parse_response_format(
    raw_response_format: RawAppleAdapterResponseFormat,
    line_number: usize,
    message_index: usize,
) -> Result<AppleAdapterResponseFormat, AppleAdapterDatasetError> {
    let Some(format_type) = raw_response_format.format_type else {
        return Err(AppleAdapterDatasetError::MissingResponseFormatType {
            line_number,
            message_index,
        });
    };
    if format_type != "json_schema" {
        return Err(AppleAdapterDatasetError::UnsupportedResponseFormatType {
            line_number,
            message_index,
            format_type,
        });
    }
    let Some(raw_json_schema) = raw_response_format.json_schema else {
        return Err(AppleAdapterDatasetError::MissingResponseSchemaDefinition {
            line_number,
            message_index,
        });
    };
    let Some(name) = raw_json_schema.name else {
        return Err(AppleAdapterDatasetError::MissingResponseSchemaName {
            line_number,
            message_index,
        });
    };
    if name.trim().is_empty() {
        return Err(AppleAdapterDatasetError::MissingResponseSchemaName {
            line_number,
            message_index,
        });
    }
    let Some(schema) = raw_json_schema.schema else {
        return Err(AppleAdapterDatasetError::MissingResponseSchema {
            line_number,
            message_index,
        });
    };
    Ok(AppleAdapterResponseFormat {
        json_schema: AppleAdapterJsonSchemaDefinition { name, schema },
    })
}

fn parse_structured_assistant_output(
    assistant_content: &str,
    line_number: usize,
    message_index: usize,
    require_json_object: bool,
) -> Result<Option<Value>, AppleAdapterDatasetError> {
    let trimmed = assistant_content.trim();
    let looks_like_json = trimmed.starts_with('{');
    if !looks_like_json && !require_json_object {
        return Ok(None);
    }
    let parsed = serde_json::from_str::<Value>(trimmed).map_err(|err| {
        AppleAdapterDatasetError::StructuredAssistantContentInvalidJson {
            line_number,
            message_index,
            message: err.to_string(),
        }
    })?;
    if !parsed.is_object() {
        return Err(
            AppleAdapterDatasetError::StructuredAssistantContentMustBeObject {
                line_number,
                message_index,
            },
        );
    }
    Ok(Some(parsed))
}

fn derive_sample_kind(
    has_tools: bool,
    has_response_format: bool,
    has_structured_assistant_output: bool,
) -> AppleAdapterSampleKind {
    if has_tools {
        AppleAdapterSampleKind::ToolCalling
    } else if has_response_format {
        AppleAdapterSampleKind::GuidedGenerationWithSchema
    } else if has_structured_assistant_output {
        AppleAdapterSampleKind::SchemaFreeGuidedGeneration
    } else {
        AppleAdapterSampleKind::SupervisedFineTune
    }
}

fn count_dataset_context_tokens(metadata: &AppleAdapterDatasetMetadata) -> u32 {
    let mut token_count = 0_u32;
    if let Some(default_instruction) = &metadata.default_instruction {
        token_count = token_count
            .saturating_add(context_marker_tokens("default_instruction"))
            .saturating_add(lexical_token_count(default_instruction));
    }
    if let Some(locale) = &metadata.locale {
        token_count = token_count
            .saturating_add(context_marker_tokens("locale"))
            .saturating_add(lexical_token_count(locale));
    }
    token_count
}

fn count_message_tokens(message: &AppleAdapterMessage) -> u32 {
    context_marker_tokens(message.role.label())
        .saturating_add(lexical_token_count(message.content.as_str()))
}

fn count_tool_tokens(tools: &[AppleAdapterToolDefinition]) -> u32 {
    context_marker_tokens("tools")
        .saturating_add(lexical_token_count(canonical_json(tools).as_str()))
}

fn count_response_schema_tokens(response_format: &AppleAdapterResponseFormat) -> u32 {
    context_marker_tokens("response_schema").saturating_add(lexical_token_count(
        canonical_json(response_format).as_str(),
    ))
}

fn context_marker_tokens(label: &str) -> u32 {
    1_u32.saturating_add(lexical_token_count(label))
}

fn lexical_token_count(input: &str) -> u32 {
    let mut token_count = 0_u32;
    let mut in_word = false;
    for ch in input.chars() {
        if ch.is_whitespace() {
            if in_word {
                token_count = token_count.saturating_add(1);
                in_word = false;
            }
            continue;
        }

        let is_word_character = ch.is_alphanumeric() || ch == '_';
        if is_word_character {
            in_word = true;
            continue;
        }

        if in_word {
            token_count = token_count.saturating_add(1);
            in_word = false;
        }
        token_count = token_count.saturating_add(1);
    }
    if in_word {
        token_count = token_count.saturating_add(1);
    }
    token_count
}

fn stable_sample_digest(
    sample_kind: AppleAdapterSampleKind,
    messages: &[AppleAdapterMessage],
    tools: &[AppleAdapterToolDefinition],
    response_format: Option<&AppleAdapterResponseFormat>,
    structured_assistant_output: Option<&Value>,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_apple_adapter_sample|");
    hasher.update(match sample_kind {
        AppleAdapterSampleKind::SupervisedFineTune => b"sft".as_slice(),
        AppleAdapterSampleKind::SchemaFreeGuidedGeneration => b"schema_free".as_slice(),
        AppleAdapterSampleKind::GuidedGenerationWithSchema => b"guided_generation".as_slice(),
        AppleAdapterSampleKind::ToolCalling => b"tool_calling".as_slice(),
    });
    hasher.update(b"|messages|");
    hasher.update(canonical_json(messages).as_bytes());
    if !tools.is_empty() {
        hasher.update(b"|tools|");
        hasher.update(canonical_json(tools).as_bytes());
    }
    if let Some(response_format) = response_format {
        hasher.update(b"|response_format|");
        hasher.update(canonical_json(response_format).as_bytes());
    }
    if let Some(structured_assistant_output) = structured_assistant_output {
        hasher.update(b"|structured|");
        hasher.update(canonical_json(structured_assistant_output).as_bytes());
    }
    hex::encode(hasher.finalize())
}

fn stable_hex_digest(prefix: &[u8], segments: &[&str]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    for segment in segments {
        hasher.update(segment.as_bytes());
        hasher.update(b"|");
    }
    hex::encode(hasher.finalize())
}

fn truncated_hex_digest(prefix: &[u8], segments: &[&str], hex_chars: usize) -> String {
    let digest = stable_hex_digest(prefix, segments);
    let truncate_at = hex_chars.min(digest.len());
    digest[..truncate_at].to_string()
}

fn canonical_json<T>(value: T) -> String
where
    T: Serialize,
{
    serde_json::to_string(&value).expect("Apple adapter dataset values should serialize")
}

fn dataset_record_encoding_label(record_encoding: DatasetRecordEncoding) -> &'static [u8] {
    match record_encoding {
        DatasetRecordEncoding::JsonlText => b"jsonl_text",
        DatasetRecordEncoding::JsonlConversation => b"jsonl_conversation",
        DatasetRecordEncoding::TokenIdsLeU32 => b"token_ids_le_u32",
        DatasetRecordEncoding::PreferenceJsonl => b"preference_jsonl",
        DatasetRecordEncoding::Binary => b"binary",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DatasetPackingMode, OverlongSequencePosture, TokenizerFamily};

    fn sample_metadata() -> AppleAdapterDatasetMetadata {
        AppleAdapterDatasetMetadata::new(
            TokenizerDigest::new(
                TokenizerFamily::SentencePiece,
                "apple-tokenizer-digest-v1",
                32_768,
            )
            .with_special_tokens_digest("apple-special-tokens-v1")
            .with_template_digest("apple-template-v1"),
            "apple-prompt-shaping-v1",
        )
        .with_default_instruction("A conversation between a user and a helpful assistant.")
        .with_locale("en-US")
    }

    fn import_fixture(path: &str) -> AppleAdapterDatasetContract {
        AppleAdapterDatasetContract::from_jsonl_str(
            match path {
                "minimal" => include_str!("../../fixtures/apple_adapter/datasets/minimal_sft_train.jsonl"),
                "schema_free" => include_str!(
                    "../../fixtures/apple_adapter/datasets/schema_free_guided_generation_train.jsonl"
                ),
                "guided" => include_str!(
                    "../../fixtures/apple_adapter/datasets/guided_generation_with_schema_train.jsonl"
                ),
                "tools" => include_str!("../../fixtures/apple_adapter/datasets/tool_calling_train.jsonl"),
                _ => unreachable!("unsupported fixture id"),
            },
            sample_metadata(),
        )
        .expect("fixture should import")
    }

    #[test]
    fn apple_adapter_positive_fixtures_import_into_typed_records() {
        let minimal = import_fixture("minimal");
        assert_eq!(minimal.samples.len(), 1);
        assert_eq!(
            minimal.samples[0].sample_kind,
            AppleAdapterSampleKind::SupervisedFineTune
        );
        assert!(minimal.samples[0].response_format.is_none());
        assert!(minimal.samples[0].tools.is_empty());
        assert!(minimal.samples[0].structured_assistant_output.is_none());

        let schema_free = import_fixture("schema_free");
        assert_eq!(
            schema_free.samples[0].sample_kind,
            AppleAdapterSampleKind::SchemaFreeGuidedGeneration
        );
        assert_eq!(
            schema_free.samples[0]
                .structured_assistant_output
                .as_ref()
                .and_then(|value| value.get("title"))
                .and_then(Value::as_str),
            Some("Day at the Beach")
        );

        let guided = import_fixture("guided");
        assert_eq!(
            guided.samples[0].sample_kind,
            AppleAdapterSampleKind::GuidedGenerationWithSchema
        );
        assert_eq!(
            guided.samples[0]
                .response_format
                .as_ref()
                .map(|format| format.json_schema.name.as_str()),
            Some("Response<Date>")
        );
        assert_eq!(
            guided.samples[0]
                .structured_assistant_output
                .as_ref()
                .and_then(|value| value.get("response"))
                .and_then(Value::as_object)
                .and_then(|response| response.get("year"))
                .and_then(Value::as_i64),
            Some(1976)
        );

        let tools = import_fixture("tools");
        assert_eq!(
            tools.samples[0].sample_kind,
            AppleAdapterSampleKind::ToolCalling
        );
        assert_eq!(tools.samples[0].tools.len(), 2);
        assert_eq!(
            tools.samples[0].tools[0].function.name,
            "get_current_weather"
        );
        assert_eq!(tools.samples[0].tools[1].function.name, "lookup_stock");
    }

    #[test]
    fn apple_adapter_negative_fixtures_fail_with_typed_reasons() {
        let invalid_system = AppleAdapterDatasetContract::from_jsonl_str(
            include_str!("../../fixtures/apple_adapter/datasets/invalid_system_not_first.jsonl"),
            sample_metadata(),
        )
        .expect_err("fixture should fail");
        assert!(matches!(
            invalid_system,
            AppleAdapterDatasetError::SystemMessageNotFirst { .. }
        ));

        let invalid_last = AppleAdapterDatasetContract::from_jsonl_str(
            include_str!("../../fixtures/apple_adapter/datasets/invalid_assistant_not_last.jsonl"),
            sample_metadata(),
        )
        .expect_err("fixture should fail");
        assert!(matches!(
            invalid_last,
            AppleAdapterDatasetError::AssistantBeforeUser { .. }
        ));

        let invalid_duplicate = AppleAdapterDatasetContract::from_jsonl_str(
            include_str!("../../fixtures/apple_adapter/datasets/invalid_duplicate_roles.jsonl"),
            sample_metadata(),
        )
        .expect_err("fixture should fail");
        assert!(matches!(
            invalid_duplicate,
            AppleAdapterDatasetError::ConsecutiveDuplicateRole { .. }
        ));

        let invalid_response_schema = AppleAdapterDatasetContract::from_jsonl_str(
            include_str!(
                "../../fixtures/apple_adapter/datasets/invalid_missing_response_schema.jsonl"
            ),
            sample_metadata(),
        )
        .expect_err("fixture should fail");
        assert!(matches!(
            invalid_response_schema,
            AppleAdapterDatasetError::MissingResponseSchemaDefinition { .. }
        ));

        let invalid_tool = AppleAdapterDatasetContract::from_jsonl_str(
            include_str!("../../fixtures/apple_adapter/datasets/invalid_tool_definition.jsonl"),
            sample_metadata(),
        )
        .expect_err("fixture should fail");
        assert!(matches!(
            invalid_tool,
            AppleAdapterDatasetError::MissingToolFunctionName { .. }
        ));
    }

    #[test]
    fn apple_adapter_packing_is_deterministic_and_uses_explicit_lineage() {
        let input = format!(
            "{}\n{}\n{}",
            include_str!("../../fixtures/apple_adapter/datasets/minimal_sft_train.jsonl").trim(),
            include_str!(
                "../../fixtures/apple_adapter/datasets/guided_generation_with_schema_train.jsonl"
            )
            .trim(),
            include_str!("../../fixtures/apple_adapter/datasets/tool_calling_train.jsonl").trim()
        );
        let dataset =
            AppleAdapterDatasetContract::from_jsonl_str(input.as_str(), sample_metadata())
                .expect("dataset should import");
        let policy =
            DatasetPackingPolicy::new(DatasetPackingMode::PackIntoContextWindow, 96, 192, 2)
                .with_pad_to_multiple_of(8)
                .with_overlong_sequence_posture(OverlongSequencePosture::Refuse);
        let captures = vec![
            AppleAdapterSampleTokenCapture::new(
                dataset.samples[0].sample_id.clone(),
                dataset.metadata.tokenizer.tokenizer_digest.clone(),
                dataset.metadata.prompt_shaping_digest.clone(),
                28,
                18,
            ),
            AppleAdapterSampleTokenCapture::new(
                dataset.samples[1].sample_id.clone(),
                dataset.metadata.tokenizer.tokenizer_digest.clone(),
                dataset.metadata.prompt_shaping_digest.clone(),
                30,
                14,
            )
            .with_response_schema_tokens(22),
            AppleAdapterSampleTokenCapture::new(
                dataset.samples[2].sample_id.clone(),
                dataset.metadata.tokenizer.tokenizer_digest.clone(),
                dataset.metadata.prompt_shaping_digest.clone(),
                26,
                16,
            )
            .with_tool_tokens(24),
        ];

        let first_plan = dataset
            .plan_packing(captures.as_slice(), &policy)
            .expect("packing should succeed");
        let second_plan = dataset
            .plan_packing(captures.as_slice(), &policy)
            .expect("packing should succeed");
        assert_eq!(first_plan, second_plan);
        assert_eq!(first_plan.total_source_sequences, 3);
        assert_eq!(first_plan.batches.len(), 2);
        assert_eq!(first_plan.batches[0].rows.len(), 2);
        assert_eq!(first_plan.batches[0].rows[0].source_sequences.len(), 1);
    }

    #[test]
    fn apple_adapter_packing_refuses_tokenizer_and_prompt_shaping_drift() {
        let dataset = import_fixture("guided");
        let policy = DatasetPackingPolicy::new(DatasetPackingMode::BatchByTokenBudget, 128, 128, 4);
        let drifted_tokenizer = vec![
            AppleAdapterSampleTokenCapture::new(
                dataset.samples[0].sample_id.clone(),
                "other-tokenizer",
                dataset.metadata.prompt_shaping_digest.clone(),
                30,
                14,
            )
            .with_response_schema_tokens(22),
        ];
        let tokenizer_err = dataset
            .plan_packing(drifted_tokenizer.as_slice(), &policy)
            .expect_err("tokenizer drift should fail");
        assert!(matches!(
            tokenizer_err,
            AppleAdapterDatasetError::TokenizerDrift { .. }
        ));

        let drifted_prompt = vec![
            AppleAdapterSampleTokenCapture::new(
                dataset.samples[0].sample_id.clone(),
                dataset.metadata.tokenizer.tokenizer_digest.clone(),
                "other-prompt-shaping",
                30,
                14,
            )
            .with_response_schema_tokens(22),
        ];
        let prompt_err = dataset
            .plan_packing(drifted_prompt.as_slice(), &policy)
            .expect_err("prompt-shaping drift should fail");
        assert!(matches!(
            prompt_err,
            AppleAdapterDatasetError::PromptShapingDrift { .. }
        ));
    }

    #[test]
    fn apple_adapter_packing_requires_tool_and_schema_token_capture() {
        let tools = import_fixture("tools");
        let policy = DatasetPackingPolicy::new(DatasetPackingMode::BatchByTokenBudget, 128, 128, 4);
        let tool_capture = vec![AppleAdapterSampleTokenCapture::new(
            tools.samples[0].sample_id.clone(),
            tools.metadata.tokenizer.tokenizer_digest.clone(),
            tools.metadata.prompt_shaping_digest.clone(),
            26,
            16,
        )];
        let tool_err = tools
            .plan_packing(tool_capture.as_slice(), &policy)
            .expect_err("missing tool capture should fail");
        assert!(matches!(
            tool_err,
            AppleAdapterDatasetError::MissingToolTokenCapture { .. }
        ));

        let guided = import_fixture("guided");
        let guided_capture = vec![AppleAdapterSampleTokenCapture::new(
            guided.samples[0].sample_id.clone(),
            guided.metadata.tokenizer.tokenizer_digest.clone(),
            guided.metadata.prompt_shaping_digest.clone(),
            30,
            14,
        )];
        let schema_err = guided
            .plan_packing(guided_capture.as_slice(), &policy)
            .expect_err("missing schema capture should fail");
        assert!(matches!(
            schema_err,
            AppleAdapterDatasetError::MissingResponseSchemaTokenCapture { .. }
        ));
    }

    #[test]
    fn runtime_profile_derives_stable_lineage_metadata() {
        let profile = AppleAdapterRuntimeCompatibilityProfile::new(
            "apple-foundation-model",
            "general",
            "default",
        )
        .with_locale("en-US")
        .with_default_instruction(APPLE_ADAPTER_DEFAULT_INSTRUCTION)
        .with_bridge_version("1.0.0")
        .with_bridge_platform("macOS");
        let metadata = profile.dataset_metadata();

        assert_eq!(profile.base_model_signature().len(), 40);
        assert_eq!(metadata.tokenizer.family, TokenizerFamily::Custom);
        assert_eq!(
            metadata.tokenizer.template_digest.as_deref(),
            Some(metadata.prompt_shaping_digest.as_str())
        );
        assert_eq!(
            metadata.default_instruction.as_deref(),
            Some(APPLE_ADAPTER_DEFAULT_INSTRUCTION)
        );
        assert_eq!(metadata.locale.as_deref(), Some("en-US"));

        let changed_locale = profile.clone().with_locale("fr-FR");
        assert_eq!(
            profile.base_model_signature(),
            changed_locale.base_model_signature()
        );
        assert_ne!(
            metadata.prompt_shaping_digest,
            changed_locale.prompt_shaping_digest()
        );
        assert_ne!(
            metadata.tokenizer.tokenizer_digest,
            changed_locale.tokenizer_digest().tokenizer_digest
        );
    }

    #[test]
    fn derived_token_captures_follow_repo_owned_preprocessing_path() {
        let multi_turn = AppleAdapterDatasetContract::from_jsonl_str(
            r#"[{"role":"user","content":"Explain what a mutex does."},{"role":"assistant","content":"A mutex guards shared state."},{"role":"user","content":"Now say it in one sentence."},{"role":"assistant","content":"A mutex lets only one thread access shared state at a time."}]"#,
            AppleAdapterRuntimeCompatibilityProfile::new(
                "apple-foundation-model",
                "general",
                "default",
            )
            .with_locale("en-US")
            .with_default_instruction(APPLE_ADAPTER_DEFAULT_INSTRUCTION)
            .dataset_metadata(),
        )
        .expect("dataset should import");
        let captures = multi_turn
            .derive_token_captures()
            .expect("token captures should derive");
        assert_eq!(captures.len(), 1);
        assert_eq!(
            captures[0].prompt_tokens,
            count_dataset_context_tokens(&multi_turn.metadata)
                + count_message_tokens(&multi_turn.samples[0].messages[0])
                + count_message_tokens(&multi_turn.samples[0].messages[1])
                + count_message_tokens(&multi_turn.samples[0].messages[2])
        );
        assert_eq!(
            captures[0].completion_tokens,
            count_message_tokens(&multi_turn.samples[0].messages[3])
        );

        let combined = AppleAdapterDatasetContract::from_jsonl_str(
            &format!(
                "{}\n{}\n{}",
                include_str!("../../fixtures/apple_adapter/datasets/minimal_sft_train.jsonl")
                    .trim(),
                include_str!(
                    "../../fixtures/apple_adapter/datasets/guided_generation_with_schema_train.jsonl"
                )
                .trim(),
                include_str!("../../fixtures/apple_adapter/datasets/tool_calling_train.jsonl")
                    .trim()
            ),
            AppleAdapterRuntimeCompatibilityProfile::new(
                "apple-foundation-model",
                "general",
                "default",
            )
            .with_locale("en-US")
            .with_default_instruction(APPLE_ADAPTER_DEFAULT_INSTRUCTION)
            .dataset_metadata(),
        )
        .expect("combined dataset should import");
        let combined_captures = combined
            .derive_token_captures()
            .expect("combined captures should derive");
        assert_eq!(combined_captures.len(), 3);
        assert_eq!(combined_captures[0].tool_tokens, 0);
        assert!(combined_captures[1].response_schema_tokens > 0);
        assert!(combined_captures[2].tool_tokens > 0);
    }
}
