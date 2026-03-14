//! Resumable streamed dataset, checkpoint, and policy-weight delivery contracts
//! for Psionic.

use psionic_runtime::{
    KvResidencyExternalLocator, RuntimeDispatchPlan, RuntimeDispatchPolicy,
    RuntimeOptimizationBenchmark, RuntimeWorkClass, RuntimeWorkItem, benchmark_dispatch_plan,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

const KV_CACHE_CHECKPOINT_FAMILY_PREFIX: &str = "serve.kv_cache";

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
    /// Training-policy or weight-state shard delivery.
    PolicyWeights,
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
            Self::PolicyWeights => "policy_weights",
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

/// Policy-weight-scoped identity carried alongside one stream.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DatastreamPolicyWeightBinding {
    /// Stable policy family or identifier.
    pub policy_id: String,
    /// Monotonic policy revision.
    pub policy_revision: u64,
    /// Stable shard identifier.
    pub shard_id: String,
    /// Stable shard index inside the broadcast.
    pub shard_index: usize,
    /// Total shard count for the assembled artifact.
    pub shard_count: usize,
    /// Stable digest for the assembled full-weight artifact.
    pub assembled_artifact_digest: String,
    /// Publication timestamp for freshness enforcement.
    pub published_at_ms: u64,
    /// Freshness window for accepting this artifact.
    pub freshness_window_ms: u64,
}

impl DatastreamPolicyWeightBinding {
    /// Creates one policy-weight binding from explicit policy and shard facts.
    #[must_use]
    pub fn new(
        policy_id: impl Into<String>,
        policy_revision: u64,
        shard_id: impl Into<String>,
        shard_index: usize,
        shard_count: usize,
        assembled_artifact_digest: impl Into<String>,
        published_at_ms: u64,
        freshness_window_ms: u64,
    ) -> Self {
        Self {
            policy_id: policy_id.into(),
            policy_revision,
            shard_id: shard_id.into(),
            shard_index,
            shard_count: shard_count.max(1),
            assembled_artifact_digest: assembled_artifact_digest.into(),
            published_at_ms,
            freshness_window_ms: freshness_window_ms.max(1),
        }
    }

    /// Returns the last admissible timestamp for this shard.
    #[must_use]
    pub fn fresh_until_ms(&self) -> u64 {
        self.published_at_ms
            .saturating_add(self.freshness_window_ms)
    }

    /// Returns whether the shard is stale at the provided timestamp.
    #[must_use]
    pub fn is_stale_at_ms(&self, observed_at_ms: u64) -> bool {
        observed_at_ms > self.fresh_until_ms()
    }
}

/// Control-plane-visible mirror type for one heavy artifact.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DatastreamMirrorKind {
    /// Pull the artifact from an HTTP mirror.
    HttpPull,
    /// Resolve the artifact through relay metadata.
    Relay,
}

/// Lightweight mirror metadata for one heavy artifact.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DatastreamMirrorLocator {
    /// Stable mirror identifier.
    pub mirror_id: String,
    /// Mirror transport kind.
    pub kind: DatastreamMirrorKind,
    /// HTTP URL or relay locator.
    pub locator: String,
}

