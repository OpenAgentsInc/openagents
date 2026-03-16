//! Rust-native dataset, tokenizer, split, streamed-iteration, and packing
//! contracts for Psionic.

#![cfg_attr(
    test,
    allow(clippy::expect_used, clippy::panic, clippy::panic_in_result_fn)
)]

mod apple_adapter;
mod apple_adapter_curation;
mod tassadar;

use std::collections::{BTreeMap, BTreeSet};

use psionic_datastream::{
    DatastreamDatasetBinding, DatastreamEncoding, DatastreamManifest, DatastreamManifestRef,
    DatastreamSubjectKind,
};
use psionic_runtime::{DeterminismContractError, GeneratorState, RuntimeDeterminismContract};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use thiserror::Error;

pub use apple_adapter::*;
pub use apple_adapter_curation::*;
pub use tassadar::*;

/// Human-readable crate ownership summary.
pub const CRATE_ROLE: &str =
    "dataset, tokenizer, split, iteration, and packing contracts for Psionic";

/// Stable ABI version for Psionic-native data contracts.
pub const DATA_ABI_VERSION: &str = "psionic.data.v1";

/// Stable versioned dataset identity.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct DatasetKey {
    /// Stable dataset reference.
    pub dataset_ref: String,
    /// Immutable dataset version.
    pub version: String,
}

impl DatasetKey {
    /// Creates a dataset key.
    #[must_use]
    pub fn new(dataset_ref: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            dataset_ref: dataset_ref.into(),
            version: version.into(),
        }
    }

    /// Returns the canonical `dataset_ref@version` storage key.
    #[must_use]
    pub fn storage_key(&self) -> String {
        format!("{}@{}", self.dataset_ref, self.version)
    }

    /// Returns a datastream binding aligned to this dataset key.
    #[must_use]
    pub fn datastream_binding(
        &self,
        split: impl Into<String>,
        shard_key: impl Into<String>,
    ) -> DatastreamDatasetBinding {
        DatastreamDatasetBinding::new(self.storage_key())
            .with_split(split)
            .with_shard_key(shard_key)
    }
}

/// Tokenizer family admitted by the canonical data contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TokenizerFamily {
    /// SentencePiece or Unigram model family.
    SentencePiece,
    /// GPT-style byte-pair encoding family.
    BytePairEncoding,
    /// WordPiece family.
    WordPiece,
    /// Generic unigram family.
    Unigram,
    /// Non-standard custom tokenizer package.
    Custom,
}

/// Stable digest summary for one tokenizer contract.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenizerDigest {
    /// Tokenizer family.
    pub family: TokenizerFamily,
    /// Stable digest over the tokenizer package.
    pub tokenizer_digest: String,
    /// Vocabulary size surfaced to training or eval loops.
    pub vocab_size: u32,
    /// Optional digest over special-token configuration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub special_tokens_digest: Option<String>,
    /// Optional digest over the prompt or chat template.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub template_digest: Option<String>,
}

impl TokenizerDigest {
    /// Creates a tokenizer digest summary.
    #[must_use]
    pub fn new(
        family: TokenizerFamily,
        tokenizer_digest: impl Into<String>,
        vocab_size: u32,
    ) -> Self {
        Self {
            family,
            tokenizer_digest: tokenizer_digest.into(),
            vocab_size,
            special_tokens_digest: None,
            template_digest: None,
        }
    }

    /// Attaches a special-token digest.
    #[must_use]
    pub fn with_special_tokens_digest(mut self, special_tokens_digest: impl Into<String>) -> Self {
        self.special_tokens_digest = Some(special_tokens_digest.into());
        self
    }

    /// Attaches a template digest.
    #[must_use]
    pub fn with_template_digest(mut self, template_digest: impl Into<String>) -> Self {
        self.template_digest = Some(template_digest.into());
        self
    }

    /// Returns a stable digest over the tokenizer contract.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(b"psionic_tokenizer_digest|");
        hasher.update(tokenizer_family_label(self.family));
        hasher.update(b"|");
        hasher.update(self.tokenizer_digest.as_bytes());
        hasher.update(b"|");
        hasher.update(self.vocab_size.to_string().as_bytes());
        if let Some(digest) = &self.special_tokens_digest {
            hasher.update(b"|special|");
            hasher.update(digest.as_bytes());
        }
        if let Some(digest) = &self.template_digest {
            hasher.update(b"|template|");
            hasher.update(digest.as_bytes());
        }
        hex::encode(hasher.finalize())
    }
}

/// High-level record encoding for one dataset manifest.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DatasetRecordEncoding {
    /// JSONL text or prompt-completion records.
    JsonlText,
    /// JSONL conversation turns or messages.
    JsonlConversation,
    /// Token IDs streamed as little-endian `u32`.
    TokenIdsLeU32,
    /// Preference or ranking pairs in JSONL.
    PreferenceJsonl,
    /// Opaque binary or custom records.
    Binary,
}

/// High-level split role for one dataset.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DatasetSplitKind {
    /// Main training split.
    Train,
    /// Validation or tuning split.
    Validation,
    /// Final test split.
    Test,
    /// Held-out benchmark split.
    HeldOut,
    /// Preference or ranking split.
    Preference,
    /// Replay or off-policy split.
    Replay,
    /// Custom workload-specific split.
    Custom,
}

/// Iteration mode for streamed dataset access.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DatasetIterationMode {
    /// Stop once the declared split is exhausted.
    SinglePass,
    /// Restart from the beginning and advance epoch when exhausted.
    Repeat,
}

/// Shard ordering policy for iteration.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DatasetShardOrdering {
    /// Use manifest order.
    ManifestOrder,
    /// Reorder shards deterministically by `shuffle_seed` and `epoch`.
    DeterministicShuffle,
}

/// Packing policy family for long-context or token-budgeted workloads.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DatasetPackingMode {
    /// Concatenate multiple short sequences into one context window.
    PackIntoContextWindow,
    /// Keep each sequence independent but group rows into batches by token budget.
    BatchByTokenBudget,
}

/// What to do when one input sequence is longer than the configured limit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OverlongSequencePosture {
    /// Refuse planning and surface a typed error.
    Refuse,
    /// Drop the sequence and record it in the plan.
    Drop,
}

/// One tokenized shard that belongs to one dataset split.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DatasetShardManifest {
    /// Stable shard identity inside the split.
    pub shard_key: String,
    /// Datastream-backed manifest reference for the shard payload.
    pub manifest: DatastreamManifestRef,
    /// Number of logical sequences carried by the shard.
    pub sequence_count: u64,
    /// Number of tokens carried by the shard.
    pub token_count: u64,
    /// Smallest declared sequence length in tokens.
    pub min_sequence_tokens: u32,
    /// Largest declared sequence length in tokens.
    pub max_sequence_tokens: u32,
}

impl DatasetShardManifest {
    /// Creates a shard manifest and validates that its datastream binding lines up with the
    /// dataset and split identity.
    pub fn new(
        dataset: &DatasetKey,
        split_name: impl Into<String>,
        shard_key: impl Into<String>,
        manifest: DatastreamManifestRef,
        sequence_count: u64,
        token_count: u64,
        min_sequence_tokens: u32,
        max_sequence_tokens: u32,
    ) -> Result<Self, DatasetContractError> {
        let split_name = split_name.into();
        let shard_key = shard_key.into();
        validate_shard_contract(
            dataset,
            split_name.as_str(),
            shard_key.as_str(),
            &manifest,
            sequence_count,
            token_count,
            min_sequence_tokens,
            max_sequence_tokens,
        )?;
        Ok(Self {
            shard_key,
            manifest,
            sequence_count,
            token_count,
            min_sequence_tokens,
            max_sequence_tokens,
        })
    }
}

/// Declared split inside a versioned dataset manifest.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DatasetSplitDeclaration {
    /// Stable split name, such as `train`.
    pub split_name: String,
    /// High-level split role.
    pub kind: DatasetSplitKind,
    /// Total number of sequences across all shards.
    pub sequence_count: u64,
    /// Total number of tokens across all shards.
    pub token_count: u64,
    /// Sharded datastream refs for the split.
    pub shards: Vec<DatasetShardManifest>,
}

impl DatasetSplitDeclaration {
    /// Creates a split declaration from sharded datastream-backed manifests.
    pub fn new(
        dataset: &DatasetKey,
        split_name: impl Into<String>,
        kind: DatasetSplitKind,
        shards: Vec<DatasetShardManifest>,
    ) -> Result<Self, DatasetContractError> {
        let split_name = split_name.into();
        if split_name.trim().is_empty() {
            return Err(DatasetContractError::MissingSplitName);
        }
        if shards.is_empty() {
            return Err(DatasetContractError::SplitHasNoShards {
                split_name: split_name.clone(),
            });
        }
        let mut shard_keys = BTreeSet::new();
        let mut sequence_count = 0_u64;
        let mut token_count = 0_u64;
        for shard in &shards {
            if !shard_keys.insert(shard.shard_key.clone()) {
                return Err(DatasetContractError::DuplicateShard {
                    split_name: split_name.clone(),
                    shard_key: shard.shard_key.clone(),
                });
            }
            validate_shard_contract(
                dataset,
                split_name.as_str(),
                shard.shard_key.as_str(),
                &shard.manifest,
                shard.sequence_count,
                shard.token_count,
                shard.min_sequence_tokens,
                shard.max_sequence_tokens,
            )?;
            sequence_count = sequence_count.saturating_add(shard.sequence_count);
            token_count = token_count.saturating_add(shard.token_count);
        }
        Ok(Self {
            split_name,
            kind,
            sequence_count,
            token_count,
            shards,
        })
    }

    fn avg_sequence_tokens_for_shard(&self, shard: &DatasetShardManifest) -> u64 {
        if shard.sequence_count == 0 {
            0
        } else {
            shard.token_count.div_ceil(shard.sequence_count)
        }
    }
}

/// Full canonical manifest for one versioned dataset.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DatasetManifest {
    /// Stable ABI version.
    pub abi_version: String,
    /// Stable dataset identity.
    pub key: DatasetKey,
    /// Human-readable dataset name.
    pub display_name: String,
    /// Record encoding presented to train or eval loops.
    pub record_encoding: DatasetRecordEncoding,
    /// Tokenizer contract used to produce tokenized shards.
    pub tokenizer: TokenizerDigest,
    /// Long-context window the dataset was prepared against when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_window_tokens: Option<u32>,
    /// Optional provenance digest over upstream source material.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provenance_digest: Option<String>,
    /// Declared splits.
    pub splits: Vec<DatasetSplitDeclaration>,
    /// Extension metadata.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub metadata: BTreeMap<String, Value>,
}

impl DatasetManifest {
    /// Creates a canonical dataset manifest with ABI version pinned.
    #[must_use]
    pub fn new(
        key: DatasetKey,
        display_name: impl Into<String>,
        record_encoding: DatasetRecordEncoding,
        tokenizer: TokenizerDigest,
    ) -> Self {
        Self {
            abi_version: String::from(DATA_ABI_VERSION),
            key,
            display_name: display_name.into(),
            record_encoding,
            tokenizer,
            context_window_tokens: None,
            provenance_digest: None,
            splits: Vec::new(),
            metadata: BTreeMap::new(),
        }
    }

    /// Attaches a declared context window.
    #[must_use]
    pub const fn with_context_window_tokens(mut self, context_window_tokens: u32) -> Self {
        self.context_window_tokens = Some(context_window_tokens);
        self
    }

    /// Attaches a provenance digest.
    #[must_use]
    pub fn with_provenance_digest(mut self, provenance_digest: impl Into<String>) -> Self {
        self.provenance_digest = Some(provenance_digest.into());
        self
    }

    /// Attaches split declarations.
    #[must_use]
    pub fn with_splits(mut self, splits: Vec<DatasetSplitDeclaration>) -> Self {
        self.splits = splits;
        self
    }

    /// Returns the canonical storage key.
    #[must_use]
    pub fn storage_key(&self) -> String {
        self.key.storage_key()
    }

    /// Returns the split declaration for the provided split name.
    pub fn split(&self, split_name: &str) -> Option<&DatasetSplitDeclaration> {
        self.splits
            .iter()
            .find(|split| split.split_name == split_name)
    }

