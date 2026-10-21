//! Stage-state messages between pipeline stages, and the admission that
//! refuses a bad message before a stage consumes any of its state.
//!
//! A pipeline group splits a model's layers across stage processes. For each
//! step, a stage receives one [`StageMessage`] from the stage before it: the
//! group identity (manifest digest and generation), the request and attempt,
//! the receiving stage and its layer range, the step number and first
//! position, the step's token IDs, and typed [`Section`]s that hold the
//! residual streams and whatever shared state later layers read.
//!
//! The binary codec ([`encode`], [`decode`]) is versioned, little-endian, and
//! bounded by [`CodecLimits`]: every length is checked against the limits and
//! the remaining bytes before anything is allocated. A [`StageInbox`] then
//! checks the message against the stage's [`StageContract`] and its record
//! of the active request. It refuses a duplicate, stale, missing, reordered,
//! or cross-request message, or a section with the wrong dtype, shape, or row
//! continuity, and changes no state when it refuses. A [`StageWorker`]
//! couples an inbox with a [`StageExecutor`], the model-specific half.
//!
//! The model-independent parts live here. `deepseek_v41_stage` supplies the
//! DeepSeek-V4.1 contracts, wire sizes, group manifest, and placement, and
//! `psionic-serve` adapts the Gemma 4 pipeline step to the same message.

use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    fmt,
    ops::Range,
};

use serde::{Deserialize, Serialize};

/// The first four bytes of every encoded stage message.
pub const STAGE_STATE_MAGIC: [u8; 4] = *b"PSST";

/// The codec version this build writes and reads.
pub const STAGE_STATE_VERSION: u16 = 1;

/// Bytes in the fixed message header, before the request ID, the tokens,
/// and the sections.
pub const MESSAGE_HEADER_BYTES: usize =
    4 + 2 + 2 + 32 + 8 + 4 + 2 + 1 + 1 + 4 + 4 + 8 + 8 + 2 + 4 + 2;

/// Bytes in a section header, before its shape and data.
pub const SECTION_HEADER_BYTES: usize = 1 + 1 + 1 + 1 + 4 + 8 + 8;

/// Largest tensor rank a section may declare.
pub const MAX_SECTION_RANK: usize = 4;

/// How many retired `(request, attempt)` pairs an inbox remembers so that a
/// late message from a finished attempt can't start it again.
pub const RETIRED_REQUESTS: usize = 256;

/// Element type of a section's data.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum WireDtype {
    /// IEEE 754 binary32.
    F32,
    /// bfloat16, rounded to nearest even from `f32`.
    Bf16,
    /// Signed 32-bit integer.
    I32,
    /// Unsigned 32-bit integer.
    U32,
}

impl WireDtype {
    /// Bytes per element.
    #[must_use]
    pub const fn bytes(self) -> usize {
        match self {
            Self::F32 | Self::I32 | Self::U32 => 4,
            Self::Bf16 => 2,
        }
    }

    const fn tag(self) -> u8 {
        match self {
            Self::F32 => 1,
            Self::Bf16 => 2,
            Self::I32 => 3,
            Self::U32 => 4,
        }
    }

    const fn from_tag(tag: u8) -> Option<Self> {
        match tag {
            1 => Some(Self::F32),
            2 => Some(Self::Bf16),
            3 => Some(Self::I32),
            4 => Some(Self::U32),
            _ => None,
        }
    }
}

/// What a section holds. Sections that belong to a layer name it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum SectionKind {
    /// Residual streams, `[tokens, streams, hidden]`.
    Residual,
    /// The last sub-block's pre-mix for each token, `[tokens, streams]`. A
    /// layer's attention collapses its input streams with the previous
    /// layer's FFN pre-mix, so the mix crosses with the residual.
    PreMix,
    /// Rows of a source layer's KV that later layers reuse, `[rows, width]`,
    /// appended at the section's `first_row`. For a model with separate keys
    /// and values, these are the keys.
    SourceKv { layer: u32 },
    /// Values that pair with [`SectionKind::SourceKv`] when a model keeps
    /// them separately.
    SourceValue { layer: u32 },
    /// Rows of a source layer's indexer keys, `[rows, width]`, appended at
    /// `first_row`.
    SourceIndexKeys { layer: u32 },
    /// Candidate block IDs a layer chose for each token, `[tokens, blocks]`.
    Candidates { layer: u32 },
    /// The latest indexer selection of a layer for each token,
    /// `[tokens, top_k]`.
    IndexSelection { layer: u32 },
    /// Token history that an n-gram lookup needs before the step's first
    /// token, `[entries]`, oldest first.
    EngramHistory,
    /// Output logits of the last stage, `[tokens, vocabulary]`.
    Logits,
}

impl SectionKind {
    const fn tag(self) -> (u8, u32) {
        match self {
            Self::Residual => (1, 0),
            Self::SourceKv { layer } => (2, layer),
            Self::SourceValue { layer } => (3, layer),
            Self::SourceIndexKeys { layer } => (4, layer),
            Self::Candidates { layer } => (5, layer),
            Self::IndexSelection { layer } => (6, layer),
            Self::EngramHistory => (7, 0),
            Self::Logits => (8, 0),
            Self::PreMix => (9, 0),
        }
    }