impl DatastreamMirrorLocator {
    /// Creates one mirror locator.
    #[must_use]
    pub fn new(
        mirror_id: impl Into<String>,
        kind: DatastreamMirrorKind,
        locator: impl Into<String>,
    ) -> Self {
        Self {
            mirror_id: mirror_id.into(),
            kind,
            locator: locator.into(),
        }
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
    /// Optional policy-weight binding.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub policy_weight_binding: Option<DatastreamPolicyWeightBinding>,
    /// Optional mirror metadata for replay-safe control-plane refs.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mirrors: Vec<DatastreamMirrorLocator>,
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
            policy_weight_binding: None,
            mirrors: Vec::new(),
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

    /// Attaches policy-weight identity for the stream.
    #[must_use]
    pub fn with_policy_weight_binding(
        mut self,
        policy_weight_binding: DatastreamPolicyWeightBinding,
    ) -> Self {
        self.policy_weight_binding = Some(policy_weight_binding);
        self
    }

    /// Adds one mirror locator for this artifact.
    #[must_use]
    pub fn with_mirror(mut self, mirror: DatastreamMirrorLocator) -> Self {
        self.mirrors.push(mirror);
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
        if let Some(policy_weight_binding) = &self.policy_weight_binding {
            hasher.update(b"|policy_weight|");
            hasher.update(policy_weight_binding.policy_id.as_bytes());
            hasher.update(b"|");
            hasher.update(policy_weight_binding.policy_revision.to_string().as_bytes());
            hasher.update(b"|");
            hasher.update(policy_weight_binding.shard_id.as_bytes());
            hasher.update(b"|");
            hasher.update(policy_weight_binding.shard_index.to_string().as_bytes());
            hasher.update(b"|");
            hasher.update(policy_weight_binding.shard_count.to_string().as_bytes());
            hasher.update(b"|");
            hasher.update(policy_weight_binding.assembled_artifact_digest.as_bytes());
            hasher.update(b"|");
            hasher.update(policy_weight_binding.published_at_ms.to_string().as_bytes());
            hasher.update(b"|");
            hasher.update(policy_weight_binding.freshness_window_ms.to_string().as_bytes());
        }
        for mirror in &self.mirrors {
            hasher.update(b"|mirror|");
            hasher.update(mirror.mirror_id.as_bytes());
            hasher.update(b"|");
            hasher.update(mirror_kind_label(mirror.kind));
            hasher.update(b"|");
            hasher.update(mirror.locator.as_bytes());
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
            policy_weight_binding: self.policy_weight_binding.clone(),
            mirrors: self.mirrors.clone(),
        }
    }

    /// Exports this manifest as an explicit datastream-backed KV locator.
    pub fn kv_cache_external_locator(
        &self,
    ) -> Result<KvResidencyExternalLocator, DatastreamTransferError> {
        self.manifest_ref().kv_cache_external_locator()
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
    /// Optional policy-weight binding.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub policy_weight_binding: Option<DatastreamPolicyWeightBinding>,
    /// Optional mirror metadata.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mirrors: Vec<DatastreamMirrorLocator>,
}

impl DatastreamManifestRef {
    /// Exports this manifest reference as an explicit datastream-backed KV locator.
    pub fn kv_cache_external_locator(
        &self,
    ) -> Result<KvResidencyExternalLocator, DatastreamTransferError> {
        let checkpoint_family = self
            .checkpoint_binding
            .as_ref()
            .map(|binding| binding.checkpoint_family.as_str());
        if self.subject != DatastreamSubjectKind::Checkpoint
            || !checkpoint_family
                .is_some_and(|family| family.starts_with(KV_CACHE_CHECKPOINT_FAMILY_PREFIX))
        {
            return Err(DatastreamTransferError::KvCacheContractInvalid {
                stream_id: self.stream_id.clone(),
                subject: self.subject,
                checkpoint_family: checkpoint_family.map(String::from),
            });
        }

        let mut detail = format!(
            "checkpoint-backed distributed KV tier via family `{}`",
            checkpoint_family.unwrap_or_default()
        );
        if let Some(binding) = &self.checkpoint_binding {
            if let Some(checkpoint_ref) = &binding.checkpoint_ref {
                detail.push_str(" ref `");
                detail.push_str(checkpoint_ref);
                detail.push('`');
            }
            if let Some(step) = binding.step {
                detail.push_str(" step ");
                detail.push_str(&step.to_string());
            }
        }

        Ok(KvResidencyExternalLocator::datastream(
            self.stream_id.clone(),
            self.manifest_digest.clone(),
            self.object_digest.clone(),
            self.total_bytes,
        )
        .with_detail(detail))
    }

    fn policy_weight_binding(
        &self,
    ) -> Result<&DatastreamPolicyWeightBinding, DatastreamTransferError> {
        if self.subject != DatastreamSubjectKind::PolicyWeights {
            return Err(DatastreamTransferError::PolicyWeightContractInvalid {
                stream_id: self.stream_id.clone(),
                subject: self.subject,
            });
        }
        self.policy_weight_binding.as_ref().ok_or_else(|| {
            DatastreamTransferError::PolicyWeightBindingMissing {
                stream_id: self.stream_id.clone(),
            }
        })
    }

    /// Exports this manifest reference as a lightweight control-plane ref for policy weights.
    pub fn policy_weight_control_plane_ref(
        &self,
        observed_at_ms: u64,
    ) -> Result<DatastreamPolicyWeightControlPlaneRef, DatastreamTransferError> {
        let binding = self.policy_weight_binding()?;
        if binding.is_stale_at_ms(observed_at_ms) {
            return Err(DatastreamTransferError::PolicyWeightStale {
                stream_id: self.stream_id.clone(),
                policy_id: binding.policy_id.clone(),
                policy_revision: binding.policy_revision,
                published_at_ms: binding.published_at_ms,
                freshness_window_ms: binding.freshness_window_ms,
                observed_at_ms,
            });
        }
        DatastreamPolicyWeightControlPlaneRef::new(self.clone())
    }
}

/// Lightweight control-plane reference for one heavy policy-weight shard.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DatastreamPolicyWeightControlPlaneRef {
    /// Stable stream identifier.
    pub stream_id: String,
    /// Stable manifest digest for the heavy artifact.
    pub manifest_digest: String,
    /// Stable object digest for the shard payload.
    pub object_digest: String,
    /// Stable policy identifier.
    pub policy_id: String,
    /// Monotonic policy revision.
    pub policy_revision: u64,
    /// Stable shard identifier.
    pub shard_id: String,
    /// Stable shard index.
    pub shard_index: usize,
    /// Total shard count.
    pub shard_count: usize,
    /// Assembled full-artifact digest.
    pub assembled_artifact_digest: String,
    /// Publication timestamp.
    pub published_at_ms: u64,
    /// Freshness window.
    pub freshness_window_ms: u64,
    /// Control-plane-visible mirrors.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mirrors: Vec<DatastreamMirrorLocator>,
    /// Stable digest over the control-plane ref.
    pub control_plane_digest: String,
}

impl DatastreamPolicyWeightControlPlaneRef {
    /// Builds one lightweight control-plane ref from a manifest ref.
    pub fn new(manifest: DatastreamManifestRef) -> Result<Self, DatastreamTransferError> {
        let binding = manifest.policy_weight_binding.clone().ok_or_else(|| {
            DatastreamTransferError::PolicyWeightBindingMissing {
                stream_id: manifest.stream_id.clone(),
            }
        })?;
        let control_plane_digest = stable_policy_weight_control_plane_digest(
            manifest.stream_id.as_str(),
            manifest.manifest_digest.as_str(),
            manifest.object_digest.as_str(),
            &binding,
            manifest.mirrors.as_slice(),
        );
        Ok(Self {
            stream_id: manifest.stream_id,
            manifest_digest: manifest.manifest_digest,
            object_digest: manifest.object_digest,
            policy_id: binding.policy_id,
            policy_revision: binding.policy_revision,
            shard_id: binding.shard_id,
            shard_index: binding.shard_index,
            shard_count: binding.shard_count,
            assembled_artifact_digest: binding.assembled_artifact_digest,
            published_at_ms: binding.published_at_ms,
            freshness_window_ms: binding.freshness_window_ms,
            mirrors: manifest.mirrors,
            control_plane_digest,
        })
    }

