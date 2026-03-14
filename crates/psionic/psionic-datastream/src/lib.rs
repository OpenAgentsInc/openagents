//! Resumable streamed dataset and checkpoint delivery contracts for Psionic.

use psionic_runtime::{
    RuntimeDispatchPlan, RuntimeDispatchPolicy, RuntimeOptimizationBenchmark, RuntimeWorkClass,
    RuntimeWorkItem, benchmark_dispatch_plan,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// High-level subject being delivered over the data plane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DatastreamSubjectKind {
    /// Tokenized corpus or dataset shard delivery.
    TokenizedCorpus,
    /// Evaluation bundle or harness artifact delivery.
    EvalBundle,
    /// Model checkpoint or optimizer-state delivery.
    Checkpoint,
    /// Served-artifact delivery such as sharded weights.
    ServedArtifact,
    /// Adapter or LoRA package delivery.
    AdapterPackage,
}

impl DatastreamSubjectKind {
    /// Returns a stable machine-checkable label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::TokenizedCorpus => "tokenized_corpus",
            Self::EvalBundle => "eval_bundle",
            Self::Checkpoint => "checkpoint",
            Self::ServedArtifact => "served_artifact",
            Self::AdapterPackage => "adapter_package",
        }
    }
}

/// Logical encoding of the streamed payload.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DatastreamEncoding {
    /// Opaque binary payload.
    RawBinary,
    /// JSONL records.
    Jsonl,
    /// Little-endian token ID stream.
    TokenIdsLeU32,
    /// Safetensors payload.
    Safetensors,
    /// Tar archive.
    TarArchive,
}

impl DatastreamEncoding {
    /// Returns a stable machine-checkable label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RawBinary => "raw_binary",
            Self::Jsonl => "jsonl",
            Self::TokenIdsLeU32 => "token_ids_le_u32",
            Self::Safetensors => "safetensors",
            Self::TarArchive => "tar_archive",
        }
    }
}

/// Compression codec applied to the payload, when one exists.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DatastreamCompression {
    /// Gzip-compressed payload.
    Gzip,
    /// Zstd-compressed payload.
    Zstd,
}

impl DatastreamCompression {
    /// Returns a stable machine-checkable label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Gzip => "gzip",
            Self::Zstd => "zstd",
        }
    }
}

/// Dataset-scoped identity carried alongside one stream.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DatastreamDatasetBinding {
    /// Stable dataset identifier.
    pub dataset_id: String,
    /// Optional split name such as `train` or `validation`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub split: Option<String>,
    /// Optional shard identity within the split.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shard_key: Option<String>,
}

impl DatastreamDatasetBinding {
    /// Creates a dataset binding from one stable dataset ID.
    #[must_use]
    pub fn new(dataset_id: impl Into<String>) -> Self {
        Self {
            dataset_id: dataset_id.into(),
            split: None,
            shard_key: None,
        }
    }

    /// Attaches the split name.
    #[must_use]
    pub fn with_split(mut self, split: impl Into<String>) -> Self {
        self.split = Some(split.into());
        self
    }

    /// Attaches the shard key.
    #[must_use]
    pub fn with_shard_key(mut self, shard_key: impl Into<String>) -> Self {
        self.shard_key = Some(shard_key.into());
        self
    }
}

/// Checkpoint-scoped identity carried alongside one stream.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DatastreamCheckpointBinding {
    /// Stable checkpoint family such as `serve.tensor` or `train.decoder`.
    pub checkpoint_family: String,
    /// Optional stable checkpoint reference.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checkpoint_ref: Option<String>,
    /// Optional logical step or epoch marker.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub step: Option<u64>,
}

impl DatastreamCheckpointBinding {
    /// Creates a checkpoint binding from one stable family.
    #[must_use]
    pub fn new(checkpoint_family: impl Into<String>) -> Self {
        Self {
            checkpoint_family: checkpoint_family.into(),
            checkpoint_ref: None,
            step: None,
        }
    }

    /// Attaches the checkpoint reference.
    #[must_use]
    pub fn with_checkpoint_ref(mut self, checkpoint_ref: impl Into<String>) -> Self {
        self.checkpoint_ref = Some(checkpoint_ref.into());
        self
    }

    /// Attaches the checkpoint step.
    #[must_use]
    pub const fn with_step(mut self, step: u64) -> Self {
        self.step = Some(step);
        self
    }
}

/// One stable chunk boundary inside a streamed payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DatastreamChunkDescriptor {
    /// Stable chunk index inside the manifest.
    pub index: usize,
    /// Starting byte offset for this chunk.
    pub offset: u64,
    /// Number of bytes carried by the chunk.
    pub length: usize,
    /// Stable digest over the chunk payload only.
    pub chunk_digest: String,
}

