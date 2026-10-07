use std::{
    collections::BTreeMap,
    fs,
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
};

use psionic_datastream::{
    DatastreamEncoding, DatastreamManifest, DatastreamManifestRef, DatastreamSubjectKind,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    DatasetContractError, DatasetIterationMode, DatasetKey, DatasetManifest, DatasetRecordEncoding,
    DatasetShardManifest, DatasetSplitDeclaration, DatasetSplitKind, TokenizerDigest,
};

/// Stable shard magic for the current public Parameter Golf FineWeb export.
pub const PARAMETER_GOLF_SHARD_MAGIC: i32 = 20240520;
/// Stable shard version for the current public Parameter Golf FineWeb export.
pub const PARAMETER_GOLF_SHARD_VERSION: i32 = 1;
/// Fixed number of little-endian `i32` values in the shard header.
pub const PARAMETER_GOLF_SHARD_HEADER_INTS: usize = 256;
/// Header size in bytes for the current shard format.
pub const PARAMETER_GOLF_SHARD_HEADER_BYTES: usize =
    PARAMETER_GOLF_SHARD_HEADER_INTS * std::mem::size_of::<i32>();
/// Token width in bytes for the current shard format.
pub const PARAMETER_GOLF_TOKEN_BYTES: usize = std::mem::size_of::<u16>();
/// Canonical train split name for the Parameter Golf lane.
pub const PARAMETER_GOLF_TRAIN_SPLIT_NAME: &str = "train";
/// Canonical validation split name for the Parameter Golf lane.
pub const PARAMETER_GOLF_VALIDATION_SPLIT_NAME: &str = "validation";
/// Canonical validation-identity statement for the current public challenge.
pub const PARAMETER_GOLF_VALIDATION_IDENTITY: &str =
    "fineweb_val_* fixed first-50k-doc validation split";
/// Canonical train-selection statement for the current public challenge.
pub const PARAMETER_GOLF_TRAIN_SELECTION_POSTURE: &str = "prefix of frozen shuffled export";
/// Canonical stream-model label for current challenge shards.
pub const PARAMETER_GOLF_STREAM_MODEL: &str = "contiguous_token_stream";

/// Stable split family surfaced by the Parameter Golf data lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParameterGolfSplitKind {
    Train,
    Validation,
}

impl ParameterGolfSplitKind {
    /// Returns the canonical Psionic split name.
    #[must_use]
    pub const fn split_name(self) -> &'static str {
        match self {
            Self::Train => PARAMETER_GOLF_TRAIN_SPLIT_NAME,
            Self::Validation => PARAMETER_GOLF_VALIDATION_SPLIT_NAME,
        }
    }

    /// Returns the canonical file stem prefix used by the public export.
    #[must_use]
    pub const fn file_prefix(self) -> &'static str {
        match self {
            Self::Train => "fineweb_train_",
            Self::Validation => "fineweb_val_",
        }
    }
}

/// Parsed shard identity for one public Parameter Golf FineWeb file.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterGolfShardIdentity {
    /// Train or validation role.
    pub split_kind: ParameterGolfSplitKind,
    /// Zero-based shard index inside the split.
    pub shard_index: u32,
    /// Stable shard key used by Psionic manifests.
    pub shard_key: String,
    /// Original file name on disk.
    pub file_name: String,
}

impl ParameterGolfShardIdentity {
    /// Parses one shard identity from a local file path.
    pub fn parse_path(path: &Path) -> Result<Self, ParameterGolfDataError> {
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| ParameterGolfDataError::InvalidShardFileName {
                path: path.display().to_string(),
            })?;

        let (split_kind, index_text) = if let Some(index_text) =
            file_name.strip_prefix(ParameterGolfSplitKind::Train.file_prefix())
        {
            (
                ParameterGolfSplitKind::Train,
                index_text.strip_suffix(".bin"),
            )
        } else if let Some(index_text) =
            file_name.strip_prefix(ParameterGolfSplitKind::Validation.file_prefix())
        {
            (
                ParameterGolfSplitKind::Validation,
                index_text.strip_suffix(".bin"),
            )
        } else {
            return Err(ParameterGolfDataError::InvalidShardFileName {
                path: path.display().to_string(),
            });
        };
        let Some(index_text) = index_text else {
            return Err(ParameterGolfDataError::InvalidShardFileName {
                path: path.display().to_string(),
            });
        };
        if index_text.len() != 6 || !index_text.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(ParameterGolfDataError::InvalidShardFileName {
                path: path.display().to_string(),
            });
        }
        let shard_index = index_text.parse::<u32>().map_err(|_| {
            ParameterGolfDataError::InvalidShardFileName {
                path: path.display().to_string(),
            }
        })?;
        let shard_key = file_name.trim_end_matches(".bin").to_string();
        Ok(Self {
            split_kind,
            shard_index,
            shard_key,
            file_name: file_name.to_string(),
        })
    }
}

/// Parsed public shard header for one Parameter Golf FineWeb shard.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterGolfShardHeader {
    /// Current stable shard magic.
    pub magic: i32,
    /// Current stable shard version.
    pub version: i32,
    /// Number of `u16` tokens in the payload.
    pub token_count: u32,
}

impl ParameterGolfShardHeader {
    /// Parses one shard header from an in-memory file image.
    pub fn from_file_bytes(path: &Path, bytes: &[u8]) -> Result<Self, ParameterGolfDataError> {
        if bytes.len() < PARAMETER_GOLF_SHARD_HEADER_BYTES {
            return Err(ParameterGolfDataError::ShortHeader {
                path: path.display().to_string(),
                actual_bytes: bytes.len(),
            });
        }
        let magic = i32::from_le_bytes(
            bytes[0..4]
                .try_into()
                .expect("fixed 4-byte slice should convert"),
        );
        if magic != PARAMETER_GOLF_SHARD_MAGIC {
            return Err(ParameterGolfDataError::UnexpectedShardMagic {
                path: path.display().to_string(),
                expected: PARAMETER_GOLF_SHARD_MAGIC,
                actual: magic,
            });
        }
        let version = i32::from_le_bytes(
            bytes[4..8]
                .try_into()
                .expect("fixed 4-byte slice should convert"),
        );
        if version != PARAMETER_GOLF_SHARD_VERSION {
            return Err(ParameterGolfDataError::UnexpectedShardVersion {
                path: path.display().to_string(),
                expected: PARAMETER_GOLF_SHARD_VERSION,
                actual: version,
            });
        }
        let raw_token_count = i32::from_le_bytes(
            bytes[8..12]
                .try_into()
                .expect("fixed 4-byte slice should convert"),
        );
        if raw_token_count < 0 {
            return Err(ParameterGolfDataError::NegativeTokenCount {
                path: path.display().to_string(),
                actual: raw_token_count,
            });
        }
        let token_count = raw_token_count as u32;
        let expected_bytes = PARAMETER_GOLF_SHARD_HEADER_BYTES as u64
            + u64::from(token_count) * PARAMETER_GOLF_TOKEN_BYTES as u64;
        let actual_bytes = bytes.len() as u64;
        if actual_bytes != expected_bytes {
            return Err(ParameterGolfDataError::ShardSizeMismatch {
                path: path.display().to_string(),
                expected_bytes,
                actual_bytes,
            });
        }
        Ok(Self {
            magic,
            version,
            token_count,
        })
    }

    /// Reads and validates one shard header from disk.
    pub fn read_path(path: &Path) -> Result<Self, ParameterGolfDataError> {
        let bytes = fs::read(path).map_err(|error| ParameterGolfDataError::Io {
            path: path.display().to_string(),
            detail: error.to_string(),
        })?;
        Self::from_file_bytes(path, bytes.as_slice())
    }
}

/// Stable receipt for one local shard admitted into the Parameter Golf lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterGolfShardReceipt {
    /// Local file path used to build the receipt.
    pub path: String,
    /// Parsed shard identity.
    pub identity: ParameterGolfShardIdentity,
    /// Parsed shard header.
    pub header: ParameterGolfShardHeader,
    /// Compact datastream reference for the shard payload.
    pub manifest: DatastreamManifestRef,
}

/// One repo-owned Parameter Golf dataset bundle built from local shard files.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfDatasetBundle {
    /// Canonical dataset manifest for the selected train-prefix plus validation split.
    pub manifest: DatasetManifest,
    /// Selected train-shard receipts in deterministic manifest order.
    pub train_shards: Vec<ParameterGolfShardReceipt>,
    /// Validation-shard receipts in deterministic manifest order.
    pub validation_shards: Vec<ParameterGolfShardReceipt>,
}

/// Resume-safe token-stream cursor for Parameter Golf shard playback.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterGolfTokenStreamCursor {
    /// Split owned by this stream cursor.
    pub split_name: String,
    /// Number of complete passes through the split.
    pub cycle: u32,
    /// Index into the ordered shard list for the current cycle.
    pub next_shard_index: usize,
    /// Token offset inside the current shard.
    pub next_token_offset_in_shard: u64,
    /// Total emitted tokens across all windows.
    pub emitted_tokens: u64,
}