    /// Returns the last admissible timestamp for this ref.
    #[must_use]
    pub fn fresh_until_ms(&self) -> u64 {
        self.published_at_ms
            .saturating_add(self.freshness_window_ms)
    }

    /// Returns an estimate of the ref size without heavy payload bytes.
    #[must_use]
    pub fn estimated_control_plane_bytes(&self) -> usize {
        self.stream_id.len()
            + self.manifest_digest.len()
            + self.object_digest.len()
            + self.policy_id.len()
            + self.shard_id.len()
            + self.assembled_artifact_digest.len()
            + self.control_plane_digest.len()
            + self
                .mirrors
                .iter()
                .map(|mirror| mirror.mirror_id.len() + mirror.locator.len() + 8)
                .sum::<usize>()
            + 64
    }
}

/// Control-plane summary for a multi-shard policy-weight broadcast.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DatastreamPolicyWeightBroadcastManifest {
    /// Stable policy identifier.
    pub policy_id: String,
    /// Monotonic policy revision.
    pub policy_revision: u64,
    /// Stable digest for the assembled full-weight artifact.
    pub assembled_artifact_digest: String,
    /// Publication timestamp shared by the broadcast.
    pub published_at_ms: u64,
    /// Freshness window for the full broadcast.
    pub freshness_window_ms: u64,
    /// Ordered shard refs.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub shards: Vec<DatastreamPolicyWeightControlPlaneRef>,
    /// Stable digest over the full broadcast contract.
    pub broadcast_digest: String,
}