/// Full manifest for one resumable data-plane stream.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DatastreamManifest {
    /// Stable stream identifier.
    pub stream_id: String,
    /// High-level subject being delivered.
    pub subject: DatastreamSubjectKind,
    /// Stable digest over the full payload.
    pub object_digest: String,
    /// Total payload size in bytes.
    pub total_bytes: u64,
    /// Target chunk size for the stream.
    pub chunk_bytes: usize,
    /// Payload encoding.
    pub encoding: DatastreamEncoding,
    /// Optional compression codec.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compression: Option<DatastreamCompression>,
    /// Optional provenance digest for the payload source.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provenance_digest: Option<String>,
    /// Optional dataset binding.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dataset_binding: Option<DatastreamDatasetBinding>,
    /// Optional checkpoint binding.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checkpoint_binding: Option<DatastreamCheckpointBinding>,
    /// Stable chunk descriptors in transfer order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub chunks: Vec<DatastreamChunkDescriptor>,
}

impl DatastreamManifest {
    /// Builds a manifest directly from one payload and chunk size.
    #[must_use]
    pub fn from_bytes(
        stream_id: impl Into<String>,
        subject: DatastreamSubjectKind,
        payload: &[u8],
        chunk_bytes: usize,
        encoding: DatastreamEncoding,
    ) -> Self {
        let chunk_bytes = chunk_bytes.max(1);
        let mut chunks = Vec::new();
        let mut offset = 0_u64;
        for (index, chunk) in payload.chunks(chunk_bytes).enumerate() {
            chunks.push(DatastreamChunkDescriptor {
                index,
                offset,
                length: chunk.len(),
                chunk_digest: digest_bytes(chunk),
            });
            offset = offset.saturating_add(len_to_u64(chunk.len()));
        }
        Self {
            stream_id: stream_id.into(),
            subject,
            object_digest: digest_bytes(payload),
            total_bytes: offset,
            chunk_bytes,
            encoding,
            compression: None,
            provenance_digest: None,
            dataset_binding: None,
            checkpoint_binding: None,
            chunks,
        }
    }

    /// Attaches the compression codec.
    #[must_use]
    pub const fn with_compression(mut self, compression: DatastreamCompression) -> Self {
        self.compression = Some(compression);
        self
    }

    /// Attaches the provenance digest.
    #[must_use]
    pub fn with_provenance_digest(mut self, provenance_digest: impl Into<String>) -> Self {
        self.provenance_digest = Some(provenance_digest.into());
        self
    }

    /// Attaches dataset identity for the stream.
    #[must_use]
    pub fn with_dataset_binding(mut self, dataset_binding: DatastreamDatasetBinding) -> Self {
        self.dataset_binding = Some(dataset_binding);
        self
    }

    /// Attaches checkpoint identity for the stream.
    #[must_use]
    pub fn with_checkpoint_binding(
        mut self,
        checkpoint_binding: DatastreamCheckpointBinding,
    ) -> Self {
        self.checkpoint_binding = Some(checkpoint_binding);
        self
    }

