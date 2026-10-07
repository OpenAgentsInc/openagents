use std::collections::{BTreeMap, BTreeSet};

use psionic_adapters::{
    AdapterArtifactIdentity, LmHeadLoraAdapterArtifact, LmHeadLoraLoadError, LmHeadLoraRuntimeError,
};
use psionic_datastream::DatastreamPolicyWeightBroadcastManifest;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{OpenAdapterReferenceModel, PolicyRevision};

/// High-level health state for the bounded training sampler service.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrainingSamplerHealthState {
    /// No active revision has been loaded yet.
    Uninitialized,
    /// One active revision is loaded and still fresh enough to serve.
    Ready,
    /// One active revision is loaded but has exceeded freshness policy.
    Stale,
}

/// Freshness posture for one served revision.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrainingSamplerFreshnessPosture {
    /// The revision is fresh enough to serve.
    Fresh,
    /// The revision is present but stale.
    Stale,
}

/// Simple service counters for inspectable request volume.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainingSamplerRequestCounters {
    /// Successful revision refreshes adopted by the service.
    pub refresh_count: u64,
    /// Successful completions-style requests.
    pub completion_request_count: u64,
    /// Successful chat-completions-style requests.
    pub chat_request_count: u64,
    /// Successful per-token logprob queries.
    pub logprob_query_count: u64,
}

/// One decode token admitted by the sampler vocabulary.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainingSamplerVocabularyToken {
    /// Stable token identifier.
    pub token_id: u32,
    /// Stable text emitted when the token is decoded.
    pub token_text: String,
}

impl TrainingSamplerVocabularyToken {
    /// Creates one vocabulary token.
    #[must_use]
    pub fn new(token_id: u32, token_text: impl Into<String>) -> Self {
        Self {
            token_id,
            token_text: token_text.into(),
        }
    }
}

/// One prompt-encoder feature used to turn prompt text into the hidden-state
/// surface expected by the open-adapter reference backend.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TrainingSamplerPromptFeature {
    /// Lower-level term matched against normalized prompt text.
    pub term: String,
    /// Hidden feature vector added when the term is present.
    pub hidden_features: Vec<f32>,
}

impl TrainingSamplerPromptFeature {
    /// Creates one prompt feature.
    #[must_use]
    pub fn new(term: impl Into<String>, hidden_features: Vec<f32>) -> Self {
        Self {
            term: term.into(),
            hidden_features,
        }
    }
}

/// Policy contract for the bounded training sampler service.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainingSamplerServicePolicy {
    /// Maximum prompt length admitted by the service.
    pub max_prompt_chars: usize,
    /// Maximum generation length for one completion/chat request.
    pub max_completion_tokens: u32,
    /// Maximum token count admitted by one logprob query.
    pub max_logprob_tokens: usize,
    /// Maximum top-logprob count admitted by one request.
    pub max_top_logprobs: usize,
    /// Maximum policy age admitted before the service refuses requests.
    pub freshness_budget_ms: u64,
    /// Optional stop token that terminates generation without surfacing the token.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_token_id: Option<u32>,
}

impl Default for TrainingSamplerServicePolicy {
    fn default() -> Self {
        Self {
            max_prompt_chars: 4_096,
            max_completion_tokens: 64,
            max_logprob_tokens: 256,
            max_top_logprobs: 5,
            freshness_budget_ms: 300_000,
            stop_token_id: None,
        }
    }
}

/// Full config for the first trainer-integrated sampler service.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OpenAdapterTrainingSamplerConfig {
    /// Stable service identifier.
    pub service_id: String,
    /// Stable policy family admitted by this service.
    pub policy_family: String,
    /// Base-model and tokenizer identity reused from the open-adapter trainer lane.
    pub model: OpenAdapterReferenceModel,
    /// Decode vocabulary surfaced by the service.
    pub vocabulary: Vec<TrainingSamplerVocabularyToken>,
    /// Optional prompt-feature lexicon used by the reference prompt encoder.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub prompt_feature_lexicon: Vec<TrainingSamplerPromptFeature>,
    /// Request and freshness policy.
    pub policy: TrainingSamplerServicePolicy,
}

impl OpenAdapterTrainingSamplerConfig {
    fn validate(&self) -> Result<(), TrainingSamplerServiceError> {
        if self.service_id.trim().is_empty() {
            return Err(TrainingSamplerServiceError::MissingServiceId);
        }
        if self.policy_family.trim().is_empty() {
            return Err(TrainingSamplerServiceError::MissingPolicyFamily);
        }
        if self.model.base_model_id.trim().is_empty() {
            return Err(TrainingSamplerServiceError::MissingBaseModelId);
        }
        if self.model.base_model_revision.trim().is_empty() {
            return Err(TrainingSamplerServiceError::MissingBaseModelRevision);
        }
        if self.model.base_served_artifact_digest.trim().is_empty() {
            return Err(TrainingSamplerServiceError::MissingBaseServedArtifactDigest);
        }
        if self.model.tokenizer.tokenizer_digest.trim().is_empty() {
            return Err(TrainingSamplerServiceError::MissingTokenizerDigest);
        }
        if self.model.hidden_size == 0 {
            return Err(TrainingSamplerServiceError::InvalidHiddenSize);
        }
        if self.model.vocab_size == 0 {
            return Err(TrainingSamplerServiceError::InvalidVocabSize);
        }
        if self.policy.max_prompt_chars == 0 {
            return Err(TrainingSamplerServiceError::InvalidPolicy(
                "max_prompt_chars must be greater than zero",
            ));
        }
        if self.policy.max_completion_tokens == 0 {
            return Err(TrainingSamplerServiceError::InvalidPolicy(
                "max_completion_tokens must be greater than zero",
            ));
        }
        if self.policy.max_logprob_tokens == 0 {
            return Err(TrainingSamplerServiceError::InvalidPolicy(
                "max_logprob_tokens must be greater than zero",
            ));
        }
        if self.policy.max_top_logprobs == 0 {
            return Err(TrainingSamplerServiceError::InvalidPolicy(
                "max_top_logprobs must be greater than zero",
            ));
        }
        if self.policy.freshness_budget_ms == 0 {
            return Err(TrainingSamplerServiceError::InvalidPolicy(
                "freshness_budget_ms must be greater than zero",
            ));
        }
        self.canonical_vocabulary()?;
        self.canonical_prompt_features()?;
        Ok(())
    }

    fn canonical_vocabulary(
        &self,
    ) -> Result<Vec<TrainingSamplerVocabularyToken>, TrainingSamplerServiceError> {
        let mut sorted = self.vocabulary.clone();
        sorted.sort_by_key(|token| token.token_id);
        let observed_ids = sorted
            .iter()
            .map(|token| token.token_id)
            .collect::<Vec<_>>();
        let expected_ids = (0..self.model.vocab_size)
            .map(|index| index as u32)
            .collect::<Vec<_>>();
        if observed_ids != expected_ids {
            return Err(TrainingSamplerServiceError::InvalidVocabularyCoverage {
                expected_ids,
                observed_ids,
            });
        }
        for token in &sorted {
            if token.token_text.trim().is_empty() {
                return Err(TrainingSamplerServiceError::MissingTokenText {
                    token_id: token.token_id,
                });
            }
        }
        Ok(sorted)
    }

    fn canonical_prompt_features(
        &self,
    ) -> Result<BTreeMap<String, Vec<f32>>, TrainingSamplerServiceError> {
        let mut features = BTreeMap::new();
        let mut seen = BTreeSet::new();
        for feature in &self.prompt_feature_lexicon {
            let term = normalize_term(feature.term.as_str());
            if term.is_empty() {
                return Err(TrainingSamplerServiceError::MissingPromptFeatureTerm);
            }
            if feature.hidden_features.len() != self.model.hidden_size {
                return Err(TrainingSamplerServiceError::PromptFeatureWidthMismatch {
                    term,
                    expected: self.model.hidden_size,
                    actual: feature.hidden_features.len(),
                });
            }
            if !seen.insert(term.clone()) {
                return Err(TrainingSamplerServiceError::DuplicatePromptFeatureTerm { term });
            }
            features.insert(term, feature.hidden_features.clone());
        }
        Ok(features)
    }