impl DatastreamPolicyWeightBroadcastManifest {
    /// Creates a broadcast manifest from validated shard refs.
    pub fn from_manifest_refs(
        manifest_refs: Vec<DatastreamManifestRef>,
        observed_at_ms: u64,
    ) -> Result<Self, DatastreamTransferError> {
        let mut shards = manifest_refs
            .into_iter()
            .map(|manifest_ref| manifest_ref.policy_weight_control_plane_ref(observed_at_ms))
            .collect::<Result<Vec<_>, _>>()?;
        shards.sort_by_key(|shard| shard.shard_index);
        let Some(first) = shards.first() else {
            return Err(DatastreamTransferError::PolicyWeightBroadcastEmpty);
        };
        for shard in &shards {
            if shard.policy_id != first.policy_id
                || shard.policy_revision != first.policy_revision
                || shard.assembled_artifact_digest != first.assembled_artifact_digest
                || shard.shard_count != first.shard_count
                || shard.published_at_ms != first.published_at_ms
                || shard.freshness_window_ms != first.freshness_window_ms
            {
                return Err(DatastreamTransferError::PolicyWeightBroadcastBindingMismatch {
                    expected_policy_id: first.policy_id.clone(),
                    actual_policy_id: shard.policy_id.clone(),
                    expected_policy_revision: first.policy_revision,
                    actual_policy_revision: shard.policy_revision,
                });
            }
        }
        let observed_indices = shards.iter().map(|shard| shard.shard_index).collect::<Vec<_>>();
        if observed_indices.len() != first.shard_count
            || observed_indices
                .iter()
                .copied()
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                != first.shard_count
            || !observed_indices
                .iter()
                .copied()
                .eq(0..first.shard_count)
        {
            return Err(DatastreamTransferError::PolicyWeightBroadcastShardCoverageInvalid {
                expected_shard_count: first.shard_count,
                observed_indices,
            });
        }
        let broadcast_digest = stable_policy_weight_broadcast_digest(shards.as_slice());
        Ok(Self {
            policy_id: first.policy_id.clone(),
            policy_revision: first.policy_revision,
            assembled_artifact_digest: first.assembled_artifact_digest.clone(),
            published_at_ms: first.published_at_ms,
            freshness_window_ms: first.freshness_window_ms,
            shards,
            broadcast_digest,
        })
    }

    /// Returns an estimate of control-plane bytes for the full broadcast.
    #[must_use]
    pub fn estimated_control_plane_bytes(&self) -> usize {
        self.policy_id.len()
            + self.assembled_artifact_digest.len()
            + self.broadcast_digest.len()
            + self
                .shards
                .iter()
                .map(DatastreamPolicyWeightControlPlaneRef::estimated_control_plane_bytes)
                .sum::<usize>()
            + 64
    }
}

/// One delivered shard receipt inside a broadcast.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DatastreamPolicyWeightShardReceipt {
    /// Lightweight shard ref.
    pub shard: DatastreamPolicyWeightControlPlaneRef,
    /// Underlying datastream receipt.
    pub delivery: DatastreamDeliveryReceipt,
}