    /// Returns a stable digest over the manifest contract.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(b"psionic_datastream_manifest|");
        hasher.update(self.stream_id.as_bytes());
        hasher.update(b"|");
        hasher.update(self.subject.as_str().as_bytes());
        hasher.update(b"|");
        hasher.update(self.object_digest.as_bytes());
        hasher.update(b"|");
        hasher.update(self.total_bytes.to_string().as_bytes());
        hasher.update(b"|");
        hasher.update(self.chunk_bytes.to_string().as_bytes());
        hasher.update(b"|");
        hasher.update(self.encoding.as_str().as_bytes());
        hasher.update(b"|");
        hasher.update(self.compression.map_or(b"".as_slice(), |compression| {
            compression.as_str().as_bytes()
        }));
        hasher.update(b"|");
        hasher.update(
            self.provenance_digest
                .as_deref()
                .unwrap_or_default()
                .as_bytes(),
        );
        if let Some(dataset_binding) = &self.dataset_binding {
            hasher.update(b"|dataset|");
            hasher.update(dataset_binding.dataset_id.as_bytes());
            hasher.update(b"|");
            hasher.update(
                dataset_binding
                    .split
                    .as_deref()
                    .unwrap_or_default()
                    .as_bytes(),
            );
            hasher.update(b"|");
            hasher.update(
                dataset_binding
                    .shard_key
                    .as_deref()
                    .unwrap_or_default()
                    .as_bytes(),
            );
        }
        if let Some(checkpoint_binding) = &self.checkpoint_binding {
            hasher.update(b"|checkpoint|");
            hasher.update(checkpoint_binding.checkpoint_family.as_bytes());
            hasher.update(b"|");
            hasher.update(
                checkpoint_binding
                    .checkpoint_ref
                    .as_deref()
                    .unwrap_or_default()
                    .as_bytes(),
            );
            hasher.update(b"|");
            if let Some(step) = checkpoint_binding.step {
                hasher.update(step.to_string().as_bytes());
            }
        }
        for chunk in &self.chunks {
            hasher.update(b"|chunk|");
            hasher.update(chunk.index.to_string().as_bytes());
            hasher.update(b"|");
            hasher.update(chunk.offset.to_string().as_bytes());
            hasher.update(b"|");
            hasher.update(chunk.length.to_string().as_bytes());
            hasher.update(b"|");
            hasher.update(chunk.chunk_digest.as_bytes());
        }
        hex::encode(hasher.finalize())
    }

    /// Returns the bytes committed for a resume cursor at the provided chunk boundary.
    pub fn bytes_committed_for_chunk_count(
        &self,
        next_chunk_index: usize,
    ) -> Result<u64, DatastreamTransferError> {
        if next_chunk_index > self.chunks.len() {
            return Err(DatastreamTransferError::ResumeCursorOutOfRange {
                next_chunk_index,
                chunk_count: self.chunks.len(),
            });
        }
        if next_chunk_index == self.chunks.len() {
            Ok(self.total_bytes)
        } else {
            Ok(self.chunks[next_chunk_index].offset)
        }
    }

    /// Returns a compact manifest reference suitable for other crates.
    #[must_use]
    pub fn manifest_ref(&self) -> DatastreamManifestRef {
        DatastreamManifestRef {
            stream_id: self.stream_id.clone(),
            manifest_digest: self.stable_digest(),
            subject: self.subject,
            object_digest: self.object_digest.clone(),
            total_bytes: self.total_bytes,
            chunk_count: self.chunks.len(),
            chunk_bytes: self.chunk_bytes,
            encoding: self.encoding,
            compression: self.compression,
            provenance_digest: self.provenance_digest.clone(),
            dataset_binding: self.dataset_binding.clone(),
            checkpoint_binding: self.checkpoint_binding.clone(),
        }
    }

    /// Returns a worker dispatch plan for chunk delivery under the Psionic data-plane policy.
    #[must_use]
    pub fn recommended_dispatch_plan(&self, max_workers: usize) -> RuntimeDispatchPlan {
        let items = self
            .chunks
            .iter()
            .map(|chunk| {
                RuntimeWorkItem::new(
                    RuntimeWorkClass::DatastreamChunk,
                    chunk.length.div_ceil(self.chunk_bytes.max(1)),
                    len_to_u64(chunk.length),
                )
            })
            .collect::<Vec<_>>();
        RuntimeDispatchPlan::plan(
            RuntimeDispatchPolicy::data_plane_default(max_workers),
            &items,
        )
    }

    /// Returns a repeatable benchmark for chunk-delivery scheduling.
    #[must_use]
    pub fn dispatch_benchmark(&self, max_workers: usize) -> RuntimeOptimizationBenchmark {
        let items = self
            .chunks
            .iter()
            .map(|chunk| {
                RuntimeWorkItem::new(
                    RuntimeWorkClass::DatastreamChunk,
                    chunk.length.div_ceil(self.chunk_bytes.max(1)),
                    len_to_u64(chunk.length),
                )
            })
            .collect::<Vec<_>>();
        benchmark_dispatch_plan(
            "datastream_chunk_delivery",
            RuntimeDispatchPolicy::data_plane_default(max_workers),
            &items,
        )
    }

    /// Validates that a payload still matches this manifest.
    pub fn validate_payload(&self, payload: &[u8]) -> Result<(), DatastreamTransferError> {
        let actual_total_bytes = len_to_u64(payload.len());
        if actual_total_bytes != self.total_bytes {
            return Err(DatastreamTransferError::ManifestTotalBytesMismatch {
                expected: self.total_bytes,
                actual: actual_total_bytes,
            });
        }
        let actual_object_digest = digest_bytes(payload);
        if actual_object_digest != self.object_digest {
            return Err(DatastreamTransferError::ManifestObjectDigestMismatch {
                expected: self.object_digest.clone(),
                actual: actual_object_digest,
            });
        }
        for chunk in &self.chunks {
            let (start, end) = chunk_bounds(chunk)?;
            if end > payload.len() {
                return Err(DatastreamTransferError::PayloadSliceOutOfRange {
                    index: chunk.index,
                    end,
                    payload_len: payload.len(),
                });
            }
            let actual = &payload[start..end];
            if actual.len() != chunk.length {
                return Err(DatastreamTransferError::ChunkLengthMismatch {
                    index: chunk.index,
                    expected: chunk.length,
                    actual: actual.len(),
                });
            }
            let actual_digest = digest_bytes(actual);
            if actual_digest != chunk.chunk_digest {
                return Err(DatastreamTransferError::ChunkDigestMismatch {
                    index: chunk.index,
                    expected: chunk.chunk_digest.clone(),
                    actual: actual_digest,
                });
            }
        }
        Ok(())
    }
}