    /// Returns a stable digest over the manifest contract.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(b"psionic_dataset_manifest|");
        hasher.update(self.abi_version.as_bytes());
        hasher.update(b"|");
        hasher.update(self.storage_key().as_bytes());
        hasher.update(b"|");
        hasher.update(self.display_name.as_bytes());
        hasher.update(b"|");
        hasher.update(dataset_record_encoding_label(self.record_encoding));
        hasher.update(b"|");
        hasher.update(self.tokenizer.stable_digest().as_bytes());
        if let Some(context_window_tokens) = self.context_window_tokens {
            hasher.update(b"|context|");
            hasher.update(context_window_tokens.to_string().as_bytes());
        }
        if let Some(provenance_digest) = &self.provenance_digest {
            hasher.update(b"|provenance|");
            hasher.update(provenance_digest.as_bytes());
        }
        for split in &self.splits {
            hasher.update(b"|split|");
            hasher.update(split.split_name.as_bytes());
            hasher.update(b"|");
            hasher.update(dataset_split_kind_label(split.kind));
            hasher.update(b"|");
            hasher.update(split.sequence_count.to_string().as_bytes());
            hasher.update(b"|");
            hasher.update(split.token_count.to_string().as_bytes());
            for shard in &split.shards {
                hasher.update(b"|shard|");
                hasher.update(shard.shard_key.as_bytes());
                hasher.update(b"|");
                hasher.update(shard.manifest.manifest_digest.as_bytes());
                hasher.update(b"|");
                hasher.update(shard.sequence_count.to_string().as_bytes());
                hasher.update(b"|");
                hasher.update(shard.token_count.to_string().as_bytes());
                hasher.update(b"|");
                hasher.update(shard.min_sequence_tokens.to_string().as_bytes());
                hasher.update(b"|");
                hasher.update(shard.max_sequence_tokens.to_string().as_bytes());
            }
        }
        hex::encode(hasher.finalize())
    }

    /// Validates the manifest contract.
    pub fn validate(&self) -> Result<(), DatasetContractError> {
        if self.abi_version != DATA_ABI_VERSION {
            return Err(DatasetContractError::UnsupportedAbiVersion {
                abi_version: self.abi_version.clone(),
            });
        }
        if self.key.dataset_ref.trim().is_empty() {
            return Err(DatasetContractError::MissingDatasetRef);
        }
        if self.key.version.trim().is_empty() {
            return Err(DatasetContractError::MissingVersion);
        }
        if self.display_name.trim().is_empty() {
            return Err(DatasetContractError::MissingDisplayName);
        }
        if self.tokenizer.tokenizer_digest.trim().is_empty() {
            return Err(DatasetContractError::MissingTokenizerDigest);
        }
        if self.tokenizer.vocab_size == 0 {
            return Err(DatasetContractError::InvalidTokenizerVocabSize);
        }
        if self
            .context_window_tokens
            .is_some_and(|context_window_tokens| context_window_tokens == 0)
        {
            return Err(DatasetContractError::InvalidContextWindow);
        }
        if self.splits.is_empty() {
            return Err(DatasetContractError::DatasetHasNoSplits);
        }
        let mut split_names = BTreeSet::new();
        for split in &self.splits {
            if !split_names.insert(split.split_name.clone()) {
                return Err(DatasetContractError::DuplicateSplit {
                    split_name: split.split_name.clone(),
                });
            }
            if split.shards.is_empty() {
                return Err(DatasetContractError::SplitHasNoShards {
                    split_name: split.split_name.clone(),
                });
            }
            let derived_sequence_count =
                split.shards.iter().map(|shard| shard.sequence_count).sum();
            let derived_token_count = split.shards.iter().map(|shard| shard.token_count).sum();
            if derived_sequence_count != split.sequence_count {
                return Err(DatasetContractError::SplitSequenceCountMismatch {
                    split_name: split.split_name.clone(),
                    declared: split.sequence_count,
                    derived: derived_sequence_count,
                });
            }
            if derived_token_count != split.token_count {
                return Err(DatasetContractError::SplitTokenCountMismatch {
                    split_name: split.split_name.clone(),
                    declared: split.token_count,
                    derived: derived_token_count,
                });
            }
            for shard in &split.shards {
                validate_shard_contract(
                    &self.key,
                    split.split_name.as_str(),
                    shard.shard_key.as_str(),
                    &shard.manifest,
                    shard.sequence_count,
                    shard.token_count,
                    shard.min_sequence_tokens,
                    shard.max_sequence_tokens,
                )?;
            }
        }
        Ok(())
    }
}

/// Resume-safe iteration cursor for one split.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DatasetIterationCursor {
    /// Split owned by this cursor.
    pub split_name: String,
    /// Current epoch.
    pub epoch: u32,
    /// Index into the ordered shard list for this epoch.
    pub next_shard_index: usize,
    /// Sequence offset within the current shard.
    pub next_sequence_index_in_shard: u64,
    /// Total emitted sequences across all windows.
    pub emitted_sequences: u64,
}

impl DatasetIterationCursor {
    /// Creates a fresh cursor for one split.
    #[must_use]
    pub fn new(split_name: impl Into<String>) -> Self {
        Self {
            split_name: split_name.into(),
            epoch: 0,
            next_shard_index: 0,
            next_sequence_index_in_shard: 0,
            emitted_sequences: 0,
        }
    }
}

/// One planned span over a tokenized shard.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DatasetStreamSpan {
    /// Stable shard identity.
    pub shard_key: String,
    /// Datastream manifest ref for the shard.
    pub manifest: DatastreamManifestRef,
    /// First logical sequence index consumed from the shard.
    pub start_sequence_index: u64,
    /// Number of sequences consumed from the shard.
    pub sequence_count: u64,
    /// Approximate token count covered by this span, based on the shard-average sequence length.
    pub approx_token_count: u64,
}

/// One deterministic iteration window over a split.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DatasetIterationWindow {
    /// Stable window identifier.
    pub window_id: String,
    /// Stable digest over the iteration contract.
    pub contract_digest: String,
    /// Cursor before this window was planned.
    pub start_cursor: DatasetIterationCursor,
    /// Cursor after this window was planned.
    pub end_cursor: DatasetIterationCursor,
    /// Shard spans included in the window.
    pub spans: Vec<DatasetStreamSpan>,
    /// Whether a single-pass iterator is exhausted after this window.
    pub exhausted: bool,
}

/// Reusable streamed iteration contract.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DatasetIterationContract {
    /// Stable dataset identity this contract applies to.
    pub dataset: DatasetKey,
    /// Split name.
    pub split_name: String,
    /// Iteration mode.
    pub mode: DatasetIterationMode,
    /// Shard ordering policy.
    pub shard_ordering: DatasetShardOrdering,
    /// Seed used for deterministic shuffle.
    pub shuffle_seed: u64,
}

impl DatasetIterationContract {
    /// Creates an iteration contract for one split.
    #[must_use]
    pub fn new(dataset: DatasetKey, split_name: impl Into<String>) -> Self {
        Self {
            dataset,
            split_name: split_name.into(),
            mode: DatasetIterationMode::SinglePass,
            shard_ordering: DatasetShardOrdering::ManifestOrder,
            shuffle_seed: 0,
        }
    }

    /// Attaches an iteration mode.
    #[must_use]
    pub const fn with_mode(mut self, mode: DatasetIterationMode) -> Self {
        self.mode = mode;
        self
    }

    /// Attaches a shard ordering policy.
    #[must_use]
    pub const fn with_shard_ordering(mut self, shard_ordering: DatasetShardOrdering) -> Self {
        self.shard_ordering = shard_ordering;
        self
    }

    /// Attaches an explicit shuffle seed.
    #[must_use]
    pub const fn with_shuffle_seed(mut self, shuffle_seed: u64) -> Self {
        self.shuffle_seed = shuffle_seed;
        self
    }

    /// Returns a stable digest over the iteration contract.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(b"psionic_dataset_iteration_contract|");
        hasher.update(self.dataset.storage_key().as_bytes());
        hasher.update(b"|");
        hasher.update(self.split_name.as_bytes());
        hasher.update(b"|");
        hasher.update(dataset_iteration_mode_label(self.mode));
        hasher.update(b"|");
        hasher.update(dataset_shard_ordering_label(self.shard_ordering));
        hasher.update(b"|");
        hasher.update(self.shuffle_seed.to_string().as_bytes());
        hex::encode(hasher.finalize())
    }

    /// Plans a deterministic streamed iteration window over one dataset split.
    pub fn plan_window(
        &self,
        manifest: &DatasetManifest,
        cursor: &DatasetIterationCursor,
        max_sequences: u64,
    ) -> Result<Option<DatasetIterationWindow>, DatasetContractError> {
        if max_sequences == 0 {
            return Err(DatasetContractError::InvalidIterationWindowSize);
        }
        manifest.validate()?;
        if manifest.key != self.dataset {
            return Err(DatasetContractError::DatasetMismatch {
                expected: self.dataset.storage_key(),
                actual: manifest.storage_key(),
            });
        }
        if cursor.split_name != self.split_name {
            return Err(DatasetContractError::IterationCursorSplitMismatch {
                expected: self.split_name.clone(),
                actual: cursor.split_name.clone(),
            });
        }
        let Some(split) = manifest.split(self.split_name.as_str()) else {
            return Err(DatasetContractError::UnknownSplit {
                split_name: self.split_name.clone(),
            });
        };
        let start_cursor = cursor.clone();
        let mut end_cursor = cursor.clone();
        let mut spans = Vec::new();
        let mut sequences_remaining = max_sequences;

        while sequences_remaining > 0 {
            let ordered_shards = ordered_shards(
                split,
                self.shard_ordering,
                self.shuffle_seed,
                end_cursor.epoch,
            );
            if ordered_shards.is_empty() {
                break;
            }
            if end_cursor.next_shard_index >= ordered_shards.len() {
                match self.mode {
                    DatasetIterationMode::SinglePass => break,
                    DatasetIterationMode::Repeat => {
                        end_cursor.epoch = end_cursor.epoch.saturating_add(1);
                        end_cursor.next_shard_index = 0;
                        end_cursor.next_sequence_index_in_shard = 0;
                        continue;
                    }
                }
            }
            let shard = ordered_shards[end_cursor.next_shard_index];
            if end_cursor.next_sequence_index_in_shard >= shard.sequence_count {
                end_cursor.next_shard_index = end_cursor.next_shard_index.saturating_add(1);
                end_cursor.next_sequence_index_in_shard = 0;
                continue;
            }
            let shard_remaining = shard
                .sequence_count
                .saturating_sub(end_cursor.next_sequence_index_in_shard);
            let take_count = shard_remaining.min(sequences_remaining);
            let avg_tokens = split.avg_sequence_tokens_for_shard(shard);
            spans.push(DatasetStreamSpan {
                shard_key: shard.shard_key.clone(),
                manifest: shard.manifest.clone(),
                start_sequence_index: end_cursor.next_sequence_index_in_shard,
                sequence_count: take_count,
                approx_token_count: avg_tokens.saturating_mul(take_count),
            });
            end_cursor.next_sequence_index_in_shard = end_cursor
                .next_sequence_index_in_shard
                .saturating_add(take_count);
            end_cursor.emitted_sequences = end_cursor.emitted_sequences.saturating_add(take_count);
            sequences_remaining = sequences_remaining.saturating_sub(take_count);
            if end_cursor.next_sequence_index_in_shard >= shard.sequence_count {
                end_cursor.next_shard_index = end_cursor.next_shard_index.saturating_add(1);
                end_cursor.next_sequence_index_in_shard = 0;
            }
        }

        if spans.is_empty() {
            return Ok(None);
        }

        let exhausted = self.mode == DatasetIterationMode::SinglePass
            && end_cursor.next_shard_index >= split.shards.len()
            && end_cursor.next_sequence_index_in_shard == 0;
        let contract_digest = self.stable_digest();
        let window_id = stable_iteration_window_id(
            contract_digest.as_str(),
            start_cursor.epoch,
            start_cursor.next_shard_index,
            start_cursor.next_sequence_index_in_shard,
            end_cursor.emitted_sequences,
        );
        Ok(Some(DatasetIterationWindow {
            window_id,
            contract_digest,
            start_cursor,
            end_cursor,
            spans,
            exhausted,
        }))
    }
}

/// One logical input sequence available for packing.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DatasetSequenceDescriptor {
    /// Stable sequence identifier.
    pub sequence_id: String,
    /// Stable shard identity.
    pub shard_key: String,
    /// Sequence index inside the source shard.
    pub sequence_index: u64,
    /// Token count for the sequence.
    pub token_count: u32,
}

impl DatasetSequenceDescriptor {
    /// Creates a sequence descriptor.
    #[must_use]
    pub fn new(
        sequence_id: impl Into<String>,
        shard_key: impl Into<String>,
        sequence_index: u64,
        token_count: u32,
    ) -> Self {
        Self {
            sequence_id: sequence_id.into(),
            shard_key: shard_key.into(),
            sequence_index,
            token_count,
        }
    }
}

/// Reusable long-context or batch-packing policy.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DatasetPackingPolicy {
    /// Packing strategy.
    pub packing_mode: DatasetPackingMode,
    /// Maximum tokens admitted in one packed row.
    pub max_row_tokens: u32,
    /// Maximum padded tokens admitted in one batch.
    pub max_batch_tokens: u32,
    /// Maximum packed rows admitted in one batch.
    pub max_rows_per_batch: usize,
    /// Optional padding multiple for kernels that prefer aligned row lengths.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pad_to_multiple_of: Option<u32>,
    /// Overlong sequence handling posture.
    pub overlong_sequence_posture: OverlongSequencePosture,
}

impl DatasetPackingPolicy {
    /// Creates a packing policy.
    #[must_use]
    pub fn new(
        packing_mode: DatasetPackingMode,
        max_row_tokens: u32,
        max_batch_tokens: u32,
        max_rows_per_batch: usize,
    ) -> Self {
        Self {
            packing_mode,
            max_row_tokens,
            max_batch_tokens,
            max_rows_per_batch,
            pad_to_multiple_of: None,
            overlong_sequence_posture: OverlongSequencePosture::Refuse,
        }
    }

    /// Attaches a padding multiple.
    #[must_use]
    pub const fn with_pad_to_multiple_of(mut self, pad_to_multiple_of: u32) -> Self {
        self.pad_to_multiple_of = Some(pad_to_multiple_of);
        self
    }

    /// Attaches an overlong-sequence posture.
    #[must_use]
    pub const fn with_overlong_sequence_posture(
        mut self,
        overlong_sequence_posture: OverlongSequencePosture,
    ) -> Self {
        self.overlong_sequence_posture = overlong_sequence_posture;
        self
    }

