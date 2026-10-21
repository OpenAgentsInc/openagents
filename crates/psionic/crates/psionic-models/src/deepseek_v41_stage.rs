//! DeepSeek-V4.1 stage boundaries, wire sizes, group manifest, and placement.
//!
//! The model-independent message, codec, and admission live in
//! [`crate::stage_state`]. This module says what crosses a V4.1 boundary:
//!
//! - the four hyper-connection residual streams and the last FFN sub-block's
//!   pre-mix, which the next layer's attention collapses them with;
//! - rows of the KV source that layers past the boundary reuse (layers 3–7,
//!   9–13, and 15–19 reuse layers 2, 8, and 14; layers 21–39 reuse 20);
//! - that source's indexer keys when an index-source layer past the boundary
//!   scores them (layers 24, 28, 32, and 36 score layer 20's keys);
//! - the latest indexer selection, when the boundary falls between an
//!   index source (2, 8, 14, 20, 24, 28, 32, 36) and the layers that reuse it;
//! - layer 20's candidate blocks, when an index source past the boundary
//!   reads them;
//! - Engram token history, when an Engram layer (1 or 14) is past the
//!   boundary.
//!
//! A boundary at layer 20 carries only the residual and the pre-mix. [`stage_contract`]
//! builds the inbound contract for any layer range, and [`wire_report`]
//! measures the bytes per decode token by encoding real-shaped messages.
//!
//! [`V41GroupManifest`] names each stage's layers, GPU, and Engram tables.
//! [`plan_group`] fits stages to GPU and host memory through a
//! [`PlacementMemory`] input. [`HeaderBytes`] derives it from the checkpoint
//! headers; [`MeasuredMemory`] reads W3's measurements of the CUDA runtime
//! (#12390), which are provisional until numerical qualification passes.
//! Four stages are evaluated before eight.

use std::{collections::BTreeMap, fmt, ops::Range};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    deepseek_v41_weights::{
        DEEPSEEK_V41_FLASH_INDEX_SHA256, DEEPSEEK_V41_FLASH_REPO, DEEPSEEK_V41_FLASH_REVISION,
        V41StageSelection,
    },
    stage_state::{
        CodecLimits, RowRule, Section, SectionKind, SectionSpec, StageContract, StageMessage,
        StageRefusal, StepKind, WireDtype, encode,
    },
};

/// Decoder layers.
pub const V41_LAYERS: u32 = 40;
/// Layers whose compressed KV later layers reuse.
pub const KV_SOURCES: [u32; 4] = [2, 8, 14, 20];
/// Layers that run the indexer; later layers reuse their selection.
pub const INDEX_SOURCES: [u32; 8] = [2, 8, 14, 20, 24, 28, 32, 36];
/// The layer whose candidate blocks later index sources read.
pub const CANDIDATE_SOURCE: u32 = 20;
/// Layers with an Engram lookup.
pub const ENGRAM_LAYERS: [u32; 2] = [1, 14];
/// The stage-state protocol version a manifest names.
pub const V41_STAGE_PROTOCOL: u16 = crate::stage_state::STAGE_STATE_VERSION;

/// Compression ratio of a layer's KV: 0 for the sliding window only (layers
/// 0 and 1), 2 for layers 2–19, and 1 for layers 20–39.
#[must_use]
pub const fn compress_ratio(layer: u32) -> u32 {
    match layer {
        0..=1 => 0,
        2..=19 => 2,
        _ => 1,
    }
}

fn last_at_or_before(sources: &[u32], layer: u32) -> Option<u32> {
    sources.iter().copied().filter(|s| *s <= layer).next_back()
}

/// The KV source a layer reads.
#[must_use]
pub fn kv_owner(layer: u32) -> Option<u32> {
    last_at_or_before(&KV_SOURCES, layer)
}

/// The index source whose selection a layer uses.
#[must_use]
pub fn index_owner(layer: u32) -> Option<u32> {
    last_at_or_before(&INDEX_SOURCES, layer)
}

/// Sizes that set the shape of every section.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct V41StageGeometry {
    /// Hyper-connection streams (`hc_mult`).
    pub streams: u32,
    /// Hidden size.
    pub hidden: u32,
    /// Width of one compressed KV row.
    pub head_dim: u32,
    /// Width of one indexer key row.
    pub index_dim: u32,
    /// Indices one index source selects per token.
    pub index_top_k: u32,
    /// Candidate blocks layer 20 chooses per token.
    pub candidate_blocks: u32,
    /// Compressed rows per candidate block.
    pub candidate_block_rows: u32,
    /// Longest Engram n-gram; history carries one fewer token.
    pub max_ngram: u32,
    /// Output vocabulary.
    pub vocabulary: u32,
    /// Wire dtype of the residual streams.
    pub residual_dtype: WireDtype,
    /// Wire dtype of KV and indexer key rows.
    pub cache_dtype: WireDtype,
}

impl V41StageGeometry {
    /// DeepSeek-V4.1-Flash, with BF16 residuals and cache rows, as the
    /// reference's activations are.
    #[must_use]
    pub const fn flash() -> Self {
        Self {
            streams: 4,
            hidden: 5120,
            head_dim: 512,
            index_dim: 128,
            index_top_k: 512,
            candidate_blocks: 2048,
            candidate_block_rows: 8,
            max_ngram: 4,
            vocabulary: 129_280,
            residual_dtype: WireDtype::Bf16,
            cache_dtype: WireDtype::Bf16,
        }
    }