/// Compact manifest summary that can be embedded in other contracts.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DatastreamManifestRef {
    /// Stable stream identifier.
    pub stream_id: String,
    /// Stable manifest digest.
    pub manifest_digest: String,
    /// High-level subject of the stream.
    pub subject: DatastreamSubjectKind,
    /// Stable digest over the full payload.
    pub object_digest: String,
    /// Total payload size in bytes.
    pub total_bytes: u64,
    /// Number of chunks in the stream.
    pub chunk_count: usize,
    /// Target chunk size for the stream.
    pub chunk_bytes: usize,
    /// Payload encoding.
    pub encoding: DatastreamEncoding,
    /// Optional compression codec.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compression: Option<DatastreamCompression>,
    /// Optional provenance digest.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provenance_digest: Option<String>,
    /// Optional dataset binding.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dataset_binding: Option<DatastreamDatasetBinding>,
    /// Optional checkpoint binding.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checkpoint_binding: Option<DatastreamCheckpointBinding>,
}

/// Resume cursor describing the next chunk boundary a client expects.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DatastreamResumeCursor {
    /// The next chunk index expected by the client.
    pub next_chunk_index: usize,
    /// Number of bytes already durably committed by the client.
    pub bytes_committed: u64,
}

/// Open request for one resumable stream session.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DatastreamOpenRequest {
    /// Stable manifest digest the client expects.
    pub manifest_digest: String,
    /// Resume cursor for restart-safe continuation.
    pub cursor: DatastreamResumeCursor,
    /// Maximum number of chunks the server should hand out per window.
    pub max_chunks_in_flight: usize,
}

impl DatastreamOpenRequest {
    /// Creates a new open request for one manifest digest.
    #[must_use]
    pub fn new(manifest_digest: impl Into<String>) -> Self {
        Self {
            manifest_digest: manifest_digest.into(),
            cursor: DatastreamResumeCursor::default(),
            max_chunks_in_flight: 1,
        }
    }

    /// Attaches an explicit resume cursor.
    #[must_use]
    pub const fn with_resume_cursor(mut self, cursor: DatastreamResumeCursor) -> Self {
        self.cursor = cursor;
        self
    }

    /// Attaches an explicit in-flight chunk budget.
    #[must_use]
    pub fn with_max_chunks_in_flight(mut self, max_chunks_in_flight: usize) -> Self {
        self.max_chunks_in_flight = max_chunks_in_flight.max(1);
        self
    }
}

/// One payload chunk sent over the data plane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DatastreamChunk {
    /// Stable stream identifier.
    pub stream_id: String,
    /// Stable manifest digest.
    pub manifest_digest: String,
    /// Stable chunk index.
    pub index: usize,
    /// Starting payload offset for the chunk.
    pub offset: u64,
    /// Chunk payload bytes.
    pub payload: Vec<u8>,
    /// Stable digest over the chunk payload.
    pub chunk_digest: String,
}

/// Restart-safe client progress for one stream.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DatastreamProgress {
    /// Stable manifest digest.
    pub manifest_digest: String,
    /// Next chunk index expected by the client.
    pub next_chunk_index: usize,
    /// Number of bytes durably committed by the client.
    pub bytes_committed: u64,
    /// Total stream bytes from the manifest.
    pub total_bytes: u64,
    /// Whether the client has received every chunk.
    pub complete: bool,
}

impl DatastreamProgress {
    /// Returns the progress as a resumable cursor.
    #[must_use]
    pub const fn cursor(&self) -> DatastreamResumeCursor {
        DatastreamResumeCursor {
            next_chunk_index: self.next_chunk_index,
            bytes_committed: self.bytes_committed,
        }
    }
}

/// Final verified delivery receipt for one stream.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DatastreamDeliveryReceipt {
    /// Compact manifest summary for the delivered stream.
    pub manifest: DatastreamManifestRef,
    /// Bytes durably delivered to the client.
    pub bytes_delivered: u64,
    /// Number of chunks durably delivered to the client.
    pub chunks_delivered: usize,
    /// Whether the client resumed from a non-zero cursor.
    pub resumed: bool,
}