    /// Returns a stable digest over the packing policy.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(b"psionic_dataset_packing_policy|");
        hasher.update(dataset_packing_mode_label(self.packing_mode));
        hasher.update(b"|");
        hasher.update(self.max_row_tokens.to_string().as_bytes());
        hasher.update(b"|");
        hasher.update(self.max_batch_tokens.to_string().as_bytes());
        hasher.update(b"|");
        hasher.update(self.max_rows_per_batch.to_string().as_bytes());
        hasher.update(b"|");
        hasher.update(overlong_sequence_posture_label(self.overlong_sequence_posture).as_bytes());
        if let Some(pad_to_multiple_of) = self.pad_to_multiple_of {
            hasher.update(b"|pad|");
            hasher.update(pad_to_multiple_of.to_string().as_bytes());
        }
        hex::encode(hasher.finalize())
    }

    /// Plans packed rows and batches for the provided sequence descriptors.
    pub fn plan(
        &self,
        sequences: &[DatasetSequenceDescriptor],
    ) -> Result<DatasetPackingPlan, DatasetContractError> {
        if self.max_row_tokens == 0 {
            return Err(DatasetContractError::InvalidMaxRowTokens);
        }
        if self.max_batch_tokens == 0 {
            return Err(DatasetContractError::InvalidMaxBatchTokens);
        }
        if self.max_rows_per_batch == 0 {
            return Err(DatasetContractError::InvalidMaxRowsPerBatch);
        }
        if self
            .pad_to_multiple_of
            .is_some_and(|pad_to_multiple_of| pad_to_multiple_of == 0)
        {
            return Err(DatasetContractError::InvalidPaddingMultiple);
        }

        let rows = match self.packing_mode {
            DatasetPackingMode::PackIntoContextWindow => self.plan_packed_rows(sequences)?,
            DatasetPackingMode::BatchByTokenBudget => self.plan_unpacked_rows(sequences)?,
        };
        let batches = plan_batches(rows.as_slice(), self)?;
        let total_source_tokens = sequences
            .iter()
            .map(|sequence| u64::from(sequence.token_count))
            .sum();
        let dropped_sequences = batches
            .iter()
            .flat_map(|batch| batch.rows.iter())
            .flat_map(|row| row.source_sequences.iter())
            .map(|sequence| sequence.sequence_id.clone())
            .collect::<BTreeSet<_>>();
        let dropped_sequences = sequences
            .iter()
            .filter(|sequence| !dropped_sequences.contains(sequence.sequence_id.as_str()))
            .map(|sequence| sequence.sequence_id.clone())
            .collect::<Vec<_>>();

        Ok(DatasetPackingPlan {
            policy_digest: self.stable_digest(),
            packing_mode: self.packing_mode,
            batches,
            total_source_sequences: sequences.len(),
            total_source_tokens,
            dropped_sequences,
        })
    }

    fn plan_unpacked_rows(
        &self,
        sequences: &[DatasetSequenceDescriptor],
    ) -> Result<Vec<PackedSequenceRow>, DatasetContractError> {
        let mut rows = Vec::new();
        for sequence in sequences {
            if sequence.token_count > self.max_row_tokens {
                match self.overlong_sequence_posture {
                    OverlongSequencePosture::Refuse => {
                        return Err(DatasetContractError::SequenceTooLong {
                            sequence_id: sequence.sequence_id.clone(),
                            token_count: sequence.token_count,
                            max_row_tokens: self.max_row_tokens,
                        });
                    }
                    OverlongSequencePosture::Drop => continue,
                }
            }
            let padded_tokens = padded_token_count(sequence.token_count, self.pad_to_multiple_of);
            rows.push(PackedSequenceRow {
                row_id: stable_row_id(sequence.sequence_id.as_str(), sequence.sequence_index),
                source_sequences: vec![sequence.clone()],
                token_count: sequence.token_count,
                padded_token_count: padded_tokens,
            });
        }
        Ok(rows)
    }

    fn plan_packed_rows(
        &self,
        sequences: &[DatasetSequenceDescriptor],
    ) -> Result<Vec<PackedSequenceRow>, DatasetContractError> {
        let mut rows = Vec::new();
        let mut current_sequences = Vec::new();
        let mut current_tokens = 0_u32;
        for sequence in sequences {
            if sequence.token_count > self.max_row_tokens {
                match self.overlong_sequence_posture {
                    OverlongSequencePosture::Refuse => {
                        return Err(DatasetContractError::SequenceTooLong {
                            sequence_id: sequence.sequence_id.clone(),
                            token_count: sequence.token_count,
                            max_row_tokens: self.max_row_tokens,
                        });
                    }
                    OverlongSequencePosture::Drop => continue,
                }
            }
            if current_tokens > 0
                && current_tokens.saturating_add(sequence.token_count) > self.max_row_tokens
            {
                rows.push(PackedSequenceRow {
                    row_id: stable_packed_row_id(current_sequences.as_slice()),
                    source_sequences: current_sequences,
                    token_count: current_tokens,
                    padded_token_count: padded_token_count(current_tokens, self.pad_to_multiple_of),
                });
                current_sequences = Vec::new();
                current_tokens = 0;
            }
            current_tokens = current_tokens.saturating_add(sequence.token_count);
            current_sequences.push(sequence.clone());
        }
        if !current_sequences.is_empty() {
            rows.push(PackedSequenceRow {
                row_id: stable_packed_row_id(current_sequences.as_slice()),
                source_sequences: current_sequences,
                token_count: current_tokens,
                padded_token_count: padded_token_count(current_tokens, self.pad_to_multiple_of),
            });
        }
        Ok(rows)
    }
}

/// One packed row emitted by the packing planner.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackedSequenceRow {
    /// Stable row identity.
    pub row_id: String,
    /// Source sequences carried in this row.
    pub source_sequences: Vec<DatasetSequenceDescriptor>,
    /// Unpadded token count.
    pub token_count: u32,
    /// Padded token count used for batch budget accounting.
    pub padded_token_count: u32,
}

/// One batch emitted by the packing planner.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackedBatch {
    /// Stable batch identifier.
    pub batch_id: String,
    /// Packed rows assigned to this batch.
    pub rows: Vec<PackedSequenceRow>,
    /// Total unpadded token count in the batch.
    pub token_count: u64,
    /// Total padded token count in the batch.
    pub padded_token_count: u64,
    /// Utilization in basis points against the batch token budget.
    pub fill_bps: u16,
}

/// Full output of one packing plan.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DatasetPackingPlan {
    /// Stable digest over the policy that produced this plan.
    pub policy_digest: String,
    /// Packing mode used for the plan.
    pub packing_mode: DatasetPackingMode,
    /// Planned packed batches.
    pub batches: Vec<PackedBatch>,
    /// Number of source sequences presented to the planner.
    pub total_source_sequences: usize,
    /// Total source tokens presented to the planner.
    pub total_source_tokens: u64,
    /// Sequence identifiers dropped due to `OverlongSequencePosture::Drop`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dropped_sequences: Vec<String>,
}

/// Dataset contract validation or planning error.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum DatasetContractError {
    /// The manifest used an unsupported ABI version.
    #[error("unsupported data ABI version `{abi_version}`")]
    UnsupportedAbiVersion {
        /// Observed ABI version.
        abi_version: String,
    },
    /// The dataset manifest omitted the dataset ref.
    #[error("dataset manifest is missing `dataset_ref`")]
    MissingDatasetRef,
    /// The dataset manifest omitted the version.
    #[error("dataset manifest is missing `version`")]
    MissingVersion,
    /// The dataset manifest omitted the display name.
    #[error("dataset manifest is missing `display_name`")]
    MissingDisplayName,
    /// The dataset manifest omitted the tokenizer digest.
    #[error("dataset manifest is missing `tokenizer_digest`")]
    MissingTokenizerDigest,
    /// The dataset manifest declared an invalid vocabulary size.
    #[error("dataset tokenizer `vocab_size` must be greater than zero")]
    InvalidTokenizerVocabSize,
    /// The dataset manifest declared an invalid context window.
    #[error("dataset `context_window_tokens` must be greater than zero when provided")]
    InvalidContextWindow,
    /// The dataset manifest omitted splits.
    #[error("dataset manifest must declare at least one split")]
    DatasetHasNoSplits,
    /// One split omitted its name.
    #[error("dataset split is missing `split_name`")]
    MissingSplitName,
    /// Duplicate split name.
    #[error("dataset split `{split_name}` was defined more than once")]
    DuplicateSplit {
        /// Repeated split name.
        split_name: String,
    },
    /// The split had no shards.
    #[error("dataset split `{split_name}` must declare at least one shard")]
    SplitHasNoShards {
        /// Split name.
        split_name: String,
    },
    /// Duplicate shard name within a split.
    #[error("dataset split `{split_name}` repeated shard `{shard_key}`")]
    DuplicateShard {
        /// Split name.
        split_name: String,
        /// Repeated shard key.
        shard_key: String,
    },
    /// One shard used a non-tokenized datastream subject.
    #[error(
        "dataset shard `{shard_key}` in split `{split_name}` must use `tokenized_corpus`, found `{subject}`"
    )]
    ShardSubjectMismatch {
        /// Split name.
        split_name: String,
        /// Shard key.
        shard_key: String,
        /// Observed subject.
        subject: String,
    },
    /// One shard omitted the dataset binding.
    #[error("dataset shard `{shard_key}` in split `{split_name}` is missing dataset binding")]
    MissingDatastreamDatasetBinding {
        /// Split name.
        split_name: String,
        /// Shard key.
        shard_key: String,
    },
    /// One shard's datastream dataset ID did not match the dataset key.
    #[error(
        "dataset shard `{shard_key}` in split `{split_name}` expected dataset `{expected}` but found `{actual}`"
    )]
    DatasetBindingMismatch {
        /// Split name.
        split_name: String,
        /// Shard key.
        shard_key: String,
        /// Expected dataset storage key.
        expected: String,
        /// Actual dataset ID.
        actual: String,
    },
    /// One shard's datastream split did not match the declared split.
    #[error(
        "dataset shard `{shard_key}` expected split `{expected}` but datastream binding used `{actual}`"
    )]
    SplitBindingMismatch {
        /// Shard key.
        shard_key: String,
        /// Expected split name.
        expected: String,
        /// Actual split name.
        actual: String,
    },
    /// One shard's datastream shard key did not match the declared shard key.
    #[error(
        "dataset split `{split_name}` expected shard key `{expected}` but datastream binding used `{actual}`"
    )]
    ShardBindingMismatch {
        /// Split name.
        split_name: String,
        /// Expected shard key.
        expected: String,
        /// Actual shard key.
        actual: String,
    },
    /// Invalid sequence-count declaration.
    #[error(
        "dataset shard `{shard_key}` in split `{split_name}` must declare `sequence_count > 0`"
    )]
    InvalidSequenceCount {
        /// Split name.
        split_name: String,
        /// Shard key.
        shard_key: String,
    },
    /// Invalid token-count declaration.
    #[error(
        "dataset shard `{shard_key}` in split `{split_name}` must declare `token_count >= sequence_count`"
    )]
    InvalidTokenCount {
        /// Split name.
        split_name: String,
        /// Shard key.
        shard_key: String,
    },
    /// Invalid sequence-length bounds.
    #[error(
        "dataset shard `{shard_key}` in split `{split_name}` declared invalid min/max sequence tokens"
    )]
    InvalidSequenceBounds {
        /// Split name.
        split_name: String,
        /// Shard key.
        shard_key: String,
    },
    /// Derived split sequence count did not match the declaration.
    #[error(
        "dataset split `{split_name}` declared sequence_count={declared} but shards derive {derived}"
    )]
    SplitSequenceCountMismatch {
        /// Split name.
        split_name: String,
        /// Declared count.
        declared: u64,
        /// Derived count.
        derived: u64,
    },
    /// Derived split token count did not match the declaration.
    #[error(
        "dataset split `{split_name}` declared token_count={declared} but shards derive {derived}"
    )]
    SplitTokenCountMismatch {
        /// Split name.
        split_name: String,
        /// Declared count.
        declared: u64,
        /// Derived count.
        derived: u64,
    },
    /// One iteration contract targeted the wrong dataset.
    #[error("dataset iteration expected dataset `{expected}` but manifest is `{actual}`")]
    DatasetMismatch {
        /// Expected storage key.
        expected: String,
        /// Actual storage key.
        actual: String,
    },
    /// Iteration requested an unknown split.
    #[error("dataset split `{split_name}` is not declared")]
    UnknownSplit {
        /// Requested split name.
        split_name: String,
    },
    /// The cursor belongs to a different split.
    #[error("iteration cursor expected split `{expected}` but found `{actual}`")]
    IterationCursorSplitMismatch {
        /// Expected split.
        expected: String,
        /// Actual split.
        actual: String,
    },
    /// The caller requested a zero-sized iteration window.
    #[error("dataset iteration `max_sequences` must be greater than zero")]
    InvalidIterationWindowSize,
    /// Invalid packing row budget.
    #[error("dataset packing `max_row_tokens` must be greater than zero")]
    InvalidMaxRowTokens,
    /// Invalid packing batch budget.
    #[error("dataset packing `max_batch_tokens` must be greater than zero")]
    InvalidMaxBatchTokens,
    /// Invalid row count budget.
    #[error("dataset packing `max_rows_per_batch` must be greater than zero")]
    InvalidMaxRowsPerBatch,
    /// Invalid padding multiple.
    #[error("dataset packing `pad_to_multiple_of` must be greater than zero when provided")]
    InvalidPaddingMultiple,
    /// One source sequence was too long for the configured packing policy.
    #[error(
        "sequence `{sequence_id}` uses {token_count} tokens but packing policy only allows {max_row_tokens}"
    )]
    SequenceTooLong {
        /// Sequence identifier.
        sequence_id: String,
        /// Observed token count.
        token_count: u32,
        /// Maximum row tokens.
        max_row_tokens: u32,
    },
}