    /// A tiny geometry with the same 40-layer sharing layout, for tests that
    /// need no weights.
    #[must_use]
    pub const fn synthetic() -> Self {
        Self {
            streams: 4,
            hidden: 8,
            head_dim: 4,
            index_dim: 2,
            index_top_k: 2,
            candidate_blocks: 2,
            candidate_block_rows: 2,
            max_ngram: 4,
            vocabulary: 16,
            residual_dtype: WireDtype::F32,
            cache_dtype: WireDtype::F32,
        }
    }

    /// The same geometry with other wire dtypes.
    #[must_use]
    pub const fn with_dtypes(mut self, residual: WireDtype, cache: WireDtype) -> Self {
        self.residual_dtype = residual;
        self.cache_dtype = cache;
        self
    }
}

/// The sections a stage whose first layer is `boundary` needs in every step.
/// Stage 0 needs none: it embeds the step's tokens itself.
#[must_use]
pub fn boundary_sections(geometry: &V41StageGeometry, boundary: u32) -> Vec<SectionSpec> {
    if boundary == 0 || boundary >= V41_LAYERS {
        return Vec::new();
    }
    let mut specs = vec![
        SectionSpec {
            kind: SectionKind::Residual,
            dtype: geometry.residual_dtype,
            inner: vec![geometry.streams, geometry.hidden],
            rows: RowRule::PerToken,
        },
        SectionSpec {
            kind: SectionKind::PreMix,
            dtype: WireDtype::F32,
            inner: vec![geometry.streams],
            rows: RowRule::PerToken,
        },
    ];
    if let Some(source) = kv_owner(boundary).filter(|s| *s < boundary) {
        specs.push(SectionSpec {
            kind: SectionKind::SourceKv { layer: source },
            dtype: geometry.cache_dtype,
            inner: vec![geometry.head_dim],
            rows: RowRule::Append {
                ratio: compress_ratio(source),
            },
        });
        let scored_later = INDEX_SOURCES
            .iter()
            .any(|l| *l >= boundary && kv_owner(*l) == Some(source));
        if scored_later {
            specs.push(SectionSpec {
                kind: SectionKind::SourceIndexKeys { layer: source },
                dtype: geometry.cache_dtype,
                inner: vec![geometry.index_dim],
                rows: RowRule::Append {
                    ratio: compress_ratio(source),
                },
            });
        }
    }
    if boundary > CANDIDATE_SOURCE && INDEX_SOURCES.iter().any(|l| *l >= boundary) {
        specs.push(SectionSpec {
            kind: SectionKind::Candidates {
                layer: CANDIDATE_SOURCE,
            },
            dtype: WireDtype::U32,
            inner: vec![geometry.candidate_blocks],
            rows: RowRule::PerToken,
        });
    }
    if let Some(source) = index_owner(boundary).filter(|s| *s < boundary) {
        specs.push(SectionSpec {
            kind: SectionKind::IndexSelection { layer: source },
            dtype: WireDtype::I32,
            inner: vec![geometry.index_top_k],
            rows: RowRule::PerToken,
        });
    }
    if ENGRAM_LAYERS.iter().any(|l| *l >= boundary) {
        specs.push(SectionSpec {
            kind: SectionKind::EngramHistory,
            dtype: WireDtype::U32,
            inner: Vec::new(),
            rows: RowRule::Fixed(geometry.max_ngram - 1),
        });
    }
    specs
}

/// The inbound contract of a stage that runs `layers`.
#[must_use]
pub fn stage_contract(
    geometry: &V41StageGeometry,
    stage: u16,
    layers: Range<u32>,
) -> StageContract {
    StageContract {
        stage,
        sections: boundary_sections(geometry, layers.start),
        layers,
    }
}

/// The section the last stage returns to the front.
#[must_use]
pub fn logits_spec(geometry: &V41StageGeometry) -> SectionSpec {
    SectionSpec {
        kind: SectionKind::Logits,
        dtype: WireDtype::F32,
        inner: vec![geometry.vocabulary],
        rows: RowRule::PerToken,
    }
}

/// A zero-filled decode message of real shape for a contract, at
/// `position`, with a request ID of `request_id_bytes` bytes. Ratio-2 KV
/// sources add a row when `position` is odd.
///
/// # Errors
///
/// Refuses shapes the codec can't hold.
pub fn zero_decode_message(
    contract: &StageContract,
    position: u64,
    request_id_bytes: usize,
) -> Result<StageMessage, StageRefusal> {
    let mut sections = Vec::new();
    for spec in &contract.sections {
        let (rows, first_row) = match spec.rows {
            RowRule::PerToken => (1, 0),
            RowRule::Append { ratio } => {
                let ratio = u64::from(ratio.max(1));
                ((position + 1) / ratio - position / ratio, position / ratio)
            }
            RowRule::Fixed(rows) => (u64::from(rows), 0),
        };
        let mut shape = vec![rows as u32];
        shape.extend_from_slice(&spec.inner);
        let elements: usize = shape.iter().map(|d| *d as usize).product();
        sections.push(Section {
            kind: spec.kind,
            dtype: spec.dtype,
            shape,
            first_row,
            data: vec![0; elements * spec.dtype.bytes()],
        });
    }
    Ok(StageMessage {
        manifest_digest: [0; 32],
        generation: 0,
        request_id: "r".repeat(request_id_bytes.max(1)),
        attempt: 0,
        stage: contract.stage,
        layers: contract.layers.clone(),
        step: position,
        kind: StepKind::Decode,
        position,
        tokens: vec![0],
        sections,
    })
}