/// Transfer or verification failure for one streamed payload.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum DatastreamTransferError {
    /// The requested manifest digest does not match the server-side manifest.
    #[error("datastream open request expected manifest `{expected}` but server exposed `{actual}`")]
    OpenManifestMismatch { expected: String, actual: String },
    /// The resume cursor points past the known chunk count.
    #[error(
        "resume cursor chunk index `{next_chunk_index}` is out of range for chunk count `{chunk_count}`"
    )]
    ResumeCursorOutOfRange {
        next_chunk_index: usize,
        chunk_count: usize,
    },
    /// The resume cursor byte count does not match the chunk boundary.
    #[error("resume cursor expects `{expected}` committed bytes but caller supplied `{actual}`")]
    ResumeBytesMismatch { expected: u64, actual: u64 },
    /// The manifest total bytes no longer matches the payload.
    #[error("manifest expected `{expected}` total bytes but payload had `{actual}`")]
    ManifestTotalBytesMismatch { expected: u64, actual: u64 },
    /// The manifest object digest no longer matches the payload.
    #[error("manifest expected object digest `{expected}` but payload digested to `{actual}`")]
    ManifestObjectDigestMismatch { expected: String, actual: String },
    /// The payload slice described by the manifest extends past the payload.
    #[error("manifest chunk `{index}` ended at `{end}` but payload length was `{payload_len}`")]
    PayloadSliceOutOfRange {
        index: usize,
        end: usize,
        payload_len: usize,
    },
    /// The chunk arrived out of order.
    #[error("expected chunk `{expected}` but received `{actual}`")]
    ChunkOutOfOrder { expected: usize, actual: usize },
    /// The chunk offset does not match the manifest.
    #[error("chunk `{index}` expected offset `{expected}` but received `{actual}`")]
    ChunkOffsetMismatch {
        index: usize,
        expected: u64,
        actual: u64,
    },
    /// The chunk length does not match the manifest.
    #[error("chunk `{index}` expected length `{expected}` but received `{actual}`")]
    ChunkLengthMismatch {
        index: usize,
        expected: usize,
        actual: usize,
    },
    /// The chunk digest does not match the manifest.
    #[error("chunk `{index}` expected digest `{expected}` but received `{actual}`")]
    ChunkDigestMismatch {
        index: usize,
        expected: String,
        actual: String,
    },
    /// The client attempted to finish before the full stream completed.
    #[error(
        "stream incomplete: next chunk `{next_chunk_index}` but chunk count is `{chunk_count}`"
    )]
    IncompleteStream {
        next_chunk_index: usize,
        chunk_count: usize,
    },
    /// Converting a chunk offset to a local slice failed.
    #[error("chunk `{index}` offset `{offset}` does not fit host usize")]
    ChunkOffsetTooLarge { index: usize, offset: u64 },
    /// The chunk end overflowed usize math.
    #[error("chunk `{index}` end offset overflowed local slicing arithmetic")]
    ChunkEndOverflow { index: usize },
}

/// In-memory server that exposes one manifest-backed payload over resumable windows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InMemoryDatastreamServer {
    manifest: DatastreamManifest,
    payload: Vec<u8>,
}

impl InMemoryDatastreamServer {
    /// Creates an in-memory server from a manifest and payload.
    pub fn new(
        manifest: DatastreamManifest,
        payload: Vec<u8>,
    ) -> Result<Self, DatastreamTransferError> {
        manifest.validate_payload(&payload)?;
        Ok(Self { manifest, payload })
    }

    /// Creates an in-memory server directly from one payload.
    #[must_use]
    pub fn from_bytes(
        stream_id: impl Into<String>,
        subject: DatastreamSubjectKind,
        payload: Vec<u8>,
        chunk_bytes: usize,
        encoding: DatastreamEncoding,
    ) -> Self {
        Self {
            manifest: DatastreamManifest::from_bytes(
                stream_id,
                subject,
                &payload,
                chunk_bytes,
                encoding,
            ),
            payload,
        }
    }

    /// Returns the stable manifest.
    #[must_use]
    pub const fn manifest(&self) -> &DatastreamManifest {
        &self.manifest
    }

    /// Returns a compact manifest reference.
    #[must_use]
    pub fn manifest_ref(&self) -> DatastreamManifestRef {
        self.manifest.manifest_ref()
    }