impl ParameterGolfTokenStreamCursor {
    /// Creates a fresh cursor for one split.
    #[must_use]
    pub fn new(split_name: impl Into<String>) -> Self {
        Self {
            split_name: split_name.into(),
            cycle: 0,
            next_shard_index: 0,
            next_token_offset_in_shard: 0,
            emitted_tokens: 0,
        }
    }
}

/// One planned span over a contiguous token shard.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterGolfTokenStreamSpan {
    /// Stable shard identity.
    pub shard_key: String,
    /// Datastream manifest for the shard.
    pub manifest: DatastreamManifestRef,
    /// First token offset consumed from the shard.
    pub start_token_offset: u64,
    /// Number of contiguous tokens consumed from the shard.
    pub token_count: u64,
}

/// One replay-safe token-stream planning window.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterGolfTokenStreamWindow {
    /// Stable window identifier.
    pub window_id: String,
    /// Stable digest over the token-stream contract.
    pub contract_digest: String,
    /// Cursor before this window.
    pub start_cursor: ParameterGolfTokenStreamCursor,
    /// Cursor after this window.
    pub end_cursor: ParameterGolfTokenStreamCursor,
    /// Ordered shard spans consumed by the window.
    pub spans: Vec<ParameterGolfTokenStreamSpan>,
    /// Whether a single-pass stream is exhausted after this window.
    pub exhausted: bool,
}

/// Replay-safe token-stream contract for the current Parameter Golf lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterGolfTokenStreamContract {
    /// Dataset identity the stream contract expects.
    pub dataset: DatasetKey,
    /// Split name, typically `train` or `validation`.
    pub split_name: String,
    /// Whether the token stream repeats or stops at EOF.
    pub mode: DatasetIterationMode,
}

impl ParameterGolfTokenStreamContract {
    /// Creates one token-stream contract for a Parameter Golf split.
    #[must_use]
    pub fn new(dataset: DatasetKey, split_name: impl Into<String>) -> Self {
        Self {
            dataset,
            split_name: split_name.into(),
            mode: DatasetIterationMode::SinglePass,
        }
    }

    /// Attaches an iteration mode.
    #[must_use]
    pub const fn with_mode(mut self, mode: DatasetIterationMode) -> Self {
        self.mode = mode;
        self
    }

    /// Returns a stable digest over the token-stream contract.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(b"psionic_parameter_golf_token_stream_contract|");
        hasher.update(self.dataset.storage_key().as_bytes());
        hasher.update(b"|");
        hasher.update(self.split_name.as_bytes());
        hasher.update(b"|");
        hasher.update(match self.mode {
            DatasetIterationMode::SinglePass => b"single_pass".as_slice(),
            DatasetIterationMode::Repeat => b"repeat".as_slice(),
        });
        hex::encode(hasher.finalize())
    }

    /// Plans a deterministic contiguous token window over one Parameter Golf split.
    pub fn plan_window(
        &self,
        manifest: &DatasetManifest,
        cursor: &ParameterGolfTokenStreamCursor,
        max_tokens: u64,
    ) -> Result<Option<ParameterGolfTokenStreamWindow>, ParameterGolfDataError> {
        if max_tokens == 0 {
            return Err(ParameterGolfDataError::InvalidTokenWindowSize);
        }
        manifest.validate()?;
        if manifest.key != self.dataset {
            return Err(ParameterGolfDataError::DatasetMismatch {
                expected: self.dataset.storage_key(),
                actual: manifest.storage_key(),
            });
        }
        if manifest.record_encoding != DatasetRecordEncoding::TokenIdsLeU16 {
            return Err(ParameterGolfDataError::UnexpectedRecordEncoding {
                actual: format!("{:?}", manifest.record_encoding),
            });
        }
        if cursor.split_name != self.split_name {
            return Err(ParameterGolfDataError::CursorSplitMismatch {
                expected: self.split_name.clone(),
                actual: cursor.split_name.clone(),
            });
        }
        let Some(split) = manifest.split(self.split_name.as_str()) else {
            return Err(ParameterGolfDataError::UnknownSplit {
                split_name: self.split_name.clone(),
            });
        };
        let start_cursor = cursor.clone();
        let mut end_cursor = cursor.clone();
        let mut spans = Vec::new();
        let mut remaining = max_tokens;

        while remaining > 0 {
            if split.shards.is_empty() {
                break;
            }
            if end_cursor.next_shard_index >= split.shards.len() {
                if self.mode == DatasetIterationMode::SinglePass {
                    break;
                }
                end_cursor.cycle = end_cursor.cycle.saturating_add(1);
                end_cursor.next_shard_index = 0;
                end_cursor.next_token_offset_in_shard = 0;
            }
            let shard = &split.shards[end_cursor.next_shard_index];
            if end_cursor.next_token_offset_in_shard >= shard.token_count {
                end_cursor.next_shard_index += 1;
                end_cursor.next_token_offset_in_shard = 0;
                continue;
            }
            let available = shard
                .token_count
                .saturating_sub(end_cursor.next_token_offset_in_shard);
            let token_count = available.min(remaining);
            spans.push(ParameterGolfTokenStreamSpan {
                shard_key: shard.shard_key.clone(),
                manifest: shard.manifest.clone(),
                start_token_offset: end_cursor.next_token_offset_in_shard,
                token_count,
            });
            end_cursor.next_token_offset_in_shard = end_cursor
                .next_token_offset_in_shard
                .saturating_add(token_count);
            end_cursor.emitted_tokens = end_cursor.emitted_tokens.saturating_add(token_count);
            remaining -= token_count;
            if end_cursor.next_token_offset_in_shard == shard.token_count {
                end_cursor.next_shard_index += 1;
                end_cursor.next_token_offset_in_shard = 0;
            }
        }

        if spans.is_empty() {
            return Ok(None);
        }

        let exhausted = self.mode == DatasetIterationMode::SinglePass
            && end_cursor.next_shard_index >= split.shards.len();
        let contract_digest = self.stable_digest();
        let window_id = stable_parameter_golf_token_window_id(
            contract_digest.as_str(),
            &start_cursor,
            &end_cursor,
            spans.as_slice(),
        );
        Ok(Some(ParameterGolfTokenStreamWindow {
            window_id,
            contract_digest,
            start_cursor,
            end_cursor,
            spans,
            exhausted,
        }))
    }
}

/// Token role admitted by the current SentencePiece byte-accounting oracle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParameterGolfSentencePieceTokenKind {
    Normal,
    Byte,
    Control,
    Unknown,
    Unused,
}

/// One SentencePiece token entry used to rebuild the current byte-accounting LUTs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterGolfSentencePieceTokenEntry {
    /// Stable token id inside the tokenizer vocabulary.
    pub token_id: u32,
    /// Raw piece string reported by SentencePiece.
    pub piece: String,
    /// Token role for the byte-accounting oracle.
    pub kind: ParameterGolfSentencePieceTokenKind,
}

impl ParameterGolfSentencePieceTokenEntry {
    /// Creates one token entry.
    #[must_use]
    pub fn new(
        token_id: u32,
        piece: impl Into<String>,
        kind: ParameterGolfSentencePieceTokenKind,
    ) -> Self {
        Self {
            token_id,
            piece: piece.into(),
            kind,
        }
    }
}

/// Byte-accounting lookup tables that mirror `build_sentencepiece_luts(...)`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterGolfSentencePieceByteLuts {
    /// Base byte count for each token id.
    pub base_bytes_lut: Vec<i16>,
    /// Whether the token contributes one leading-space byte when the previous token is not a boundary.
    pub has_leading_space_lut: Vec<bool>,
    /// Whether the token should suppress the leading-space carry from the following token.
    pub is_boundary_token_lut: Vec<bool>,
}

impl ParameterGolfSentencePieceByteLuts {
    /// Returns the table size shared by all three LUTs.
    #[must_use]
    pub fn table_size(&self) -> usize {
        self.base_bytes_lut.len()
    }

    /// Rebuilds the exact current challenge byte-accounting LUTs from explicit SentencePiece token metadata.
    pub fn build(
        tokenizer_vocab_size: usize,
        tokens: &[ParameterGolfSentencePieceTokenEntry],
    ) -> Result<Self, ParameterGolfDataError> {
        let max_token_id = tokens
            .iter()
            .map(|token| token.token_id as usize)
            .max()
            .unwrap_or(0);
        let table_size = tokenizer_vocab_size.max(max_token_id.saturating_add(1));
        let mut base_bytes_lut = vec![0_i16; table_size];
        let mut has_leading_space_lut = vec![false; table_size];
        let mut is_boundary_token_lut = vec![true; table_size];
        let mut seen = std::collections::BTreeSet::new();

        for token in tokens {
            if !seen.insert(token.token_id) {
                return Err(ParameterGolfDataError::DuplicateSentencePieceTokenId {
                    token_id: token.token_id,
                });
            }
            let index = token.token_id as usize;
            match token.kind {
                ParameterGolfSentencePieceTokenKind::Control
                | ParameterGolfSentencePieceTokenKind::Unknown
                | ParameterGolfSentencePieceTokenKind::Unused => {}
                ParameterGolfSentencePieceTokenKind::Byte => {
                    is_boundary_token_lut[index] = false;
                    base_bytes_lut[index] = 1;
                }
                ParameterGolfSentencePieceTokenKind::Normal => {
                    is_boundary_token_lut[index] = false;
                    let mut piece = token.piece.as_str();
                    if let Some(stripped) = piece.strip_prefix('▁') {
                        has_leading_space_lut[index] = true;
                        piece = stripped;
                    }
                    let byte_count = piece.as_bytes().len();
                    let byte_count = i16::try_from(byte_count).map_err(|_| {
                        ParameterGolfDataError::SentencePieceByteCountOverflow {
                            token_id: token.token_id,
                            actual: byte_count,
                        }
                    })?;
                    base_bytes_lut[index] = byte_count;
                }
            }
        }

        Ok(Self {
            base_bytes_lut,
            has_leading_space_lut,
            is_boundary_token_lut,
        })
    }