    const fn from_tag(tag: u8, layer: u32) -> Option<Self> {
        match (tag, layer) {
            (1, 0) => Some(Self::Residual),
            (2, layer) => Some(Self::SourceKv { layer }),
            (3, layer) => Some(Self::SourceValue { layer }),
            (4, layer) => Some(Self::SourceIndexKeys { layer }),
            (5, layer) => Some(Self::Candidates { layer }),
            (6, layer) => Some(Self::IndexSelection { layer }),
            (7, 0) => Some(Self::EngramHistory),
            (8, 0) => Some(Self::Logits),
            (9, 0) => Some(Self::PreMix),
            _ => None,
        }
    }
}

impl fmt::Display for SectionKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Residual => f.write_str("residual"),
            Self::SourceKv { layer } => write!(f, "layer {layer} KV"),
            Self::SourceValue { layer } => write!(f, "layer {layer} values"),
            Self::SourceIndexKeys { layer } => write!(f, "layer {layer} index keys"),
            Self::Candidates { layer } => write!(f, "layer {layer} candidates"),
            Self::IndexSelection { layer } => write!(f, "layer {layer} index selection"),
            Self::EngramHistory => f.write_str("Engram history"),
            Self::Logits => f.write_str("logits"),
            Self::PreMix => f.write_str("pre-mix"),
        }
    }
}

/// Whether a step is a prefill chunk or one decode token.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StepKind {
    /// Several prompt tokens at once.
    Prefill,
    /// One token.
    Decode,
}

impl StepKind {
    const fn tag(self) -> u8 {
        match self {
            Self::Prefill => 1,
            Self::Decode => 2,
        }
    }

    const fn from_tag(tag: u8) -> Option<Self> {
        match tag {
            1 => Some(Self::Prefill),
            2 => Some(Self::Decode),
            _ => None,
        }
    }
}

/// One typed tensor in a message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Section {
    /// What the section holds.
    pub kind: SectionKind,
    /// Element type of `data`.
    pub dtype: WireDtype,
    /// Dimensions, outermost first. The first dimension is rows or tokens.
    pub shape: Vec<u32>,
    /// For appended rows, the index of the first row. Zero otherwise.
    pub first_row: u64,
    /// Little-endian element bytes; the length is the shape's product times
    /// the dtype's size.
    pub data: Vec<u8>,
}

impl Section {
    /// A section of `f32` values written as `dtype` (`F32` or `Bf16`).
    ///
    /// # Errors
    ///
    /// Refuses an integer dtype or a shape that doesn't match the values.
    pub fn floats(
        kind: SectionKind,
        dtype: WireDtype,
        shape: Vec<u32>,
        first_row: u64,
        values: &[f32],
    ) -> Result<Self, StageRefusal> {
        check_elements(&shape, values.len())?;
        let data = match dtype {
            WireDtype::F32 => values.iter().flat_map(|v| v.to_le_bytes()).collect(),
            WireDtype::Bf16 => values
                .iter()
                .flat_map(|v| half::bf16::from_f32(*v).to_bits().to_le_bytes())
                .collect(),
            WireDtype::I32 | WireDtype::U32 => {
                return Err(StageRefusal::Malformed(String::from(
                    "float values need a float dtype",
                )));
            }
        };
        Ok(Self {
            kind,
            dtype,
            shape,
            first_row,
            data,
        })
    }

    /// A section of `i32` values.
    ///
    /// # Errors
    ///
    /// Refuses a shape that doesn't match the values.
    pub fn i32s(kind: SectionKind, shape: Vec<u32>, values: &[i32]) -> Result<Self, StageRefusal> {
        check_elements(&shape, values.len())?;
        Ok(Self {
            kind,
            dtype: WireDtype::I32,
            shape,
            first_row: 0,
            data: values.iter().flat_map(|v| v.to_le_bytes()).collect(),
        })
    }

    /// A section of `u32` values.
    ///
    /// # Errors
    ///
    /// Refuses a shape that doesn't match the values.
    pub fn u32s(kind: SectionKind, shape: Vec<u32>, values: &[u32]) -> Result<Self, StageRefusal> {
        check_elements(&shape, values.len())?;
        Ok(Self {
            kind,
            dtype: WireDtype::U32,
            shape,
            first_row: 0,
            data: values.iter().flat_map(|v| v.to_le_bytes()).collect(),
        })
    }

    /// Elements in the section.
    #[must_use]
    pub fn elements(&self) -> usize {
        self.data.len() / self.dtype.bytes()
    }

    /// The values as `f32`, widening BF16.
    ///
    /// # Errors
    ///
    /// Refuses an integer section.
    pub fn to_f32(&self) -> Result<Vec<f32>, StageRefusal> {
        match self.dtype {
            WireDtype::F32 => Ok(self
                .data
                .chunks_exact(4)
                .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                .collect()),
            WireDtype::Bf16 => Ok(self
                .data
                .chunks_exact(2)
                .map(|c| half::bf16::from_bits(u16::from_le_bytes([c[0], c[1]])).to_f32())
                .collect()),
            WireDtype::I32 | WireDtype::U32 => Err(self.wrong_type("float")),
        }
    }