    /// Opens a resumable stream session for one client.
    pub fn open(
        &self,
        request: DatastreamOpenRequest,
    ) -> Result<DatastreamServerSession<'_>, DatastreamTransferError> {
        let manifest_digest = self.manifest.stable_digest();
        if request.manifest_digest != manifest_digest {
            return Err(DatastreamTransferError::OpenManifestMismatch {
                expected: request.manifest_digest,
                actual: manifest_digest,
            });
        }
        let expected_bytes = self
            .manifest
            .bytes_committed_for_chunk_count(request.cursor.next_chunk_index)?;
        if expected_bytes != request.cursor.bytes_committed {
            return Err(DatastreamTransferError::ResumeBytesMismatch {
                expected: expected_bytes,
                actual: request.cursor.bytes_committed,
            });
        }
        Ok(DatastreamServerSession {
            server: self,
            next_chunk_index: request.cursor.next_chunk_index,
            max_chunks_in_flight: request.max_chunks_in_flight.max(1),
        })
    }

    fn chunk(&self, index: usize) -> Result<Option<DatastreamChunk>, DatastreamTransferError> {
        let Some(descriptor) = self.manifest.chunks.get(index) else {
            return Ok(None);
        };
        let (start, end) = chunk_bounds(descriptor)?;
        if end > self.payload.len() {
            return Err(DatastreamTransferError::PayloadSliceOutOfRange {
                index,
                end,
                payload_len: self.payload.len(),
            });
        }
        Ok(Some(DatastreamChunk {
            stream_id: self.manifest.stream_id.clone(),
            manifest_digest: self.manifest.stable_digest(),
            index,
            offset: descriptor.offset,
            payload: self.payload[start..end].to_vec(),
            chunk_digest: descriptor.chunk_digest.clone(),
        }))
    }
}

/// One resumable server-side streaming session.
#[derive(Debug)]
pub struct DatastreamServerSession<'a> {
    server: &'a InMemoryDatastreamServer,
    next_chunk_index: usize,
    max_chunks_in_flight: usize,
}

impl DatastreamServerSession<'_> {
    /// Returns the next contiguous chunk window and advances the session cursor.
    pub fn next_window(&mut self) -> Result<Vec<DatastreamChunk>, DatastreamTransferError> {
        let mut chunks = Vec::new();
        for _ in 0..self.max_chunks_in_flight {
            let Some(chunk) = self.server.chunk(self.next_chunk_index)? else {
                break;
            };
            self.next_chunk_index = self.next_chunk_index.saturating_add(1);
            chunks.push(chunk);
        }
        Ok(chunks)
    }

    /// Returns the current resumable cursor for the session.
    pub fn cursor(&self) -> Result<DatastreamResumeCursor, DatastreamTransferError> {
        Ok(DatastreamResumeCursor {
            next_chunk_index: self.next_chunk_index,
            bytes_committed: self
                .server
                .manifest
                .bytes_committed_for_chunk_count(self.next_chunk_index)?,
        })
    }
}

/// In-memory client that verifies and materializes one manifest-backed payload.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InMemoryDatastreamClient {
    manifest: DatastreamManifest,
    received: Vec<u8>,
    next_chunk_index: usize,
    resumed: bool,
}

impl InMemoryDatastreamClient {
    /// Creates a fresh client for one manifest.
    #[must_use]
    pub fn new(manifest: DatastreamManifest) -> Self {
        Self {
            manifest,
            received: Vec::new(),
            next_chunk_index: 0,
            resumed: false,
        }
    }

    /// Restores a client from already committed bytes and the next chunk boundary.
    pub fn resume(
        manifest: DatastreamManifest,
        received: Vec<u8>,
        next_chunk_index: usize,
    ) -> Result<Self, DatastreamTransferError> {
        let expected_bytes = manifest.bytes_committed_for_chunk_count(next_chunk_index)?;
        let actual_bytes = len_to_u64(received.len());
        if expected_bytes != actual_bytes {
            return Err(DatastreamTransferError::ResumeBytesMismatch {
                expected: expected_bytes,
                actual: actual_bytes,
            });
        }
        for chunk in manifest.chunks.iter().take(next_chunk_index) {
            let (start, end) = chunk_bounds(chunk)?;
            if end > received.len() {
                return Err(DatastreamTransferError::PayloadSliceOutOfRange {
                    index: chunk.index,
                    end,
                    payload_len: received.len(),
                });
            }
            let actual = &received[start..end];
            let actual_digest = digest_bytes(actual);
            if actual_digest != chunk.chunk_digest {
                return Err(DatastreamTransferError::ChunkDigestMismatch {
                    index: chunk.index,
                    expected: chunk.chunk_digest.clone(),
                    actual: actual_digest,
                });
            }
        }
        Ok(Self {
            manifest,
            received,
            next_chunk_index,
            resumed: next_chunk_index > 0,
        })
    }