    /// Counts target bytes with the same leading-space adjustment used by the public challenge scripts.
    pub fn count_target_bytes(
        &self,
        previous_token_ids: &[u32],
        target_token_ids: &[u32],
    ) -> Result<u64, ParameterGolfDataError> {
        if previous_token_ids.len() != target_token_ids.len() {
            return Err(ParameterGolfDataError::ByteAccountingLengthMismatch {
                previous_len: previous_token_ids.len(),
                target_len: target_token_ids.len(),
            });
        }
        let mut total = 0_u64;
        for (&previous, &target) in previous_token_ids.iter().zip(target_token_ids.iter()) {
            let previous_index = previous as usize;
            let target_index = target as usize;
            if previous_index >= self.table_size() {
                return Err(ParameterGolfDataError::TokenIdOutOfRange {
                    token_id: previous,
                    table_size: self.table_size(),
                });
            }
            if target_index >= self.table_size() {
                return Err(ParameterGolfDataError::TokenIdOutOfRange {
                    token_id: target,
                    table_size: self.table_size(),
                });
            }
            let mut token_bytes = i64::from(self.base_bytes_lut[target_index]);
            if self.has_leading_space_lut[target_index]
                && !self.is_boundary_token_lut[previous_index]
            {
                token_bytes += 1;
            }
            total = total.saturating_add(token_bytes.max(0) as u64);
        }
        Ok(total)
    }

    /// Converts mean token NLL in natural-log units into the challenge's `bits per byte`.
    pub fn bits_per_byte_from_mean_nll(
        &self,
        mean_nll_nats_per_token: f64,
        previous_token_ids: &[u32],
        target_token_ids: &[u32],
    ) -> Result<f64, ParameterGolfDataError> {
        let token_count = target_token_ids.len() as u64;
        if token_count == 0 {
            return Err(ParameterGolfDataError::EmptyTokenBatch);
        }
        let byte_count = self.count_target_bytes(previous_token_ids, target_token_ids)?;
        if byte_count == 0 {
            return Err(ParameterGolfDataError::ZeroByteCount);
        }
        let bits_per_token = mean_nll_nats_per_token / std::f64::consts::LN_2;
        let tokens_per_byte = token_count as f64 / byte_count as f64;
        Ok(bits_per_token * tokens_per_byte)
    }
}

/// Builds the canonical Parameter Golf SentencePiece byte-accounting LUTs from
/// one explicit `.vocab` artifact emitted beside the challenge tokenizer
/// model.
pub fn parameter_golf_sentencepiece_entries_from_vocab_path(
    vocab_path: impl AsRef<Path>,
) -> Result<Vec<ParameterGolfSentencePieceTokenEntry>, ParameterGolfDataError> {
    let vocab_path = vocab_path.as_ref();
    let vocab_text =
        fs::read_to_string(vocab_path).map_err(|error| ParameterGolfDataError::Io {
            path: vocab_path.display().to_string(),
            detail: error.to_string(),
        })?;
    let mut entries = Vec::new();
    for (line_index, line) in vocab_text.lines().enumerate() {
        let piece = line
            .split_once('\t')
            .map(|(piece, _)| piece)
            .or_else(|| (!line.trim().is_empty()).then_some(line))
            .ok_or_else(|| ParameterGolfDataError::SentencePieceVocabDecode {
                path: vocab_path.display().to_string(),
                detail: format!("line {} is empty", line_index + 1),
            })?;
        entries.push(ParameterGolfSentencePieceTokenEntry::new(
            line_index as u32,
            piece,
            sentencepiece_token_kind_from_vocab_piece(piece),
        ));
    }
    Ok(entries)
}

/// Loads the canonical Parameter Golf SentencePiece token entries from the
/// sibling `.vocab` artifact beside one tokenizer `.model` path.
pub fn parameter_golf_sentencepiece_entries_from_tokenizer_path(
    tokenizer_path: impl AsRef<Path>,
) -> Result<Vec<ParameterGolfSentencePieceTokenEntry>, ParameterGolfDataError> {
    let tokenizer_path = tokenizer_path.as_ref();
    let mut vocab_path = tokenizer_path.to_path_buf();
    vocab_path.set_extension("vocab");
    if !vocab_path.is_file() {
        return Err(ParameterGolfDataError::MissingTokenizerVocab {
            tokenizer_path: tokenizer_path.display().to_string(),
            vocab_path: vocab_path.display().to_string(),
        });
    }
    parameter_golf_sentencepiece_entries_from_vocab_path(vocab_path)
}

/// Builds the canonical Parameter Golf SentencePiece byte-accounting LUTs from
/// one explicit `.vocab` artifact emitted beside the challenge tokenizer
/// model.
pub fn parameter_golf_sentencepiece_byte_luts_from_vocab_path(
    vocab_path: impl AsRef<Path>,
) -> Result<ParameterGolfSentencePieceByteLuts, ParameterGolfDataError> {
    let entries = parameter_golf_sentencepiece_entries_from_vocab_path(vocab_path)?;
    ParameterGolfSentencePieceByteLuts::build(entries.len(), entries.as_slice())
}

/// Builds the canonical Parameter Golf SentencePiece byte-accounting LUTs from
/// the sibling `.vocab` artifact beside one tokenizer `.model` path.
pub fn parameter_golf_sentencepiece_byte_luts_from_tokenizer_path(
    tokenizer_path: impl AsRef<Path>,
) -> Result<ParameterGolfSentencePieceByteLuts, ParameterGolfDataError> {
    let entries = parameter_golf_sentencepiece_entries_from_tokenizer_path(tokenizer_path)?;
    ParameterGolfSentencePieceByteLuts::build(entries.len(), entries.as_slice())
}

#[derive(Deserialize)]
struct BuiltinParameterGolfOracleParityFixture {
    sentencepiece_entries: Vec<BuiltinParameterGolfSentencePieceEntry>,
}

#[derive(Deserialize)]
struct BuiltinParameterGolfSentencePieceEntry {
    token_id: u32,
    piece: String,
    kind: String,
}