/// Bytes on one boundary for a decode token.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct BoundaryWire {
    /// First layer of the receiving stage.
    pub boundary: u32,
    /// Encoded bytes of each section at a position where ratio-2 sources
    /// complete a row.
    pub sections: Vec<(String, usize)>,
    /// Encoded message bytes when ratio-2 sources complete a row.
    pub peak_bytes: usize,
    /// Mean encoded bytes over two consecutive decode tokens.
    pub mean_bytes: f64,
}

/// Wire bytes per decode token on each boundary of a split.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct WireReport {
    /// The geometry measured.
    pub geometry: V41StageGeometry,
    /// Request ID bytes assumed in each header.
    pub request_id_bytes: usize,
    /// Each forward boundary.
    pub boundaries: Vec<BoundaryWire>,
    /// Bytes the last stage returns to the front for one token's logits.
    pub logits_return_bytes: usize,
    /// Sum of `peak_bytes` over the boundaries plus the logits return.
    pub token_peak_bytes: usize,
}

/// Measures a split by encoding zero-filled messages of the real shapes.
///
/// # Errors
///
/// Refuses shapes the default codec limits can't hold.
pub fn wire_report(
    geometry: &V41StageGeometry,
    split: &[Range<u32>],
    request_id_bytes: usize,
) -> Result<WireReport, StageRefusal> {
    let limits = CodecLimits::default();
    let mut boundaries = Vec::new();
    for (stage, layers) in split.iter().enumerate().skip(1) {
        let contract = stage_contract(geometry, stage as u16, layers.clone());
        let peak = zero_decode_message(&contract, 1, request_id_bytes)?;
        let even = zero_decode_message(&contract, 2, request_id_bytes)?;
        let peak_bytes = encode(&peak, &limits)?.len();
        let even_bytes = encode(&even, &limits)?.len();
        boundaries.push(BoundaryWire {
            boundary: layers.start,
            sections: peak
                .sections
                .iter()
                .map(|s| (s.kind.to_string(), s.wire_bytes()))
                .collect(),
            peak_bytes,
            mean_bytes: (peak_bytes + even_bytes) as f64 / 2.0,
        });
    }
    let logits = logits_spec(geometry);
    let logits_section = Section {
        kind: logits.kind,
        dtype: logits.dtype,
        shape: vec![1, geometry.vocabulary],
        first_row: 0,
        data: vec![0; geometry.vocabulary as usize * logits.dtype.bytes()],
    };
    let logits_return_bytes = crate::stage_state::MESSAGE_HEADER_BYTES
        + request_id_bytes.max(1)
        + 4
        + logits_section.wire_bytes();
    let token_peak_bytes =
        boundaries.iter().map(|b| b.peak_bytes).sum::<usize>() + logits_return_bytes;
    Ok(WireReport {
        geometry: *geometry,
        request_id_bytes,
        boundaries,
        logits_return_bytes,
        token_peak_bytes,
    })
}

// ---------------------------------------------------------------------------
// Group manifest
// ---------------------------------------------------------------------------

/// Schema label of [`V41GroupManifest`].
pub const GROUP_MANIFEST_SCHEMA: &str = "psionic.deepseek_v41.group_manifest.v1";

/// Where an Engram table lives.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EngramLocation {
    /// Host RAM of the machine running the layer's stage; touched rows are
    /// copied to the GPU.
    HostMemory,
}

/// One Engram table's placement.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct V41EngramPlacement {
    /// The Engram layer.
    pub layer: u32,
    /// Where the table lives.
    pub location: EngramLocation,
    /// The machine holding it.
    pub host: String,
    /// Table and scale bytes.
    pub bytes: u64,
}

/// One stage of a group.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct V41StageEntry {
    /// Index in the group.
    pub stage: u16,
    /// First decoder layer.
    pub first_layer: u32,
    /// One past the last decoder layer.
    pub end_layer: u32,
    /// Whether the stage holds the embeddings.
    pub embeddings: bool,
    /// Whether the stage holds the final norm and output head.
    pub head: bool,
    /// The machine.
    pub host: String,
    /// The GPU ordinal on that machine.
    pub gpu: u32,
    /// The GPU memory the plan counted on.
    pub gpu_capacity_bytes: u64,
    /// Weight bytes on the GPU.
    pub gpu_weight_bytes: u64,
    /// Bytes reserved for caches, activations, and workspace.
    pub gpu_runtime_bytes: u64,
    /// Engram tables this stage reads.
    pub engram: Vec<V41EngramPlacement>,
}

impl V41StageEntry {
    /// The stage's layers.
    #[must_use]
    pub const fn layers(&self) -> Range<u32> {
        self.first_layer..self.end_layer
    }
}