    /// Returns current verified progress.
    pub fn progress(&self) -> Result<DatastreamProgress, DatastreamTransferError> {
        Ok(DatastreamProgress {
            manifest_digest: self.manifest.stable_digest(),
            next_chunk_index: self.next_chunk_index,
            bytes_committed: self
                .manifest
                .bytes_committed_for_chunk_count(self.next_chunk_index)?,
            total_bytes: self.manifest.total_bytes,
            complete: self.next_chunk_index == self.manifest.chunks.len(),
        })
    }

    /// Applies one verified chunk to the local materialized payload.
    pub fn apply_chunk(
        &mut self,
        chunk: DatastreamChunk,
    ) -> Result<DatastreamProgress, DatastreamTransferError> {
        if chunk.manifest_digest != self.manifest.stable_digest() {
            return Err(DatastreamTransferError::OpenManifestMismatch {
                expected: self.manifest.stable_digest(),
                actual: chunk.manifest_digest,
            });
        }
        if chunk.index != self.next_chunk_index {
            return Err(DatastreamTransferError::ChunkOutOfOrder {
                expected: self.next_chunk_index,
                actual: chunk.index,
            });
        }
        let Some(descriptor) = self.manifest.chunks.get(chunk.index) else {
            return Err(DatastreamTransferError::ResumeCursorOutOfRange {
                next_chunk_index: chunk.index,
                chunk_count: self.manifest.chunks.len(),
            });
        };
        if chunk.offset != descriptor.offset {
            return Err(DatastreamTransferError::ChunkOffsetMismatch {
                index: chunk.index,
                expected: descriptor.offset,
                actual: chunk.offset,
            });
        }
        if chunk.payload.len() != descriptor.length {
            return Err(DatastreamTransferError::ChunkLengthMismatch {
                index: chunk.index,
                expected: descriptor.length,
                actual: chunk.payload.len(),
            });
        }
        let actual_digest = digest_bytes(&chunk.payload);
        if actual_digest != descriptor.chunk_digest || chunk.chunk_digest != descriptor.chunk_digest
        {
            return Err(DatastreamTransferError::ChunkDigestMismatch {
                index: chunk.index,
                expected: descriptor.chunk_digest.clone(),
                actual: actual_digest,
            });
        }
        self.received.extend_from_slice(&chunk.payload);
        self.next_chunk_index = self.next_chunk_index.saturating_add(1);
        self.progress()
    }

    /// Finalizes the verified stream and returns a delivery receipt.
    pub fn finish(self) -> Result<DatastreamDeliveryReceipt, DatastreamTransferError> {
        if self.next_chunk_index != self.manifest.chunks.len() {
            return Err(DatastreamTransferError::IncompleteStream {
                next_chunk_index: self.next_chunk_index,
                chunk_count: self.manifest.chunks.len(),
            });
        }
        self.manifest.validate_payload(&self.received)?;
        Ok(DatastreamDeliveryReceipt {
            manifest: self.manifest.manifest_ref(),
            bytes_delivered: len_to_u64(self.received.len()),
            chunks_delivered: self.next_chunk_index,
            resumed: self.resumed,
        })
    }

    /// Returns the verified bytes committed so far.
    #[must_use]
    pub fn received_bytes(&self) -> &[u8] {
        &self.received
    }
}

fn digest_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

fn len_to_u64(len: usize) -> u64 {
    match u64::try_from(len) {
        Ok(value) => value,
        Err(_) => u64::MAX,
    }
}