/// Final receipt for one multi-shard policy-weight broadcast.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DatastreamPolicyWeightBroadcastReceipt {
    /// Lightweight broadcast contract.
    pub broadcast: DatastreamPolicyWeightBroadcastManifest,
    /// Delivered shard receipts.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub shards: Vec<DatastreamPolicyWeightShardReceipt>,
    /// Total bytes delivered across all shards.
    pub bytes_delivered: u64,
    /// Stable digest over the receipt.
    pub receipt_digest: String,
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
    /// The manifest reference does not describe a valid distributed KV contract.
    #[error(
        "datastream `{stream_id}` is not a valid KV-cache contract: subject `{subject:?}`, checkpoint family `{checkpoint_family:?}`"
    )]
    KvCacheContractInvalid {
        stream_id: String,
        subject: DatastreamSubjectKind,
        checkpoint_family: Option<String>,
    },
    /// The manifest reference is not a valid policy-weight contract.
    #[error(
        "datastream `{stream_id}` is not a valid policy-weight contract: subject `{subject:?}`"
    )]
    PolicyWeightContractInvalid {
        stream_id: String,
        subject: DatastreamSubjectKind,
    },
    /// A policy-weight manifest is missing its binding.
    #[error("datastream `{stream_id}` is missing policy-weight binding")]
    PolicyWeightBindingMissing { stream_id: String },
    /// A policy-weight ref is stale for the requested timestamp.
    #[error(
        "policy-weight datastream `{stream_id}` for policy `{policy_id}` revision `{policy_revision}` is stale at `{observed_at_ms}` (published `{published_at_ms}`, freshness window `{freshness_window_ms}`)"
    )]
    PolicyWeightStale {
        stream_id: String,
        policy_id: String,
        policy_revision: u64,
        published_at_ms: u64,
        freshness_window_ms: u64,
        observed_at_ms: u64,
    },
    /// The broadcast contains no shard refs.
    #[error("policy-weight broadcast cannot be empty")]
    PolicyWeightBroadcastEmpty,
    /// Not every shard agreed on the same policy and assembly identity.
    #[error(
        "policy-weight broadcast binding mismatch: expected `{expected_policy_id}` revision `{expected_policy_revision}`, found `{actual_policy_id}` revision `{actual_policy_revision}`"
    )]
    PolicyWeightBroadcastBindingMismatch {
        expected_policy_id: String,
        actual_policy_id: String,
        expected_policy_revision: u64,
        actual_policy_revision: u64,
    },
    /// The broadcast shards are missing coverage or contain duplicates.
    #[error(
        "policy-weight broadcast shard coverage invalid: expected `{expected_shard_count}` shards but saw indices `{observed_indices:?}`"
    )]
    PolicyWeightBroadcastShardCoverageInvalid {
        expected_shard_count: usize,
        observed_indices: Vec<usize>,
    },
    /// The delivered shards do not reassemble to the expected full-artifact digest.
    #[error(
        "policy-weight assembled artifact digest mismatch: expected `{expected}` but assembled `{actual}`"
    )]
    PolicyWeightAssemblyDigestMismatch { expected: String, actual: String },
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

/// In-memory multi-shard policy-weight broadcast built on the resumable datastream plane.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InMemoryPolicyWeightBroadcast {
    broadcast: DatastreamPolicyWeightBroadcastManifest,
    servers: Vec<InMemoryDatastreamServer>,
}

impl InMemoryPolicyWeightBroadcast {
    /// Creates a broadcast from manifest-backed shard servers.
    pub fn new(
        mut servers: Vec<InMemoryDatastreamServer>,
        observed_at_ms: u64,
    ) -> Result<Self, DatastreamTransferError> {
        servers.sort_by_key(|server| {
            server
                .manifest()
                .policy_weight_binding
                .as_ref()
                .map_or(usize::MAX, |binding| binding.shard_index)
        });
        let broadcast = DatastreamPolicyWeightBroadcastManifest::from_manifest_refs(
            servers.iter().map(InMemoryDatastreamServer::manifest_ref).collect(),
            observed_at_ms,
        )?;
        Ok(Self { broadcast, servers })
    }

    /// Returns the lightweight control-plane summary for the broadcast.
    #[must_use]
    pub const fn broadcast(&self) -> &DatastreamPolicyWeightBroadcastManifest {
        &self.broadcast
    }

    /// Downloads every shard over the resumable heavy artifact plane and assembles a receipt.
    pub fn deliver(
        &self,
        observed_at_ms: u64,
        max_chunks_in_flight: usize,
    ) -> Result<DatastreamPolicyWeightBroadcastReceipt, DatastreamTransferError> {
        let mut shard_receipts = Vec::new();
        let mut assembled = Vec::new();
        for server in &self.servers {
            let manifest = server.manifest();
            let manifest_ref = manifest.manifest_ref();
            let shard_ref = manifest_ref.policy_weight_control_plane_ref(observed_at_ms)?;
            let mut session = server.open(
                DatastreamOpenRequest::new(manifest.stable_digest())
                    .with_max_chunks_in_flight(max_chunks_in_flight),
            )?;
            let mut client = InMemoryDatastreamClient::new(manifest.clone());
            loop {
                let window = session.next_window()?;
                if window.is_empty() {
                    break;
                }
                for chunk in window {
                    client.apply_chunk(chunk)?;
                }
            }
            assembled.extend_from_slice(client.received_bytes());
            let delivery = client.finish()?;
            shard_receipts.push(DatastreamPolicyWeightShardReceipt {
                shard: shard_ref,
                delivery,
            });
        }
        let actual_assembled_digest = digest_bytes(&assembled);
        if actual_assembled_digest != self.broadcast.assembled_artifact_digest {
            return Err(DatastreamTransferError::PolicyWeightAssemblyDigestMismatch {
                expected: self.broadcast.assembled_artifact_digest.clone(),
                actual: actual_assembled_digest,
            });
        }
        let bytes_delivered = shard_receipts
            .iter()
            .fold(0_u64, |acc, receipt| acc.saturating_add(receipt.delivery.bytes_delivered));
        let receipt_digest =
            stable_policy_weight_broadcast_receipt_digest(&self.broadcast, shard_receipts.as_slice(), bytes_delivered);
        Ok(DatastreamPolicyWeightBroadcastReceipt {
            broadcast: self.broadcast.clone(),
            shards: shard_receipts,
            bytes_delivered,
            receipt_digest,
        })
    }
}