/// A V4.1 pipeline group: the checkpoint, the protocol, where the memory
/// numbers came from, and each stage. The group generation is separate: a
/// replacement stage raises it without changing the plan.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct V41GroupManifest {
    /// [`GROUP_MANIFEST_SCHEMA`].
    pub schema: String,
    /// Checkpoint repository.
    pub repo: String,
    /// Checkpoint revision.
    pub revision: String,
    /// SHA-256 of the checkpoint's safetensors index.
    pub index_sha256: String,
    /// Stage-state protocol version.
    pub protocol_version: u16,
    /// Which [`PlacementMemory`] produced the byte counts.
    pub memory_source: String,
    /// Stages in layer order.
    pub stages: Vec<V41StageEntry>,
}

/// Why a manifest is invalid.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ManifestError(pub String);

impl fmt::Display for ManifestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid group manifest: {}", self.0)
    }
}

impl std::error::Error for ManifestError {}

impl V41GroupManifest {
    /// SHA-256 of the manifest's JSON, which every stage message carries.
    #[must_use]
    pub fn digest(&self) -> [u8; 32] {
        let json = serde_json::to_vec(self).unwrap_or_default();
        Sha256::digest(json).into()
    }

    /// [`V41GroupManifest::digest`] in hex.
    #[must_use]
    pub fn digest_hex(&self) -> String {
        hex::encode(self.digest())
    }

    /// The stage that runs a layer.
    #[must_use]
    pub fn stage_of_layer(&self, layer: u32) -> Option<&V41StageEntry> {
        self.stages.iter().find(|s| s.layers().contains(&layer))
    }

    /// What a stage loads from the checkpoint.
    #[must_use]
    pub fn selection(&self, stage: usize) -> Option<V41StageSelection> {
        self.stages.get(stage).map(|s| V41StageSelection {
            layers: s.layers(),
            embeddings: s.embeddings,
            head: s.head,
            mtp: false,
            vision: false,
        })
    }

    /// A stage's inbound contract.
    #[must_use]
    pub fn contract(&self, geometry: &V41StageGeometry, stage: usize) -> Option<StageContract> {
        self.stages
            .get(stage)
            .map(|s| stage_contract(geometry, s.stage, s.layers()))
    }

    /// The decoder layer ranges in stage order.
    #[must_use]
    pub fn split(&self) -> Vec<Range<u32>> {
        self.stages.iter().map(V41StageEntry::layers).collect()
    }