fn validate_shard_contract(
    dataset: &DatasetKey,
    split_name: &str,
    shard_key: &str,
    manifest: &DatastreamManifestRef,
    sequence_count: u64,
    token_count: u64,
    min_sequence_tokens: u32,
    max_sequence_tokens: u32,
) -> Result<(), DatasetContractError> {
    if manifest.subject != DatastreamSubjectKind::TokenizedCorpus {
        return Err(DatasetContractError::ShardSubjectMismatch {
            split_name: String::from(split_name),
            shard_key: String::from(shard_key),
            subject: String::from(manifest.subject.as_str()),
        });
    }
    let Some(binding) = &manifest.dataset_binding else {
        return Err(DatasetContractError::MissingDatastreamDatasetBinding {
            split_name: String::from(split_name),
            shard_key: String::from(shard_key),
        });
    };
    if binding.dataset_id != dataset.storage_key() {
        return Err(DatasetContractError::DatasetBindingMismatch {
            split_name: String::from(split_name),
            shard_key: String::from(shard_key),
            expected: dataset.storage_key(),
            actual: binding.dataset_id.clone(),
        });
    }
    if binding.split.as_deref() != Some(split_name) {
        return Err(DatasetContractError::SplitBindingMismatch {
            shard_key: String::from(shard_key),
            expected: String::from(split_name),
            actual: binding.split.clone().unwrap_or_default(),
        });
    }
    if binding.shard_key.as_deref() != Some(shard_key) {
        return Err(DatasetContractError::ShardBindingMismatch {
            split_name: String::from(split_name),
            expected: String::from(shard_key),
            actual: binding.shard_key.clone().unwrap_or_default(),
        });
    }
    if sequence_count == 0 {
        return Err(DatasetContractError::InvalidSequenceCount {
            split_name: String::from(split_name),
            shard_key: String::from(shard_key),
        });
    }
    if token_count < sequence_count {
        return Err(DatasetContractError::InvalidTokenCount {
            split_name: String::from(split_name),
            shard_key: String::from(shard_key),
        });
    }
    if min_sequence_tokens == 0
        || max_sequence_tokens == 0
        || min_sequence_tokens > max_sequence_tokens
    {
        return Err(DatasetContractError::InvalidSequenceBounds {
            split_name: String::from(split_name),
            shard_key: String::from(shard_key),
        });
    }
    Ok(())
}

fn ordered_shards<'a>(
    split: &'a DatasetSplitDeclaration,
    ordering: DatasetShardOrdering,
    shuffle_seed: u64,
    epoch: u32,
) -> Vec<&'a DatasetShardManifest> {
    let mut shards = split.shards.iter().collect::<Vec<_>>();
    if ordering == DatasetShardOrdering::DeterministicShuffle {
        shards.sort_by_key(|shard| stable_order_key(shuffle_seed, epoch, shard.shard_key.as_str()));
    }
    shards
}

fn stable_order_key(shuffle_seed: u64, epoch: u32, shard_key: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_dataset_shard_order|");
    hasher.update(shuffle_seed.to_string().as_bytes());
    hasher.update(b"|");
    hasher.update(epoch.to_string().as_bytes());
    hasher.update(b"|");
    hasher.update(shard_key.as_bytes());
    hex::encode(hasher.finalize())
}

fn stable_iteration_window_id(
    contract_digest: &str,
    start_epoch: u32,
    start_shard_index: usize,
    start_sequence_index_in_shard: u64,
    emitted_sequences: u64,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_dataset_iteration_window|");
    hasher.update(contract_digest.as_bytes());
    hasher.update(b"|");
    hasher.update(start_epoch.to_string().as_bytes());
    hasher.update(b"|");
    hasher.update(start_shard_index.to_string().as_bytes());
    hasher.update(b"|");
    hasher.update(start_sequence_index_in_shard.to_string().as_bytes());
    hasher.update(b"|");
    hasher.update(emitted_sequences.to_string().as_bytes());
    hex::encode(hasher.finalize())
}

fn stable_row_id(sequence_id: &str, sequence_index: u64) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_dataset_row|");
    hasher.update(sequence_id.as_bytes());
    hasher.update(b"|");
    hasher.update(sequence_index.to_string().as_bytes());
    hex::encode(hasher.finalize())
}

fn stable_packed_row_id(sequences: &[DatasetSequenceDescriptor]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_dataset_packed_row|");
    for sequence in sequences {
        hasher.update(sequence.sequence_id.as_bytes());
        hasher.update(b"|");
        hasher.update(sequence.sequence_index.to_string().as_bytes());
        hasher.update(b"|");
        hasher.update(sequence.token_count.to_string().as_bytes());
    }
    hex::encode(hasher.finalize())
}

fn padded_token_count(token_count: u32, pad_to_multiple_of: Option<u32>) -> u32 {
    let Some(pad_to_multiple_of) = pad_to_multiple_of else {
        return token_count;
    };
    if token_count == 0 {
        return 0;
    }
    token_count.div_ceil(pad_to_multiple_of) * pad_to_multiple_of
}

fn plan_batches(
    rows: &[PackedSequenceRow],
    policy: &DatasetPackingPolicy,
) -> Result<Vec<PackedBatch>, DatasetContractError> {
    let mut batches = Vec::new();
    let mut current_rows = Vec::new();
    let mut current_tokens = 0_u64;
    let mut current_padded_tokens = 0_u64;

    for row in rows {
        let row_padded_tokens = u64::from(row.padded_token_count);
        if row_padded_tokens > u64::from(policy.max_batch_tokens) {
            return Err(DatasetContractError::SequenceTooLong {
                sequence_id: row.source_sequences[0].sequence_id.clone(),
                token_count: row.padded_token_count,
                max_row_tokens: policy.max_batch_tokens,
            });
        }
        let would_exceed_token_budget = current_padded_tokens.saturating_add(row_padded_tokens)
            > u64::from(policy.max_batch_tokens);
        let would_exceed_row_budget = current_rows.len() >= policy.max_rows_per_batch;
        if !current_rows.is_empty() && (would_exceed_token_budget || would_exceed_row_budget) {
            batches.push(build_batch(
                current_rows,
                current_tokens,
                current_padded_tokens,
                policy,
            ));
            current_rows = Vec::new();
            current_tokens = 0;
            current_padded_tokens = 0;
        }
        current_tokens = current_tokens.saturating_add(u64::from(row.token_count));
        current_padded_tokens = current_padded_tokens.saturating_add(row_padded_tokens);
        current_rows.push(row.clone());
    }

    if !current_rows.is_empty() {
        batches.push(build_batch(
            current_rows,
            current_tokens,
            current_padded_tokens,
            policy,
        ));
    }
    Ok(batches)
}

fn build_batch(
    rows: Vec<PackedSequenceRow>,
    token_count: u64,
    padded_token_count: u64,
    policy: &DatasetPackingPolicy,
) -> PackedBatch {
    let fill_bps = ((padded_token_count.saturating_mul(10_000))
        / u64::from(policy.max_batch_tokens))
    .min(10_000) as u16;
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_dataset_batch|");
    hasher.update(policy.stable_digest().as_bytes());
    for row in &rows {
        hasher.update(b"|");
        hasher.update(row.row_id.as_bytes());
    }
    PackedBatch {
        batch_id: hex::encode(hasher.finalize()),
        rows,
        token_count,
        padded_token_count,
        fill_bps,
    }
}

fn tokenizer_family_label(family: TokenizerFamily) -> &'static [u8] {
    match family {
        TokenizerFamily::SentencePiece => b"sentence_piece",
        TokenizerFamily::BytePairEncoding => b"byte_pair_encoding",
        TokenizerFamily::WordPiece => b"word_piece",
        TokenizerFamily::Unigram => b"unigram",
        TokenizerFamily::Custom => b"custom",
    }
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

fn dataset_split_kind_label(kind: DatasetSplitKind) -> &'static [u8] {
    match kind {
        DatasetSplitKind::Train => b"train",
        DatasetSplitKind::Validation => b"validation",
        DatasetSplitKind::Test => b"test",
        DatasetSplitKind::HeldOut => b"held_out",
        DatasetSplitKind::Preference => b"preference",
        DatasetSplitKind::Replay => b"replay",
        DatasetSplitKind::Custom => b"custom",
    }
}

fn dataset_iteration_mode_label(mode: DatasetIterationMode) -> &'static [u8] {
    match mode {
        DatasetIterationMode::SinglePass => b"single_pass",
        DatasetIterationMode::Repeat => b"repeat",
    }
}

fn dataset_shard_ordering_label(ordering: DatasetShardOrdering) -> &'static [u8] {
    match ordering {
        DatasetShardOrdering::ManifestOrder => b"manifest_order",
        DatasetShardOrdering::DeterministicShuffle => b"deterministic_shuffle",
    }
}

fn dataset_packing_mode_label(mode: DatasetPackingMode) -> &'static [u8] {
    match mode {
        DatasetPackingMode::PackIntoContextWindow => b"pack_into_context_window",
        DatasetPackingMode::BatchByTokenBudget => b"batch_by_token_budget",
    }
}

fn overlong_sequence_posture_label(posture: OverlongSequencePosture) -> &'static str {
    match posture {
        OverlongSequencePosture::Refuse => "refuse",
        OverlongSequencePosture::Drop => "drop",
    }
}

fn distributed_sampler_partition_kind_label(
    partition_kind: DistributedSamplerPartitionKind,
) -> &'static str {
    match partition_kind {
        DistributedSamplerPartitionKind::ContiguousShardBlocks => "contiguous_shard_blocks",
        DistributedSamplerPartitionKind::StridedShards => "strided_shards",
        DistributedSamplerPartitionKind::ElasticRebalance => "elastic_rebalance",
    }
}

fn distributed_worker_coordination_mode_label(
    coordination_mode: DistributedWorkerCoordinationMode,
) -> &'static str {
    match coordination_mode {
        DistributedWorkerCoordinationMode::EpochBarrier => "epoch_barrier",
        DistributedWorkerCoordinationMode::StepBarrier => "step_barrier",
        DistributedWorkerCoordinationMode::ElasticMembership => "elastic_membership",
    }
}

fn digest_lines(lines: Vec<String>) -> String {
    let mut sorted = lines;
    sorted.sort();
    let mut hasher = Sha256::new();
    for line in sorted {
        hasher.update(line.as_bytes());
        hasher.update(b"\n");
    }
    hex::encode(hasher.finalize())
}

/// Dataset access family surfaced by the reusable ingress layer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DatasetSourceKind {
    MapStyle,
    IterableStreaming,
}

/// Source contract for one dataset ingress path.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DatasetSourceContract {
    pub source_kind: DatasetSourceKind,
    pub iteration: DatasetIterationContract,
}

impl DatasetSourceContract {
    #[must_use]
    pub const fn new(source_kind: DatasetSourceKind, iteration: DatasetIterationContract) -> Self {
        Self {
            source_kind,
            iteration,
        }
    }

    pub fn validate(&self, manifest: &DatasetManifest) -> Result<(), DataIngressContractError> {
        manifest.validate()?;
        if manifest.split(self.iteration.split_name.as_str()).is_none() {
            return Err(DataIngressContractError::UnknownSplit {
                split_name: self.iteration.split_name.clone(),
            });
        }
        if manifest.key != self.iteration.dataset {
            return Err(DataIngressContractError::DatasetMismatch {
                expected: self.iteration.dataset.storage_key(),
                actual: manifest.storage_key(),
            });
        }
        Ok(())
    }
}

/// Sampler family surfaced by the reusable ingress layer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DatasetSamplerKind {
    Sequential,
    DeterministicShuffle,
    WeightedRoundRobin,
}

/// Sampler contract above the iteration substrate.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DatasetSamplerContract {
    pub sampler_kind: DatasetSamplerKind,
    pub seed: u64,
    pub drop_last: bool,
}

impl DatasetSamplerContract {
    #[must_use]
    pub const fn new(sampler_kind: DatasetSamplerKind) -> Self {
        Self {
            sampler_kind,
            seed: 0,
            drop_last: false,
        }
    }

    #[must_use]
    pub const fn with_seed(mut self, seed: u64) -> Self {
        self.seed = seed;
        self
    }

    pub fn validate(&self, source: &DatasetSourceContract) -> Result<(), DataIngressContractError> {
        match self.sampler_kind {
            DatasetSamplerKind::Sequential => Ok(()),
            DatasetSamplerKind::DeterministicShuffle => {
                if source.iteration.shard_ordering != DatasetShardOrdering::DeterministicShuffle {
                    return Err(DataIngressContractError::SamplerOrderingMismatch {
                        sampler_kind: String::from("deterministic_shuffle"),
                        ordering: String::from(
                            std::str::from_utf8(dataset_shard_ordering_label(
                                source.iteration.shard_ordering,
                            ))
                            .unwrap_or("unknown"),
                        ),
                    });
                }
                Ok(())
            }
            DatasetSamplerKind::WeightedRoundRobin => {
                Err(DataIngressContractError::UnsupportedSamplerKind {
                    sampler_kind: String::from("weighted_round_robin"),
                })
            }
        }
    }
}