fn stable_policy_weight_control_plane_digest(
    stream_id: &str,
    manifest_digest: &str,
    object_digest: &str,
    binding: &DatastreamPolicyWeightBinding,
    mirrors: &[DatastreamMirrorLocator],
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_policy_weight_control_plane_ref|");
    hasher.update(stream_id.as_bytes());
    hasher.update(b"|");
    hasher.update(manifest_digest.as_bytes());
    hasher.update(b"|");
    hasher.update(object_digest.as_bytes());
    hasher.update(b"|");
    hasher.update(binding.policy_id.as_bytes());
    hasher.update(b"|");
    hasher.update(binding.policy_revision.to_string().as_bytes());
    hasher.update(b"|");
    hasher.update(binding.shard_id.as_bytes());
    hasher.update(b"|");
    hasher.update(binding.shard_index.to_string().as_bytes());
    hasher.update(b"|");
    hasher.update(binding.shard_count.to_string().as_bytes());
    hasher.update(b"|");
    hasher.update(binding.assembled_artifact_digest.as_bytes());
    hasher.update(b"|");
    hasher.update(binding.published_at_ms.to_string().as_bytes());
    hasher.update(b"|");
    hasher.update(binding.freshness_window_ms.to_string().as_bytes());
    for mirror in mirrors {
        hasher.update(b"|mirror|");
        hasher.update(mirror.mirror_id.as_bytes());
        hasher.update(b"|");
        hasher.update(mirror_kind_label(mirror.kind));
        hasher.update(b"|");
        hasher.update(mirror.locator.as_bytes());
    }
    hex::encode(hasher.finalize())
}

fn stable_policy_weight_broadcast_digest(
    shards: &[DatastreamPolicyWeightControlPlaneRef],
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_policy_weight_broadcast_manifest|");
    for shard in shards {
        hasher.update(b"|shard|");
        hasher.update(shard.control_plane_digest.as_bytes());
    }
    hex::encode(hasher.finalize())
}

fn stable_policy_weight_broadcast_receipt_digest(
    broadcast: &DatastreamPolicyWeightBroadcastManifest,
    shards: &[DatastreamPolicyWeightShardReceipt],
    bytes_delivered: u64,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_policy_weight_broadcast_receipt|");
    hasher.update(broadcast.broadcast_digest.as_bytes());
    hasher.update(b"|bytes|");
    hasher.update(bytes_delivered.to_string().as_bytes());
    for shard in shards {
        hasher.update(b"|receipt|");
        hasher.update(shard.shard.control_plane_digest.as_bytes());
        hasher.update(b"|");
        hasher.update(shard.delivery.manifest.manifest_digest.as_bytes());
        hasher.update(b"|");
        hasher.update(shard.delivery.bytes_delivered.to_string().as_bytes());
    }
    hex::encode(hasher.finalize())
}