    /// Checks coverage, order, embeddings and head, Engram placement, and
    /// that each stage's planned bytes fit its GPU.
    ///
    /// # Errors
    ///
    /// The first problem found.
    pub fn validate(&self) -> Result<(), ManifestError> {
        let fail = |m: String| Err(ManifestError(m));
        if self.schema != GROUP_MANIFEST_SCHEMA {
            return fail(format!("schema {}", self.schema));
        }
        if self.stages.is_empty() {
            return fail(String::from("no stages"));
        }
        let mut next = 0;
        let last = self.stages.len() - 1;
        for (i, s) in self.stages.iter().enumerate() {
            if usize::from(s.stage) != i {
                return fail(format!("stage {} at index {i}", s.stage));
            }
            if s.first_layer != next || s.end_layer <= s.first_layer {
                return fail(format!(
                    "stage {i} layers {:?} don't follow {next}",
                    s.layers()
                ));
            }
            next = s.end_layer;
            if s.embeddings != (i == 0) {
                return fail(format!("stage {i} embeddings"));
            }
            if s.head != (i == last) {
                return fail(format!("stage {i} head"));
            }
            if s.gpu_weight_bytes.saturating_add(s.gpu_runtime_bytes) > s.gpu_capacity_bytes {
                return fail(format!("stage {i} exceeds its GPU"));
            }
            for e in &s.engram {
                if !ENGRAM_LAYERS.contains(&e.layer) || !s.layers().contains(&e.layer) {
                    return fail(format!("stage {i} holds layer {} Engram", e.layer));
                }
                if e.host != s.host {
                    return fail(format!("layer {} Engram is on another host", e.layer));
                }
            }
        }
        if next != V41_LAYERS {
            return fail(format!("layers end at {next}"));
        }
        for layer in ENGRAM_LAYERS {
            let placed = self
                .stages
                .iter()
                .flat_map(|s| &s.engram)
                .filter(|e| e.layer == layer)
                .count();
            if placed != 1 {
                return fail(format!("layer {layer} Engram placed {placed} times"));
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Placement
// ---------------------------------------------------------------------------

/// Memory a placement counts. [`HeaderBytes`] implements it from checkpoint
/// headers and [`MeasuredMemory`] from W3's measured runtime.
pub trait PlacementMemory {
    /// A short label recorded in the manifest.
    fn source(&self) -> String;
    /// GPU weight bytes of a decoder layer, without Engram tables.
    fn layer_gpu_bytes(&self, layer: u32) -> u64;
    /// Engram table bytes of a layer, which live in host memory.
    fn layer_engram_bytes(&self, layer: u32) -> u64;
    /// Embedding bytes, on the first stage.
    fn embeddings_bytes(&self) -> u64;
    /// Final norm and output head bytes, on the last stage.
    fn head_bytes(&self) -> u64;
    /// GPU bytes beyond weights that a stage running `layers` needs: caches,
    /// activations, kernel workspace, staging, and driver memory.
    fn stage_runtime_bytes(&self, layers: Range<u32>) -> u64;
}

/// A fixed reserve, until measured peaks exist.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeReserve {
    /// Bytes every stage reserves.
    pub per_stage_bytes: u64,
    /// Bytes reserved per decoder layer.
    pub per_layer_bytes: u64,
}

/// Placement memory from the checkpoint's shard headers. MTP and vision are
/// left out.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HeaderBytes {
    layers: Vec<(u64, u64)>,
    embeddings: u64,
    head: u64,
    reserve: RuntimeReserve,
}

#[derive(Deserialize)]
struct ReportFile {
    report: ReportBody,
}

#[derive(Deserialize)]
struct ReportBody {
    layers: Vec<ReportRow>,
    parts: BTreeMap<String, ReportRow>,
}

#[derive(Deserialize)]
struct ReportRow {
    bytes: u64,
    engram_table_bytes: u64,
}

const FLASH_HEADER_REPORT: &str =
    include_str!("testdata/deepseek_v41_weights/weights_report_headers.json");

impl HeaderBytes {
    /// Reads a `weights_report` JSON file (the `report` object at its root,
    /// as `testdata/deepseek_v41_weights/weights_report_headers.json`).
    ///
    /// # Errors
    ///
    /// Refuses JSON that isn't a report with 40 layers, embeddings, and head.
    pub fn from_report_json(json: &str, reserve: RuntimeReserve) -> Result<Self, ManifestError> {
        let file: ReportFile =
            serde_json::from_str(json).map_err(|e| ManifestError(format!("report: {e}")))?;
        if file.report.layers.len() != V41_LAYERS as usize {
            return Err(ManifestError(format!(
                "report has {} layers",
                file.report.layers.len()
            )));
        }
        let part = |name: &str| {
            file.report
                .parts
                .get(name)
                .map(|p| p.bytes)
                .ok_or_else(|| ManifestError(format!("report has no {name}")))
        };
        Ok(Self {
            layers: file
                .report
                .layers
                .iter()
                .map(|l| (l.bytes - l.engram_table_bytes, l.engram_table_bytes))
                .collect(),
            embeddings: part("embeddings")?,
            head: part("head")?,
            reserve,
        })
    }

    /// The pinned Flash checkpoint's header report.
    ///
    /// # Errors
    ///
    /// Only if the retained report is damaged.
    pub fn flash(reserve: RuntimeReserve) -> Result<Self, ManifestError> {
        Self::from_report_json(FLASH_HEADER_REPORT, reserve)
    }
}

impl PlacementMemory for HeaderBytes {
    fn source(&self) -> String {
        String::from("header_bytes")
    }
    fn layer_gpu_bytes(&self, layer: u32) -> u64 {
        self.layers.get(layer as usize).map_or(0, |l| l.0)
    }
    fn layer_engram_bytes(&self, layer: u32) -> u64 {
        self.layers.get(layer as usize).map_or(0, |l| l.1)
    }
    fn embeddings_bytes(&self) -> u64 {
        self.embeddings
    }
    fn head_bytes(&self) -> u64 {
        self.head
    }
    fn stage_runtime_bytes(&self, layers: Range<u32>) -> u64 {
        self.reserve.per_stage_bytes
            + self.reserve.per_layer_bytes * u64::from(layers.end - layers.start)
    }
}

/// Schema label of [`MeasuredMemory`]'s file.
pub const MEASURED_MEMORY_SCHEMA: &str = "psionic.deepseek_v41.placement_memory.v1";

/// The runtime a measured placement input describes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MeasuredRuntime {
    /// Such as `psionic-v41-stage/1+cuda`.
    pub name: String,
    /// The source commit the stage worker was built from.
    pub commit: String,
    /// SHA-256 of the stage worker binary.
    pub binary_sha256: String,
    /// The GPU and its architecture.
    pub gpu: String,
    /// Weight, activation, cache, and residual precision, and the reduction
    /// order.
    pub precision: String,
}

/// One decoder layer as the runtime loaded it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MeasuredLayer {
    pub layer: u32,
    /// Weight and scale bytes on the GPU, routed experts included.
    pub gpu_weight_bytes: u64,
    /// The layer's own request state on the GPU (window ring and mixes).
    pub gpu_state_bytes: u64,
    /// Page-locked host bytes of the layer's Engram table.
    pub engram_host_bytes: u64,
}

/// How a stage's runtime bytes grow, beyond its weights.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MeasuredRuntimeBytes {
    /// Bytes every stage needs whatever its layers or context: the CUDA
    /// context, kernels, constants, fixed scratch, captured graphs, and
    /// selections. The largest residual across the measured stages.
    pub per_stage_bytes: u64,
    /// KV and indexer-key bytes per stored row of a KV source; a source of
    /// ratio `r` stores `max_context / r + 1` rows.
    pub source_row_bytes: u64,
    /// The compressor's pending group, per unit of ratio, on a stage that
    /// holds its source layer.
    pub compressor_bytes_per_ratio: u64,
    /// Scratch bytes per row of the stage's largest KV source (index and
    /// block scores).
    pub score_bytes_per_row: u64,
    /// Step buffer bytes per token a step may carry.
    pub step_token_bytes: u64,
}