    /// The values as `i32`.
    ///
    /// # Errors
    ///
    /// Refuses a section of another dtype.
    pub fn to_i32(&self) -> Result<Vec<i32>, StageRefusal> {
        if self.dtype != WireDtype::I32 {
            return Err(self.wrong_type("i32"));
        }
        Ok(self
            .data
            .chunks_exact(4)
            .map(|c| i32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect())
    }

    /// The values as `u32`.
    ///
    /// # Errors
    ///
    /// Refuses a section of another dtype.
    pub fn to_u32(&self) -> Result<Vec<u32>, StageRefusal> {
        if self.dtype != WireDtype::U32 {
            return Err(self.wrong_type("u32"));
        }
        Ok(self
            .data
            .chunks_exact(4)
            .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect())
    }

    fn wrong_type(&self, wanted: &str) -> StageRefusal {
        StageRefusal::Section {
            kind: self.kind,
            detail: format!("{wanted} values requested from a {:?} section", self.dtype),
        }
    }

    /// Bytes this section takes on the wire.
    #[must_use]
    pub fn wire_bytes(&self) -> usize {
        SECTION_HEADER_BYTES + 4 * self.shape.len() + self.data.len()
    }
}

fn check_elements(shape: &[u32], len: usize) -> Result<(), StageRefusal> {
    let product = shape
        .iter()
        .try_fold(1usize, |acc, d| acc.checked_mul(*d as usize));
    if product != Some(len) {
        return Err(StageRefusal::Malformed(format!(
            "shape {shape:?} does not hold {len} values"
        )));
    }
    Ok(())
}

/// One step's state, sent from one stage to the next.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StageMessage {
    /// SHA-256 of the group manifest the sender serves under.
    pub manifest_digest: [u8; 32],
    /// The group generation; a replacement stage raises it.
    pub generation: u64,
    /// The request this step belongs to.
    pub request_id: String,
    /// The request's attempt number.
    pub attempt: u32,
    /// The receiving stage's index.
    pub stage: u16,
    /// The receiving stage's layers, end exclusive.
    pub layers: Range<u32>,
    /// The step number within the attempt, starting at zero.
    pub step: u64,
    /// Prefill chunk or decode token.
    pub kind: StepKind,
    /// Position of the first token in `tokens`.
    pub position: u64,
    /// The step's token IDs. The front owns sampling and this stream.
    pub tokens: Vec<u32>,
    /// Typed state, at most one section of each kind.
    pub sections: Vec<Section>,
}

impl StageMessage {
    /// The section of a kind, if present.
    #[must_use]
    pub fn section(&self, kind: SectionKind) -> Option<&Section> {
        self.sections.iter().find(|s| s.kind == kind)
    }

    /// The message for the next hop: the same request and step, addressed to
    /// `stage` with its `layers`, carrying `sections`.
    #[must_use]
    pub fn next_hop(&self, stage: u16, layers: Range<u32>, sections: Vec<Section>) -> Self {
        Self {
            manifest_digest: self.manifest_digest,
            generation: self.generation,
            request_id: self.request_id.clone(),
            attempt: self.attempt,
            stage,
            layers,
            step: self.step,
            kind: self.kind,
            position: self.position,
            tokens: self.tokens.clone(),
            sections,
        }
    }

    /// Bytes this message takes on the wire.
    #[must_use]
    pub fn wire_bytes(&self) -> usize {
        MESSAGE_HEADER_BYTES
            + self.request_id.len()
            + 4 * self.tokens.len()
            + self.sections.iter().map(Section::wire_bytes).sum::<usize>()
    }
}

/// Bounds the codec enforces before it allocates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CodecLimits {
    /// Largest encoded message.
    pub max_message_bytes: usize,
    /// Largest request ID, in bytes.
    pub max_request_id_bytes: usize,
    /// Most tokens in one step.
    pub max_tokens: usize,
    /// Most sections in one message.
    pub max_sections: usize,
    /// Largest data of one section.
    pub max_section_bytes: usize,
}

impl Default for CodecLimits {
    /// 256 MiB messages, 128-byte request IDs, 8,192-token steps, and 64
    /// sections.
    fn default() -> Self {
        Self {
            max_message_bytes: 256 << 20,
            max_request_id_bytes: 128,
            max_tokens: 8192,
            max_sections: 64,
            max_section_bytes: 256 << 20,
        }
    }
}

/// Why a stage refused a message. Every refusal leaves the stage's state as
/// it was.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StageRefusal {
    /// A length exceeds a [`CodecLimits`] bound.
    Oversize {
        what: &'static str,
        limit: usize,
        actual: usize,
    },
    /// The bytes don't form a valid message.
    Malformed(String),
    /// The message uses a codec version this build doesn't read.
    UnsupportedVersion(u16),
    /// The message names a different group manifest.
    WrongManifest,
    /// The message is from an older group generation.
    StaleGeneration { current: u64, got: u64 },
    /// The message is from a generation this stage hasn't joined.
    FutureGeneration { current: u64, got: u64 },
    /// The message is addressed to another stage or layer range.
    WrongStage {
        stage: u16,
        layers: Range<u32>,
        got_stage: u16,
        got_layers: Range<u32>,
    },
    /// Another request holds this stage.
    CrossRequest { active: String, got: String },
    /// The message belongs to an attempt that has ended or been replaced.
    StaleAttempt { request_id: String, attempt: u32 },
    /// A newer attempt arrived while the current one is still open.
    AttemptOpen { current: u32, got: u32 },
    /// The step was already consumed.
    Duplicate { step: u64 },
    /// A step is missing before this one, or this one came early.
    OutOfOrder { expected: u64, got: u64 },
    /// The first position doesn't follow the last step.
    PositionMismatch { expected: u64, got: u64 },
    /// The step kind or token count doesn't fit the step.
    Step(String),
    /// A section the contract requires is absent.
    MissingSection(SectionKind),
    /// A section the contract doesn't name is present.
    UnexpectedSection(SectionKind),
    /// A section has the wrong dtype or shape.
    Section { kind: SectionKind, detail: String },
    /// Appended rows don't continue the rows already received.
    RowGap {
        kind: SectionKind,
        expected: u64,
        got: u64,
    },
}