/// Returns the canonical Parameter Golf SentencePiece byte-accounting LUTs from
/// the frozen oracle parity fixture shipped in this repo.
pub fn builtin_parameter_golf_sentencepiece_byte_luts(
) -> Result<ParameterGolfSentencePieceByteLuts, ParameterGolfDataError> {
    let fixture: BuiltinParameterGolfOracleParityFixture = serde_json::from_str(include_str!(
        "../../../fixtures/parameter_golf/parity/parameter_golf_oracle_parity_fixture.json"
    ))
    .map_err(|error| ParameterGolfDataError::BuiltinOracleFixtureDecode {
        detail: error.to_string(),
    })?;
    let entries = fixture
        .sentencepiece_entries
        .into_iter()
        .map(|entry| {
            Ok::<ParameterGolfSentencePieceTokenEntry, ParameterGolfDataError>(
                ParameterGolfSentencePieceTokenEntry::new(
                    entry.token_id,
                    entry.piece,
                    builtin_sentencepiece_token_kind(entry.kind.as_str())?,
                ),
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    ParameterGolfSentencePieceByteLuts::build(1024, entries.as_slice())
}

fn builtin_sentencepiece_token_kind(
    kind: &str,
) -> Result<ParameterGolfSentencePieceTokenKind, ParameterGolfDataError> {
    match kind {
        "normal" => Ok(ParameterGolfSentencePieceTokenKind::Normal),
        "byte" => Ok(ParameterGolfSentencePieceTokenKind::Byte),
        "control" => Ok(ParameterGolfSentencePieceTokenKind::Control),
        "unknown" => Ok(ParameterGolfSentencePieceTokenKind::Unknown),
        "unused" => Ok(ParameterGolfSentencePieceTokenKind::Unused),
        other => Err(ParameterGolfDataError::BuiltinOracleFixtureDecode {
            detail: format!("unknown sentencepiece token kind `{other}`"),
        }),
    }
}

fn sentencepiece_token_kind_from_vocab_piece(piece: &str) -> ParameterGolfSentencePieceTokenKind {
    match piece {
        "<unk>" => ParameterGolfSentencePieceTokenKind::Unknown,
        "<pad>" | "<s>" | "</s>" => ParameterGolfSentencePieceTokenKind::Control,
        _ if piece.starts_with("<unused") && piece.ends_with('>') => {
            ParameterGolfSentencePieceTokenKind::Unused
        }
        _ if sentencepiece_piece_is_byte(piece) => ParameterGolfSentencePieceTokenKind::Byte,
        _ => ParameterGolfSentencePieceTokenKind::Normal,
    }
}

fn sentencepiece_piece_is_byte(piece: &str) -> bool {
    if piece.len() != 6 || !piece.starts_with("<0x") || !piece.ends_with('>') {
        return false;
    }
    piece
        .trim_start_matches("<0x")
        .trim_end_matches('>')
        .chars()
        .all(|ch| ch.is_ascii_hexdigit())
}

/// Loads and validates one current-format Parameter Golf shard as a `u16` token vector.
pub fn load_parameter_golf_shard_tokens(
    path: impl AsRef<Path>,
) -> Result<Vec<u16>, ParameterGolfDataError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| ParameterGolfDataError::Io {
        path: path.display().to_string(),
        detail: error.to_string(),
    })?;
    let header = ParameterGolfShardHeader::from_file_bytes(path, bytes.as_slice())?;
    let payload = &bytes[PARAMETER_GOLF_SHARD_HEADER_BYTES..];
    let tokens = payload
        .chunks_exact(PARAMETER_GOLF_TOKEN_BYTES)
        .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
        .collect::<Vec<_>>();
    if tokens.len() != header.token_count as usize {
        return Err(ParameterGolfDataError::TokenPayloadShortRead {
            path: path.display().to_string(),
            expected_tokens: header.token_count as usize,
            actual_tokens: tokens.len(),
        });
    }
    Ok(tokens)
}

/// Loads one contiguous token slice from a current-format Parameter Golf shard without reading the whole payload.
pub fn load_parameter_golf_shard_token_slice(
    path: impl AsRef<Path>,
    start_token_offset: u64,
    token_count: u64,
) -> Result<Vec<u16>, ParameterGolfDataError> {
    let path = path.as_ref();
    let header = ParameterGolfShardHeader::read_path(path)?;
    let available_tokens = header.token_count as u64;
    let end_token_offset = start_token_offset.saturating_add(token_count);
    if start_token_offset > available_tokens || end_token_offset > available_tokens {
        return Err(ParameterGolfDataError::TokenSliceOutOfBounds {
            path: path.display().to_string(),
            start_token_offset,
            token_count,
            available_tokens,
        });
    }
    let byte_count = token_count
        .checked_mul(PARAMETER_GOLF_TOKEN_BYTES as u64)
        .ok_or(ParameterGolfDataError::TokenSliceTooLarge { token_count })?;
    let byte_count = usize::try_from(byte_count)
        .map_err(|_| ParameterGolfDataError::TokenSliceTooLarge { token_count })?;
    let start_byte_offset = PARAMETER_GOLF_SHARD_HEADER_BYTES as u64
        + (start_token_offset * PARAMETER_GOLF_TOKEN_BYTES as u64);

    let mut file = File::open(path).map_err(|error| ParameterGolfDataError::Io {
        path: path.display().to_string(),
        detail: error.to_string(),
    })?;
    file.seek(SeekFrom::Start(start_byte_offset))
        .map_err(|error| ParameterGolfDataError::Io {
            path: path.display().to_string(),
            detail: error.to_string(),
        })?;
    let mut bytes = vec![0_u8; byte_count];
    file.read_exact(bytes.as_mut_slice())
        .map_err(|error| ParameterGolfDataError::Io {
            path: path.display().to_string(),
            detail: error.to_string(),
        })?;

    Ok(bytes
        .chunks_exact(PARAMETER_GOLF_TOKEN_BYTES)
        .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
        .collect())
}

/// Materializes one planned Parameter Golf token window from the selected local shard bundle.
pub fn materialize_parameter_golf_token_window(
    bundle: &ParameterGolfDatasetBundle,
    window: &ParameterGolfTokenStreamWindow,
) -> Result<Vec<u16>, ParameterGolfDataError> {
    let split_name = window.start_cursor.split_name.as_str();
    let receipts = match split_name {
        PARAMETER_GOLF_TRAIN_SPLIT_NAME => bundle.train_shards.as_slice(),
        PARAMETER_GOLF_VALIDATION_SPLIT_NAME => bundle.validation_shards.as_slice(),
        _ => {
            return Err(ParameterGolfDataError::UnknownSplit {
                split_name: split_name.to_string(),
            });
        }
    };
    let total_tokens = window
        .spans
        .iter()
        .map(|span| span.token_count)
        .sum::<u64>();
    let total_tokens =
        usize::try_from(total_tokens).map_err(|_| ParameterGolfDataError::TokenSliceTooLarge {
            token_count: total_tokens as u64,
        })?;
    let mut tokens = Vec::with_capacity(total_tokens);
    for span in &window.spans {
        let Some(receipt) = receipts
            .iter()
            .find(|receipt| receipt.identity.shard_key == span.shard_key)
        else {
            return Err(ParameterGolfDataError::UnknownWindowShard {
                split_name: split_name.to_string(),
                shard_key: span.shard_key.clone(),
            });
        };
        tokens.extend(load_parameter_golf_shard_token_slice(
            receipt.path.as_str(),
            span.start_token_offset,
            span.token_count,
        )?);
    }
    Ok(tokens)
}

/// Loads the fixed validation split in manifest order and trims it with the same usable-token rule as the public challenge scripts.
pub fn load_parameter_golf_validation_tokens_from_paths(
    paths: &[PathBuf],
    seq_len: usize,
) -> Result<Vec<u16>, ParameterGolfDataError> {
    if paths.is_empty() {
        return Err(ParameterGolfDataError::MissingValidationShardPaths);
    }
    if seq_len == 0 {
        return Err(ParameterGolfDataError::InvalidValidationSequenceLength);
    }
    let mut ordered_paths = paths.to_vec();
    ordered_paths.sort();
    let mut tokens = Vec::new();
    for path in ordered_paths {
        tokens.extend(load_parameter_golf_shard_tokens(path)?);
    }
    let usable = ((tokens.len().saturating_sub(1)) / seq_len) * seq_len;
    if usable == 0 {
        return Err(ParameterGolfDataError::ValidationSplitTooShort {
            seq_len,
            token_count: tokens.len(),
        });
    }
    tokens.truncate(usable + 1);
    Ok(tokens)
}

/// Builds a Parameter Golf dataset bundle from one local shard directory.
pub fn parameter_golf_dataset_bundle_from_local_dir(
    dataset_key: DatasetKey,
    dataset_root: impl AsRef<Path>,
    variant: impl Into<String>,
    tokenizer: TokenizerDigest,
    tokenizer_artifact_ref: impl Into<String>,
    train_shard_limit: Option<usize>,
) -> Result<ParameterGolfDatasetBundle, ParameterGolfDataError> {
    let dataset_root = dataset_root.as_ref();
    let variant = variant.into();
    let tokenizer_artifact_ref = tokenizer_artifact_ref.into();

    let mut train_paths = Vec::new();
    let mut validation_paths = Vec::new();
    for entry in fs::read_dir(dataset_root).map_err(|error| ParameterGolfDataError::Io {
        path: dataset_root.display().to_string(),
        detail: error.to_string(),
    })? {
        let entry = entry.map_err(|error| ParameterGolfDataError::Io {
            path: dataset_root.display().to_string(),
            detail: error.to_string(),
        })?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        match ParameterGolfShardIdentity::parse_path(path.as_path()) {
            Ok(identity) => match identity.split_kind {
                ParameterGolfSplitKind::Train => train_paths.push(path),
                ParameterGolfSplitKind::Validation => validation_paths.push(path),
            },
            Err(ParameterGolfDataError::InvalidShardFileName { .. }) => {}
            Err(error) => return Err(error),
        }
    }

    let mut train_receipts = build_receipts_for_paths(
        &dataset_key,
        PARAMETER_GOLF_TRAIN_SPLIT_NAME,
        ParameterGolfSplitKind::Train,
        train_paths,
    )?;
    let validation_receipts = build_receipts_for_paths(
        &dataset_key,
        PARAMETER_GOLF_VALIDATION_SPLIT_NAME,
        ParameterGolfSplitKind::Validation,
        validation_paths,
    )?;

    if train_receipts.is_empty() {
        return Err(ParameterGolfDataError::MissingTrainShards {
            dataset_root: dataset_root.display().to_string(),
        });
    }
    if validation_receipts.is_empty() {
        return Err(ParameterGolfDataError::MissingValidationShards {
            dataset_root: dataset_root.display().to_string(),
        });
    }

    let available_train_shards = train_receipts.len();
    let selected_train_shards = train_shard_limit.unwrap_or(available_train_shards);
    if selected_train_shards == 0 || selected_train_shards > available_train_shards {
        return Err(ParameterGolfDataError::InvalidTrainShardLimit {
            requested: selected_train_shards,
            available: available_train_shards,
        });
    }
    train_receipts.truncate(selected_train_shards);

    let train_split = DatasetSplitDeclaration::new(
        &dataset_key,
        PARAMETER_GOLF_TRAIN_SPLIT_NAME,
        DatasetSplitKind::Train,
        train_receipts
            .iter()
            .map(|receipt| shard_manifest_from_receipt(&dataset_key, receipt))
            .collect::<Result<Vec<_>, _>>()?,
    )?;
    let validation_split = DatasetSplitDeclaration::new(
        &dataset_key,
        PARAMETER_GOLF_VALIDATION_SPLIT_NAME,
        DatasetSplitKind::Validation,
        validation_receipts
            .iter()
            .map(|receipt| shard_manifest_from_receipt(&dataset_key, receipt))
            .collect::<Result<Vec<_>, _>>()?,
    )?;

    let provenance_digest = stable_parameter_golf_provenance_digest(
        variant.as_str(),
        tokenizer_artifact_ref.as_str(),
        train_receipts.as_slice(),
        validation_receipts.as_slice(),
    );
    let mut metadata = BTreeMap::<String, Value>::new();
    metadata.insert(
        String::from("parameter_golf_variant"),
        Value::String(variant),
    );
    metadata.insert(
        String::from("parameter_golf_tokenizer_artifact_ref"),
        Value::String(tokenizer_artifact_ref),
    );
    metadata.insert(
        String::from("parameter_golf_validation_identity"),
        Value::String(String::from(PARAMETER_GOLF_VALIDATION_IDENTITY)),
    );
    metadata.insert(
        String::from("parameter_golf_train_selection_posture"),
        Value::String(String::from(PARAMETER_GOLF_TRAIN_SELECTION_POSTURE)),
    );
    metadata.insert(
        String::from("parameter_golf_stream_model"),
        Value::String(String::from(PARAMETER_GOLF_STREAM_MODEL)),
    );
    metadata.insert(
        String::from("parameter_golf_shard_magic"),
        json!(PARAMETER_GOLF_SHARD_MAGIC),
    );
    metadata.insert(
        String::from("parameter_golf_shard_version"),
        json!(PARAMETER_GOLF_SHARD_VERSION),
    );
    metadata.insert(
        String::from("parameter_golf_token_dtype"),
        Value::String(String::from("u16_le")),
    );
    metadata.insert(
        String::from("parameter_golf_train_available_shards"),
        json!(available_train_shards),
    );
    metadata.insert(
        String::from("parameter_golf_train_selected_shards"),
        json!(selected_train_shards),
    );

    let manifest = DatasetManifest::new(
        dataset_key,
        "Parameter Golf FineWeb challenge export",
        DatasetRecordEncoding::TokenIdsLeU16,
        tokenizer,
    )
    .with_context_window_tokens(1024)
    .with_provenance_digest(provenance_digest)
    .with_splits(vec![train_split, validation_split])
    .with_metadata(metadata);
    manifest.validate()?;

    Ok(ParameterGolfDatasetBundle {
        manifest,
        train_shards: train_receipts,
        validation_shards: validation_receipts,
    })
}

/// Validation or planning error for the bounded Parameter Golf data lane.
#[derive(Debug, Error)]
pub enum ParameterGolfDataError {
    #[error("parameter golf data IO failed for `{path}`: {detail}")]
    Io { path: String, detail: String },
    #[error("parameter golf shard file name is invalid: `{path}`")]
    InvalidShardFileName { path: String },
    #[error("parameter golf shard header for `{path}` is too short: {actual_bytes} bytes")]
    ShortHeader { path: String, actual_bytes: usize },
    #[error("parameter golf shard `{path}` expected magic {expected} but found {actual}")]
    UnexpectedShardMagic {
        path: String,
        expected: i32,
        actual: i32,
    },
    #[error("parameter golf shard `{path}` expected version {expected} but found {actual}")]
    UnexpectedShardVersion {
        path: String,
        expected: i32,
        actual: i32,
    },
    #[error("parameter golf shard `{path}` reported negative token count {actual}")]
    NegativeTokenCount { path: String, actual: i32 },
    #[error(
        "parameter golf shard `{path}` expected {expected_bytes} bytes but found {actual_bytes}"
    )]
    ShardSizeMismatch {
        path: String,
        expected_bytes: u64,
        actual_bytes: u64,
    },
    #[error(
        "parameter golf shard `{path}` expected {expected_tokens} tokens but decoded {actual_tokens}"
    )]
    TokenPayloadShortRead {
        path: String,
        expected_tokens: usize,
        actual_tokens: usize,
    },
    #[error(
        "parameter golf shard `{path}` token slice starting at {start_token_offset} with {token_count} tokens exceeds available token count {available_tokens}"
    )]
    TokenSliceOutOfBounds {
        path: String,
        start_token_offset: u64,
        token_count: u64,
        available_tokens: u64,
    },
    #[error(
        "parameter golf token slice of {token_count} tokens exceeds supported host buffer size"
    )]
    TokenSliceTooLarge { token_count: u64 },
    #[error("parameter golf validation loading requires at least one shard path")]
    MissingValidationShardPaths,
    #[error("parameter golf validation loading requires `seq_len > 0`")]
    InvalidValidationSequenceLength,
    #[error(
        "parameter golf validation split is too short for seq_len={seq_len}, only {token_count} tokens were available"
    )]
    ValidationSplitTooShort { seq_len: usize, token_count: usize },
    #[error("parameter golf train split is missing from `{dataset_root}`")]
    MissingTrainShards { dataset_root: String },
    #[error("parameter golf validation split is missing from `{dataset_root}`")]
    MissingValidationShards { dataset_root: String },
    #[error(
        "parameter golf shard indices for split `{split}` must be contiguous from zero, expected {expected} but found {actual}"
    )]
    NonContiguousShardIndices {
        split: String,
        expected: u32,
        actual: u32,
    },
    #[error(
        "parameter golf requested {requested} train shards but only {available} are available"
    )]
    InvalidTrainShardLimit { requested: usize, available: usize },
    #[error("parameter golf token window size must be greater than zero")]
    InvalidTokenWindowSize,
    #[error("parameter golf builtin oracle fixture decode failed: {detail}")]
    BuiltinOracleFixtureDecode { detail: String },
    #[error(
        "parameter golf tokenizer `{tokenizer_path}` requires sibling vocab artifact `{vocab_path}` for byte accounting"
    )]
    MissingTokenizerVocab {
        tokenizer_path: String,
        vocab_path: String,
    },
    #[error("parameter golf SentencePiece vocab decode failed for `{path}`: {detail}")]
    SentencePieceVocabDecode { path: String, detail: String },
    #[error("parameter golf SentencePiece token id `{token_id}` is duplicated")]
    DuplicateSentencePieceTokenId { token_id: u32 },
    #[error(
        "parameter golf SentencePiece token `{token_id}` expands to {actual} bytes, which exceeds the supported i16 LUT range"
    )]
    SentencePieceByteCountOverflow { token_id: u32, actual: usize },
    #[error(
        "parameter golf byte accounting requires equal previous/target lengths, found previous={previous_len} target={target_len}"
    )]
    ByteAccountingLengthMismatch {
        previous_len: usize,
        target_len: usize,
    },
    #[error(
        "parameter golf byte accounting token id `{token_id}` is outside the LUT table size {table_size}"
    )]
    TokenIdOutOfRange { token_id: u32, table_size: usize },
    #[error("parameter golf byte accounting requires at least one target token")]
    EmptyTokenBatch,
    #[error("parameter golf byte accounting produced zero counted bytes")]
    ZeroByteCount,
    #[error("parameter golf expected dataset `{expected}` but found `{actual}`")]
    DatasetMismatch { expected: String, actual: String },
    #[error("parameter golf expected split `{expected}` but found cursor split `{actual}`")]
    CursorSplitMismatch { expected: String, actual: String },
    #[error("parameter golf manifest does not declare split `{split_name}`")]
    UnknownSplit { split_name: String },
    #[error(
        "parameter golf token window references unknown shard `{shard_key}` in split `{split_name}`"
    )]
    UnknownWindowShard {
        split_name: String,
        shard_key: String,
    },
    #[error(
        "parameter golf manifest expected `record_encoding=token_ids_le_u16`, found `{actual}`"
    )]
    UnexpectedRecordEncoding { actual: String },
    #[error(transparent)]
    Dataset(#[from] DatasetContractError),
}