/// A measured stage peak the model is checked against.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MeasuredCheck {
    pub first_layer: u32,
    pub end_layer: u32,
    pub max_context: u64,
    pub max_step_tokens: u64,
    /// Device bytes in use at the stage's peak, from `cudaMemGetInfo`.
    pub peak_used_bytes: u64,
}

/// The measured placement input file (W3, #12390).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MeasuredMemoryFile {
    /// [`MEASURED_MEMORY_SCHEMA`].
    pub schema: String,
    /// `provisional` until numerical qualification passes.
    pub status: String,
    pub runtime: MeasuredRuntime,
    pub repo: String,
    pub revision: String,
    pub index_sha256: String,
    /// Where each part lives: GPU weights and caches, Engram on the host.
    pub host_gpu_split: String,
    /// Contexts the stages were loaded with.
    pub measured_contexts: Vec<u64>,
    /// The longest prompt a group's front can send in its one prefill step.
    pub served_max_prompt_tokens: u64,
    /// Decoder layers in order.
    pub layers: Vec<MeasuredLayer>,
    pub embeddings_bytes: u64,
    pub head_bytes: u64,
    pub runtime_bytes: MeasuredRuntimeBytes,
    /// Stage peaks the model must cover.
    pub checks: Vec<MeasuredCheck>,
}

/// Placement memory from W3's measurements of the original-weight CUDA
/// runtime, with independent context and step capacities.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MeasuredMemory {
    file: MeasuredMemoryFile,
    max_context: u64,
    max_step_tokens: u64,
}

/// The measured file for the pinned Flash checkpoint on the RTX PRO 6000.
const FLASH_MEASURED: &str =
    include_str!("testdata/deepseek_v41_weights/placement_memory_measured.json");

impl MeasuredMemory {
    /// Reads a [`MEASURED_MEMORY_SCHEMA`] file for stages loaded with
    /// `max_context` positions and steps of `max_step_tokens`.
    ///
    /// # Errors
    ///
    /// Refuses another schema, another checkpoint, a file without 40
    /// layers, and a zero context or step size.
    pub fn from_json(
        json: &str,
        max_context: u64,
        max_step_tokens: u64,
    ) -> Result<Self, ManifestError> {
        let file: MeasuredMemoryFile = serde_json::from_str(json)
            .map_err(|e| ManifestError(format!("measured memory: {e}")))?;
        if file.schema != MEASURED_MEMORY_SCHEMA {
            return Err(ManifestError(format!(
                "measured memory schema {}",
                file.schema
            )));
        }
        if file.repo != DEEPSEEK_V41_FLASH_REPO
            || file.revision != DEEPSEEK_V41_FLASH_REVISION
            || file.index_sha256 != DEEPSEEK_V41_FLASH_INDEX_SHA256
        {
            return Err(ManifestError(String::from(
                "measured memory is for another checkpoint",
            )));
        }
        if file.layers.len() != V41_LAYERS as usize
            || file
                .layers
                .iter()
                .enumerate()
                .any(|(i, l)| l.layer as usize != i)
        {
            return Err(ManifestError(String::from(
                "measured memory needs layers 0 to 39 in order",
            )));
        }
        if max_context == 0 || max_step_tokens == 0 || max_step_tokens > max_context {
            return Err(ManifestError(format!(
                "context {max_context} with steps of {max_step_tokens}"
            )));
        }
        Ok(Self {
            file,
            max_context,
            max_step_tokens,
        })
    }

    /// The pinned Flash checkpoint's measurements, for a serving stage of
    /// `max_context` positions (its step size equals its context).
    ///
    /// # Errors
    ///
    /// Only if the retained file is damaged, or `max_context` is zero.
    pub fn flash(max_context: u64) -> Result<Self, ManifestError> {
        Self::from_json(FLASH_MEASURED, max_context, max_context)
    }

    /// Measured placement inputs for a bounded-chunk serving profile.
    pub fn flash_sized(max_context: u64, max_step_tokens: u64) -> Result<Self, ManifestError> {
        Self::from_json(FLASH_MEASURED, max_context, max_step_tokens)
    }

    /// The measurements.
    #[must_use]
    pub fn file(&self) -> &MeasuredMemoryFile {
        &self.file
    }

    /// The runtime bytes a stage running `layers` needs at a context and
    /// step size.
    #[must_use]
    pub fn runtime_bytes_at(
        &self,
        layers: Range<u32>,
        max_context: u64,
        max_step_tokens: u64,
    ) -> u64 {
        let r = self.file.runtime_bytes;
        let mut sources = Vec::new();
        for layer in layers.clone() {
            if compress_ratio(layer) == 0 {
                continue;
            }
            if let Some(owner) = kv_owner(layer).filter(|o| !sources.contains(o)) {
                sources.push(owner);
            }
        }
        let mut bytes = r.per_stage_bytes;
        let mut largest = 0;
        for owner in sources {
            let ratio = u64::from(compress_ratio(owner).max(1));
            let rows = max_context / ratio + 1;
            largest = largest.max(rows);
            bytes += rows * r.source_row_bytes;
            if layers.contains(&owner) {
                bytes += ratio * r.compressor_bytes_per_ratio;
            }
        }
        bytes += largest.max(1) * r.score_bytes_per_row;
        bytes += max_step_tokens * r.step_token_bytes;
        bytes
            + layers
                .map(|l| self.file.layers[l as usize].gpu_state_bytes)
                .sum::<u64>()
    }
}