impl fmt::Display for StageRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Oversize {
                what,
                limit,
                actual,
            } => write!(f, "{what} is {actual}, over the limit of {limit}"),
            Self::Malformed(detail) => write!(f, "malformed stage message: {detail}"),
            Self::UnsupportedVersion(v) => write!(f, "unsupported stage message version {v}"),
            Self::WrongManifest => f.write_str("message names a different group manifest"),
            Self::StaleGeneration { current, got } => {
                write!(f, "generation {got} is older than the group's {current}")
            }
            Self::FutureGeneration { current, got } => {
                write!(f, "generation {got} is newer than this stage's {current}")
            }
            Self::WrongStage {
                stage,
                layers,
                got_stage,
                got_layers,
            } => write!(
                f,
                "message for stage {got_stage} layers {got_layers:?} reached stage {stage} layers {layers:?}"
            ),
            Self::CrossRequest { active, got } => {
                write!(f, "request {got} arrived while {active} holds the stage")
            }
            Self::StaleAttempt {
                request_id,
                attempt,
            } => write!(f, "attempt {attempt} of {request_id} has ended"),
            Self::AttemptOpen { current, got } => {
                write!(f, "attempt {got} arrived while attempt {current} is open")
            }
            Self::Duplicate { step } => write!(f, "step {step} was already consumed"),
            Self::OutOfOrder { expected, got } => {
                write!(f, "expected step {expected}, got step {got}")
            }
            Self::PositionMismatch { expected, got } => {
                write!(f, "expected position {expected}, got {got}")
            }
            Self::Step(detail) => write!(f, "invalid step: {detail}"),
            Self::MissingSection(kind) => write!(f, "missing section: {kind}"),
            Self::UnexpectedSection(kind) => write!(f, "unexpected section: {kind}"),
            Self::Section { kind, detail } => write!(f, "section {kind}: {detail}"),
            Self::RowGap {
                kind,
                expected,
                got,
            } => write!(f, "section {kind} starts at row {got}, expected {expected}"),
        }
    }
}

impl std::error::Error for StageRefusal {}

// ---------------------------------------------------------------------------
// Codec
// ---------------------------------------------------------------------------

/// Encodes a message.
///
/// # Errors
///
/// Refuses a message that breaks a limit or whose sections don't match their
/// shapes, so that nothing is sent that [`decode`] would refuse.
pub fn encode(message: &StageMessage, limits: &CodecLimits) -> Result<Vec<u8>, StageRefusal> {
    check_header_limits(message, limits)?;
    let mut kinds = BTreeSet::new();
    for section in &message.sections {
        if !kinds.insert(section.kind) {
            return Err(StageRefusal::Malformed(format!(
                "two {} sections",
                section.kind
            )));
        }
        check_section(section, limits)?;
    }
    let total = message.wire_bytes();
    bound("message bytes", limits.max_message_bytes, total)?;
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(&STAGE_STATE_MAGIC);
    out.extend_from_slice(&STAGE_STATE_VERSION.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&message.manifest_digest);
    out.extend_from_slice(&message.generation.to_le_bytes());
    out.extend_from_slice(&message.attempt.to_le_bytes());
    out.extend_from_slice(&message.stage.to_le_bytes());
    out.push(message.kind.tag());
    out.push(0);
    out.extend_from_slice(&message.layers.start.to_le_bytes());
    out.extend_from_slice(&message.layers.end.to_le_bytes());
    out.extend_from_slice(&message.step.to_le_bytes());
    out.extend_from_slice(&message.position.to_le_bytes());
    out.extend_from_slice(&(message.request_id.len() as u16).to_le_bytes());
    out.extend_from_slice(&(message.tokens.len() as u32).to_le_bytes());
    out.extend_from_slice(&(message.sections.len() as u16).to_le_bytes());
    out.extend_from_slice(message.request_id.as_bytes());
    for token in &message.tokens {
        out.extend_from_slice(&token.to_le_bytes());
    }
    for section in &message.sections {
        let (tag, layer) = section.kind.tag();
        out.push(tag);
        out.push(section.dtype.tag());
        out.push(section.shape.len() as u8);
        out.push(0);
        out.extend_from_slice(&layer.to_le_bytes());
        out.extend_from_slice(&section.first_row.to_le_bytes());
        out.extend_from_slice(&(section.data.len() as u64).to_le_bytes());
        for dim in &section.shape {
            out.extend_from_slice(&dim.to_le_bytes());
        }
        out.extend_from_slice(&section.data);
    }
    debug_assert_eq!(out.len(), total);
    Ok(out)
}

fn bound(what: &'static str, limit: usize, actual: usize) -> Result<(), StageRefusal> {
    if actual > limit {
        return Err(StageRefusal::Oversize {
            what,
            limit,
            actual,
        });
    }
    Ok(())
}