fn build_receipts_for_paths(
    dataset_key: &DatasetKey,
    split_name: &str,
    split_kind: ParameterGolfSplitKind,
    mut paths: Vec<PathBuf>,
) -> Result<Vec<ParameterGolfShardReceipt>, ParameterGolfDataError> {
    paths.sort();
    let mut receipts = Vec::new();
    for path in paths {
        let identity = ParameterGolfShardIdentity::parse_path(path.as_path())?;
        if identity.split_kind != split_kind {
            return Err(ParameterGolfDataError::InvalidShardFileName {
                path: path.display().to_string(),
            });
        }
        let expected_index = receipts.len() as u32;
        if identity.shard_index != expected_index {
            return Err(ParameterGolfDataError::NonContiguousShardIndices {
                split: String::from(split_name),
                expected: expected_index,
                actual: identity.shard_index,
            });
        }
        let bytes = fs::read(path.as_path()).map_err(|error| ParameterGolfDataError::Io {
            path: path.display().to_string(),
            detail: error.to_string(),
        })?;
        let header = ParameterGolfShardHeader::from_file_bytes(path.as_path(), bytes.as_slice())?;
        let stream_id = format!(
            "parameter_golf/{}/{}/{}",
            dataset_key.storage_key(),
            split_name,
            identity.shard_key
        );
        let manifest = DatastreamManifest::from_bytes(
            stream_id,
            DatastreamSubjectKind::TokenizedCorpus,
            bytes.as_slice(),
            1 << 20,
            DatastreamEncoding::TokenIdsLeU16,
        )
        .with_dataset_binding(
            dataset_key.datastream_binding(split_name, identity.shard_key.clone()),
        )
        .manifest_ref();
        receipts.push(ParameterGolfShardReceipt {
            path: path.display().to_string(),
            identity,
            header,
            manifest,
        });
    }
    Ok(receipts)
}