fn mirror_kind_label(kind: DatastreamMirrorKind) -> &'static [u8] {
    match kind {
        DatastreamMirrorKind::HttpPull => b"http_pull",
        DatastreamMirrorKind::Relay => b"relay",
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
        DatastreamMirrorKind, DatastreamMirrorLocator, DatastreamOpenRequest,
        DatastreamPolicyWeightBinding, DatastreamSubjectKind, DatastreamTransferError,
        InMemoryDatastreamClient, InMemoryDatastreamServer, InMemoryPolicyWeightBroadcast,
    };

    fn assembled_digest(shards: &[&[u8]]) -> String {
        let bytes = shards.iter().flat_map(|shard| shard.iter().copied()).collect::<Vec<_>>();
        super::digest_bytes(&bytes)
    }

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

    #[test]
    fn checkpoint_manifest_can_export_kv_cache_external_locator()
    -> Result<(), Box<dyn std::error::Error>> {
        let payload = b"kv-cache-pages-for-request-17".to_vec();
        let manifest = super::DatastreamManifest::from_bytes(
            "kv-cache-stream-17",
            DatastreamSubjectKind::Checkpoint,
            &payload,
            8,
            DatastreamEncoding::RawBinary,
        )
        .with_checkpoint_binding(
            DatastreamCheckpointBinding::new("serve.kv_cache.layer_shard")
                .with_checkpoint_ref("checkpoint://cluster/kv-cache-stream-17")
                .with_step(17),
        );

        let locator = manifest.kv_cache_external_locator()?;
        assert_eq!(
            locator.kind,
            psionic_runtime::KvResidencyLocatorKind::Datastream
        );
        assert_eq!(locator.locator_id, "kv-cache-stream-17");
        assert_eq!(locator.locator_digest, manifest.stable_digest());
        assert_eq!(locator.object_digest, manifest.object_digest);
        assert_eq!(locator.total_bytes, payload.len() as u64);
        assert!(
            locator
                .detail
                .as_deref()
                .is_some_and(|detail| detail.contains("serve.kv_cache.layer_shard"))
        );
        Ok(())
    }

    #[test]
    fn non_kv_checkpoint_manifest_is_refused_as_external_kv_locator() {
        let manifest = super::DatastreamManifest::from_bytes(
            "checkpoint-train-17",
            DatastreamSubjectKind::Checkpoint,
            b"weights",
            4,
            DatastreamEncoding::RawBinary,
        )
        .with_checkpoint_binding(DatastreamCheckpointBinding::new("train.decoder").with_step(17));

        let error = match manifest.kv_cache_external_locator() {
            Ok(_) => panic!("non-KV checkpoint manifest should be refused"),
            Err(error) => error,
        };
        assert_eq!(
            error,
            DatastreamTransferError::KvCacheContractInvalid {
                stream_id: String::from("checkpoint-train-17"),
                subject: DatastreamSubjectKind::Checkpoint,
                checkpoint_family: Some(String::from("train.decoder")),
            }
        );
    }

    #[test]
    fn non_checkpoint_manifest_is_refused_as_external_kv_locator() {
        let manifest = super::DatastreamManifest::from_bytes(
            "dataset-17",
            DatastreamSubjectKind::TokenizedCorpus,
            b"tokens",
            4,
            DatastreamEncoding::RawBinary,
        );

        let error = match manifest.kv_cache_external_locator() {
            Ok(_) => panic!("dataset manifest should be refused"),
            Err(error) => error,
        };
        assert_eq!(
            error,
            DatastreamTransferError::KvCacheContractInvalid {
                stream_id: String::from("dataset-17"),
                subject: DatastreamSubjectKind::TokenizedCorpus,
                checkpoint_family: None,
            }
        );
    }

    #[test]
    fn policy_weight_broadcast_keeps_control_plane_lightweight_and_delivers_bytes()
    -> Result<(), Box<dyn std::error::Error>> {
        let shard_a = b"policy-weight-shard-a".repeat(32);
        let shard_b = b"policy-weight-shard-b".repeat(32);
        let assembled_artifact_digest = assembled_digest(&[shard_a.as_slice(), shard_b.as_slice()]);
        let published_at_ms = 1_000;
        let freshness_window_ms = 5_000;
        let manifest_a = super::DatastreamManifest::from_bytes(
            "policy-weights-0",
            DatastreamSubjectKind::PolicyWeights,
            &shard_a,
            5,
            DatastreamEncoding::Safetensors,
        )
        .with_policy_weight_binding(DatastreamPolicyWeightBinding::new(
            "policy.decoder",
            7,
            "shard-a",
            0,
            2,
            assembled_artifact_digest.clone(),
            published_at_ms,
            freshness_window_ms,
        ))
        .with_mirror(DatastreamMirrorLocator::new(
            "mirror-http-a",
            DatastreamMirrorKind::HttpPull,
            "https://weights.example/shard-a",
        ));
        let manifest_b = super::DatastreamManifest::from_bytes(
            "policy-weights-1",
            DatastreamSubjectKind::PolicyWeights,
            &shard_b,
            5,
            DatastreamEncoding::Safetensors,
        )
        .with_policy_weight_binding(DatastreamPolicyWeightBinding::new(
            "policy.decoder",
            7,
            "shard-b",
            1,
            2,
            assembled_artifact_digest.clone(),
            published_at_ms,
            freshness_window_ms,
        ))
        .with_mirror(DatastreamMirrorLocator::new(
            "relay-weights",
            DatastreamMirrorKind::Relay,
            "wss://relay.example/policy.decoder/7",
        ));
        let broadcast = InMemoryPolicyWeightBroadcast::new(
            vec![
                InMemoryDatastreamServer::new(manifest_a, shard_a.clone())?,
                InMemoryDatastreamServer::new(manifest_b, shard_b.clone())?,
            ],
            1_500,
        )?;

        assert_eq!(broadcast.broadcast().shards.len(), 2);
        assert!(
            broadcast.broadcast().estimated_control_plane_bytes()
                < (shard_a.len() + shard_b.len())
        );
        assert_eq!(
            broadcast.broadcast().shards[0].mirrors[0].mirror_id,
            "mirror-http-a"
        );

        let receipt = broadcast.deliver(1_500, 2)?;
        assert_eq!(receipt.bytes_delivered, (shard_a.len() + shard_b.len()) as u64);
        assert_eq!(
            receipt.broadcast.assembled_artifact_digest,
            assembled_artifact_digest
        );
        assert_eq!(receipt.shards.len(), 2);
        Ok(())
    }

    #[test]
    fn stale_policy_weight_ref_is_rejected() -> Result<(), Box<dyn std::error::Error>> {
        let shard = b"stale-policy-shard".to_vec();
        let manifest = super::DatastreamManifest::from_bytes(
            "policy-weights-stale",
            DatastreamSubjectKind::PolicyWeights,
            &shard,
            4,
            DatastreamEncoding::Safetensors,
        )
        .with_policy_weight_binding(DatastreamPolicyWeightBinding::new(
            "policy.decoder",
            8,
            "shard-a",
            0,
            1,
            assembled_digest(&[shard.as_slice()]),
            1_000,
            500,
        ));

        let error = manifest
            .manifest_ref()
            .policy_weight_control_plane_ref(2_000)
            .expect_err("stale policy-weight ref should be rejected");
        assert_eq!(
            error,
            DatastreamTransferError::PolicyWeightStale {
                stream_id: String::from("policy-weights-stale"),
                policy_id: String::from("policy.decoder"),
                policy_revision: 8,
                published_at_ms: 1_000,
                freshness_window_ms: 500,
                observed_at_ms: 2_000,
            }
        );
        Ok(())
    }

    #[test]
    fn policy_weight_broadcast_rejects_binding_mismatch()
    -> Result<(), Box<dyn std::error::Error>> {
        let shard_a = b"policy-weight-shard-a".to_vec();
        let shard_b = b"policy-weight-shard-b".to_vec();
        let manifest_a = super::DatastreamManifest::from_bytes(
            "policy-weights-0",
            DatastreamSubjectKind::PolicyWeights,
            &shard_a,
            5,
            DatastreamEncoding::Safetensors,
        )
        .with_policy_weight_binding(DatastreamPolicyWeightBinding::new(
            "policy.decoder",
            7,
            "shard-a",
            0,
            2,
            assembled_digest(&[shard_a.as_slice(), shard_b.as_slice()]),
            1_000,
            5_000,
        ));
        let manifest_b = super::DatastreamManifest::from_bytes(
            "policy-weights-1",
            DatastreamSubjectKind::PolicyWeights,
            &shard_b,
            5,
            DatastreamEncoding::Safetensors,
        )
        .with_policy_weight_binding(DatastreamPolicyWeightBinding::new(
            "policy.other",
            9,
            "shard-b",
            1,
            2,
            assembled_digest(&[shard_a.as_slice(), shard_b.as_slice()]),
            1_000,
            5_000,
        ));

        let error = InMemoryPolicyWeightBroadcast::new(
            vec![
                InMemoryDatastreamServer::new(manifest_a, shard_a)?,
                InMemoryDatastreamServer::new(manifest_b, shard_b)?,
            ],
            1_500,
        )
        .expect_err("mismatched bindings should be rejected");
        assert_eq!(
            error,
            DatastreamTransferError::PolicyWeightBroadcastBindingMismatch {
                expected_policy_id: String::from("policy.decoder"),
                actual_policy_id: String::from("policy.other"),
                expected_policy_revision: 7,
                actual_policy_revision: 9,
            }
        );
        Ok(())
    }
}