fn check_header_limits(message: &StageMessage, limits: &CodecLimits) -> Result<(), StageRefusal> {
    bound(
        "request ID bytes",
        limits.max_request_id_bytes.min(usize::from(u16::MAX)),
        message.request_id.len(),
    )?;
    bound("tokens", limits.max_tokens, message.tokens.len())?;
    bound(
        "sections",
        limits.max_sections.min(usize::from(u16::MAX)),
        message.sections.len(),
    )?;
    if message.request_id.is_empty() {
        return Err(StageRefusal::Malformed(String::from("empty request ID")));
    }
    if message.tokens.is_empty() {
        return Err(StageRefusal::Malformed(String::from(
            "a step without tokens",
        )));
    }
    if message.layers.start >= message.layers.end {
        return Err(StageRefusal::Malformed(format!(
            "empty layer range {:?}",
            message.layers
        )));
    }
    Ok(())
}

fn check_section(section: &Section, limits: &CodecLimits) -> Result<(), StageRefusal> {
    bound(
        "section bytes",
        limits.max_section_bytes,
        section.data.len(),
    )?;
    if section.shape.is_empty() || section.shape.len() > MAX_SECTION_RANK {
        return Err(StageRefusal::Malformed(format!(
            "{} has rank {}",
            section.kind,
            section.shape.len()
        )));
    }
    let elements = section
        .shape
        .iter()
        .try_fold(1usize, |acc, d| acc.checked_mul(*d as usize))
        .and_then(|n| n.checked_mul(section.dtype.bytes()));
    if elements != Some(section.data.len()) {
        return Err(StageRefusal::Malformed(format!(
            "{} shape {:?} as {:?} does not match {} data bytes",
            section.kind,
            section.shape,
            section.dtype,
            section.data.len()
        )));
    }
    Ok(())
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], StageRefusal> {
        let end = self
            .at
            .checked_add(n)
            .filter(|end| *end <= self.bytes.len())
            .ok_or_else(|| StageRefusal::Malformed(String::from("truncated message")))?;
        let out = &self.bytes[self.at..end];
        self.at = end;
        Ok(out)
    }

    fn u8(&mut self) -> Result<u8, StageRefusal> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> Result<u16, StageRefusal> {
        let b = self.take(2)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }

    fn u32(&mut self) -> Result<u32, StageRefusal> {
        let b = self.take(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    fn u64(&mut self) -> Result<u64, StageRefusal> {
        let b = self.take(8)?;
        let mut a = [0u8; 8];
        a.copy_from_slice(b);
        Ok(u64::from_le_bytes(a))
    }

    fn remaining(&self) -> usize {
        self.bytes.len() - self.at
    }
}

/// Decodes a message, checking every length against `limits` and the bytes
/// that remain before allocating.
///
/// # Errors
///
/// Refuses oversize, truncated, or trailing bytes, an unknown version, tag,
/// or dtype, a section whose data doesn't match its shape, and two sections
/// of the same kind.
pub fn decode(bytes: &[u8], limits: &CodecLimits) -> Result<StageMessage, StageRefusal> {
    bound("message bytes", limits.max_message_bytes, bytes.len())?;
    let mut r = Reader { bytes, at: 0 };
    if r.take(4)? != STAGE_STATE_MAGIC {
        return Err(StageRefusal::Malformed(String::from("not a stage message")));
    }
    let version = r.u16()?;
    if version != STAGE_STATE_VERSION {
        return Err(StageRefusal::UnsupportedVersion(version));
    }
    if r.u16()? != 0 {
        return Err(StageRefusal::Malformed(String::from("unknown flags")));
    }
    let mut manifest_digest = [0u8; 32];
    manifest_digest.copy_from_slice(r.take(32)?);
    let generation = r.u64()?;
    let attempt = r.u32()?;
    let stage = r.u16()?;
    let kind = StepKind::from_tag(r.u8()?)
        .ok_or_else(|| StageRefusal::Malformed(String::from("unknown step kind")))?;
    if r.u8()? != 0 {
        return Err(StageRefusal::Malformed(String::from("reserved byte set")));
    }
    let layers = r.u32()?..r.u32()?;
    let step = r.u64()?;
    let position = r.u64()?;
    let request_len = usize::from(r.u16()?);
    let token_count = r.u32()? as usize;
    let section_count = usize::from(r.u16()?);
    bound("request ID bytes", limits.max_request_id_bytes, request_len)?;
    bound("tokens", limits.max_tokens, token_count)?;
    bound("sections", limits.max_sections, section_count)?;
    let request_id = std::str::from_utf8(r.take(request_len)?)
        .map_err(|_| StageRefusal::Malformed(String::from("request ID is not UTF-8")))?
        .to_owned();
    let token_bytes = r.take(
        token_count
            .checked_mul(4)
            .ok_or_else(|| StageRefusal::Malformed(String::from("token count overflows")))?,
    )?;
    let tokens = token_bytes
        .chunks_exact(4)
        .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect();
    let mut sections = Vec::with_capacity(section_count.min(r.remaining() / SECTION_HEADER_BYTES));
    let mut kinds = BTreeSet::new();
    for _ in 0..section_count {
        let tag = r.u8()?;
        let dtype = WireDtype::from_tag(r.u8()?)
            .ok_or_else(|| StageRefusal::Malformed(String::from("unknown dtype")))?;
        let rank = usize::from(r.u8()?);
        if r.u8()? != 0 {
            return Err(StageRefusal::Malformed(String::from("reserved byte set")));
        }
        let layer = r.u32()?;
        let kind = SectionKind::from_tag(tag, layer)
            .ok_or_else(|| StageRefusal::Malformed(format!("unknown section tag {tag}")))?;
        let first_row = r.u64()?;
        let data_len = usize::try_from(r.u64()?).map_err(|_| StageRefusal::Oversize {
            what: "section bytes",
            limit: limits.max_section_bytes,
            actual: usize::MAX,
        })?;
        bound("section bytes", limits.max_section_bytes, data_len)?;
        if rank == 0 || rank > MAX_SECTION_RANK {
            return Err(StageRefusal::Malformed(format!("{kind} has rank {rank}")));
        }
        let mut shape = Vec::with_capacity(rank);
        for _ in 0..rank {
            shape.push(r.u32()?);
        }
        if data_len > r.remaining() {
            return Err(StageRefusal::Malformed(String::from("truncated message")));
        }
        let section = Section {
            kind,
            dtype,
            shape,
            first_row,
            data: r.take(data_len)?.to_vec(),
        };
        check_section(&section, limits)?;
        if !kinds.insert(kind) {
            return Err(StageRefusal::Malformed(format!("two {kind} sections")));
        }
        sections.push(section);
    }
    if r.remaining() != 0 {
        return Err(StageRefusal::Malformed(format!(
            "{} trailing bytes",
            r.remaining()
        )));
    }
    let message = StageMessage {
        manifest_digest,
        generation,
        request_id,
        attempt,
        stage,
        layers,
        step,
        kind,
        position,
        tokens,
        sections,
    };
    check_header_limits(&message, limits)?;
    Ok(message)
}

// ---------------------------------------------------------------------------
// Contracts and admission
// ---------------------------------------------------------------------------

/// How many rows a section has in a step.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RowRule {
    /// One row per token in the step.
    PerToken,
    /// Rows appended to a growing table. The first section of a request
    /// starts at row zero, each later one at the rows received so far, and
    /// after the step the table holds `(position + tokens) / ratio` rows.
    Append { ratio: u32 },
    /// Exactly this many rows.
    Fixed(u32),
}