fn chunk_bounds(
    descriptor: &DatastreamChunkDescriptor,
) -> Result<(usize, usize), DatastreamTransferError> {
    let start = match usize::try_from(descriptor.offset) {
        Ok(value) => value,
        Err(_) => {
            return Err(DatastreamTransferError::ChunkOffsetTooLarge {
                index: descriptor.index,
                offset: descriptor.offset,
            });
        }
    };
    let end = match start.checked_add(descriptor.length) {
        Some(value) => value,
        None => {
            return Err(DatastreamTransferError::ChunkEndOverflow {
                index: descriptor.index,
            });
        }
    };
    Ok((start, end))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic, clippy::panic_in_result_fn)]

    use super::{
        DatastreamCheckpointBinding, DatastreamDatasetBinding, DatastreamEncoding,
        DatastreamOpenRequest, DatastreamSubjectKind, DatastreamTransferError,
        InMemoryDatastreamClient, InMemoryDatastreamServer,
    };

    #[test]
    fn dataset_stream_round_trips_with_resume_and_receipt() -> Result<(), Box<dyn std::error::Error>>
    {
        let payload = b"token-0 token-1 token-2 token-3 token-4 token-5 token-6 token-7".to_vec();
        let manifest = super::DatastreamManifest::from_bytes(
            "dataset-train-0001",
            DatastreamSubjectKind::TokenizedCorpus,
            &payload,
            11,
            DatastreamEncoding::TokenIdsLeU32,
        )
        .with_dataset_binding(
            DatastreamDatasetBinding::new("tiny-corpus")
                .with_split("train")
                .with_shard_key("0001"),
        )
        .with_provenance_digest("dataset-provenance-digest");
        let server = InMemoryDatastreamServer::new(manifest.clone(), payload.clone())?;
        let mut session = server.open(
            DatastreamOpenRequest::new(manifest.stable_digest()).with_max_chunks_in_flight(2),
        )?;
        let first_window = session.next_window()?;
        let mut client = InMemoryDatastreamClient::new(manifest.clone());
        client.apply_chunk(first_window[0].clone())?;

        let mut resumed = InMemoryDatastreamClient::resume(
            manifest.clone(),
            client.received_bytes().to_vec(),
            1,
        )?;
        let mut resumed_session = server.open(
            DatastreamOpenRequest::new(manifest.stable_digest())
                .with_resume_cursor(resumed.progress()?.cursor())
                .with_max_chunks_in_flight(4),
        )?;
        loop {
            let window = resumed_session.next_window()?;
            if window.is_empty() {
                break;
            }
            for chunk in window {
                resumed.apply_chunk(chunk)?;
            }
        }

        let receipt = resumed.finish()?;
        assert_eq!(receipt.bytes_delivered, payload.len() as u64);
        assert_eq!(receipt.manifest.chunk_count, manifest.chunks.len());
        assert!(receipt.resumed);
        assert_eq!(
            receipt
                .manifest
                .dataset_binding
                .as_ref()
                .map(|binding| binding.dataset_id.as_str()),
            Some("tiny-corpus")
        );
        Ok(())
    }

    #[test]
    fn checkpoint_stream_detects_tampered_chunk() -> Result<(), Box<dyn std::error::Error>> {
        let payload = b"checkpoint-step-4096-weights".to_vec();
        let manifest = super::DatastreamManifest::from_bytes(
            "checkpoint-4096",
            DatastreamSubjectKind::Checkpoint,
            &payload,
            8,
            DatastreamEncoding::Safetensors,
        )
        .with_checkpoint_binding(
            DatastreamCheckpointBinding::new("serve.tensor")
                .with_checkpoint_ref("checkpoint://cluster/checkpoint-4096")
                .with_step(4096),
        );
        let server = InMemoryDatastreamServer::new(manifest.clone(), payload)?;
        let mut session = server.open(DatastreamOpenRequest::new(manifest.stable_digest()))?;
        let mut window = session.next_window()?;
        let mut tampered = window.remove(0);
        tampered.payload[0] ^= 0xFF;
        let mut client = InMemoryDatastreamClient::new(manifest);

        let error = match client.apply_chunk(tampered) {
            Ok(_) => panic!("tampered chunk should be refused"),
            Err(error) => error,
        };
        assert!(matches!(
            error,
            DatastreamTransferError::ChunkDigestMismatch { index: 0, .. }
        ));
        Ok(())
    }

    #[test]
    fn manifest_digest_changes_when_binding_changes() {
        let payload = b"shared-payload".to_vec();
        let dataset = super::DatastreamManifest::from_bytes(
            "shared-stream",
            DatastreamSubjectKind::TokenizedCorpus,
            &payload,
            4,
            DatastreamEncoding::RawBinary,
        )
        .with_dataset_binding(DatastreamDatasetBinding::new("tiny-corpus"));
        let checkpoint = super::DatastreamManifest::from_bytes(
            "shared-stream",
            DatastreamSubjectKind::Checkpoint,
            &payload,
            4,
            DatastreamEncoding::RawBinary,
        )
        .with_checkpoint_binding(DatastreamCheckpointBinding::new("train.decoder").with_step(17));

        assert_ne!(dataset.stable_digest(), checkpoint.stable_digest());
    }

    #[test]
    fn datastream_dispatch_plan_batches_chunks_and_beats_naive_baseline() {
        let manifest = super::DatastreamManifest::from_bytes(
            "dataset-plan",
            DatastreamSubjectKind::TokenizedCorpus,
            b"alpha-beta-gamma-delta-epsilon-zeta-eta-theta-iota-kappa",
            6,
            DatastreamEncoding::RawBinary,
        );

        let plan = manifest.recommended_dispatch_plan(4);
        let benchmark = manifest.dispatch_benchmark(4);

        assert!(plan.total_wake_events < manifest.chunks.len());
        assert!(benchmark.optimized_cost_units < benchmark.baseline_cost_units);
        assert!(benchmark.improvement_basis_points > 0);
    }
}