fn shard_manifest_from_receipt(
    dataset_key: &DatasetKey,
    receipt: &ParameterGolfShardReceipt,
) -> Result<DatasetShardManifest, DatasetContractError> {
    let split_name = receipt.identity.split_kind.split_name();
    DatasetShardManifest::new(
        dataset_key,
        split_name,
        receipt.identity.shard_key.clone(),
        receipt.manifest.clone(),
        1,
        u64::from(receipt.header.token_count),
        receipt.header.token_count,
        receipt.header.token_count,
    )
}

fn stable_parameter_golf_provenance_digest(
    variant: &str,
    tokenizer_artifact_ref: &str,
    train_receipts: &[ParameterGolfShardReceipt],
    validation_receipts: &[ParameterGolfShardReceipt],
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_parameter_golf_provenance|");
    hasher.update(variant.as_bytes());
    hasher.update(b"|");
    hasher.update(tokenizer_artifact_ref.as_bytes());
    for receipt in train_receipts.iter().chain(validation_receipts.iter()) {
        hasher.update(b"|shard|");
        hasher.update(receipt.identity.shard_key.as_bytes());
        hasher.update(b"|");
        hasher.update(receipt.manifest.object_digest.as_bytes());
        hasher.update(b"|");
        hasher.update(receipt.header.token_count.to_string().as_bytes());
    }
    hex::encode(hasher.finalize())
}