/// One section a stage expects in every step.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SectionSpec {
    /// What it holds.
    pub kind: SectionKind,
    /// The only dtype accepted.
    pub dtype: WireDtype,
    /// Dimensions after the first.
    pub inner: Vec<u32>,
    /// How many rows the first dimension has.
    pub rows: RowRule,
}

impl SectionSpec {
    fn expected_rows(&self, tokens: u64, position: u64, received: u64) -> u64 {
        match self.rows {
            RowRule::PerToken => tokens,
            RowRule::Append { ratio } => {
                ((position + tokens) / u64::from(ratio.max(1))).saturating_sub(received)
            }
            RowRule::Fixed(rows) => u64::from(rows),
        }
    }
}

/// What one stage accepts: its index, its layers, and the sections every
/// inbound step must carry, no more and no fewer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StageContract {
    /// The stage's index in the group.
    pub stage: u16,
    /// The stage's layers, end exclusive.
    pub layers: Range<u32>,
    /// Required inbound sections.
    pub sections: Vec<SectionSpec>,
}

impl StageContract {
    /// Bytes of one decode step's message under this contract, for a request
    /// ID of `request_id_bytes` at `position`.
    #[must_use]
    pub fn decode_step_bytes(&self, request_id_bytes: usize, position: u64) -> usize {
        let sections: usize = self
            .sections
            .iter()
            .map(|spec| {
                let rows = match spec.rows {
                    RowRule::PerToken => 1,
                    RowRule::Append { ratio } => {
                        let ratio = u64::from(ratio.max(1));
                        (position + 1) / ratio - position / ratio
                    }
                    RowRule::Fixed(rows) => u64::from(rows),
                };
                let elements =
                    rows as usize * spec.inner.iter().map(|d| *d as usize).product::<usize>();
                SECTION_HEADER_BYTES + 4 * (1 + spec.inner.len()) + elements * spec.dtype.bytes()
            })
            .sum();
        MESSAGE_HEADER_BYTES + request_id_bytes + 4 + sections
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ActiveRequest {
    request_id: String,
    attempt: u32,
    next_step: u64,
    next_position: u64,
    rows: BTreeMap<SectionKind, u64>,
}

/// What an inbox holds, for tests and status pages.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InboxSummary {
    /// The group generation the stage serves.
    pub generation: u64,
    /// `(request, attempt, next step, next position)` of the open request.
    pub active: Option<(String, u32, u64, u64)>,
    /// Rows received per appended section.
    pub rows: BTreeMap<SectionKind, u64>,
    /// Steps admitted since the inbox was created.
    pub admitted: u64,
}

/// A stage's receiving side. It holds one request at a time, as the
/// pipeline runs one request at a time, and refuses before it changes
/// anything.
#[derive(Clone, Debug)]
pub struct StageInbox {
    manifest_digest: [u8; 32],
    generation: u64,
    contract: StageContract,
    limits: CodecLimits,
    active: Option<ActiveRequest>,
    retired: VecDeque<(String, u32)>,
    admitted: u64,
}

impl StageInbox {
    /// An inbox for a stage of the group with this manifest digest, at this
    /// generation.
    #[must_use]
    pub fn new(
        manifest_digest: [u8; 32],
        generation: u64,
        contract: StageContract,
        limits: CodecLimits,
    ) -> Self {
        Self {
            manifest_digest,
            generation,
            contract,
            limits,
            active: None,
            retired: VecDeque::new(),
            admitted: 0,
        }
    }