    /// Encodes prompt text into the hidden-state surface expected by the first
    /// open-adapter reference serving lane.
    #[must_use]
    pub fn encode_prompt_hidden_state(&self, prompt: &str) -> Vec<f32> {
        let prompt_terms = extract_prompt_terms(prompt);
        let feature_map = self.canonical_prompt_features().unwrap_or_default();
        let mut hidden = vec![0.0_f32; self.model.hidden_size];
        let mut matched_terms = 0_u32;

        for term in &prompt_terms {
            if let Some(features) = feature_map.get(term) {
                add_assign(hidden.as_mut_slice(), features.as_slice());
                matched_terms = matched_terms.saturating_add(1);
            } else {
                add_hashed_prompt_term(hidden.as_mut_slice(), term.as_str(), 0.05);
            }
        }

        if matched_terms == 0 {
            add_hashed_prompt_term(hidden.as_mut_slice(), prompt.trim(), 0.08);
        }

        let norm = l2_norm(hidden.as_slice());
        if norm > f32::EPSILON {
            for value in &mut hidden {
                *value /= norm;
            }
        }
        hidden
    }
}

/// Refresh input for one active served revision.
#[derive(Clone, Debug, PartialEq)]
pub struct TrainingSamplerServedRevision {
    /// Policy revision that should become active.
    pub policy_revision: PolicyRevision,
    /// Concrete adapter identity served by the sampler.
    pub adapter_identity: AdapterArtifactIdentity,
    /// LoRA alpha needed to reload the adapter correctly.
    pub adapter_alpha: f32,
    /// Raw `safetensors` payload for the adapter.
    pub adapter_bytes: Vec<u8>,
    /// Optional weight-broadcast manifest pinned to this revision.
    pub policy_weight_broadcast: Option<DatastreamPolicyWeightBroadcastManifest>,
}

impl TrainingSamplerServedRevision {
    /// Creates one served revision payload.
    #[must_use]
    pub fn new(
        policy_revision: PolicyRevision,
        adapter_identity: AdapterArtifactIdentity,
        adapter_alpha: f32,
        adapter_bytes: Vec<u8>,
    ) -> Self {
        Self {
            policy_revision,
            adapter_identity,
            adapter_alpha,
            adapter_bytes,
            policy_weight_broadcast: None,
        }
    }

    /// Attaches a weight-broadcast manifest pinned to the revision.
    #[must_use]
    pub fn with_policy_weight_broadcast(
        mut self,
        policy_weight_broadcast: DatastreamPolicyWeightBroadcastManifest,
    ) -> Self {
        self.policy_weight_broadcast = Some(policy_weight_broadcast);
        self
    }
}

/// Inspectable active-revision snapshot surfaced by status and responses.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainingSamplerActiveRevisionStatus {
    /// Active policy revision.
    pub policy_revision: PolicyRevision,
    /// Stable digest for the loaded adapter identity.
    pub adapter_identity_digest: String,
    /// Stable checkpoint reference when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checkpoint_ref: Option<String>,
    /// Stable weight-broadcast digest when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub policy_weight_broadcast_digest: Option<String>,
    /// Weight-broadcast freshness budget when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub policy_weight_freshness_window_ms: Option<u64>,
    /// Logical adoption timestamp for this sampler revision.
    pub adopted_at_ms: u64,
    /// Observed policy age at the status timestamp.
    pub policy_age_ms: u64,
    /// Effective freshness budget after applying service and broadcast limits.
    pub effective_freshness_budget_ms: u64,
    /// Freshness posture at the observation timestamp.
    pub freshness_posture: TrainingSamplerFreshnessPosture,
}

/// Service-wide status surface for health, readiness, and active revision truth.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainingSamplerServiceStatus {
    /// Stable service identifier.
    pub service_id: String,
    /// Stable admitted policy family.
    pub policy_family: String,
    /// Base-model identifier the service is pinned to.
    pub base_model_id: String,
    /// Base-model revision the service is pinned to.
    pub base_model_revision: String,
    /// Stable base served-artifact digest expected across all revisions.
    pub base_served_artifact_digest: String,
    /// Stable tokenizer digest expected across all revisions.
    pub tokenizer_digest: String,
    /// Current coarse health state.
    pub health_state: TrainingSamplerHealthState,
    /// Whether the service is ready to serve fresh traffic.
    pub ready: bool,
    /// Current active revision when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_revision: Option<TrainingSamplerActiveRevisionStatus>,
    /// Request counters accumulated by the service.
    pub request_counters: TrainingSamplerRequestCounters,
}

/// Stable refresh receipt proving active revision adoption.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainingSamplerRefreshReceipt {
    /// Stable service identifier.
    pub service_id: String,
    /// Previously active revision id when one existed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_revision_id: Option<String>,
    /// Newly adopted revision id.
    pub adopted_revision_id: String,
    /// Newly adopted revision number when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub adopted_revision_number: Option<u64>,
    /// Stable adapter identity digest.
    pub adapter_identity_digest: String,
    /// Logical adoption timestamp.
    pub adopted_at_ms: u64,
    /// Stable receipt digest.
    pub receipt_digest: String,
}

/// Message role used by the chat-completions style surface.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrainingSamplerChatRole {
    /// System message.
    System,
    /// User message.
    User,
    /// Assistant message.
    Assistant,
}

impl TrainingSamplerChatRole {
    fn label(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::User => "user",
            Self::Assistant => "assistant",
        }
    }
}

/// One chat message admitted by the bounded sampler service.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainingSamplerChatMessage {
    /// Role for the message.
    pub role: TrainingSamplerChatRole,
    /// Text content for the message.
    pub content: String,
}

impl TrainingSamplerChatMessage {
    /// Creates one chat message.
    #[must_use]
    pub fn new(role: TrainingSamplerChatRole, content: impl Into<String>) -> Self {
        Self {
            role,
            content: content.into(),
        }
    }
}

/// Completion request over one raw prompt.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainingSamplerCompletionRequest {
    /// Stable request identifier.
    pub request_id: String,
    /// Prompt text.
    pub prompt: String,
    /// Requested generation length.
    pub max_tokens: u32,
    /// Optional strict revision pin.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requested_revision_id: Option<String>,
    /// Optional top-logprob count.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_logprobs: Option<usize>,
    /// Logical request timestamp.
    pub requested_at_ms: u64,
}

/// Chat-completions style request over structured messages.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainingSamplerChatRequest {
    /// Stable request identifier.
    pub request_id: String,
    /// Ordered chat messages.
    pub messages: Vec<TrainingSamplerChatMessage>,
    /// Requested generation length.
    pub max_tokens: u32,
    /// Optional strict revision pin.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requested_revision_id: Option<String>,
    /// Optional top-logprob count.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_logprobs: Option<usize>,
    /// Logical request timestamp.
    pub requested_at_ms: u64,
}

/// Per-token logprob query over one prompt plus one candidate continuation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainingSamplerLogprobRequest {
    /// Stable request identifier.
    pub request_id: String,
    /// Prompt text used as the autoregressive prefix.
    pub prompt: String,
    /// Candidate continuation token ids.
    pub continuation_token_ids: Vec<u32>,
    /// Optional strict revision pin.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requested_revision_id: Option<String>,
    /// Optional top-logprob count.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_logprobs: Option<usize>,
    /// Logical request timestamp.
    pub requested_at_ms: u64,
}

/// One top-logprob candidate surfaced for a generated or queried token position.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TrainingSamplerTopLogprob {
    /// Candidate token identifier.
    pub token_id: u32,
    /// Candidate token text.
    pub token_text: String,
    /// Candidate token logprob.
    pub logprob: f32,
}

/// One generated token or one queried continuation token.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TrainingSamplerGeneratedToken {
    /// Zero-based position in the generated/query continuation.
    pub position: usize,
    /// Selected token identifier.
    pub token_id: u32,
    /// Selected token text.
    pub token_text: String,
    /// Selected token logprob.
    pub logprob: f32,
    /// Top-logprob candidates surfaced for the same position.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub top_logprobs: Vec<TrainingSamplerTopLogprob>,
}

/// Termination posture for generation requests.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrainingSamplerFinishReason {
    /// Generation stopped because the configured stop token was selected.
    StopToken,
    /// Generation stopped because the token budget ended.
    MaxTokens,
}

/// Shared response shape for completions and chat-completions requests.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TrainingSamplerGenerationResponse {
    /// Stable request identifier.
    pub request_id: String,
    /// Active revision snapshot observed during the request.
    pub active_revision: TrainingSamplerActiveRevisionStatus,
    /// Stable digest over the rendered prompt.
    pub prompt_digest: String,
    /// Final decoded text.
    pub text: String,
    /// Generated token records.
    pub tokens: Vec<TrainingSamplerGeneratedToken>,
    /// Termination posture.
    pub finish_reason: TrainingSamplerFinishReason,
    /// Stable response digest.
    pub response_digest: String,
}

/// Response for a per-token logprob query.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TrainingSamplerLogprobResponse {
    /// Stable request identifier.
    pub request_id: String,
    /// Active revision snapshot observed during the request.
    pub active_revision: TrainingSamplerActiveRevisionStatus,
    /// Stable digest over the rendered prompt.
    pub prompt_digest: String,
    /// Per-token continuation records.
    pub tokens: Vec<TrainingSamplerGeneratedToken>,
    /// Sum of per-token logprobs across the continuation.
    pub joint_logprob: f32,
    /// Stable response digest.
    pub response_digest: String,
}