/// Batch-sampler contract above sampler and packing policy.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DatasetBatchSamplerContract {
    pub sampler: DatasetSamplerContract,
    pub max_sequences_per_batch: usize,
    pub max_tokens_per_batch: u32,
    pub packing_policy: DatasetPackingPolicy,
}

impl DatasetBatchSamplerContract {
    #[must_use]
    pub fn new(
        sampler: DatasetSamplerContract,
        max_sequences_per_batch: usize,
        max_tokens_per_batch: u32,
        packing_policy: DatasetPackingPolicy,
    ) -> Self {
        Self {
            sampler,
            max_sequences_per_batch,
            max_tokens_per_batch,
            packing_policy,
        }
    }

    pub fn validate(&self, source: &DatasetSourceContract) -> Result<(), DataIngressContractError> {
        self.sampler.validate(source)?;
        if self.max_sequences_per_batch == 0 {
            return Err(DataIngressContractError::InvalidMaxSequencesPerBatch);
        }
        if self.max_tokens_per_batch == 0 {
            return Err(DataIngressContractError::InvalidMaxTokensPerBatch);
        }
        Ok(())
    }
}

/// Host-to-device staging posture surfaced by the reusable ingress layer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HostDeviceStagingMode {
    DirectHost,
    PinnedPrefetch,
    AsyncPrefetch,
}

/// Host-device staging contract for one ingress path.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostDeviceStagingContract {
    pub staging_mode: HostDeviceStagingMode,
    pub target_device_label: String,
    pub prefetch_batch_count: u32,
    pub pin_host_memory: bool,
}

impl HostDeviceStagingContract {
    #[must_use]
    pub fn new(
        staging_mode: HostDeviceStagingMode,
        target_device_label: impl Into<String>,
    ) -> Self {
        Self {
            staging_mode,
            target_device_label: target_device_label.into(),
            prefetch_batch_count: 0,
            pin_host_memory: false,
        }
    }

    #[must_use]
    pub const fn with_prefetch_batch_count(mut self, prefetch_batch_count: u32) -> Self {
        self.prefetch_batch_count = prefetch_batch_count;
        self
    }

    #[must_use]
    pub const fn with_pin_host_memory(mut self, pin_host_memory: bool) -> Self {
        self.pin_host_memory = pin_host_memory;
        self
    }

    pub fn validate(&self) -> Result<(), DataIngressContractError> {
        if self.target_device_label.trim().is_empty() {
            return Err(DataIngressContractError::MissingTargetDeviceLabel);
        }
        match self.staging_mode {
            HostDeviceStagingMode::DirectHost => {
                if self.prefetch_batch_count != 0 || self.pin_host_memory {
                    return Err(DataIngressContractError::InvalidDirectHostStaging);
                }
            }
            HostDeviceStagingMode::PinnedPrefetch | HostDeviceStagingMode::AsyncPrefetch => {
                if self.prefetch_batch_count == 0 {
                    return Err(DataIngressContractError::InvalidPrefetchBatchCount);
                }
                if !self.pin_host_memory {
                    return Err(DataIngressContractError::PinnedHostMemoryRequired);
                }
            }
        }
        Ok(())
    }
}

/// Aggregate reusable ingress contract over source, sampler, batching, and staging.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DataIngressContract {
    pub source: DatasetSourceContract,
    pub batch_sampler: DatasetBatchSamplerContract,
    pub staging: HostDeviceStagingContract,
}

impl DataIngressContract {
    #[must_use]
    pub fn new(
        source: DatasetSourceContract,
        batch_sampler: DatasetBatchSamplerContract,
        staging: HostDeviceStagingContract,
    ) -> Self {
        Self {
            source,
            batch_sampler,
            staging,
        }
    }

    pub fn validate(&self, manifest: &DatasetManifest) -> Result<(), DataIngressContractError> {
        self.source.validate(manifest)?;
        self.batch_sampler.validate(&self.source)?;
        self.staging.validate()
    }

    fn stable_digest(&self) -> String {
        digest_lines(vec![
            format!("source_kind={:?}", self.source.source_kind),
            format!("iteration={}", self.source.iteration.stable_digest()),
            format!("sampler_kind={:?}", self.batch_sampler.sampler.sampler_kind),
            format!("sampler_seed={}", self.batch_sampler.sampler.seed),
            format!("drop_last={}", self.batch_sampler.sampler.drop_last),
            format!(
                "max_sequences_per_batch={}",
                self.batch_sampler.max_sequences_per_batch
            ),
            format!(
                "max_tokens_per_batch={}",
                self.batch_sampler.max_tokens_per_batch
            ),
            format!(
                "packing_policy={}",
                self.batch_sampler.packing_policy.stable_digest()
            ),
            format!("staging_mode={:?}", self.staging.staging_mode),
            format!("target_device={}", self.staging.target_device_label),
            format!("prefetch_batch_count={}", self.staging.prefetch_batch_count),
            format!("pin_host_memory={}", self.staging.pin_host_memory),
        ])
    }
}

/// Error returned while validating reusable ingress contracts.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum DataIngressContractError {
    #[error(transparent)]
    Dataset(#[from] DatasetContractError),
    #[error("dataset split `{split_name}` is not declared by the manifest")]
    UnknownSplit { split_name: String },
    #[error("data ingress expected dataset `{expected}` but found `{actual}`")]
    DatasetMismatch { expected: String, actual: String },
    #[error("sampler kind `{sampler_kind}` is not supported in the bounded current scope")]
    UnsupportedSamplerKind { sampler_kind: String },
    #[error("sampler `{sampler_kind}` requires matching source ordering, found `{ordering}`")]
    SamplerOrderingMismatch {
        sampler_kind: String,
        ordering: String,
    },
    #[error("batch sampler must declare `max_sequences_per_batch > 0`")]
    InvalidMaxSequencesPerBatch,
    #[error("batch sampler must declare `max_tokens_per_batch > 0`")]
    InvalidMaxTokensPerBatch,
    #[error("host-device staging requires a non-empty target device label")]
    MissingTargetDeviceLabel,
    #[error("direct-host staging must not request prefetch or pinned-host memory")]
    InvalidDirectHostStaging,
    #[error("prefetch staging requires `prefetch_batch_count > 0`")]
    InvalidPrefetchBatchCount,
    #[error("prefetch staging requires pinned host memory")]
    PinnedHostMemoryRequired,
}

/// Status for one bounded data-ingress capability case.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DataIngressCapabilityStatus {
    Supported,
    Refused,
}

/// One machine-readable bounded data-ingress capability case.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DataIngressCapabilityCaseResult {
    pub case_id: String,
    pub source_kind: DatasetSourceKind,
    pub sampler_kind: DatasetSamplerKind,
    pub staging_mode: HostDeviceStagingMode,
    pub status: DataIngressCapabilityStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contract_digest: Option<String>,
    pub bounded_scope: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refusal: Option<String>,
}

/// Machine-readable bounded report for reusable data-ingress semantics.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DataIngressSemanticsReport {
    pub schema_version: u32,
    pub current_scope_window: String,
    pub cases: Vec<DataIngressCapabilityCaseResult>,
    pub report_digest: String,
}

impl DataIngressSemanticsReport {
    fn new(
        current_scope_window: impl Into<String>,
        cases: Vec<DataIngressCapabilityCaseResult>,
    ) -> Self {
        let current_scope_window = current_scope_window.into();
        let report_digest = digest_lines(
            std::iter::once(format!("current_scope_window={current_scope_window}"))
                .chain(cases.iter().flat_map(|case| {
                    let mut lines = vec![
                        format!("case_id={}", case.case_id),
                        format!("source_kind={:?}", case.source_kind),
                        format!("sampler_kind={:?}", case.sampler_kind),
                        format!("staging_mode={:?}", case.staging_mode),
                        format!("status={:?}", case.status),
                        format!("bounded_scope={}", case.bounded_scope),
                    ];
                    if let Some(contract_digest) = &case.contract_digest {
                        lines.push(format!("contract_digest={contract_digest}"));
                    }
                    if let Some(refusal) = &case.refusal {
                        lines.push(format!("refusal={refusal}"));
                    }
                    lines
                }))
                .collect(),
        );
        Self {
            schema_version: 1,
            current_scope_window,
            cases,
            report_digest,
        }
    }

    #[must_use]
    pub fn stable_signature_lines(&self) -> Vec<String> {
        let mut lines = vec![
            format!("schema_version={}", self.schema_version),
            format!("current_scope_window={}", self.current_scope_window),
            format!("report_digest={}", self.report_digest),
        ];
        for case in &self.cases {
            lines.push(format!(
                "{}|{:?}|{:?}|{:?}|{:?}",
                case.case_id, case.source_kind, case.sampler_kind, case.staging_mode, case.status
            ));
        }
        lines
    }
}

/// Supported partitioning families for bounded distributed data-feed planning.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DistributedSamplerPartitionKind {
    /// Assign one contiguous shard block to each worker.
    ContiguousShardBlocks,
    /// Assign every `world_size`-th shard to each worker in global order.
    StridedShards,
    /// Placeholder for future elastic or rebalance-aware partitioning.
    ElasticRebalance,
}

/// Partitioning contract layered on top of local data ingress.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DistributedSamplerPartitionContract {
    /// Partitioning strategy used across workers.
    pub partition_kind: DistributedSamplerPartitionKind,
}

impl DistributedSamplerPartitionContract {
    /// Creates one partitioning contract.
    #[must_use]
    pub const fn new(partition_kind: DistributedSamplerPartitionKind) -> Self {
        Self { partition_kind }
    }

    fn validate(&self) -> Result<(), DistributedDataFeedContractError> {
        match self.partition_kind {
            DistributedSamplerPartitionKind::ContiguousShardBlocks
            | DistributedSamplerPartitionKind::StridedShards => Ok(()),
            DistributedSamplerPartitionKind::ElasticRebalance => {
                Err(DistributedDataFeedContractError::UnsupportedPartitionKind {
                    partition_kind: String::from(distributed_sampler_partition_kind_label(
                        self.partition_kind,
                    )),
                })
            }
        }
    }

    fn stable_digest(&self) -> String {
        digest_lines(vec![format!(
            "partition_kind={}",
            distributed_sampler_partition_kind_label(self.partition_kind)
        )])
    }
}

/// Supported worker-coordination families for the bounded distributed data-feed surface.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DistributedWorkerCoordinationMode {
    /// Workers must finish one epoch partition before advancing together.
    EpochBarrier,
    /// Workers must report progress on a fixed batch cadence.
    StepBarrier,
    /// Placeholder for later elastic membership or topology revision.
    ElasticMembership,
}

/// Fixed worker-topology and barrier contract for distributed data feed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DistributedWorkerCoordinationContract {
    /// Stable replica-group identity.
    pub replica_group: String,
    /// Current worker rank.
    pub rank: usize,
    /// Declared worker count for the group.
    pub world_size: usize,
    /// Coordination family.
    pub coordination_mode: DistributedWorkerCoordinationMode,
    /// Sync cadence for step-barrier coordination.
    pub sync_interval_batches: u64,
}

impl DistributedWorkerCoordinationContract {
    /// Creates one worker-coordination contract.
    #[must_use]
    pub fn new(
        replica_group: impl Into<String>,
        rank: usize,
        world_size: usize,
        coordination_mode: DistributedWorkerCoordinationMode,
    ) -> Self {
        Self {
            replica_group: replica_group.into(),
            rank,
            world_size,
            coordination_mode,
            sync_interval_batches: 1,
        }
    }

    /// Attaches an explicit step-barrier cadence.
    #[must_use]
    pub const fn with_sync_interval_batches(mut self, sync_interval_batches: u64) -> Self {
        self.sync_interval_batches = sync_interval_batches;
        self
    }

    fn validate(&self) -> Result<(), DistributedDataFeedContractError> {
        if self.replica_group.trim().is_empty() {
            return Err(DistributedDataFeedContractError::MissingReplicaGroup);
        }
        if self.world_size == 0 || self.rank >= self.world_size {
            return Err(DistributedDataFeedContractError::InvalidWorkerTopology {
                rank: self.rank,
                world_size: self.world_size,
            });
        }
        match self.coordination_mode {
            DistributedWorkerCoordinationMode::EpochBarrier => Ok(()),
            DistributedWorkerCoordinationMode::StepBarrier => {
                if self.sync_interval_batches == 0 {
                    return Err(DistributedDataFeedContractError::InvalidSyncIntervalBatches);
                }
                Ok(())
            }
            DistributedWorkerCoordinationMode::ElasticMembership => Err(
                DistributedDataFeedContractError::UnsupportedCoordinationMode {
                    coordination_mode: String::from(distributed_worker_coordination_mode_label(
                        self.coordination_mode,
                    )),
                },
            ),
        }
    }

    fn stable_digest(&self) -> String {
        digest_lines(vec![
            format!("replica_group={}", self.replica_group),
            format!("rank={}", self.rank),
            format!("world_size={}", self.world_size),
            format!(
                "coordination_mode={}",
                distributed_worker_coordination_mode_label(self.coordination_mode)
            ),
            format!("sync_interval_batches={}", self.sync_interval_batches),
        ])
    }
}

/// Replayable ordering contract for one distributed input plan.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DistributedReplayOrderingContract {
    /// Runtime-owned replay seed discipline used to derive per-rank ordering.
    pub determinism: RuntimeDeterminismContract,
    /// Epoch or logical ordering generation.
    pub epoch: u32,
}