fn stable_parameter_golf_token_window_id(
    contract_digest: &str,
    start_cursor: &ParameterGolfTokenStreamCursor,
    end_cursor: &ParameterGolfTokenStreamCursor,
    spans: &[ParameterGolfTokenStreamSpan],
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_parameter_golf_token_window|");
    hasher.update(contract_digest.as_bytes());
    hasher.update(b"|");
    hasher.update(start_cursor.split_name.as_bytes());
    hasher.update(b"|");
    hasher.update(start_cursor.cycle.to_string().as_bytes());
    hasher.update(b"|");
    hasher.update(start_cursor.next_shard_index.to_string().as_bytes());
    hasher.update(b"|");
    hasher.update(
        start_cursor
            .next_token_offset_in_shard
            .to_string()
            .as_bytes(),
    );
    hasher.update(b"|");
    hasher.update(end_cursor.cycle.to_string().as_bytes());
    hasher.update(b"|");
    hasher.update(end_cursor.next_shard_index.to_string().as_bytes());
    hasher.update(b"|");
    hasher.update(end_cursor.next_token_offset_in_shard.to_string().as_bytes());
    for span in spans {
        hasher.update(b"|span|");
        hasher.update(span.shard_key.as_bytes());
        hasher.update(b"|");
        hasher.update(span.start_token_offset.to_string().as_bytes());
        hasher.update(b"|");
        hasher.update(span.token_count.to_string().as_bytes());
        hasher.update(b"|");
        hasher.update(span.manifest.manifest_digest.as_bytes());
    }
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TokenizerFamily;
    use serde::Deserialize;

    fn sample_tokenizer() -> TokenizerDigest {
        TokenizerDigest::new(TokenizerFamily::SentencePiece, "sp1024-digest", 1024)
            .with_special_tokens_digest("sp1024-special")
    }

    struct TempDirGuard {
        path: PathBuf,
    }

    impl TempDirGuard {
        fn new(label: &str) -> Self {
            let mut path = std::env::temp_dir();
            let unique = format!(
                "psionic-param-golf-{}-{}-{}",
                label,
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .expect("time should be monotonic")
                    .as_nanos()
            );
            path.push(unique);
            fs::create_dir_all(&path).expect("temp dir should be created");
            Self { path }
        }
    }

    impl Drop for TempDirGuard {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn write_shard(path: &Path, tokens: &[u16]) {
        let mut bytes = vec![0_u8; PARAMETER_GOLF_SHARD_HEADER_BYTES];
        bytes[0..4].copy_from_slice(&PARAMETER_GOLF_SHARD_MAGIC.to_le_bytes());
        bytes[4..8].copy_from_slice(&PARAMETER_GOLF_SHARD_VERSION.to_le_bytes());
        bytes[8..12].copy_from_slice(&(tokens.len() as i32).to_le_bytes());
        for token in tokens {
            bytes.extend_from_slice(&token.to_le_bytes());
        }
        fs::write(path, bytes).expect("shard should be written");
    }

    #[test]
    fn shard_loader_reads_current_public_u16_format() {
        let temp = TempDirGuard::new("loader");
        let shard_path = temp.path.join("fineweb_train_000000.bin");
        write_shard(&shard_path, &[7, 8, 9, 10]);

        let identity = ParameterGolfShardIdentity::parse_path(&shard_path)
            .expect("shard identity should parse");
        let header =
            ParameterGolfShardHeader::read_path(&shard_path).expect("header should validate");
        let tokens = load_parameter_golf_shard_tokens(&shard_path).expect("tokens should load");

        assert_eq!(identity.split_kind, ParameterGolfSplitKind::Train);
        assert_eq!(identity.shard_index, 0);
        assert_eq!(header.token_count, 4);
        assert_eq!(tokens, vec![7, 8, 9, 10]);
    }

    #[test]
    fn shard_loader_refuses_bad_magic_and_size_mismatch() {
        let temp = TempDirGuard::new("errors");
        let bad_magic = temp.path.join("fineweb_train_000000.bin");
        let mut bytes = vec![0_u8; PARAMETER_GOLF_SHARD_HEADER_BYTES];
        bytes[0..4].copy_from_slice(&0_i32.to_le_bytes());
        bytes[4..8].copy_from_slice(&PARAMETER_GOLF_SHARD_VERSION.to_le_bytes());
        bytes[8..12].copy_from_slice(&1_i32.to_le_bytes());
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        fs::write(&bad_magic, bytes).expect("bad shard should be written");
        let bad_magic_err =
            load_parameter_golf_shard_tokens(&bad_magic).expect_err("bad magic should be refused");
        assert!(matches!(
            bad_magic_err,
            ParameterGolfDataError::UnexpectedShardMagic { .. }
        ));

        let bad_size = temp.path.join("fineweb_val_000000.bin");
        let mut short_bytes = vec![0_u8; PARAMETER_GOLF_SHARD_HEADER_BYTES];
        short_bytes[0..4].copy_from_slice(&PARAMETER_GOLF_SHARD_MAGIC.to_le_bytes());
        short_bytes[4..8].copy_from_slice(&PARAMETER_GOLF_SHARD_VERSION.to_le_bytes());
        short_bytes[8..12].copy_from_slice(&4_i32.to_le_bytes());
        short_bytes.extend_from_slice(&[1_u8, 2, 3, 4]);
        fs::write(&bad_size, short_bytes).expect("short shard should be written");
        let bad_size_err = load_parameter_golf_shard_tokens(&bad_size)
            .expect_err("size mismatch should be refused");
        assert!(matches!(
            bad_size_err,
            ParameterGolfDataError::ShardSizeMismatch { .. }
        ));
    }

    #[test]
    fn dataset_bundle_requires_contiguous_train_prefix_and_fixed_validation_split() {
        let temp = TempDirGuard::new("bundle");
        write_shard(&temp.path.join("fineweb_train_000000.bin"), &[1, 2, 3]);
        write_shard(&temp.path.join("fineweb_train_000001.bin"), &[4, 5, 6, 7]);
        write_shard(&temp.path.join("fineweb_val_000000.bin"), &[8, 9, 10]);

        let bundle = parameter_golf_dataset_bundle_from_local_dir(
            DatasetKey::new("dataset://parameter-golf/fineweb-sp1024", "2026.03.18"),
            &temp.path,
            "sp1024",
            sample_tokenizer(),
            "data/tokenizers/fineweb_1024_bpe.model",
            Some(1),
        )
        .expect("bundle should validate");

        assert_eq!(
            bundle.manifest.record_encoding,
            DatasetRecordEncoding::TokenIdsLeU16
        );
        assert_eq!(bundle.train_shards.len(), 1);
        assert_eq!(bundle.validation_shards.len(), 1);
        assert_eq!(
            bundle
                .manifest
                .metadata
                .get("parameter_golf_validation_identity")
                .and_then(Value::as_str),
            Some(PARAMETER_GOLF_VALIDATION_IDENTITY)
        );
        assert_eq!(
            bundle
                .manifest
                .metadata
                .get("parameter_golf_train_selected_shards")
                .and_then(Value::as_u64),
            Some(1)
        );
        assert_eq!(
            bundle
                .manifest
                .split(PARAMETER_GOLF_TRAIN_SPLIT_NAME)
                .expect("train split should exist")
                .shards[0]
                .token_count,
            3
        );
    }

    #[test]
    fn dataset_bundle_refuses_noncontiguous_train_shards() {
        let temp = TempDirGuard::new("gap");
        write_shard(&temp.path.join("fineweb_train_000001.bin"), &[1, 2, 3]);
        write_shard(&temp.path.join("fineweb_val_000000.bin"), &[8, 9, 10]);

        let err = parameter_golf_dataset_bundle_from_local_dir(
            DatasetKey::new("dataset://parameter-golf/fineweb-sp1024", "2026.03.18"),
            &temp.path,
            "sp1024",
            sample_tokenizer(),
            "data/tokenizers/fineweb_1024_bpe.model",
            None,
        )
        .expect_err("non-contiguous train shards should be refused");
        assert!(matches!(
            err,
            ParameterGolfDataError::NonContiguousShardIndices { .. }
        ));
    }

    #[test]
    fn shard_token_slice_loads_requested_range() {
        let temp = TempDirGuard::new("slice");
        let shard = temp.path.join("fineweb_train_000000.bin");
        write_shard(&shard, &[7, 8, 9, 10, 11]);

        let slice = load_parameter_golf_shard_token_slice(&shard, 1, 3).expect("slice should load");
        assert_eq!(slice, vec![8, 9, 10]);
    }

    #[test]
    fn materialize_token_window_concatenates_ordered_spans() {
        let temp = TempDirGuard::new("materialize");
        write_shard(&temp.path.join("fineweb_train_000000.bin"), &[1, 2, 3]);
        write_shard(&temp.path.join("fineweb_train_000001.bin"), &[4, 5]);
        write_shard(&temp.path.join("fineweb_val_000000.bin"), &[6, 7, 8, 9]);
        let bundle = parameter_golf_dataset_bundle_from_local_dir(
            DatasetKey::new("dataset://parameter-golf/fineweb-sp1024", "2026.03.18"),
            &temp.path,
            "sp1024",
            sample_tokenizer(),
            "data/tokenizers/fineweb_1024_bpe.model",
            None,
        )
        .expect("bundle should validate");

        let contract = ParameterGolfTokenStreamContract::new(
            bundle.manifest.key.clone(),
            PARAMETER_GOLF_TRAIN_SPLIT_NAME,
        )
        .with_mode(DatasetIterationMode::Repeat);
        let cursor = ParameterGolfTokenStreamCursor::new(PARAMETER_GOLF_TRAIN_SPLIT_NAME);
        let first = contract
            .plan_window(&bundle.manifest, &cursor, 4)
            .expect("first window should validate")
            .expect("first window should exist");
        let second = contract
            .plan_window(&bundle.manifest, &first.end_cursor, 4)
            .expect("second window should validate")
            .expect("second window should exist");

        let first_tokens =
            materialize_parameter_golf_token_window(&bundle, &first).expect("first tokens");
        let second_tokens =
            materialize_parameter_golf_token_window(&bundle, &second).expect("second tokens");

        assert_eq!(first_tokens, vec![1, 2, 3, 4]);
        assert_eq!(second_tokens, vec![5, 1, 2, 3]);
    }

    #[test]
    fn token_stream_contract_replays_contiguous_windows_and_wraps_deterministically() {
        let temp = TempDirGuard::new("stream");
        write_shard(&temp.path.join("fineweb_train_000000.bin"), &[1, 2, 3]);
        write_shard(&temp.path.join("fineweb_train_000001.bin"), &[4, 5]);
        write_shard(&temp.path.join("fineweb_val_000000.bin"), &[6, 7, 8, 9]);
        let bundle = parameter_golf_dataset_bundle_from_local_dir(
            DatasetKey::new("dataset://parameter-golf/fineweb-sp1024", "2026.03.18"),
            &temp.path,
            "sp1024",
            sample_tokenizer(),
            "data/tokenizers/fineweb_1024_bpe.model",
            None,
        )
        .expect("bundle should validate");

        let contract = ParameterGolfTokenStreamContract::new(
            bundle.manifest.key.clone(),
            PARAMETER_GOLF_TRAIN_SPLIT_NAME,
        )
        .with_mode(DatasetIterationMode::Repeat);
        let cursor = ParameterGolfTokenStreamCursor::new(PARAMETER_GOLF_TRAIN_SPLIT_NAME);
        let first = contract
            .plan_window(&bundle.manifest, &cursor, 4)
            .expect("first window should validate")
            .expect("first window should exist");
        assert_eq!(first.spans.len(), 2);
        assert_eq!(first.spans[0].shard_key, "fineweb_train_000000");
        assert_eq!(first.spans[0].token_count, 3);
        assert_eq!(first.spans[1].shard_key, "fineweb_train_000001");
        assert_eq!(first.spans[1].token_count, 1);

        let second = contract
            .plan_window(&bundle.manifest, &first.end_cursor, 4)
            .expect("second window should validate")
            .expect("second window should exist");
        assert_eq!(second.spans.len(), 2);
        assert_eq!(second.spans[0].shard_key, "fineweb_train_000001");
        assert_eq!(second.spans[0].start_token_offset, 1);
        assert_eq!(second.spans[0].token_count, 1);
        assert_eq!(second.spans[1].shard_key, "fineweb_train_000000");
        assert_eq!(second.spans[1].token_count, 3);
        assert_eq!(second.end_cursor.cycle, 1);
        assert_eq!(second.end_cursor.emitted_tokens, 8);
    }

    #[derive(Deserialize)]
    struct OracleParityFixture {
        seq_len: usize,
        sentencepiece_entries: Vec<SentencePieceFixtureEntry>,
        validation_shards: Vec<ValidationShardFixture>,
        loss_fixture: LossFixture,
        oracles: BTreeMap<String, OracleFixture>,
    }

    #[derive(Deserialize)]
    struct SentencePieceFixtureEntry {
        token_id: u32,
        piece: String,
        kind: String,
    }

    #[derive(Deserialize)]
    struct ValidationShardFixture {
        file_name: String,
        tokens: Vec<u16>,
        file_hex: String,
    }

    #[derive(Deserialize)]
    struct LossFixture {
        prev_token_ids: Vec<u32>,
        target_token_ids: Vec<u32>,
        logits: Vec<Vec<f64>>,
    }

    #[derive(Deserialize)]
    struct OracleFixture {
        validation_tokens: Vec<u16>,
        luts: OracleLuts,
        val_loss: f64,
        byte_count: u64,
        val_bpb: f64,
    }

    #[derive(Deserialize)]
    struct OracleLuts {
        base_bytes_lut: Vec<i16>,
        has_leading_space_lut: Vec<bool>,
        is_boundary_token_lut: Vec<bool>,
    }

    fn load_oracle_parity_fixture() -> OracleParityFixture {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/parameter_golf/parity/parameter_golf_oracle_parity_fixture.json");
        serde_json::from_slice(&fs::read(path).expect("fixture should exist"))
            .expect("fixture json should deserialize")
    }

    fn token_kind_from_fixture(kind: &str) -> ParameterGolfSentencePieceTokenKind {
        match kind {
            "normal" => ParameterGolfSentencePieceTokenKind::Normal,
            "byte" => ParameterGolfSentencePieceTokenKind::Byte,
            "control" => ParameterGolfSentencePieceTokenKind::Control,
            "unknown" => ParameterGolfSentencePieceTokenKind::Unknown,
            "unused" => ParameterGolfSentencePieceTokenKind::Unused,
            other => panic!("unknown fixture token kind `{other}`"),
        }
    }

    fn mean_cross_entropy_from_logits(logits: &[Vec<f64>], targets: &[u32]) -> f64 {
        let mut total = 0.0_f64;
        for (row, &target) in logits.iter().zip(targets.iter()) {
            let max_logit = row.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            let logsumexp = max_logit
                + row
                    .iter()
                    .map(|value| (*value - max_logit).exp())
                    .sum::<f64>()
                    .ln();
            total += logsumexp - row[target as usize];
        }
        total / targets.len() as f64
    }

    #[test]
    fn sentencepiece_luts_match_current_piece_and_boundary_rules() {
        let luts = ParameterGolfSentencePieceByteLuts::build(
            8,
            &[
                ParameterGolfSentencePieceTokenEntry::new(
                    0,
                    "<unk>",
                    ParameterGolfSentencePieceTokenKind::Unknown,
                ),
                ParameterGolfSentencePieceTokenEntry::new(
                    1,
                    "<s>",
                    ParameterGolfSentencePieceTokenKind::Control,
                ),
                ParameterGolfSentencePieceTokenEntry::new(
                    2,
                    "▁hello",
                    ParameterGolfSentencePieceTokenKind::Normal,
                ),
                ParameterGolfSentencePieceTokenEntry::new(
                    3,
                    "world",
                    ParameterGolfSentencePieceTokenKind::Normal,
                ),
                ParameterGolfSentencePieceTokenEntry::new(
                    4,
                    "<0x41>",
                    ParameterGolfSentencePieceTokenKind::Byte,
                ),
                ParameterGolfSentencePieceTokenEntry::new(
                    5,
                    "<unused>",
                    ParameterGolfSentencePieceTokenKind::Unused,
                ),
            ],
        )
        .expect("luts should build");

        assert_eq!(luts.table_size(), 8);
        assert_eq!(luts.base_bytes_lut[0], 0);
        assert!(luts.is_boundary_token_lut[0]);
        assert_eq!(luts.base_bytes_lut[2], 5);
        assert!(luts.has_leading_space_lut[2]);
        assert!(!luts.is_boundary_token_lut[2]);
        assert_eq!(luts.base_bytes_lut[3], 5);
        assert!(!luts.has_leading_space_lut[3]);
        assert_eq!(luts.base_bytes_lut[4], 1);
        assert!(!luts.is_boundary_token_lut[4]);
        assert!(luts.is_boundary_token_lut[5]);
        assert!(luts.is_boundary_token_lut[7]);
    }

    #[test]
    fn sentencepiece_byte_accounting_only_adds_leading_space_after_non_boundary_tokens() {
        let luts = ParameterGolfSentencePieceByteLuts::build(
            6,
            &[
                ParameterGolfSentencePieceTokenEntry::new(
                    0,
                    "<unk>",
                    ParameterGolfSentencePieceTokenKind::Unknown,
                ),
                ParameterGolfSentencePieceTokenEntry::new(
                    1,
                    "<s>",
                    ParameterGolfSentencePieceTokenKind::Control,
                ),
                ParameterGolfSentencePieceTokenEntry::new(
                    2,
                    "▁hello",
                    ParameterGolfSentencePieceTokenKind::Normal,
                ),
                ParameterGolfSentencePieceTokenEntry::new(
                    3,
                    "world",
                    ParameterGolfSentencePieceTokenKind::Normal,
                ),
                ParameterGolfSentencePieceTokenEntry::new(
                    4,
                    "<0x41>",
                    ParameterGolfSentencePieceTokenKind::Byte,
                ),
            ],
        )
        .expect("luts should build");

        let byte_count = luts
            .count_target_bytes(&[1, 3, 4, 0], &[2, 2, 2, 3])
            .expect("byte count should compute");
        assert_eq!(byte_count, 5 + 6 + 6 + 5);
    }

    #[test]
    fn sentencepiece_bpb_formula_matches_reference_components() {
        let luts = ParameterGolfSentencePieceByteLuts::build(
            4,
            &[
                ParameterGolfSentencePieceTokenEntry::new(
                    0,
                    "<s>",
                    ParameterGolfSentencePieceTokenKind::Control,
                ),
                ParameterGolfSentencePieceTokenEntry::new(
                    1,
                    "▁ab",
                    ParameterGolfSentencePieceTokenKind::Normal,
                ),
                ParameterGolfSentencePieceTokenEntry::new(
                    2,
                    "cd",
                    ParameterGolfSentencePieceTokenKind::Normal,
                ),
            ],
        )
        .expect("luts should build");

        let mean_nll = std::f64::consts::LN_2;
        let bpb = luts
            .bits_per_byte_from_mean_nll(mean_nll, &[0, 1], &[1, 2])
            .expect("bpb should compute");
        assert!((bpb - (2.0 / 4.0)).abs() < 1e-12);
    }

    #[test]
    fn frozen_oracle_fixture_matches_python_and_mlx_reference_paths() {
        let fixture = load_oracle_parity_fixture();
        let temp = TempDirGuard::new("oracle-fixture");
        let mut paths = Vec::new();
        for shard in &fixture.validation_shards {
            let path = temp.path.join(&shard.file_name);
            let bytes = hex::decode(&shard.file_hex).expect("fixture hex should decode");
            fs::write(&path, bytes).expect("fixture shard should write");
            assert_eq!(
                load_parameter_golf_shard_tokens(&path).expect("shard should load"),
                shard.tokens
            );
            paths.push(path);
        }

        let validation_tokens =
            load_parameter_golf_validation_tokens_from_paths(paths.as_slice(), fixture.seq_len)
                .expect("validation tokens should load");
        for oracle in fixture.oracles.values() {
            assert_eq!(validation_tokens, oracle.validation_tokens);
        }

        let entries = fixture
            .sentencepiece_entries
            .iter()
            .map(|entry| {
                ParameterGolfSentencePieceTokenEntry::new(
                    entry.token_id,
                    entry.piece.clone(),
                    token_kind_from_fixture(entry.kind.as_str()),
                )
            })
            .collect::<Vec<_>>();
        let luts = ParameterGolfSentencePieceByteLuts::build(8, entries.as_slice())
            .expect("luts should build");
        for oracle in fixture.oracles.values() {
            assert_eq!(luts.base_bytes_lut, oracle.luts.base_bytes_lut);
            assert_eq!(
                luts.has_leading_space_lut,
                oracle.luts.has_leading_space_lut
            );
            assert_eq!(
                luts.is_boundary_token_lut,
                oracle.luts.is_boundary_token_lut
            );
        }

        let val_loss = mean_cross_entropy_from_logits(
            fixture.loss_fixture.logits.as_slice(),
            fixture.loss_fixture.target_token_ids.as_slice(),
        );
        for oracle in fixture.oracles.values() {
            assert!((val_loss - oracle.val_loss).abs() < 1e-12);
        }

        let byte_count = luts
            .count_target_bytes(
                fixture.loss_fixture.prev_token_ids.as_slice(),
                fixture.loss_fixture.target_token_ids.as_slice(),
            )
            .expect("byte count should compute");
        for oracle in fixture.oracles.values() {
            assert_eq!(byte_count, oracle.byte_count);
        }

        let val_bpb = luts
            .bits_per_byte_from_mean_nll(
                val_loss,
                fixture.loss_fixture.prev_token_ids.as_slice(),
                fixture.loss_fixture.target_token_ids.as_slice(),
            )
            .expect("val_bpb should compute");
        for oracle in fixture.oracles.values() {
            assert!((val_bpb - oracle.val_bpb).abs() < 1e-12);
        }
    }

    #[test]
    fn builtin_sentencepiece_byte_luts_match_frozen_oracle_fixture() {
        let fixture = load_oracle_parity_fixture();
        let builtin = builtin_parameter_golf_sentencepiece_byte_luts()
            .expect("builtin oracle byte luts should load");
        for oracle in fixture.oracles.values() {
            assert_eq!(
                &builtin.base_bytes_lut[..oracle.luts.base_bytes_lut.len()],
                oracle.luts.base_bytes_lut.as_slice()
            );
            assert_eq!(
                &builtin.has_leading_space_lut[..oracle.luts.has_leading_space_lut.len()],
                oracle.luts.has_leading_space_lut.as_slice()
            );
            assert_eq!(
                &builtin.is_boundary_token_lut[..oracle.luts.is_boundary_token_lut.len()],
                oracle.luts.is_boundary_token_lut.as_slice()
            );
        }
    }

    #[test]
    fn sentencepiece_byte_luts_from_vocab_path_tracks_real_vocab_rows() {
        let temp = TempDirGuard::new("sentencepiece-vocab");
        let vocab_path = temp.path.join("fineweb_1024_bpe.vocab");
        fs::write(
            &vocab_path,
            "<pad>\t0\n<s>\t0\n</s>\t0\n<unk>\t0\n<0x41>\t0\n▁hello\t-1\nworld\t-2\n<unused0>\t-3\n",
        )
        .expect("fixture vocab should write");

        let luts = parameter_golf_sentencepiece_byte_luts_from_vocab_path(&vocab_path)
            .expect("vocab-derived byte luts should load");

        assert_eq!(luts.table_size(), 8);
        assert!(luts.is_boundary_token_lut[0]);
        assert!(luts.is_boundary_token_lut[1]);
        assert!(luts.is_boundary_token_lut[2]);
        assert!(luts.is_boundary_token_lut[3]);
        assert_eq!(luts.base_bytes_lut[4], 1);
        assert!(!luts.is_boundary_token_lut[4]);
        assert_eq!(luts.base_bytes_lut[5], 5);
        assert!(luts.has_leading_space_lut[5]);
        assert_eq!(luts.base_bytes_lut[6], 5);
        assert!(luts.is_boundary_token_lut[7]);
    }

    #[test]
    fn sentencepiece_byte_luts_from_tokenizer_path_requires_sibling_vocab() {
        let temp = TempDirGuard::new("sentencepiece-model");
        let tokenizer_path = temp.path.join("fineweb_1024_bpe.model");
        fs::write(&tokenizer_path, b"model").expect("fixture model should write");

        let error = parameter_golf_sentencepiece_byte_luts_from_tokenizer_path(&tokenizer_path)
            .expect_err("missing vocab should refuse");
        assert!(matches!(
            error,
            ParameterGolfDataError::MissingTokenizerVocab { .. }
        ));
    }
}