/// Fail-closed errors returned by the bounded training sampler service.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum TrainingSamplerServiceError {
    /// The config omitted its stable service identifier.
    #[error("training sampler config is missing `service_id`")]
    MissingServiceId,
    /// The config omitted its admitted policy family.
    #[error("training sampler config is missing `policy_family`")]
    MissingPolicyFamily,
    /// The config omitted the base-model id.
    #[error("training sampler config is missing `base_model_id`")]
    MissingBaseModelId,
    /// The config omitted the base-model revision.
    #[error("training sampler config is missing `base_model_revision`")]
    MissingBaseModelRevision,
    /// The config omitted the base served-artifact digest.
    #[error("training sampler config is missing `base_served_artifact_digest`")]
    MissingBaseServedArtifactDigest,
    /// The config omitted the tokenizer digest.
    #[error("training sampler config is missing `tokenizer_digest`")]
    MissingTokenizerDigest,
    /// The config declared an invalid hidden width.
    #[error("training sampler config declared hidden_size=0")]
    InvalidHiddenSize,
    /// The config declared an invalid vocab width.
    #[error("training sampler config declared vocab_size=0")]
    InvalidVocabSize,
    /// One policy knob was invalid.
    #[error("training sampler config policy is invalid: {0}")]
    InvalidPolicy(&'static str),
    /// The decode vocabulary did not cover the expected contiguous id set.
    #[error(
        "training sampler vocabulary must cover token ids {expected_ids:?}; observed {observed_ids:?}"
    )]
    InvalidVocabularyCoverage {
        /// Expected contiguous ids.
        expected_ids: Vec<u32>,
        /// Observed ids after sorting.
        observed_ids: Vec<u32>,
    },
    /// One vocabulary token omitted its text.
    #[error("training sampler vocabulary token `{token_id}` is missing `token_text`")]
    MissingTokenText {
        /// Token identifier with the missing text.
        token_id: u32,
    },
    /// One prompt feature omitted its normalized term.
    #[error("training sampler prompt feature is missing `term`")]
    MissingPromptFeatureTerm,
    /// One prompt feature width did not match the model hidden size.
    #[error(
        "training sampler prompt feature `{term}` hidden width mismatch: expected {expected}, found {actual}"
    )]
    PromptFeatureWidthMismatch {
        /// Prompt feature term.
        term: String,
        /// Expected hidden size.
        expected: usize,
        /// Observed hidden size.
        actual: usize,
    },
    /// Prompt features repeated the same normalized term.
    #[error("training sampler prompt feature `{term}` is duplicated")]
    DuplicatePromptFeatureTerm {
        /// Repeated normalized term.
        term: String,
    },
    /// The service has not loaded any revision yet.
    #[error("training sampler service has no active revision")]
    MissingActiveRevision,
    /// One request pinned a revision id that is not active.
    #[error(
        "training sampler requested revision `{requested_revision_id}` is unavailable; active revision is `{active_revision_id}`"
    )]
    RequestedRevisionUnavailable {
        /// Requested revision id.
        requested_revision_id: String,
        /// Currently active revision id.
        active_revision_id: String,
    },
    /// The active revision exceeded the freshness budget.
    #[error(
        "training sampler active revision `{revision_id}` is stale: age {age_ms}ms exceeds freshness budget {freshness_budget_ms}ms"
    )]
    StaleActiveRevision {
        /// Active revision id.
        revision_id: String,
        /// Observed policy age.
        age_ms: u64,
        /// Effective freshness budget.
        freshness_budget_ms: u64,
    },
    /// One refresh attempted to load a different policy family.
    #[error(
        "training sampler refresh rejected revision `{revision_id}` because policy family `{actual_policy_family}` does not match service family `{expected_policy_family}`"
    )]
    RefreshFamilyMismatch {
        /// Revision id being loaded.
        revision_id: String,
        /// Service policy family.
        expected_policy_family: String,
        /// Revision family from the payload.
        actual_policy_family: String,
    },
    /// One refresh carried a mismatched weight-broadcast family.
    #[error(
        "training sampler refresh rejected revision `{revision_id}` because weight-broadcast family `{actual_policy_family}` does not match service family `{expected_policy_family}`"
    )]
    RefreshBroadcastFamilyMismatch {
        /// Revision id being loaded.
        revision_id: String,
        /// Service policy family.
        expected_policy_family: String,
        /// Broadcast family from the payload.
        actual_policy_family: String,
    },
    /// One refresh carried a broadcast revision that drifted from the policy revision.
    #[error(
        "training sampler refresh rejected revision `{revision_id}` because broadcast revision {broadcast_revision} does not match policy revision {policy_revision}"
    )]
    RefreshBroadcastRevisionMismatch {
        /// Revision id being loaded.
        revision_id: String,
        /// Broadcast revision number.
        broadcast_revision: u64,
        /// Policy revision number.
        policy_revision: u64,
    },
    /// The refresh attempted to move backwards in revision number.
    #[error(
        "training sampler refresh rejected revision `{revision_id}` because revision number {requested_revision_number} is not newer than active {current_revision_number}"
    )]
    NonMonotonicRevision {
        /// Incoming revision id.
        revision_id: String,
        /// Active revision number.
        current_revision_number: u64,
        /// Requested revision number.
        requested_revision_number: u64,
    },
    /// The refresh attempted to move backwards in logical time.
    #[error(
        "training sampler refresh rejected revision `{revision_id}` because produced_at_ms {requested_produced_at_ms} is not newer than active {current_produced_at_ms}"
    )]
    NonMonotonicTimestamp {
        /// Incoming revision id.
        revision_id: String,
        /// Active production timestamp.
        current_produced_at_ms: u64,
        /// Requested production timestamp.
        requested_produced_at_ms: u64,
    },
    /// The adapter alpha was invalid.
    #[error(
        "training sampler refresh rejected revision `{revision_id}` because adapter_alpha `{adapter_alpha}` is invalid"
    )]
    InvalidAdapterAlpha {
        /// Revision id being loaded.
        revision_id: String,
        /// Invalid alpha value.
        adapter_alpha: f32,
    },
    /// The adapter bytes were empty.
    #[error(
        "training sampler refresh rejected revision `{revision_id}` because adapter_bytes is empty"
    )]
    EmptyAdapterBytes {
        /// Revision id being loaded.
        revision_id: String,
    },
    /// The bytes did not match the supplied adapter-identity digest.
    #[error(
        "training sampler refresh rejected revision `{revision_id}` because adapter digest `{actual_digest}` does not match identity digest `{expected_digest}`"
    )]
    AdapterDigestMismatch {
        /// Revision id being loaded.
        revision_id: String,
        /// Digest from the identity object.
        expected_digest: String,
        /// Digest of the supplied bytes.
        actual_digest: String,
    },
    /// The adapter targeted a different base-model field than the service.
    #[error(
        "training sampler refresh rejected revision `{revision_id}` because adapter field `{field}` expected `{expected}` but found `{actual}`"
    )]
    AdapterBaseMismatch {
        /// Revision id being loaded.
        revision_id: String,
        /// Mismatched field label.
        field: &'static str,
        /// Expected service value.
        expected: String,
        /// Actual adapter value.
        actual: String,
    },
    /// The adapter hidden width mismatched the service model.
    #[error(
        "training sampler refresh rejected revision `{revision_id}` because adapter hidden size expected {expected} but found {actual}"
    )]
    AdapterHiddenSizeMismatch {
        /// Revision id being loaded.
        revision_id: String,
        /// Expected hidden size.
        expected: usize,
        /// Actual hidden size.
        actual: usize,
    },
    /// The adapter vocab width mismatched the service model.
    #[error(
        "training sampler refresh rejected revision `{revision_id}` because adapter vocab size expected {expected} but found {actual}"
    )]
    AdapterVocabSizeMismatch {
        /// Revision id being loaded.
        revision_id: String,
        /// Expected vocab size.
        expected: usize,
        /// Actual vocab size.
        actual: usize,
    },
    /// One request omitted its prompt after trimming.
    #[error("training sampler request `{request_id}` prompt is empty")]
    EmptyPrompt {
        /// Stable request identifier.
        request_id: String,
    },
    /// One chat request omitted all messages.
    #[error("training sampler chat request `{request_id}` must include at least one message")]
    EmptyMessages {
        /// Stable request identifier.
        request_id: String,
    },
    /// One prompt exceeded the configured length budget.
    #[error("training sampler request `{request_id}` prompt length {observed} exceeds max {max}")]
    PromptTooLong {
        /// Stable request identifier.
        request_id: String,
        /// Observed character length.
        observed: usize,
        /// Maximum admitted length.
        max: usize,
    },
    /// One request asked for too many completion tokens.
    #[error(
        "training sampler request `{request_id}` asked for {observed} completion tokens but max is {max}"
    )]
    CompletionBudgetExceeded {
        /// Stable request identifier.
        request_id: String,
        /// Observed request budget.
        observed: u32,
        /// Maximum admitted budget.
        max: u32,
    },
    /// One request asked for too many top-logprob candidates.
    #[error(
        "training sampler request `{request_id}` asked for top_logprobs={observed} but max is {max}"
    )]
    TopLogprobsBudgetExceeded {
        /// Stable request identifier.
        request_id: String,
        /// Observed top-logprobs request.
        observed: usize,
        /// Maximum admitted request.
        max: usize,
    },
    /// One logprob query carried no candidate continuation.
    #[error(
        "training sampler logprob request `{request_id}` must include at least one continuation token"
    )]
    EmptyContinuation {
        /// Stable request identifier.
        request_id: String,
    },
    /// One logprob query exceeded the configured continuation budget.
    #[error(
        "training sampler logprob request `{request_id}` asked for {observed} continuation tokens but max is {max}"
    )]
    LogprobBudgetExceeded {
        /// Stable request identifier.
        request_id: String,
        /// Observed continuation length.
        observed: usize,
        /// Maximum admitted continuation length.
        max: usize,
    },
    /// One request named a token outside the decode vocabulary.
    #[error(
        "training sampler request `{request_id}` named token `{token_id}` outside vocab size {vocab_size}"
    )]
    UnknownTokenId {
        /// Stable request identifier.
        request_id: String,
        /// Token identifier.
        token_id: u32,
        /// Service vocab size.
        vocab_size: usize,
    },
    /// Adapter parsing failed.
    #[error(transparent)]
    AdapterLoad(#[from] LmHeadLoraLoadError),
    /// Adapter runtime application failed.
    #[error(transparent)]
    AdapterRuntime(#[from] LmHeadLoraRuntimeError),
}

#[derive(Clone, Debug)]
struct ActiveServedRevision {
    policy_revision: PolicyRevision,
    adapter_identity: AdapterArtifactIdentity,
    adapter: LmHeadLoraAdapterArtifact,
    policy_weight_broadcast: Option<DatastreamPolicyWeightBroadcastManifest>,
    adopted_at_ms: u64,
}

impl ActiveServedRevision {
    fn status(
        &self,
        service_freshness_budget_ms: u64,
        observed_at_ms: u64,
    ) -> TrainingSamplerActiveRevisionStatus {
        let broadcast_budget = self
            .policy_weight_broadcast
            .as_ref()
            .map(|broadcast| broadcast.freshness_window_ms);
        let effective_freshness_budget_ms = broadcast_budget
            .map(|broadcast_budget| broadcast_budget.min(service_freshness_budget_ms))
            .unwrap_or(service_freshness_budget_ms);
        let policy_age_ms = observed_at_ms.saturating_sub(self.policy_revision.produced_at_ms);
        let freshness_posture = if policy_age_ms <= effective_freshness_budget_ms {
            TrainingSamplerFreshnessPosture::Fresh
        } else {
            TrainingSamplerFreshnessPosture::Stale
        };
        TrainingSamplerActiveRevisionStatus {
            policy_revision: self.policy_revision.clone(),
            adapter_identity_digest: self.adapter_identity.stable_digest(),
            checkpoint_ref: self
                .policy_revision
                .checkpoint
                .as_ref()
                .and_then(|checkpoint| checkpoint.checkpoint_ref.clone()),
            policy_weight_broadcast_digest: self
                .policy_weight_broadcast
                .as_ref()
                .map(|broadcast| broadcast.broadcast_digest.clone()),
            policy_weight_freshness_window_ms: self
                .policy_weight_broadcast
                .as_ref()
                .map(|broadcast| broadcast.freshness_window_ms),
            adopted_at_ms: self.adopted_at_ms,
            policy_age_ms,
            effective_freshness_budget_ms,
            freshness_posture,
        }
    }
}

/// First trainer-integrated sampler service above the open-adapter reference lane.
#[derive(Clone, Debug)]
pub struct TrainingSamplerService {
    config: OpenAdapterTrainingSamplerConfig,
    token_texts: Vec<String>,
    prompt_features: BTreeMap<String, Vec<f32>>,
    base_projection: Vec<f32>,
    active_revision: Option<ActiveServedRevision>,
    request_counters: TrainingSamplerRequestCounters,
}

impl TrainingSamplerService {
    /// Builds the bounded sampler service.
    pub fn new(
        config: OpenAdapterTrainingSamplerConfig,
    ) -> Result<Self, TrainingSamplerServiceError> {
        config.validate()?;
        let vocabulary = config.canonical_vocabulary()?;
        let token_texts = vocabulary
            .into_iter()
            .map(|token| token.token_text)
            .collect::<Vec<_>>();
        let prompt_features = config.canonical_prompt_features()?;
        let base_projection = seeded_matrix(
            format!(
                "{}|{}|base_projection|{}x{}",
                config.model.base_model_id,
                config.model.base_model_revision,
                config.model.vocab_size,
                config.model.hidden_size,
            )
            .as_str(),
            config.model.vocab_size,
            config.model.hidden_size,
            0.04,
        );
        Ok(Self {
            config,
            token_texts,
            prompt_features,
            base_projection,
            active_revision: None,
            request_counters: TrainingSamplerRequestCounters::default(),
        })
    }

    /// Returns the immutable sampler config.
    #[must_use]
    pub fn config(&self) -> &OpenAdapterTrainingSamplerConfig {
        &self.config
    }

    /// Returns a status snapshot for health and readiness inspection.
    #[must_use]
    pub fn status(&self, observed_at_ms: u64) -> TrainingSamplerServiceStatus {
        let active_revision = self
            .active_revision
            .as_ref()
            .map(|active| active.status(self.config.policy.freshness_budget_ms, observed_at_ms));
        let health_state = match &active_revision {
            None => TrainingSamplerHealthState::Uninitialized,
            Some(active) if active.freshness_posture == TrainingSamplerFreshnessPosture::Fresh => {
                TrainingSamplerHealthState::Ready
            }
            Some(_) => TrainingSamplerHealthState::Stale,
        };
        TrainingSamplerServiceStatus {
            service_id: self.config.service_id.clone(),
            policy_family: self.config.policy_family.clone(),
            base_model_id: self.config.model.base_model_id.clone(),
            base_model_revision: self.config.model.base_model_revision.clone(),
            base_served_artifact_digest: self.config.model.base_served_artifact_digest.clone(),
            tokenizer_digest: self.config.model.tokenizer.tokenizer_digest.clone(),
            health_state,
            ready: health_state == TrainingSamplerHealthState::Ready,
            active_revision,
            request_counters: self.request_counters.clone(),
        }
    }

    /// Refreshes the active served revision without restarting the process.
    pub fn refresh_revision(
        &mut self,
        revision: TrainingSamplerServedRevision,
        adopted_at_ms: u64,
    ) -> Result<TrainingSamplerRefreshReceipt, TrainingSamplerServiceError> {
        validate_served_revision_against_service(&self.config, &revision)?;
        if revision.policy_revision.policy_family != self.config.policy_family {
            return Err(TrainingSamplerServiceError::RefreshFamilyMismatch {
                revision_id: revision.policy_revision.revision_id.clone(),
                expected_policy_family: self.config.policy_family.clone(),
                actual_policy_family: revision.policy_revision.policy_family.clone(),
            });
        }
        if let Some(broadcast) = &revision.policy_weight_broadcast {
            if broadcast.policy_id != self.config.policy_family {
                return Err(
                    TrainingSamplerServiceError::RefreshBroadcastFamilyMismatch {
                        revision_id: revision.policy_revision.revision_id.clone(),
                        expected_policy_family: self.config.policy_family.clone(),
                        actual_policy_family: broadcast.policy_id.clone(),
                    },
                );
            }
            if let Some(policy_revision_number) = revision.policy_revision.revision_number {
                if broadcast.policy_revision != policy_revision_number {
                    return Err(
                        TrainingSamplerServiceError::RefreshBroadcastRevisionMismatch {
                            revision_id: revision.policy_revision.revision_id.clone(),
                            broadcast_revision: broadcast.policy_revision,
                            policy_revision: policy_revision_number,
                        },
                    );
                }
            }
        }
        if let Some(active) = &self.active_revision {
            if let (Some(current), Some(requested)) = (
                active.policy_revision.revision_number,
                revision.policy_revision.revision_number,
            ) {
                if requested <= current {
                    return Err(TrainingSamplerServiceError::NonMonotonicRevision {
                        revision_id: revision.policy_revision.revision_id.clone(),
                        current_revision_number: current,
                        requested_revision_number: requested,
                    });
                }
            } else if revision.policy_revision.produced_at_ms
                <= active.policy_revision.produced_at_ms
            {
                return Err(TrainingSamplerServiceError::NonMonotonicTimestamp {
                    revision_id: revision.policy_revision.revision_id.clone(),
                    current_produced_at_ms: active.policy_revision.produced_at_ms,
                    requested_produced_at_ms: revision.policy_revision.produced_at_ms,
                });
            }
        }

        let adapter = LmHeadLoraAdapterArtifact::from_safetensors_bytes(
            revision.adapter_bytes.as_slice(),
            revision.adapter_identity.clone(),
            revision.adapter_alpha,
        )?;
        if adapter.hidden_size != self.config.model.hidden_size {
            return Err(TrainingSamplerServiceError::AdapterHiddenSizeMismatch {
                revision_id: revision.policy_revision.revision_id.clone(),
                expected: self.config.model.hidden_size,
                actual: adapter.hidden_size,
            });
        }
        if adapter.vocab_size != self.config.model.vocab_size {
            return Err(TrainingSamplerServiceError::AdapterVocabSizeMismatch {
                revision_id: revision.policy_revision.revision_id.clone(),
                expected: self.config.model.vocab_size,
                actual: adapter.vocab_size,
            });
        }

        let previous_revision_id = self
            .active_revision
            .as_ref()
            .map(|active| active.policy_revision.revision_id.clone());
        let adapter_identity_digest = revision.adapter_identity.stable_digest();
        let adopted_revision_id = revision.policy_revision.revision_id.clone();
        let adopted_revision_number = revision.policy_revision.revision_number;
        self.active_revision = Some(ActiveServedRevision {
            policy_revision: revision.policy_revision,
            adapter_identity: revision.adapter_identity,
            adapter,
            policy_weight_broadcast: revision.policy_weight_broadcast,
            adopted_at_ms,
        });
        self.request_counters.refresh_count = self.request_counters.refresh_count.saturating_add(1);
        Ok(TrainingSamplerRefreshReceipt {
            service_id: self.config.service_id.clone(),
            previous_revision_id,
            adopted_revision_id: adopted_revision_id.clone(),
            adopted_revision_number,
            adapter_identity_digest: adapter_identity_digest.clone(),
            adopted_at_ms,
            receipt_digest: stable_refresh_receipt_digest(
                self.config.service_id.as_str(),
                adopted_revision_id.as_str(),
                adopted_revision_number,
                adapter_identity_digest.as_str(),
                adopted_at_ms,
            ),
        })
    }

    /// Runs one completions-style inference request against the active revision.
    pub fn complete(
        &mut self,
        request: &TrainingSamplerCompletionRequest,
    ) -> Result<TrainingSamplerGenerationResponse, TrainingSamplerServiceError> {
        validate_prompt_request(
            &self.config,
            request.request_id.as_str(),
            request.prompt.as_str(),
            request.max_tokens,
            request.top_logprobs,
        )?;
        let rendered_prompt = request.prompt.trim().to_string();
        let response = self.generate_response(
            request.request_id.as_str(),
            rendered_prompt.as_str(),
            request.requested_revision_id.as_deref(),
            request.max_tokens,
            request.top_logprobs.unwrap_or(0),
            request.requested_at_ms,
        )?;
        self.request_counters.completion_request_count = self
            .request_counters
            .completion_request_count
            .saturating_add(1);
        Ok(response)
    }

    /// Runs one chat-completions-style inference request against the active revision.
    pub fn chat_complete(
        &mut self,
        request: &TrainingSamplerChatRequest,
    ) -> Result<TrainingSamplerGenerationResponse, TrainingSamplerServiceError> {
        if request.messages.is_empty() {
            return Err(TrainingSamplerServiceError::EmptyMessages {
                request_id: request.request_id.clone(),
            });
        }
        let rendered_prompt = render_chat_prompt(request.messages.as_slice());
        validate_prompt_request(
            &self.config,
            request.request_id.as_str(),
            rendered_prompt.as_str(),
            request.max_tokens,
            request.top_logprobs,
        )?;
        let response = self.generate_response(
            request.request_id.as_str(),
            rendered_prompt.as_str(),
            request.requested_revision_id.as_deref(),
            request.max_tokens,
            request.top_logprobs.unwrap_or(0),
            request.requested_at_ms,
        )?;
        self.request_counters.chat_request_count =
            self.request_counters.chat_request_count.saturating_add(1);
        Ok(response)
    }

    /// Returns per-token logprobs for one candidate continuation against the active revision.
    pub fn token_logprobs(
        &mut self,
        request: &TrainingSamplerLogprobRequest,
    ) -> Result<TrainingSamplerLogprobResponse, TrainingSamplerServiceError> {
        validate_logprob_request(&self.config, request)?;
        let active = self.require_fresh_active_revision(
            request.requested_revision_id.as_deref(),
            request.requested_at_ms,
        )?;
        let prompt_digest = stable_prompt_digest(request.prompt.as_str());
        let top_logprobs = request.top_logprobs.unwrap_or(0);
        let mut prompt = request.prompt.trim().to_string();
        let mut tokens = Vec::with_capacity(request.continuation_token_ids.len());
        let mut joint_logprob = 0.0_f32;
        for (position, token_id) in request.continuation_token_ids.iter().copied().enumerate() {
            let distribution = self.distribution_for_prompt(active, prompt.as_str())?;
            let token_index = token_id as usize;
            let probability = *distribution.get(token_index).ok_or_else(|| {
                TrainingSamplerServiceError::UnknownTokenId {
                    request_id: request.request_id.clone(),
                    token_id,
                    vocab_size: self.config.model.vocab_size,
                }
            })?;
            let logprob = probability.max(f32::EPSILON).ln();
            joint_logprob += logprob;
            let token_text = self.token_text(token_id).ok_or_else(|| {
                TrainingSamplerServiceError::UnknownTokenId {
                    request_id: request.request_id.clone(),
                    token_id,
                    vocab_size: self.config.model.vocab_size,
                }
            })?;
            tokens.push(TrainingSamplerGeneratedToken {
                position,
                token_id,
                token_text: token_text.to_string(),
                logprob,
                top_logprobs: self.top_logprobs(distribution.as_slice(), top_logprobs),
            });
            prompt = append_token_to_prompt(prompt.as_str(), token_text);
        }
        let active_status = active.status(
            self.config.policy.freshness_budget_ms,
            request.requested_at_ms,
        );
        let response = TrainingSamplerLogprobResponse {
            request_id: request.request_id.clone(),
            active_revision: active_status,
            prompt_digest,
            tokens: tokens.clone(),
            joint_logprob,
            response_digest: stable_logprob_response_digest(
                request.request_id.as_str(),
                active.policy_revision.revision_id.as_str(),
                prompt.as_str(),
                joint_logprob,
                tokens.as_slice(),
            ),
        };
        self.request_counters.logprob_query_count =
            self.request_counters.logprob_query_count.saturating_add(1);
        Ok(response)
    }

    fn generate_response(
        &self,
        request_id: &str,
        prompt: &str,
        requested_revision_id: Option<&str>,
        max_tokens: u32,
        top_logprobs: usize,
        requested_at_ms: u64,
    ) -> Result<TrainingSamplerGenerationResponse, TrainingSamplerServiceError> {
        let active = self.require_fresh_active_revision(requested_revision_id, requested_at_ms)?;
        let prompt_digest = stable_prompt_digest(prompt);
        let mut rendered_prompt = prompt.to_string();
        let mut text = String::new();
        let mut tokens = Vec::new();
        let mut finish_reason = TrainingSamplerFinishReason::MaxTokens;
        for position in 0..max_tokens as usize {
            let distribution = self.distribution_for_prompt(active, rendered_prompt.as_str())?;
            let token_index = argmax_index(distribution.as_slice());
            let token_id = token_index as u32;
            if self
                .config
                .policy
                .stop_token_id
                .is_some_and(|stop| stop == token_id)
            {
                finish_reason = TrainingSamplerFinishReason::StopToken;
                break;
            }
            let token_text = self.token_text(token_id).ok_or_else(|| {
                TrainingSamplerServiceError::UnknownTokenId {
                    request_id: request_id.to_string(),
                    token_id,
                    vocab_size: self.config.model.vocab_size,
                }
            })?;
            let probability = distribution[token_index];
            let logprob = probability.max(f32::EPSILON).ln();
            tokens.push(TrainingSamplerGeneratedToken {
                position,
                token_id,
                token_text: token_text.to_string(),
                logprob,
                top_logprobs: self.top_logprobs(distribution.as_slice(), top_logprobs),
            });
            text = append_token_to_prompt(text.as_str(), token_text);
            rendered_prompt = append_token_to_prompt(rendered_prompt.as_str(), token_text);
        }
        let active_status = active.status(self.config.policy.freshness_budget_ms, requested_at_ms);
        Ok(TrainingSamplerGenerationResponse {
            request_id: request_id.to_string(),
            active_revision: active_status.clone(),
            prompt_digest,
            text,
            tokens: tokens.clone(),
            finish_reason,
            response_digest: stable_generation_response_digest(
                request_id,
                active_status.policy_revision.revision_id.as_str(),
                prompt,
                finish_reason,
                tokens.as_slice(),
            ),
        })
    }

    fn require_fresh_active_revision(
        &self,
        requested_revision_id: Option<&str>,
        observed_at_ms: u64,
    ) -> Result<&ActiveServedRevision, TrainingSamplerServiceError> {
        let active = self
            .active_revision
            .as_ref()
            .ok_or(TrainingSamplerServiceError::MissingActiveRevision)?;
        if let Some(requested_revision_id) = requested_revision_id {
            if requested_revision_id != active.policy_revision.revision_id {
                return Err(TrainingSamplerServiceError::RequestedRevisionUnavailable {
                    requested_revision_id: requested_revision_id.to_string(),
                    active_revision_id: active.policy_revision.revision_id.clone(),
                });
            }
        }
        let status = active.status(self.config.policy.freshness_budget_ms, observed_at_ms);
        if status.freshness_posture == TrainingSamplerFreshnessPosture::Stale {
            return Err(TrainingSamplerServiceError::StaleActiveRevision {
                revision_id: active.policy_revision.revision_id.clone(),
                age_ms: status.policy_age_ms,
                freshness_budget_ms: status.effective_freshness_budget_ms,
            });
        }
        Ok(active)
    }

    fn distribution_for_prompt(
        &self,
        active: &ActiveServedRevision,
        prompt: &str,
    ) -> Result<Vec<f32>, TrainingSamplerServiceError> {
        let hidden = self.encode_prompt_hidden_state(prompt);
        let mut logits = mat_vec(
            self.base_projection.as_slice(),
            self.config.model.vocab_size,
            self.config.model.hidden_size,
            hidden.as_slice(),
        );
        active
            .adapter
            .apply_to_logits(hidden.as_slice(), logits.as_mut_slice())?;
        Ok(softmax(logits.as_slice()))
    }

    fn encode_prompt_hidden_state(&self, prompt: &str) -> Vec<f32> {
        let prompt_terms = extract_prompt_terms(prompt);
        let mut hidden = vec![0.0_f32; self.config.model.hidden_size];
        let mut matched_terms = 0_u32;
        for term in &prompt_terms {
            if let Some(features) = self.prompt_features.get(term) {
                add_assign(hidden.as_mut_slice(), features.as_slice());
                matched_terms = matched_terms.saturating_add(1);
            } else {
                add_hashed_prompt_term(hidden.as_mut_slice(), term.as_str(), 0.05);
            }
        }
        if matched_terms == 0 {
            add_hashed_prompt_term(hidden.as_mut_slice(), prompt.trim(), 0.08);
        }
        let norm = l2_norm(hidden.as_slice());
        if norm > f32::EPSILON {
            for value in &mut hidden {
                *value /= norm;
            }
        }
        hidden
    }

    fn token_text(&self, token_id: u32) -> Option<&str> {
        self.token_texts.get(token_id as usize).map(String::as_str)
    }

    fn top_logprobs(&self, distribution: &[f32], limit: usize) -> Vec<TrainingSamplerTopLogprob> {
        if limit == 0 {
            return Vec::new();
        }
        let mut candidates = distribution
            .iter()
            .enumerate()
            .map(|(token_index, probability)| TrainingSamplerTopLogprob {
                token_id: token_index as u32,
                token_text: self
                    .token_text(token_index as u32)
                    .unwrap_or("")
                    .to_string(),
                logprob: probability.max(f32::EPSILON).ln(),
            })
            .collect::<Vec<_>>();
        candidates.sort_by(|left, right| {
            right
                .logprob
                .partial_cmp(&left.logprob)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        candidates.truncate(limit.min(self.config.model.vocab_size));
        candidates
    }
}

fn validate_served_revision_against_service(
    config: &OpenAdapterTrainingSamplerConfig,
    revision: &TrainingSamplerServedRevision,
) -> Result<(), TrainingSamplerServiceError> {
    if !revision.adapter_alpha.is_finite() || revision.adapter_alpha <= 0.0 {
        return Err(TrainingSamplerServiceError::InvalidAdapterAlpha {
            revision_id: revision.policy_revision.revision_id.clone(),
            adapter_alpha: revision.adapter_alpha,
        });
    }
    if revision.adapter_bytes.is_empty() {
        return Err(TrainingSamplerServiceError::EmptyAdapterBytes {
            revision_id: revision.policy_revision.revision_id.clone(),
        });
    }
    let actual_digest = hex::encode(Sha256::digest(revision.adapter_bytes.as_slice()));
    if actual_digest != revision.adapter_identity.artifact_digest {
        return Err(TrainingSamplerServiceError::AdapterDigestMismatch {
            revision_id: revision.policy_revision.revision_id.clone(),
            expected_digest: revision.adapter_identity.artifact_digest.clone(),
            actual_digest,
        });
    }
    validate_adapter_identity_field(
        revision.policy_revision.revision_id.as_str(),
        "base_model_id",
        config.model.base_model_id.as_str(),
        revision.adapter_identity.base_model_id.as_str(),
    )?;
    validate_adapter_identity_field(
        revision.policy_revision.revision_id.as_str(),
        "base_model_revision",
        config.model.base_model_revision.as_str(),
        revision.adapter_identity.base_model_revision.as_str(),
    )?;
    validate_adapter_identity_field(
        revision.policy_revision.revision_id.as_str(),
        "base_served_artifact_digest",
        config.model.base_served_artifact_digest.as_str(),
        revision
            .adapter_identity
            .base_served_artifact_digest
            .as_str(),
    )?;
    Ok(())
}

fn validate_adapter_identity_field(
    revision_id: &str,
    field: &'static str,
    expected: &str,
    actual: &str,
) -> Result<(), TrainingSamplerServiceError> {
    if expected != actual {
        return Err(TrainingSamplerServiceError::AdapterBaseMismatch {
            revision_id: revision_id.to_string(),
            field,
            expected: expected.to_string(),
            actual: actual.to_string(),
        });
    }
    Ok(())
}

fn validate_prompt_request(
    config: &OpenAdapterTrainingSamplerConfig,
    request_id: &str,
    prompt: &str,
    max_tokens: u32,
    top_logprobs: Option<usize>,
) -> Result<(), TrainingSamplerServiceError> {
    let trimmed_prompt = prompt.trim();
    if trimmed_prompt.is_empty() {
        return Err(TrainingSamplerServiceError::EmptyPrompt {
            request_id: request_id.to_string(),
        });
    }
    if trimmed_prompt.chars().count() > config.policy.max_prompt_chars {
        return Err(TrainingSamplerServiceError::PromptTooLong {
            request_id: request_id.to_string(),
            observed: trimmed_prompt.chars().count(),
            max: config.policy.max_prompt_chars,
        });
    }
    if max_tokens == 0 || max_tokens > config.policy.max_completion_tokens {
        return Err(TrainingSamplerServiceError::CompletionBudgetExceeded {
            request_id: request_id.to_string(),
            observed: max_tokens,
            max: config.policy.max_completion_tokens,
        });
    }
    if let Some(top_logprobs) = top_logprobs {
        if top_logprobs > config.policy.max_top_logprobs {
            return Err(TrainingSamplerServiceError::TopLogprobsBudgetExceeded {
                request_id: request_id.to_string(),
                observed: top_logprobs,
                max: config.policy.max_top_logprobs,
            });
        }
    }
    Ok(())
}

fn validate_logprob_request(
    config: &OpenAdapterTrainingSamplerConfig,
    request: &TrainingSamplerLogprobRequest,
) -> Result<(), TrainingSamplerServiceError> {
    validate_prompt_request(
        config,
        request.request_id.as_str(),
        request.prompt.as_str(),
        1,
        request.top_logprobs,
    )?;
    if request.continuation_token_ids.is_empty() {
        return Err(TrainingSamplerServiceError::EmptyContinuation {
            request_id: request.request_id.clone(),
        });
    }
    if request.continuation_token_ids.len() > config.policy.max_logprob_tokens {
        return Err(TrainingSamplerServiceError::LogprobBudgetExceeded {
            request_id: request.request_id.clone(),
            observed: request.continuation_token_ids.len(),
            max: config.policy.max_logprob_tokens,
        });
    }
    for token_id in &request.continuation_token_ids {
        if *token_id as usize >= config.model.vocab_size {
            return Err(TrainingSamplerServiceError::UnknownTokenId {
                request_id: request.request_id.clone(),
                token_id: *token_id,
                vocab_size: config.model.vocab_size,
            });
        }
    }
    Ok(())
}

fn render_chat_prompt(messages: &[TrainingSamplerChatMessage]) -> String {
    messages
        .iter()
        .map(|message| format!("{}: {}", message.role.label(), message.content.trim()))
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

fn extract_prompt_terms(prompt: &str) -> Vec<String> {
    let mut terms = Vec::new();
    let mut current = String::new();
    for ch in prompt.chars() {
        if ch.is_ascii_alphanumeric() {
            current.push(ch.to_ascii_lowercase());
        } else if !current.is_empty() {
            terms.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        terms.push(current);
    }
    terms
}

fn normalize_term(term: &str) -> String {
    extract_prompt_terms(term).join(" ")
}

fn add_hashed_prompt_term(hidden: &mut [f32], term: &str, scale: f32) {
    let normalized = term.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        return;
    }
    let digest = Sha256::digest(normalized.as_bytes());
    for (index, value) in hidden.iter_mut().enumerate() {
        let byte = digest[index % digest.len()] as f32 / u8::MAX as f32;
        *value += ((byte * 2.0) - 1.0) * scale;
    }
}

fn append_token_to_prompt(prefix: &str, token_text: &str) -> String {
    if prefix.trim().is_empty() {
        token_text.to_string()
    } else {
        format!("{prefix} {token_text}")
    }
}

fn argmax_index(values: &[f32]) -> usize {
    let mut best_index = 0_usize;
    let mut best_value = f32::NEG_INFINITY;
    for (index, value) in values.iter().copied().enumerate() {
        if value > best_value {
            best_index = index;
            best_value = value;
        }
    }
    best_index
}

fn mat_vec(matrix: &[f32], rows: usize, cols: usize, vector: &[f32]) -> Vec<f32> {
    let mut out = vec![0.0_f32; rows];
    for row in 0..rows {
        let mut total = 0.0_f32;
        for col in 0..cols {
            total += matrix[row * cols + col] * vector[col];
        }
        out[row] = total;
    }
    out
}

fn add_assign(dst: &mut [f32], src: &[f32]) {
    for (left, right) in dst.iter_mut().zip(src) {
        *left += right;
    }
}

fn softmax(logits: &[f32]) -> Vec<f32> {
    let max = logits.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let exp = logits
        .iter()
        .map(|value| (*value - max).exp())
        .collect::<Vec<_>>();
    let sum = exp.iter().sum::<f32>().max(f32::EPSILON);
    exp.into_iter().map(|value| value / sum).collect()
}

fn l2_norm(values: &[f32]) -> f32 {
    values.iter().map(|value| value * value).sum::<f32>().sqrt()
}

fn seeded_matrix(seed: &str, rows: usize, cols: usize, scale: f32) -> Vec<f32> {
    (0..rows * cols)
        .map(|index| {
            let digest = Sha256::digest(format!("{seed}|{index}").as_bytes());
            let raw = u16::from_le_bytes([digest[0], digest[1]]) as f32 / u16::MAX as f32;
            ((raw * 2.0) - 1.0) * scale
        })
        .collect()
}

fn stable_prompt_digest(prompt: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_training_sampler_prompt|");
    hasher.update(prompt.trim().as_bytes());
    hex::encode(hasher.finalize())
}

fn stable_refresh_receipt_digest(
    service_id: &str,
    adopted_revision_id: &str,
    adopted_revision_number: Option<u64>,
    adapter_identity_digest: &str,
    adopted_at_ms: u64,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_training_sampler_refresh|");
    hasher.update(service_id.as_bytes());
    hasher.update(b"|");
    hasher.update(adopted_revision_id.as_bytes());
    if let Some(adopted_revision_number) = adopted_revision_number {
        hasher.update(b"|");
        hasher.update(adopted_revision_number.to_le_bytes());
    }
    hasher.update(b"|");
    hasher.update(adapter_identity_digest.as_bytes());
    hasher.update(b"|");
    hasher.update(adopted_at_ms.to_le_bytes());
    hex::encode(hasher.finalize())
}

fn stable_generation_response_digest(
    request_id: &str,
    revision_id: &str,
    prompt: &str,
    finish_reason: TrainingSamplerFinishReason,
    tokens: &[TrainingSamplerGeneratedToken],
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_training_sampler_generation|");
    hasher.update(request_id.as_bytes());
    hasher.update(b"|");
    hasher.update(revision_id.as_bytes());
    hasher.update(b"|");
    hasher.update(stable_prompt_digest(prompt).as_bytes());
    hasher.update(b"|");
    hasher.update(match finish_reason {
        TrainingSamplerFinishReason::StopToken => b"stop_token".as_slice(),
        TrainingSamplerFinishReason::MaxTokens => b"max_tokens".as_slice(),
    });
    for token in tokens {
        hasher.update(b"|");
        hasher.update(token.position.to_le_bytes());
        hasher.update(b"|");
        hasher.update(token.token_id.to_le_bytes());
        hasher.update(b"|");
        hasher.update(token.logprob.to_bits().to_le_bytes());
    }
    hex::encode(hasher.finalize())
}

fn stable_logprob_response_digest(
    request_id: &str,
    revision_id: &str,
    final_prompt: &str,
    joint_logprob: f32,
    tokens: &[TrainingSamplerGeneratedToken],
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_training_sampler_logprob|");
    hasher.update(request_id.as_bytes());
    hasher.update(b"|");
    hasher.update(revision_id.as_bytes());
    hasher.update(b"|");
    hasher.update(stable_prompt_digest(final_prompt).as_bytes());
    hasher.update(b"|");
    hasher.update(joint_logprob.to_bits().to_le_bytes());
    for token in tokens {
        hasher.update(b"|");
        hasher.update(token.position.to_le_bytes());
        hasher.update(b"|");
        hasher.update(token.token_id.to_le_bytes());
        hasher.update(b"|");
        hasher.update(token.logprob.to_bits().to_le_bytes());
    }
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use psionic_data::{TokenizerDigest, TokenizerFamily};
    use psionic_runtime::TrainingCheckpointReference;

    use super::*;
    use crate::{
        OpenAdapterAdmissibleModelFamily, OpenAdapterExecutionConfig, OpenAdapterLmHeadTarget,
        OpenAdapterPrecisionPolicy, OpenAdapterSftRunRequest, OpenAdapterTrainingExecutionBackend,
        TrainingLoopBudget, TrainingOptimizerConfig, TrainingOptimizerResidencyPolicy,
        run_open_adapter_sft_export,
    };

    fn tokenizer() -> TokenizerDigest {
        TokenizerDigest::new(TokenizerFamily::BytePairEncoding, "sampler-tokenizer", 64)
            .with_template_digest("sampler-template-v1")
    }

    fn training_config(run_id: &str) -> OpenAdapterExecutionConfig {
        OpenAdapterExecutionConfig {
            run_id: run_id.to_string(),
            checkpoint_family: "training.sampler.reference".to_string(),
            execution_backend_label: crate::OPEN_ADAPTER_CUDA_BACKEND_LABEL.to_string(),
            admissible_model_family: OpenAdapterAdmissibleModelFamily::GptOssDecoderLmHeadLora,
            budget: TrainingLoopBudget::new(8, 1, 1).expect("budget"),
            batch_size: 2,
            precision_policy: OpenAdapterPrecisionPolicy::F32Reference,
            model: OpenAdapterReferenceModel {
                base_model_id: "gpt-oss-20b".to_string(),
                base_model_revision: "2026-04".to_string(),
                base_served_artifact_digest: "sha256:base-open-adapter".to_string(),
                tokenizer: tokenizer(),
                hidden_size: 4,
                vocab_size: 4,
                target: OpenAdapterLmHeadTarget {
                    target_id: "lm_head".to_string(),
                    lora_rank: 2,
                    lora_alpha: 8.0,
                    optimizer: TrainingOptimizerConfig::adamw(0.2, 0.9, 0.99, 1e-8)
                        .with_gradient_clip_norm(1.0),
                    optimizer_residency_policy: TrainingOptimizerResidencyPolicy::host_only(),
                },
            },
        }
    }

    fn sampler_config() -> OpenAdapterTrainingSamplerConfig {
        OpenAdapterTrainingSamplerConfig {
            service_id: "training-sampler-service".to_string(),
            policy_family: "training.sampler.reference".to_string(),
            model: training_config("sampler-config").model,
            vocabulary: vec![
                TrainingSamplerVocabularyToken::new(0, "umbrella"),
                TrainingSamplerVocabularyToken::new(1, "coat"),
                TrainingSamplerVocabularyToken::new(2, "sandals"),
                TrainingSamplerVocabularyToken::new(3, "<stop>"),
            ],
            prompt_feature_lexicon: vec![
                TrainingSamplerPromptFeature::new("rain", vec![1.0, 0.0, 0.0, 0.0]),
                TrainingSamplerPromptFeature::new("cold", vec![0.0, 1.0, 0.0, 0.0]),
                TrainingSamplerPromptFeature::new("hot", vec![0.0, 0.0, 1.0, 0.0]),
            ],
            policy: TrainingSamplerServicePolicy {
                max_prompt_chars: 512,
                max_completion_tokens: 4,
                max_logprob_tokens: 8,
                max_top_logprobs: 3,
                freshness_budget_ms: 5_000,
                stop_token_id: Some(3),
            },
        }
    }

    fn checkpoint_reference(
        checkpoint_ref: &str,
        started_at_ms: u64,
    ) -> TrainingCheckpointReference {
        TrainingCheckpointReference::new(
            "training.sampler.reference",
            format!("stream://{checkpoint_ref}"),
            format!("manifest://{checkpoint_ref}"),
            format!("object://{checkpoint_ref}"),
            "node-a",
            1,
            "cluster-digest-training-sampler",
            "topology-digest-training-sampler",
            started_at_ms,
        )
        .with_checkpoint_ref(checkpoint_ref)
        .with_step(12)
    }

    fn samples_for_prompt(
        config: &OpenAdapterTrainingSamplerConfig,
        prompt: &str,
        target_token_id: u32,
        sample_prefix: &str,
    ) -> Vec<crate::OpenAdapterHiddenStateSample> {
        let hidden = config.encode_prompt_hidden_state(prompt);
        (0..4)
            .map(|index| {
                crate::OpenAdapterHiddenStateSample::new(
                    format!("{sample_prefix}-{index}"),
                    hidden.clone(),
                    target_token_id,
                    8 - index,
                )
                .expect("sample")
            })
            .collect()
    }

    fn served_revision_from_training(
        run_id: &str,
        adapter_revision: &str,
        prompt: &str,
        target_token_id: u32,
        revision_number: u64,
        produced_at_ms: u64,
    ) -> TrainingSamplerServedRevision {
        let config = sampler_config();
        let backend = OpenAdapterTrainingExecutionBackend::new(
            training_config(run_id),
            samples_for_prompt(&config, prompt, target_token_id, adapter_revision),
        )
        .expect("backend");
        let outcome = run_open_adapter_sft_export(
            &backend,
            &OpenAdapterSftRunRequest {
                dataset_ref: format!("dataset://{run_id}"),
                validator_policy_ref: format!("validator://{run_id}"),
                adapter_id: "weather-kit".to_string(),
                adapter_revision: adapter_revision.to_string(),
                started_at_ms: produced_at_ms,
                step_duration_ms: 20,
            },
        )
        .expect("training outcome");
        TrainingSamplerServedRevision::new(
            PolicyRevision::new(
                "training.sampler.reference",
                format!("policy:{adapter_revision}"),
                outcome.summary.adapter_artifact_digest.clone(),
                produced_at_ms,
            )
            .with_revision_number(revision_number)
            .with_checkpoint(checkpoint_reference(
                format!("checkpoint://{run_id}/{adapter_revision}").as_str(),
                produced_at_ms,
            )),
            outcome.adapter_identity.clone(),
            outcome.summary.lora_alpha,
            outcome.adapter_bytes.clone(),
        )
    }

    #[test]
    fn training_sampler_service_refreshes_and_serves_generation_and_logprobs()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut service = TrainingSamplerService::new(sampler_config())?;
        assert_eq!(
            service.status(1_000).health_state,
            TrainingSamplerHealthState::Uninitialized
        );

        let refresh = service.refresh_revision(
            served_revision_from_training("run-a", "rev-a", "rain", 0, 1, 1_000),
            1_200,
        )?;
        assert_eq!(refresh.adopted_revision_number, Some(1));

        let completion = service.complete(&TrainingSamplerCompletionRequest {
            request_id: "completion-a".to_string(),
            prompt: "rain".to_string(),
            max_tokens: 1,
            requested_revision_id: Some("policy:rev-a".to_string()),
            top_logprobs: Some(2),
            requested_at_ms: 1_300,
        })?;
        assert_eq!(completion.text, "umbrella");
        assert_eq!(completion.tokens.len(), 1);
        assert_eq!(completion.tokens[0].token_text, "umbrella");
        assert_eq!(
            completion.active_revision.policy_revision.revision_id,
            "policy:rev-a"
        );
        assert_eq!(
            service.status(1_300).health_state,
            TrainingSamplerHealthState::Ready
        );

        let chat = service.chat_complete(&TrainingSamplerChatRequest {
            request_id: "chat-a".to_string(),
            messages: vec![TrainingSamplerChatMessage::new(
                TrainingSamplerChatRole::User,
                "rain",
            )],
            max_tokens: 1,
            requested_revision_id: None,
            top_logprobs: Some(2),
            requested_at_ms: 1_320,
        })?;
        assert_eq!(chat.text, "umbrella");

        let logprobs = service.token_logprobs(&TrainingSamplerLogprobRequest {
            request_id: "logprob-a".to_string(),
            prompt: "rain".to_string(),
            continuation_token_ids: vec![0],
            requested_revision_id: None,
            top_logprobs: Some(2),
            requested_at_ms: 1_340,
        })?;
        assert_eq!(logprobs.tokens.len(), 1);
        assert_eq!(logprobs.tokens[0].token_text, "umbrella");
        assert!(logprobs.joint_logprob.is_finite());
        assert_eq!(service.status(1_340).request_counters.refresh_count, 1);
        assert_eq!(
            service
                .status(1_340)
                .request_counters
                .completion_request_count,
            1
        );
        assert_eq!(service.status(1_340).request_counters.chat_request_count, 1);
        assert_eq!(
            service.status(1_340).request_counters.logprob_query_count,
            1
        );
        Ok(())
    }

    #[test]
    fn training_sampler_service_hot_swaps_revisions_and_refuses_stale_or_unavailable_requests()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut service = TrainingSamplerService::new(sampler_config())?;
        service.refresh_revision(
            served_revision_from_training("run-a", "rev-a", "rain", 0, 1, 1_000),
            1_200,
        )?;
        let before = service.complete(&TrainingSamplerCompletionRequest {
            request_id: "completion-before".to_string(),
            prompt: "rain".to_string(),
            max_tokens: 1,
            requested_revision_id: None,
            top_logprobs: None,
            requested_at_ms: 1_300,
        })?;
        assert_eq!(before.text, "umbrella");

        service.refresh_revision(
            served_revision_from_training("run-b", "rev-b", "rain", 1, 2, 2_000),
            2_200,
        )?;
        let after = service.complete(&TrainingSamplerCompletionRequest {
            request_id: "completion-after".to_string(),
            prompt: "rain".to_string(),
            max_tokens: 1,
            requested_revision_id: Some("policy:rev-b".to_string()),
            top_logprobs: None,
            requested_at_ms: 2_300,
        })?;
        assert_eq!(after.text, "coat");
        assert_eq!(
            after.active_revision.policy_revision.revision_id,
            "policy:rev-b"
        );

        let unavailable = service.complete(&TrainingSamplerCompletionRequest {
            request_id: "completion-unavailable".to_string(),
            prompt: "rain".to_string(),
            max_tokens: 1,
            requested_revision_id: Some("policy:rev-a".to_string()),
            top_logprobs: None,
            requested_at_ms: 2_320,
        });
        assert_eq!(
            unavailable,
            Err(TrainingSamplerServiceError::RequestedRevisionUnavailable {
                requested_revision_id: "policy:rev-a".to_string(),
                active_revision_id: "policy:rev-b".to_string(),
            })
        );

        let stale = service.complete(&TrainingSamplerCompletionRequest {
            request_id: "completion-stale".to_string(),
            prompt: "rain".to_string(),
            max_tokens: 1,
            requested_revision_id: None,
            top_logprobs: None,
            requested_at_ms: 8_500,
        });
        assert_eq!(
            stale,
            Err(TrainingSamplerServiceError::StaleActiveRevision {
                revision_id: "policy:rev-b".to_string(),
                age_ms: 6_500,
                freshness_budget_ms: 5_000,
            })
        );
        assert_eq!(
            service.status(8_500).health_state,
            TrainingSamplerHealthState::Stale
        );
        Ok(())
    }
}