impl DistributedReplayOrderingContract {
    /// Creates one replay-ordering contract.
    #[must_use]
    pub const fn new(determinism: RuntimeDeterminismContract, epoch: u32) -> Self {
        Self { determinism, epoch }
    }

    fn validate(&self) -> Result<(), DistributedDataFeedContractError> {
        self.determinism.validate()?;
        if self.determinism.generator.is_none() {
            return Err(DistributedDataFeedContractError::BestEffortReplayUnsupported);
        }
        Ok(())
    }

    fn rank_generator(
        &self,
        coordination: &DistributedWorkerCoordinationContract,
    ) -> Result<GeneratorState, DistributedDataFeedContractError> {
        self.validate()?;
        Ok(self.determinism.derive_distributed_rank_generator(
            coordination.replica_group.clone(),
            coordination.rank,
            coordination.world_size,
        )?)
    }

    fn stable_digest(&self) -> String {
        digest_lines(vec![
            format!("epoch={}", self.epoch),
            format!(
                "determinism_generator={}",
                self.determinism
                    .generator
                    .as_ref()
                    .map(|generator| generator.seed.to_string())
                    .unwrap_or_else(|| String::from("none"))
            ),
        ])
    }
}

/// One worker-visible shard assignment in a distributed input plan.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DistributedWorkerShardAssignment {
    /// Stable shard identity.
    pub shard_key: String,
    /// Datastream reference for the shard.
    pub manifest: DatastreamManifestRef,
    /// Global shard index in replay-safe order.
    pub global_shard_index: usize,
    /// Worker-local shard index.
    pub local_shard_index: usize,
    /// Number of sequences carried by the shard.
    pub sequence_count: u64,
    /// Number of tokens carried by the shard.
    pub token_count: u64,
}

/// One replay-stable distributed worker input plan.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DistributedDataFeedPlan {
    /// Stable plan identity for this worker view.
    pub plan_id: String,
    /// Stable digest over the distributed data-feed contract.
    pub contract_digest: String,
    /// Stable digest over the global replay-safe shard order.
    pub global_order_digest: String,
    /// Stable digest over worker coordination posture.
    pub coordination_digest: String,
    /// Per-rank generator derived from runtime determinism.
    pub rank_generator: GeneratorState,
    /// Worker-visible shard assignments.
    pub assigned_shards: Vec<DistributedWorkerShardAssignment>,
}

/// Full bounded distributed data-feed contract built on local data ingress.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DistributedDataFeedContract {
    /// Local source, sampler, batching, and staging posture.
    pub ingress: DataIngressContract,
    /// Partitioning strategy across workers.
    pub partitioning: DistributedSamplerPartitionContract,
    /// Worker topology and barrier behavior.
    pub coordination: DistributedWorkerCoordinationContract,
    /// Replay-safe ordering discipline.
    pub replay: DistributedReplayOrderingContract,
}

impl DistributedDataFeedContract {
    /// Creates one distributed data-feed contract.
    #[must_use]
    pub fn new(
        ingress: DataIngressContract,
        partitioning: DistributedSamplerPartitionContract,
        coordination: DistributedWorkerCoordinationContract,
        replay: DistributedReplayOrderingContract,
    ) -> Self {
        Self {
            ingress,
            partitioning,
            coordination,
            replay,
        }
    }

    /// Validates the bounded distributed data-feed contract.
    pub fn validate(
        &self,
        manifest: &DatasetManifest,
    ) -> Result<(), DistributedDataFeedContractError> {
        self.ingress.validate(manifest)?;
        self.partitioning.validate()?;
        self.coordination.validate()?;
        self.replay.validate()?;
        Ok(())
    }

    fn stable_digest(&self) -> String {
        digest_lines(vec![
            format!("ingress={}", self.ingress.stable_digest()),
            format!("partitioning={}", self.partitioning.stable_digest()),
            format!("coordination={}", self.coordination.stable_digest()),
            format!("replay={}", self.replay.stable_digest()),
        ])
    }

    /// Plans one replay-stable worker view over the globally ordered shard list.
    pub fn plan_worker_input_order(
        &self,
        manifest: &DatasetManifest,
    ) -> Result<DistributedDataFeedPlan, DistributedDataFeedContractError> {
        self.validate(manifest)?;
        let split_name = self.ingress.source.iteration.split_name.as_str();
        let Some(split) = manifest.split(split_name) else {
            return Err(DistributedDataFeedContractError::UnknownSplit {
                split_name: String::from(split_name),
            });
        };
        let ordered = ordered_shards(
            split,
            self.ingress.source.iteration.shard_ordering,
            self.ingress.source.iteration.shuffle_seed,
            self.replay.epoch,
        );
        let global_order_digest = stable_distributed_global_order_digest(
            split_name,
            self.replay.epoch,
            ordered.as_slice(),
        );
        let rank_generator = self.replay.rank_generator(&self.coordination)?;
        let assigned_shards = partition_ordered_shards(
            ordered.as_slice(),
            self.partitioning.partition_kind,
            self.coordination.rank,
            self.coordination.world_size,
        )
        .into_iter()
        .enumerate()
        .map(
            |(local_shard_index, (global_shard_index, shard))| DistributedWorkerShardAssignment {
                shard_key: shard.shard_key.clone(),
                manifest: shard.manifest.clone(),
                global_shard_index,
                local_shard_index,
                sequence_count: shard.sequence_count,
                token_count: shard.token_count,
            },
        )
        .collect::<Vec<_>>();
        let contract_digest = self.stable_digest();
        let coordination_digest = stable_distributed_coordination_digest(
            &self.coordination,
            self.replay.epoch,
            rank_generator.seed,
        );
        let plan_id = stable_distributed_plan_id(
            contract_digest.as_str(),
            global_order_digest.as_str(),
            coordination_digest.as_str(),
            assigned_shards.as_slice(),
        );
        Ok(DistributedDataFeedPlan {
            plan_id,
            contract_digest,
            global_order_digest,
            coordination_digest,
            rank_generator,
            assigned_shards,
        })
    }
}

/// Validation or planning error for bounded distributed data feed.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum DistributedDataFeedContractError {
    #[error(transparent)]
    Ingress(#[from] DataIngressContractError),
    #[error(transparent)]
    Determinism(#[from] DeterminismContractError),
    #[error("distributed data feed expected split `{split_name}` to exist in the manifest")]
    UnknownSplit { split_name: String },
    #[error(
        "distributed replay ordering requires a seeded or strict runtime determinism contract"
    )]
    BestEffortReplayUnsupported,
    #[error("worker coordination requires a non-empty replica group")]
    MissingReplicaGroup,
    #[error("worker topology requires rank < world_size and non-zero world_size; found rank={rank} world_size={world_size}")]
    InvalidWorkerTopology { rank: usize, world_size: usize },
    #[error("step-barrier coordination requires `sync_interval_batches > 0`")]
    InvalidSyncIntervalBatches,
    #[error("current scope refuses distributed partition kind `{partition_kind}`")]
    UnsupportedPartitionKind { partition_kind: String },
    #[error("current scope refuses worker coordination mode `{coordination_mode}`")]
    UnsupportedCoordinationMode { coordination_mode: String },
}

/// Status for one bounded distributed data-feed capability case.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DistributedDataFeedCapabilityStatus {
    Supported,
    Refused,
}

/// One machine-readable distributed data-feed capability case.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DistributedDataFeedCapabilityCaseResult {
    pub case_id: String,
    pub partition_kind: DistributedSamplerPartitionKind,
    pub coordination_mode: DistributedWorkerCoordinationMode,
    pub worker_rank: usize,
    pub status: DistributedDataFeedCapabilityStatus,
    pub assigned_shard_keys: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub global_order_digest: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rank_seed: Option<u64>,
    pub bounded_scope: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refusal: Option<String>,
}

/// Machine-readable bounded report for distributed and sharded data-feed semantics.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DistributedDataFeedSemanticsReport {
    pub schema_version: u32,
    pub current_scope_window: String,
    pub cases: Vec<DistributedDataFeedCapabilityCaseResult>,
    pub report_digest: String,
}

impl DistributedDataFeedSemanticsReport {
    fn new(
        current_scope_window: impl Into<String>,
        cases: Vec<DistributedDataFeedCapabilityCaseResult>,
    ) -> Self {
        let current_scope_window = current_scope_window.into();
        let report_digest = digest_lines(
            std::iter::once(format!("current_scope_window={current_scope_window}"))
                .chain(cases.iter().flat_map(|case| {
                    let mut lines = vec![
                        format!("case_id={}", case.case_id),
                        format!(
                            "partition_kind={}",
                            distributed_sampler_partition_kind_label(case.partition_kind)
                        ),
                        format!(
                            "coordination_mode={}",
                            distributed_worker_coordination_mode_label(case.coordination_mode)
                        ),
                        format!("worker_rank={}", case.worker_rank),
                        format!("status={:?}", case.status),
                        format!("bounded_scope={}", case.bounded_scope),
                        format!("assigned_shard_keys={}", case.assigned_shard_keys.join(",")),
                    ];
                    if let Some(plan_id) = &case.plan_id {
                        lines.push(format!("plan_id={plan_id}"));
                    }
                    if let Some(global_order_digest) = &case.global_order_digest {
                        lines.push(format!("global_order_digest={global_order_digest}"));
                    }
                    if let Some(rank_seed) = case.rank_seed {
                        lines.push(format!("rank_seed={rank_seed}"));
                    }
                    if let Some(refusal) = &case.refusal {
                        lines.push(format!("refusal={refusal}"));
                    }
                    lines
                }))
                .collect(),
        );
        Self {
            schema_version: 1,
            current_scope_window,
            cases,
            report_digest,
        }
    }

    #[must_use]
    pub fn stable_signature_lines(&self) -> Vec<String> {
        let mut lines = vec![
            format!("schema_version={}", self.schema_version),
            format!("current_scope_window={}", self.current_scope_window),
            format!("report_digest={}", self.report_digest),
        ];
        for case in &self.cases {
            lines.push(format!(
                "{}|{}|{}|{}|{:?}",
                case.case_id,
                distributed_sampler_partition_kind_label(case.partition_kind),
                distributed_worker_coordination_mode_label(case.coordination_mode),
                case.worker_rank,
                case.status
            ));
        }
        lines
    }
}

/// Builds the canonical bounded distributed data-feed semantics report.
#[must_use]
pub fn builtin_distributed_data_feed_semantics_report() -> DistributedDataFeedSemanticsReport {
    let manifest = seeded_data_ingress_manifest();

    let contiguous_epoch = DistributedDataFeedContract::new(
        DataIngressContract::new(
            DatasetSourceContract::new(
                DatasetSourceKind::MapStyle,
                DatasetIterationContract::new(manifest.key.clone(), "train"),
            ),
            DatasetBatchSamplerContract::new(
                DatasetSamplerContract::new(DatasetSamplerKind::Sequential),
                2,
                24,
                DatasetPackingPolicy::new(DatasetPackingMode::BatchByTokenBudget, 12, 24, 2),
            ),
            HostDeviceStagingContract::new(HostDeviceStagingMode::DirectHost, "cpu"),
        ),
        DistributedSamplerPartitionContract::new(
            DistributedSamplerPartitionKind::ContiguousShardBlocks,
        ),
        DistributedWorkerCoordinationContract::new(
            "data_parallel",
            0,
            2,
            DistributedWorkerCoordinationMode::EpochBarrier,
        ),
        DistributedReplayOrderingContract::new(RuntimeDeterminismContract::strict(41), 0),
    );

    let strided_step = DistributedDataFeedContract::new(
        DataIngressContract::new(
            DatasetSourceContract::new(
                DatasetSourceKind::IterableStreaming,
                DatasetIterationContract::new(manifest.key.clone(), "train")
                    .with_mode(DatasetIterationMode::Repeat)
                    .with_shard_ordering(DatasetShardOrdering::DeterministicShuffle)
                    .with_shuffle_seed(7),
            ),
            DatasetBatchSamplerContract::new(
                DatasetSamplerContract::new(DatasetSamplerKind::DeterministicShuffle).with_seed(7),
                4,
                48,
                DatasetPackingPolicy::new(DatasetPackingMode::PackIntoContextWindow, 24, 48, 2),
            ),
            HostDeviceStagingContract::new(HostDeviceStagingMode::PinnedPrefetch, "cuda:0")
                .with_prefetch_batch_count(2)
                .with_pin_host_memory(true),
        ),
        DistributedSamplerPartitionContract::new(DistributedSamplerPartitionKind::StridedShards),
        DistributedWorkerCoordinationContract::new(
            "data_parallel",
            1,
            2,
            DistributedWorkerCoordinationMode::StepBarrier,
        )
        .with_sync_interval_batches(4),
        DistributedReplayOrderingContract::new(RuntimeDeterminismContract::strict(77), 3),
    );

    let elastic_membership = DistributedDataFeedContract::new(
        DataIngressContract::new(
            DatasetSourceContract::new(
                DatasetSourceKind::IterableStreaming,
                DatasetIterationContract::new(manifest.key.clone(), "train")
                    .with_mode(DatasetIterationMode::Repeat)
                    .with_shard_ordering(DatasetShardOrdering::DeterministicShuffle)
                    .with_shuffle_seed(9),
            ),
            DatasetBatchSamplerContract::new(
                DatasetSamplerContract::new(DatasetSamplerKind::DeterministicShuffle).with_seed(9),
                4,
                48,
                DatasetPackingPolicy::new(DatasetPackingMode::BatchByTokenBudget, 24, 48, 2),
            ),
            HostDeviceStagingContract::new(HostDeviceStagingMode::PinnedPrefetch, "cuda:0")
                .with_prefetch_batch_count(2)
                .with_pin_host_memory(true),
        ),
        DistributedSamplerPartitionContract::new(DistributedSamplerPartitionKind::StridedShards),
        DistributedWorkerCoordinationContract::new(
            "data_parallel",
            0,
            2,
            DistributedWorkerCoordinationMode::ElasticMembership,
        ),
        DistributedReplayOrderingContract::new(RuntimeDeterminismContract::strict(99), 1),
    );

    DistributedDataFeedSemanticsReport::new(
        String::from("psionic_distributed_data_feed_v1"),
        vec![
            supported_distributed_data_feed_case(
                "map_style.contiguous_epoch_barrier.rank0",
                &manifest,
                &contiguous_epoch,
                "Current scope supports fixed-world-size shard partitioning with contiguous worker blocks, epoch-barrier coordination, and runtime-derived replay ordering.",
            ),
            supported_distributed_data_feed_case(
                "iterable_streaming.strided_step_barrier.rank1",
                &manifest,
                &strided_step,
                "Current scope supports deterministic-shuffle iterable ingress partitioned by rank-stride, fixed step-barrier coordination, and replay-stable per-rank shard order.",
            ),
            refused_distributed_data_feed_case(
                "iterable_streaming.elastic_membership",
                &manifest,
                &elastic_membership,
                "Current scope refuses elastic membership because topology revision would change per-rank replay order without a higher-level distributed run-control contract.",
            ),
        ],
    )
}