    /// The stage's contract.
    #[must_use]
    pub fn contract(&self) -> &StageContract {
        &self.contract
    }

    /// The generation the stage serves.
    #[must_use]
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    /// The codec limits.
    #[must_use]
    pub const fn limits(&self) -> &CodecLimits {
        &self.limits
    }

    /// The inbox's state.
    #[must_use]
    pub fn summary(&self) -> InboxSummary {
        InboxSummary {
            generation: self.generation,
            active: self.active.as_ref().map(|a| {
                (
                    a.request_id.clone(),
                    a.attempt,
                    a.next_step,
                    a.next_position,
                )
            }),
            rows: self
                .active
                .as_ref()
                .map(|a| a.rows.clone())
                .unwrap_or_default(),
            admitted: self.admitted,
        }
    }

    /// Checks a message without changing anything.
    ///
    /// # Errors
    ///
    /// Returns the first reason the message can't be consumed.
    pub fn check(&self, message: &StageMessage) -> Result<(), StageRefusal> {
        if message.manifest_digest != self.manifest_digest {
            return Err(StageRefusal::WrongManifest);
        }
        if message.generation < self.generation {
            return Err(StageRefusal::StaleGeneration {
                current: self.generation,
                got: message.generation,
            });
        }
        if message.generation > self.generation {
            return Err(StageRefusal::FutureGeneration {
                current: self.generation,
                got: message.generation,
            });
        }
        if message.stage != self.contract.stage || message.layers != self.contract.layers {
            return Err(StageRefusal::WrongStage {
                stage: self.contract.stage,
                layers: self.contract.layers.clone(),
                got_stage: message.stage,
                got_layers: message.layers.clone(),
            });
        }
        let (next_step, next_position) =
            match &self.active {
                Some(active) if active.request_id != message.request_id => {
                    return Err(StageRefusal::CrossRequest {
                        active: active.request_id.clone(),
                        got: message.request_id.clone(),
                    });
                }
                Some(active) if message.attempt < active.attempt => {
                    return Err(StageRefusal::StaleAttempt {
                        request_id: message.request_id.clone(),
                        attempt: message.attempt,
                    });
                }
                Some(active) if message.attempt > active.attempt => {
                    return Err(StageRefusal::AttemptOpen {
                        current: active.attempt,
                        got: message.attempt,
                    });
                }
                Some(active) => (active.next_step, active.next_position),
                None => {
                    if self.retired.iter().any(|(id, attempt)| {
                        *id == message.request_id && message.attempt <= *attempt
                    }) {
                        return Err(StageRefusal::StaleAttempt {
                            request_id: message.request_id.clone(),
                            attempt: message.attempt,
                        });
                    }
                    (0, 0)
                }
            };
        if message.step < next_step {
            return Err(StageRefusal::Duplicate { step: message.step });
        }
        if message.step > next_step {
            return Err(StageRefusal::OutOfOrder {
                expected: next_step,
                got: message.step,
            });
        }
        if message.position != next_position {
            return Err(StageRefusal::PositionMismatch {
                expected: next_position,
                got: message.position,
            });
        }
        if message.kind == StepKind::Decode && message.tokens.len() != 1 {
            return Err(StageRefusal::Step(format!(
                "a decode step carries {} tokens",
                message.tokens.len()
            )));
        }
        let tokens = message.tokens.len() as u64;
        for section in &message.sections {
            if !self
                .contract
                .sections
                .iter()
                .any(|s| s.kind == section.kind)
            {
                return Err(StageRefusal::UnexpectedSection(section.kind));
            }
        }
        for spec in &self.contract.sections {
            let section = message
                .section(spec.kind)
                .ok_or(StageRefusal::MissingSection(spec.kind))?;
            if section.dtype != spec.dtype {
                return Err(StageRefusal::Section {
                    kind: spec.kind,
                    detail: format!("dtype {:?}, expected {:?}", section.dtype, spec.dtype),
                });
            }
            let received = self
                .active
                .as_ref()
                .and_then(|a| a.rows.get(&spec.kind).copied())
                .unwrap_or(0);
            if let RowRule::Append { .. } = spec.rows {
                if section.first_row != received {
                    return Err(StageRefusal::RowGap {
                        kind: spec.kind,
                        expected: received,
                        got: section.first_row,
                    });
                }
            } else if section.first_row != 0 {
                return Err(StageRefusal::Section {
                    kind: spec.kind,
                    detail: format!(
                        "first row {} on a section that doesn't append",
                        section.first_row
                    ),
                });
            }
            let rows = spec.expected_rows(tokens, message.position, received);
            let mut shape = vec![u32::try_from(rows).unwrap_or(u32::MAX)];
            shape.extend_from_slice(&spec.inner);
            if section.shape != shape {
                return Err(StageRefusal::Section {
                    kind: spec.kind,
                    detail: format!("shape {:?}, expected {shape:?}", section.shape),
                });
            }
        }
        Ok(())
    }