impl PlacementMemory for MeasuredMemory {
    fn source(&self) -> String {
        let step = if self.max_step_tokens == self.max_context {
            String::new()
        } else {
            format!(":step {}", self.max_step_tokens)
        };
        format!(
            "measured:{}@{}:context {}{}:{}",
            self.file.runtime.name,
            self.file.runtime.commit,
            self.max_context,
            step,
            self.file.status
        )
    }
    fn layer_gpu_bytes(&self, layer: u32) -> u64 {
        self.file
            .layers
            .get(layer as usize)
            .map_or(0, |l| l.gpu_weight_bytes)
    }
    fn layer_engram_bytes(&self, layer: u32) -> u64 {
        self.file
            .layers
            .get(layer as usize)
            .map_or(0, |l| l.engram_host_bytes)
    }
    fn embeddings_bytes(&self) -> u64 {
        self.file.embeddings_bytes
    }
    fn head_bytes(&self) -> u64 {
        self.file.head_bytes
    }
    fn stage_runtime_bytes(&self, layers: Range<u32>) -> u64 {
        self.runtime_bytes_at(layers, self.max_context, self.max_step_tokens)
    }
}

/// One GPU a group may use.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GpuSlot {
    /// The machine.
    pub host: String,
    /// The GPU ordinal.
    pub gpu: u32,
    /// Usable GPU memory.
    pub bytes: u64,
}

/// Host memory available for Engram tables on a machine.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostRam {
    /// The machine.
    pub host: String,
    /// Bytes available for Engram tables.
    pub bytes: u64,
}

/// What to place on.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlacementRequest {
    /// GPUs in stage order; stage `i` of a candidate uses `gpus[i]`.
    pub gpus: Vec<GpuSlot>,
    /// Host memory per machine.
    pub hosts: Vec<HostRam>,
    /// Stage counts to try, in order. [`PlacementRequest::new`] uses four,
    /// then eight.
    pub stage_counts: Vec<usize>,
}

impl PlacementRequest {
    /// A request that tries four stages, then eight.
    #[must_use]
    pub fn new(gpus: Vec<GpuSlot>, hosts: Vec<HostRam>) -> Self {
        Self {
            gpus,
            hosts,
            stage_counts: vec![4, 8],
        }
    }
}

/// How a candidate fared.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub enum CandidateOutcome {
    /// Every stage fits.
    Fits,
    /// Fewer GPUs than stages.
    NotEnoughGpus { have: usize, need: usize },
    /// A stage needs more GPU memory than its GPU has.
    GpuShort { stage: usize, need: u64, have: u64 },
    /// A machine lacks the host memory for its Engram tables.
    HostShort { host: String, need: u64, have: u64 },
}

/// One evaluated stage count.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct PlacementCandidate {
    /// Stages.
    pub stages: usize,
    /// Layer ranges chosen, empty if not enough GPUs.
    pub layers: Vec<Range<u32>>,
    /// GPU bytes each stage needs, weights plus runtime reserve.
    pub gpu_bytes: Vec<u64>,
    /// The result.
    pub outcome: CandidateOutcome,
}

/// A placement that fits.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Placement {
    /// Candidates evaluated, in order; the last one fits.
    pub candidates: Vec<PlacementCandidate>,
    /// The manifest of the fitting candidate.
    pub manifest: V41GroupManifest,
}

/// No candidate fits.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlacementRefused {
    /// Every candidate evaluated, with why it didn't fit.
    pub candidates: Vec<PlacementCandidate>,
}

impl fmt::Display for PlacementRefused {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("no placement fits:")?;
        for c in &self.candidates {
            write!(f, " {} stages: {:?};", c.stages, c.outcome)?;
        }
        Ok(())
    }
}

impl std::error::Error for PlacementRefused {}

fn stage_weight_bytes(
    memory: &dyn PlacementMemory,
    layers: &Range<u32>,
    first: bool,
    last: bool,
) -> u64 {
    let mut bytes: u64 = layers.clone().map(|l| memory.layer_gpu_bytes(l)).sum();
    if first {
        bytes += memory.embeddings_bytes();
    }
    if last {
        bytes += memory.head_bytes();
    }
    bytes
}

/// Splits the layers into `gpus.len()` contiguous ranges that minimize the
/// largest share of a GPU used.
fn balanced_split(memory: &dyn PlacementMemory, gpus: &[GpuSlot]) -> Vec<Range<u32>> {
    let n = gpus.len();
    let layers = V41_LAYERS as usize;
    if n == 0 || n > layers {
        return Vec::new();
    }
    let cost = |k: usize, start: usize, end: usize| {
        let range = start as u32..end as u32;
        let need = stage_weight_bytes(memory, &range, k == 0, k == n - 1)
            + memory.stage_runtime_bytes(range);
        need as f64 / gpus[k].bytes.max(1) as f64
    };
    let mut best = vec![vec![f64::INFINITY; layers + 1]; n + 1];
    let mut cut = vec![vec![0usize; layers + 1]; n + 1];
    best[0][0] = 0.0;
    for k in 1..=n {
        for end in k..=layers {
            for start in (k - 1)..end {
                let value = best[k - 1][start].max(cost(k - 1, start, end));
                if value < best[k][end] {
                    best[k][end] = value;
                    cut[k][end] = start;
                }
            }
        }
    }
    let mut ranges = Vec::with_capacity(n);
    let mut end = layers;
    for k in (1..=n).rev() {
        let start = cut[k][end];
        ranges.push(start as u32..end as u32);
        end = start;
    }
    ranges.reverse();
    ranges
}