/// Builds the canonical bounded reusable data-ingress semantics report.
#[must_use]
pub fn builtin_data_ingress_semantics_report() -> DataIngressSemanticsReport {
    let manifest = seeded_data_ingress_manifest();

    let map_style = DataIngressContract::new(
        DatasetSourceContract::new(
            DatasetSourceKind::MapStyle,
            DatasetIterationContract::new(manifest.key.clone(), "train"),
        ),
        DatasetBatchSamplerContract::new(
            DatasetSamplerContract::new(DatasetSamplerKind::Sequential),
            2,
            24,
            DatasetPackingPolicy::new(DatasetPackingMode::BatchByTokenBudget, 12, 24, 2),
        ),
        HostDeviceStagingContract::new(HostDeviceStagingMode::DirectHost, "cpu"),
    );

    let iterable_streaming = DataIngressContract::new(
        DatasetSourceContract::new(
            DatasetSourceKind::IterableStreaming,
            DatasetIterationContract::new(manifest.key.clone(), "train")
                .with_mode(DatasetIterationMode::Repeat)
                .with_shard_ordering(DatasetShardOrdering::DeterministicShuffle)
                .with_shuffle_seed(7),
        ),
        DatasetBatchSamplerContract::new(
            DatasetSamplerContract::new(DatasetSamplerKind::DeterministicShuffle).with_seed(7),
            4,
            48,
            DatasetPackingPolicy::new(DatasetPackingMode::PackIntoContextWindow, 24, 48, 2),
        ),
        HostDeviceStagingContract::new(HostDeviceStagingMode::PinnedPrefetch, "cuda:0")
            .with_prefetch_batch_count(2)
            .with_pin_host_memory(true),
    );

    let unsupported_weighted = DataIngressContract::new(
        DatasetSourceContract::new(
            DatasetSourceKind::IterableStreaming,
            DatasetIterationContract::new(manifest.key.clone(), "train")
                .with_mode(DatasetIterationMode::Repeat)
                .with_shard_ordering(DatasetShardOrdering::DeterministicShuffle)
                .with_shuffle_seed(9),
        ),
        DatasetBatchSamplerContract::new(
            DatasetSamplerContract::new(DatasetSamplerKind::WeightedRoundRobin).with_seed(9),
            4,
            48,
            DatasetPackingPolicy::new(DatasetPackingMode::BatchByTokenBudget, 24, 48, 2),
        ),
        HostDeviceStagingContract::new(HostDeviceStagingMode::PinnedPrefetch, "cuda:0")
            .with_prefetch_batch_count(2)
            .with_pin_host_memory(true),
    );

    DataIngressSemanticsReport::new(
        String::from("psionic_data_ingress_v1"),
        vec![
            supported_data_ingress_case(
                "map_style.sequential.direct_host",
                &manifest,
                &map_style,
                "Current scope supports map-style dataset access, sequential sampling, batch sampling, and direct host consumption without app-local glue.",
            ),
            supported_data_ingress_case(
                "iterable_streaming.deterministic_shuffle.pinned_prefetch",
                &manifest,
                &iterable_streaming,
                "Current scope supports iterable-streaming access with deterministic shuffle, token-budget or context packing, and pinned host prefetch into one target device lane.",
            ),
            refused_data_ingress_case(
                "iterable_streaming.weighted_round_robin",
                &manifest,
                &unsupported_weighted,
                "Current scope refuses weighted or round-robin sampler families until distributed and sharded data-feed semantics land.",
            ),
        ],
    )
}

fn seeded_data_ingress_manifest() -> DatasetManifest {
    let dataset = DatasetKey::new("openagents/math-sft", "v1");
    let train_split = DatasetSplitDeclaration::new(
        &dataset,
        "train",
        DatasetSplitKind::Train,
        vec![
            seeded_data_shard(&dataset, "train", "shard-0", 3, 24),
            seeded_data_shard(&dataset, "train", "shard-1", 2, 18),
        ],
    )
    .expect("seeded data-ingress split should validate");
    DatasetManifest::new(
        dataset,
        "OpenAgents Math SFT",
        DatasetRecordEncoding::TokenIdsLeU32,
        TokenizerDigest::new(TokenizerFamily::SentencePiece, "tokenizer-digest", 32_000),
    )
    .with_context_window_tokens(8192)
    .with_splits(vec![train_split])
}

fn seeded_data_shard(
    dataset: &DatasetKey,
    split: &str,
    shard_key: &str,
    sequence_count: u64,
    token_count: u64,
) -> DatasetShardManifest {
    let payload = vec![1_u8; 32];
    let manifest = DatastreamManifest::from_bytes(
        format!("{split}:{shard_key}"),
        DatastreamSubjectKind::TokenizedCorpus,
        payload.as_slice(),
        16,
        DatastreamEncoding::RawBinary,
    )
    .with_dataset_binding(dataset.datastream_binding(split, shard_key));
    DatasetShardManifest::new(
        dataset,
        split,
        shard_key,
        manifest.manifest_ref(),
        sequence_count,
        token_count,
        4,
        12,
    )
    .expect("seeded data shard should validate")
}

fn supported_data_ingress_case(
    case_id: &str,
    manifest: &DatasetManifest,
    contract: &DataIngressContract,
    bounded_scope: &str,
) -> DataIngressCapabilityCaseResult {
    contract
        .validate(manifest)
        .expect("seeded data-ingress case should validate");
    DataIngressCapabilityCaseResult {
        case_id: String::from(case_id),
        source_kind: contract.source.source_kind,
        sampler_kind: contract.batch_sampler.sampler.sampler_kind,
        staging_mode: contract.staging.staging_mode,
        status: DataIngressCapabilityStatus::Supported,
        contract_digest: Some(contract.stable_digest()),
        bounded_scope: String::from(bounded_scope),
        refusal: None,
    }
}

fn refused_data_ingress_case(
    case_id: &str,
    manifest: &DatasetManifest,
    contract: &DataIngressContract,
    bounded_scope: &str,
) -> DataIngressCapabilityCaseResult {
    let refusal = contract
        .validate(manifest)
        .expect_err("seeded data-ingress case should refuse");
    DataIngressCapabilityCaseResult {
        case_id: String::from(case_id),
        source_kind: contract.source.source_kind,
        sampler_kind: contract.batch_sampler.sampler.sampler_kind,
        staging_mode: contract.staging.staging_mode,
        status: DataIngressCapabilityStatus::Refused,
        contract_digest: None,
        bounded_scope: String::from(bounded_scope),
        refusal: Some(refusal.to_string()),
    }
}

fn partition_ordered_shards<'a>(
    ordered_shards: &[&'a DatasetShardManifest],
    partition_kind: DistributedSamplerPartitionKind,
    rank: usize,
    world_size: usize,
) -> Vec<(usize, &'a DatasetShardManifest)> {
    match partition_kind {
        DistributedSamplerPartitionKind::ContiguousShardBlocks => {
            let base = ordered_shards.len() / world_size;
            let extra = ordered_shards.len() % world_size;
            let start = rank.saturating_mul(base).saturating_add(rank.min(extra));
            let count = base + usize::from(rank < extra);
            ordered_shards
                .iter()
                .enumerate()
                .skip(start)
                .take(count)
                .map(|(index, shard)| (index, *shard))
                .collect()
        }
        DistributedSamplerPartitionKind::StridedShards => ordered_shards
            .iter()
            .enumerate()
            .filter(|(index, _)| index % world_size == rank)
            .map(|(index, shard)| (index, *shard))
            .collect(),
        DistributedSamplerPartitionKind::ElasticRebalance => Vec::new(),
    }
}

fn stable_distributed_global_order_digest(
    split_name: &str,
    epoch: u32,
    ordered_shards: &[&DatasetShardManifest],
) -> String {
    digest_lines(
        std::iter::once(format!("split_name={split_name}"))
            .chain(std::iter::once(format!("epoch={epoch}")))
            .chain(
                ordered_shards
                    .iter()
                    .enumerate()
                    .flat_map(|(index, shard)| {
                        [
                            format!("global_index={index}"),
                            format!("shard_key={}", shard.shard_key),
                            format!("manifest_digest={}", shard.manifest.manifest_digest),
                        ]
                    }),
            )
            .collect(),
    )
}

fn stable_distributed_coordination_digest(
    coordination: &DistributedWorkerCoordinationContract,
    epoch: u32,
    rank_seed: u64,
) -> String {
    digest_lines(vec![
        format!("replica_group={}", coordination.replica_group),
        format!("rank={}", coordination.rank),
        format!("world_size={}", coordination.world_size),
        format!(
            "coordination_mode={}",
            distributed_worker_coordination_mode_label(coordination.coordination_mode)
        ),
        format!(
            "sync_interval_batches={}",
            coordination.sync_interval_batches
        ),
        format!("epoch={epoch}"),
        format!("rank_seed={rank_seed}"),
    ])
}

fn stable_distributed_plan_id(
    contract_digest: &str,
    global_order_digest: &str,
    coordination_digest: &str,
    assigned_shards: &[DistributedWorkerShardAssignment],
) -> String {
    digest_lines(
        std::iter::once(format!("contract_digest={contract_digest}"))
            .chain(std::iter::once(format!(
                "global_order_digest={global_order_digest}"
            )))
            .chain(std::iter::once(format!(
                "coordination_digest={coordination_digest}"
            )))
            .chain(assigned_shards.iter().flat_map(|assignment| {
                [
                    format!("global_shard_index={}", assignment.global_shard_index),
                    format!("local_shard_index={}", assignment.local_shard_index),
                    format!("shard_key={}", assignment.shard_key),
                    format!("manifest_digest={}", assignment.manifest.manifest_digest),
                ]
            }))
            .collect(),
    )
}

fn supported_distributed_data_feed_case(
    case_id: &str,
    manifest: &DatasetManifest,
    contract: &DistributedDataFeedContract,
    bounded_scope: &str,
) -> DistributedDataFeedCapabilityCaseResult {
    let plan = contract
        .plan_worker_input_order(manifest)
        .expect("seeded distributed data-feed case should validate");
    DistributedDataFeedCapabilityCaseResult {
        case_id: String::from(case_id),
        partition_kind: contract.partitioning.partition_kind,
        coordination_mode: contract.coordination.coordination_mode,
        worker_rank: contract.coordination.rank,
        status: DistributedDataFeedCapabilityStatus::Supported,
        assigned_shard_keys: plan
            .assigned_shards
            .iter()
            .map(|assignment| assignment.shard_key.clone())
            .collect(),
        plan_id: Some(plan.plan_id),
        global_order_digest: Some(plan.global_order_digest),
        rank_seed: Some(plan.rank_generator.seed),
        bounded_scope: String::from(bounded_scope),
        refusal: None,
    }
}