    /// Checks a message and, only if it passes, records it as consumed.
    ///
    /// # Errors
    ///
    /// Returns the refusal from [`StageInbox::check`]; the inbox is unchanged.
    pub fn admit(&mut self, message: StageMessage) -> Result<StageMessage, StageRefusal> {
        self.check(&message)?;
        let active = self.active.get_or_insert_with(|| ActiveRequest {
            request_id: message.request_id.clone(),
            attempt: message.attempt,
            next_step: 0,
            next_position: 0,
            rows: BTreeMap::new(),
        });
        active.next_step += 1;
        active.next_position += message.tokens.len() as u64;
        for spec in &self.contract.sections {
            if let RowRule::Append { .. } = spec.rows {
                let rows = message
                    .section(spec.kind)
                    .map_or(0, |s| u64::from(s.shape[0]));
                *active.rows.entry(spec.kind).or_default() += rows;
            }
        }
        self.admitted += 1;
        Ok(message)
    }

    /// Decodes and admits a message.
    ///
    /// # Errors
    ///
    /// Returns a codec or admission refusal; the inbox is unchanged.
    pub fn admit_bytes(&mut self, bytes: &[u8]) -> Result<StageMessage, StageRefusal> {
        let message = decode(bytes, &self.limits)?;
        self.admit(message)
    }

    /// Ends a request's attempt. A late message from it is then refused. A
    /// reset for a request this stage doesn't hold only records it as ended.
    pub fn reset(&mut self, request_id: &str, attempt: u32) {
        if self
            .active
            .as_ref()
            .is_some_and(|a| a.request_id == request_id && a.attempt <= attempt)
        {
            self.active = None;
        }
        self.retired.retain(|(id, _)| id != request_id);
        if self.retired.len() == RETIRED_REQUESTS {
            self.retired.pop_front();
        }
        self.retired.push_back((request_id.to_owned(), attempt));
    }

    /// Moves the stage to a newer group generation, dropping the open
    /// request. Messages from older generations are refused afterward.
    ///
    /// # Errors
    ///
    /// Refuses a generation that isn't newer.
    pub fn advance_generation(&mut self, generation: u64) -> Result<(), StageRefusal> {
        if generation <= self.generation {
            return Err(StageRefusal::StaleGeneration {
                current: self.generation,
                got: generation,
            });
        }
        self.generation = generation;
        self.active = None;
        self.retired.clear();
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Executor and worker
// ---------------------------------------------------------------------------

/// The model-specific half of a stage: it runs its layers on an admitted
/// message and returns the sections for the next stage, or logits from the
/// last stage.
pub trait StageExecutor {
    /// The executor's own error.
    type Error: std::error::Error;

    /// Runs one admitted step.
    ///
    /// # Errors
    ///
    /// Any failure of the executor.
    fn step(&mut self, message: &StageMessage) -> Result<Vec<Section>, Self::Error>;

    /// Drops a request's state.
    fn reset(&mut self, request_id: &str);
}

/// Why a worker step failed.
#[derive(Debug)]
pub enum WorkerError<E> {
    /// The inbox refused the message; nothing ran.
    Refused(StageRefusal),
    /// The executor failed after the message was admitted. The request can't
    /// continue on this attempt.
    Executor(E),
}

impl<E: fmt::Display> fmt::Display for WorkerError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Refused(refusal) => refusal.fmt(f),
            Self::Executor(error) => write!(f, "stage executor failed: {error}"),
        }
    }
}

impl<E: fmt::Debug + fmt::Display> std::error::Error for WorkerError<E> {}

/// A stage process's step and reset handling: an inbox in front of an
/// executor. This is the model-independent form of the Gemma 4
/// `pipeline/step` and `pipeline/reset` routes.
#[derive(Debug)]
pub struct StageWorker<E> {
    inbox: StageInbox,
    executor: E,
}

impl<E: StageExecutor> StageWorker<E> {
    /// A worker with an inbox and an executor.
    pub fn new(inbox: StageInbox, executor: E) -> Self {
        Self { inbox, executor }
    }

    /// The inbox.
    pub fn inbox(&self) -> &StageInbox {
        &self.inbox
    }

    /// The executor.
    pub fn executor(&self) -> &E {
        &self.executor
    }

    /// Decodes, admits, and runs one step.
    ///
    /// # Errors
    ///
    /// A refusal leaves the inbox and the executor untouched. An executor
    /// failure ends the attempt on this stage.
    pub fn step_bytes(&mut self, bytes: &[u8]) -> Result<Vec<Section>, WorkerError<E::Error>> {
        let message = decode(bytes, &self.inbox.limits).map_err(WorkerError::Refused)?;
        self.step(message)
    }

    /// Admits and runs one step.
    ///
    /// # Errors
    ///
    /// As [`StageWorker::step_bytes`].
    pub fn step(&mut self, message: StageMessage) -> Result<Vec<Section>, WorkerError<E::Error>> {
        let message = self.inbox.admit(message).map_err(WorkerError::Refused)?;
        self.executor.step(&message).map_err(|error| {
            self.inbox.reset(&message.request_id, message.attempt);
            self.executor.reset(&message.request_id);
            WorkerError::Executor(error)
        })
    }

    /// Ends a request's attempt on this stage.
    pub fn reset(&mut self, request_id: &str, attempt: u32) {
        self.inbox.reset(request_id, attempt);
        self.executor.reset(request_id);
    }

    /// Moves to a newer generation and drops every request's state.
    ///
    /// # Errors
    ///
    /// Refuses a generation that isn't newer.
    pub fn advance_generation(&mut self, generation: u64) -> Result<(), StageRefusal> {
        let active = self.inbox.active.as_ref().map(|a| a.request_id.clone());
        self.inbox.advance_generation(generation)?;
        if let Some(request_id) = active {
            self.executor.reset(&request_id);
        }
        Ok(())
    }
}