/// Evaluates one split on the first `split.len()` GPUs and returns the
/// candidate and, if it fits, its manifest.
#[must_use]
pub fn evaluate_split(
    memory: &dyn PlacementMemory,
    gpus: &[GpuSlot],
    hosts: &[HostRam],
    split: &[Range<u32>],
) -> (PlacementCandidate, Option<V41GroupManifest>) {
    let n = split.len();
    if gpus.len() < n || n == 0 {
        return (
            PlacementCandidate {
                stages: n,
                layers: Vec::new(),
                gpu_bytes: Vec::new(),
                outcome: CandidateOutcome::NotEnoughGpus {
                    have: gpus.len(),
                    need: n,
                },
            },
            None,
        );
    }
    let mut stages = Vec::with_capacity(n);
    let mut gpu_bytes = Vec::with_capacity(n);
    let mut outcome = CandidateOutcome::Fits;
    let mut host_need: BTreeMap<&str, u64> = BTreeMap::new();
    for (k, layers) in split.iter().enumerate() {
        let gpu = &gpus[k];
        let weights = stage_weight_bytes(memory, layers, k == 0, k == n - 1);
        let runtime = memory.stage_runtime_bytes(layers.clone());
        let need = weights + runtime;
        gpu_bytes.push(need);
        if need > gpu.bytes && outcome == CandidateOutcome::Fits {
            outcome = CandidateOutcome::GpuShort {
                stage: k,
                need,
                have: gpu.bytes,
            };
        }
        let engram: Vec<V41EngramPlacement> = ENGRAM_LAYERS
            .iter()
            .filter(|l| layers.contains(l))
            .map(|&layer| V41EngramPlacement {
                layer,
                location: EngramLocation::HostMemory,
                host: gpu.host.clone(),
                bytes: memory.layer_engram_bytes(layer),
            })
            .collect();
        for e in &engram {
            *host_need.entry(gpu.host.as_str()).or_default() += e.bytes;
        }
        stages.push(V41StageEntry {
            stage: k as u16,
            first_layer: layers.start,
            end_layer: layers.end,
            embeddings: k == 0,
            head: k == n - 1,
            host: gpu.host.clone(),
            gpu: gpu.gpu,
            gpu_capacity_bytes: gpu.bytes,
            gpu_weight_bytes: weights,
            gpu_runtime_bytes: runtime,
            engram,
        });
    }
    if outcome == CandidateOutcome::Fits {
        for (host, need) in &host_need {
            let have = hosts
                .iter()
                .find(|h| h.host == *host)
                .map_or(0, |h| h.bytes);
            if *need > have {
                outcome = CandidateOutcome::HostShort {
                    host: (*host).to_owned(),
                    need: *need,
                    have,
                };
                break;
            }
        }
    }
    let manifest = (outcome == CandidateOutcome::Fits).then(|| V41GroupManifest {
        schema: GROUP_MANIFEST_SCHEMA.to_owned(),
        repo: DEEPSEEK_V41_FLASH_REPO.to_owned(),
        revision: DEEPSEEK_V41_FLASH_REVISION.to_owned(),
        index_sha256: DEEPSEEK_V41_FLASH_INDEX_SHA256.to_owned(),
        protocol_version: V41_STAGE_PROTOCOL,
        memory_source: memory.source(),
        stages,
    });
    (
        PlacementCandidate {
            stages: n,
            layers: split.to_vec(),
            gpu_bytes,
            outcome,
        },
        manifest,
    )
}

/// Tries each stage count in order (four, then eight, by default) with a
/// balanced split, and returns the first that fits.
///
/// # Errors
///
/// Refuses when no candidate fits, listing each one's outcome.
pub fn plan_group(
    memory: &dyn PlacementMemory,
    request: &PlacementRequest,
) -> Result<Placement, PlacementRefused> {
    let mut candidates = Vec::new();
    for &n in &request.stage_counts {
        if request.gpus.len() < n || n == 0 || n > V41_LAYERS as usize {
            candidates.push(PlacementCandidate {
                stages: n,
                layers: Vec::new(),
                gpu_bytes: Vec::new(),
                outcome: CandidateOutcome::NotEnoughGpus {
                    have: request.gpus.len(),
                    need: n,
                },
            });
            continue;
        }
        let gpus = &request.gpus[..n];
        let split = balanced_split(memory, gpus);
        let (candidate, manifest) = evaluate_split(memory, gpus, &request.hosts, &split);
        candidates.push(candidate);
        if let Some(manifest) = manifest {
            return Ok(Placement {
                candidates,
                manifest,
            });
        }
    }
    Err(PlacementRefused { candidates })
}

#[cfg(test)]
#[path = "deepseek_v41_stage_tests.rs"]
mod tests;