fn refused_distributed_data_feed_case(
    case_id: &str,
    manifest: &DatasetManifest,
    contract: &DistributedDataFeedContract,
    bounded_scope: &str,
) -> DistributedDataFeedCapabilityCaseResult {
    let refusal = contract
        .plan_worker_input_order(manifest)
        .expect_err("seeded distributed data-feed case should refuse");
    DistributedDataFeedCapabilityCaseResult {
        case_id: String::from(case_id),
        partition_kind: contract.partitioning.partition_kind,
        coordination_mode: contract.coordination.coordination_mode,
        worker_rank: contract.coordination.rank,
        status: DistributedDataFeedCapabilityStatus::Refused,
        assigned_shard_keys: Vec::new(),
        plan_id: None,
        global_order_digest: None,
        rank_seed: None,
        bounded_scope: String::from(bounded_scope),
        refusal: Some(refusal.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    use psionic_datastream::{DatastreamEncoding, DatastreamManifest, DatastreamSubjectKind};

    fn sample_dataset_key() -> DatasetKey {
        DatasetKey::new("dataset://openagents/math-sft", "2026.03.14")
    }

    fn sample_tokenizer() -> TokenizerDigest {
        TokenizerDigest::new(TokenizerFamily::SentencePiece, "tok-digest-1", 32_768)
            .with_special_tokens_digest("special-digest")
            .with_template_digest("template-digest")
    }

    fn sample_shard(
        dataset: &DatasetKey,
        split_name: &str,
        shard_key: &str,
        sequence_count: u64,
        token_count: u64,
    ) -> DatasetShardManifest {
        let payload = vec![1_u8; 32];
        let manifest = DatastreamManifest::from_bytes(
            format!("{split_name}-{shard_key}"),
            DatastreamSubjectKind::TokenizedCorpus,
            payload.as_slice(),
            8,
            DatastreamEncoding::TokenIdsLeU32,
        )
        .with_dataset_binding(dataset.datastream_binding(split_name, shard_key));
        DatasetShardManifest::new(
            dataset,
            split_name,
            shard_key,
            manifest.manifest_ref(),
            sequence_count,
            token_count,
            8,
            64,
        )
        .expect("sample shard should validate")
    }

    #[test]
    fn dataset_manifest_is_stable_and_machine_legible() {
        let dataset = sample_dataset_key();
        let train_split = DatasetSplitDeclaration::new(
            &dataset,
            "train",
            DatasetSplitKind::Train,
            vec![
                sample_shard(&dataset, "train", "shard-0", 4, 48),
                sample_shard(&dataset, "train", "shard-1", 5, 70),
            ],
        )
        .expect("train split should validate");
        let validation_split = DatasetSplitDeclaration::new(
            &dataset,
            "validation",
            DatasetSplitKind::Validation,
            vec![sample_shard(&dataset, "validation", "shard-0", 2, 20)],
        )
        .expect("validation split should validate");
        let manifest = DatasetManifest::new(
            dataset.clone(),
            "OpenAgents Math SFT",
            DatasetRecordEncoding::TokenIdsLeU32,
            sample_tokenizer(),
        )
        .with_context_window_tokens(8192)
        .with_provenance_digest("source-digest")
        .with_splits(vec![train_split, validation_split]);

        manifest.validate().expect("manifest should validate");
        assert_eq!(manifest.storage_key(), dataset.storage_key());
        assert_eq!(
            manifest.split("train").map(|split| split.sequence_count),
            Some(9)
        );
        assert_eq!(manifest.stable_digest(), manifest.stable_digest());
    }

    #[test]
    fn shard_refuses_wrong_subject_or_binding() {
        let dataset = sample_dataset_key();
        let payload = vec![1_u8; 16];
        let manifest = DatastreamManifest::from_bytes(
            "checkpoint",
            DatastreamSubjectKind::Checkpoint,
            payload.as_slice(),
            8,
            DatastreamEncoding::Safetensors,
        )
        .with_dataset_binding(dataset.datastream_binding("train", "shard-0"));
        let err = DatasetShardManifest::new(
            &dataset,
            "train",
            "shard-0",
            manifest.manifest_ref(),
            4,
            32,
            8,
            64,
        )
        .expect_err("wrong subject should be refused");
        assert!(matches!(
            err,
            DatasetContractError::ShardSubjectMismatch { .. }
        ));
    }

    #[test]
    fn iteration_window_tracks_resume_cursor_and_epoch_wrap() {
        let dataset = sample_dataset_key();
        let manifest = DatasetManifest::new(
            dataset.clone(),
            "OpenAgents Math SFT",
            DatasetRecordEncoding::TokenIdsLeU32,
            sample_tokenizer(),
        )
        .with_splits(vec![DatasetSplitDeclaration::new(
            &dataset,
            "train",
            DatasetSplitKind::Train,
            vec![
                sample_shard(&dataset, "train", "shard-0", 3, 24),
                sample_shard(&dataset, "train", "shard-1", 2, 18),
            ],
        )
        .expect("split should validate")]);

        let contract = DatasetIterationContract::new(dataset, "train")
            .with_mode(DatasetIterationMode::Repeat)
            .with_shard_ordering(DatasetShardOrdering::DeterministicShuffle)
            .with_shuffle_seed(7);
        let cursor = DatasetIterationCursor::new("train");
        let first_window = contract
            .plan_window(&manifest, &cursor, 4)
            .expect("planning should succeed")
            .expect("window should exist");
        assert_eq!(first_window.start_cursor.emitted_sequences, 0);
        assert_eq!(first_window.end_cursor.emitted_sequences, 4);
        assert_eq!(
            first_window
                .spans
                .iter()
                .map(|span| span.sequence_count)
                .sum::<u64>(),
            4
        );

        let second_window = contract
            .plan_window(&manifest, &first_window.end_cursor, 4)
            .expect("planning should succeed")
            .expect("window should exist");
        assert!(second_window.end_cursor.epoch >= 1);
        assert_eq!(second_window.end_cursor.emitted_sequences, 8);
    }

    #[test]
    fn packing_plan_supports_context_packing_and_batch_budgeting() {
        let policy =
            DatasetPackingPolicy::new(DatasetPackingMode::PackIntoContextWindow, 12, 24, 2)
                .with_pad_to_multiple_of(4);
        let sequences = vec![
            DatasetSequenceDescriptor::new("s1", "shard-0", 0, 5),
            DatasetSequenceDescriptor::new("s2", "shard-0", 1, 4),
            DatasetSequenceDescriptor::new("s3", "shard-1", 0, 6),
        ];

        let plan = policy
            .plan(sequences.as_slice())
            .expect("packing should succeed");
        assert_eq!(plan.batches.len(), 1);
        assert_eq!(plan.batches[0].rows.len(), 2);
        assert_eq!(plan.batches[0].rows[0].source_sequences.len(), 2);
        assert_eq!(plan.batches[0].padded_token_count, 20);
    }

    #[test]
    fn packing_plan_can_drop_overlong_sequences() {
        let policy = DatasetPackingPolicy::new(DatasetPackingMode::BatchByTokenBudget, 10, 16, 2)
            .with_overlong_sequence_posture(OverlongSequencePosture::Drop);
        let sequences = vec![
            DatasetSequenceDescriptor::new("short", "shard-0", 0, 4),
            DatasetSequenceDescriptor::new("long", "shard-0", 1, 14),
        ];

        let plan = policy
            .plan(sequences.as_slice())
            .expect("packing should succeed");
        assert_eq!(plan.batches.len(), 1);
        assert_eq!(plan.total_source_sequences, 2);
        assert_eq!(plan.dropped_sequences, vec![String::from("long")]);
    }

    #[test]
    fn data_ingress_semantics_report_tracks_seeded_source_sampler_and_staging_cases() {
        let report = builtin_data_ingress_semantics_report();
        assert_eq!(report.schema_version, 1);
        assert_eq!(report.current_scope_window, "psionic_data_ingress_v1");
        assert!(report
            .stable_signature_lines()
            .iter()
            .any(|line| line.starts_with("report_digest=")));

        let direct_host = report
            .cases
            .iter()
            .find(|case| case.case_id == "map_style.sequential.direct_host")
            .expect("missing direct-host case");
        assert_eq!(direct_host.status, DataIngressCapabilityStatus::Supported);
        assert!(direct_host.contract_digest.is_some());

        let pinned_prefetch = report
            .cases
            .iter()
            .find(|case| case.case_id == "iterable_streaming.deterministic_shuffle.pinned_prefetch")
            .expect("missing iterable-streaming prefetch case");
        assert_eq!(
            pinned_prefetch.source_kind,
            DatasetSourceKind::IterableStreaming
        );
        assert_eq!(
            pinned_prefetch.sampler_kind,
            DatasetSamplerKind::DeterministicShuffle
        );
        assert_eq!(
            pinned_prefetch.staging_mode,
            HostDeviceStagingMode::PinnedPrefetch
        );

        let refused = report
            .cases
            .iter()
            .find(|case| case.case_id == "iterable_streaming.weighted_round_robin")
            .expect("missing weighted-round-robin refusal");
        assert_eq!(refused.status, DataIngressCapabilityStatus::Refused);
        assert!(refused.refusal.is_some());
    }

    #[test]
    fn distributed_data_feed_plan_partitions_shards_without_overlap_and_with_stable_replay_order() {
        let manifest = seeded_data_ingress_manifest();
        let base_ingress = DataIngressContract::new(
            DatasetSourceContract::new(
                DatasetSourceKind::IterableStreaming,
                DatasetIterationContract::new(manifest.key.clone(), "train")
                    .with_mode(DatasetIterationMode::Repeat)
                    .with_shard_ordering(DatasetShardOrdering::DeterministicShuffle)
                    .with_shuffle_seed(7),
            ),
            DatasetBatchSamplerContract::new(
                DatasetSamplerContract::new(DatasetSamplerKind::DeterministicShuffle).with_seed(7),
                4,
                48,
                DatasetPackingPolicy::new(DatasetPackingMode::PackIntoContextWindow, 24, 48, 2),
            ),
            HostDeviceStagingContract::new(HostDeviceStagingMode::PinnedPrefetch, "cuda:0")
                .with_prefetch_batch_count(2)
                .with_pin_host_memory(true),
        );
        let rank0 = DistributedDataFeedContract::new(
            base_ingress.clone(),
            DistributedSamplerPartitionContract::new(
                DistributedSamplerPartitionKind::StridedShards,
            ),
            DistributedWorkerCoordinationContract::new(
                "data_parallel",
                0,
                2,
                DistributedWorkerCoordinationMode::StepBarrier,
            )
            .with_sync_interval_batches(4),
            DistributedReplayOrderingContract::new(RuntimeDeterminismContract::strict(77), 3),
        );
        let rank1 = DistributedDataFeedContract::new(
            base_ingress,
            DistributedSamplerPartitionContract::new(
                DistributedSamplerPartitionKind::StridedShards,
            ),
            DistributedWorkerCoordinationContract::new(
                "data_parallel",
                1,
                2,
                DistributedWorkerCoordinationMode::StepBarrier,
            )
            .with_sync_interval_batches(4),
            DistributedReplayOrderingContract::new(RuntimeDeterminismContract::strict(77), 3),
        );

        let plan0 = rank0
            .plan_worker_input_order(&manifest)
            .expect("rank0 plan should validate");
        let plan0_repeat = rank0
            .plan_worker_input_order(&manifest)
            .expect("rank0 replay plan should validate");
        let plan1 = rank1
            .plan_worker_input_order(&manifest)
            .expect("rank1 plan should validate");

        assert_eq!(plan0, plan0_repeat);
        assert_eq!(plan0.global_order_digest, plan1.global_order_digest);
        assert_ne!(plan0.rank_generator.seed, plan1.rank_generator.seed);

        let ordered_keys = ordered_shards(
            manifest.split("train").expect("train split should exist"),
            DatasetShardOrdering::DeterministicShuffle,
            7,
            3,
        )
        .into_iter()
        .map(|shard| shard.shard_key.clone())
        .collect::<Vec<_>>();
        let rank0_keys = plan0
            .assigned_shards
            .iter()
            .map(|assignment| assignment.shard_key.clone())
            .collect::<BTreeSet<_>>();
        let rank1_keys = plan1
            .assigned_shards
            .iter()
            .map(|assignment| assignment.shard_key.clone())
            .collect::<BTreeSet<_>>();
        assert!(rank0_keys.is_disjoint(&rank1_keys));
        let union = rank0_keys.union(&rank1_keys).cloned().collect::<Vec<_>>();
        assert_eq!(union, ordered_keys);
    }

    #[test]
    fn distributed_data_feed_semantics_report_tracks_partitioning_coordination_and_replay_cases() {
        let report = builtin_distributed_data_feed_semantics_report();
        assert_eq!(report.schema_version, 1);
        assert_eq!(
            report.current_scope_window,
            "psionic_distributed_data_feed_v1"
        );
        assert!(report
            .stable_signature_lines()
            .iter()
            .any(|line| line.starts_with("report_digest=")));

        let contiguous = report
            .cases
            .iter()
            .find(|case| case.case_id == "map_style.contiguous_epoch_barrier.rank0")
            .expect("missing contiguous rank0 case");
        assert_eq!(
            contiguous.status,
            DistributedDataFeedCapabilityStatus::Supported
        );
        assert_eq!(
            contiguous.partition_kind,
            DistributedSamplerPartitionKind::ContiguousShardBlocks
        );
        assert_eq!(
            contiguous.coordination_mode,
            DistributedWorkerCoordinationMode::EpochBarrier
        );
        assert!(contiguous.plan_id.is_some());
        assert!(contiguous.global_order_digest.is_some());
        assert!(contiguous.rank_seed.is_some());

        let strided = report
            .cases
            .iter()
            .find(|case| case.case_id == "iterable_streaming.strided_step_barrier.rank1")
            .expect("missing strided rank1 case");
        assert_eq!(strided.worker_rank, 1);
        assert_eq!(
            strided.status,
            DistributedDataFeedCapabilityStatus::Supported
        );
        assert_eq!(
            strided.partition_kind,
            DistributedSamplerPartitionKind::StridedShards
        );
        assert_eq!(
            strided.coordination_mode,
            DistributedWorkerCoordinationMode::StepBarrier
        );

        let refused = report
            .cases
            .iter()
            .find(|case| case.case_id == "iterable_streaming.elastic_membership")
            .expect("missing elastic-membership refusal");
        assert_eq!(refused.status, DistributedDataFeedCapabilityStatus::Refused);
        assert!(refused.refusal.is_some());
        assert!(refused
            .refusal
            .as_ref()
            .expect("refusal should exist")
            .contains("elastic_membership"));
    }
}
