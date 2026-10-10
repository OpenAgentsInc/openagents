//! Clef decision lane: Cloudflare's open-weight Clef decision models served
//! natively at `POST /v1/systemone` (psionic `docs/CLEF_NATIVE_PLAN.md`,
//! milestone M1, psionic#1159).
//!
//! A Clef model reads one prompt in a single prefill, generates nothing, and
//! returns one logit per allowed option of every question. This lane:
//!
//! - admits a `general.architecture = clef` GGUF (the
//!   `ggml-org/Clef-Flash-GGUF` layout), or a `qwen35` GGUF with Cloudflare's
//!   `joint_head.safetensors` beside it ([`ClefHeadSource`]);
//! - builds the prompt byte for byte as the reference `encode_record` does
//!   ([`encode`]);
//! - runs the Qwen3.5 backbone as a chunked prefill: on CUDA
//!   ([`crate::ClefCudaTrunk`], milestone M2, #11195) with the head's memory
//!   rows kept on the device, or on the CPU, streaming each final hidden row
//!   into the head ([`head::ClefHeadStream`]);
//! - runs the joint head in f32 and answers in the Jev / System One shape,
//!   with a `psionic` provenance block in every answer.
//!
//! Admission is by tokens: a prompt over the budget (default 16,384, the
//! head's trained length) is refused with `not_admitted` so a judge chain
//! moves to its next door, unless the request asks for
//! `truncation: "state_tail"`. Confidence is the reference's: the top
//! probability. Read probabilities, not `confidence`.

pub mod calibration;
pub mod encode;
pub mod head;
pub mod json;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use psionic_catalog::{BlobIntegrityPolicy, LocalBlobOpenOptions};
use psionic_models::{GgufBlobArtifact, GgufContent, TokenId};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::{ClefCudaHeadParams, ClefCudaTrunk, CpuGgufQwen35TextGenerationService};
use calibration::{ClefCalibration, HeadInputs, RowExport, request_digest};
use encode::{
    BudgetRefusal, ClefQuestion, ClefRequest, EncodedRecord, QuestionType, RequestLimits,
    RequestRefusal, TRAINED_LENGTH, encode_record,
};
use head::{
    ClefHeadStream, ClefHeadWeights, HEAD_NORM_EPSILON, MemoryAttention, MemoryView, head_spans,
    probabilities, run_head,
};

/// Where the backbone runs (`--decision-device`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ClefDevice {
    /// CUDA when a device is present and the trunk loads, else the CPU.
    #[default]
    Auto,
    Cpu,
    Cuda,
}

impl std::str::FromStr for ClefDevice {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "auto" => Ok(Self::Auto),
            "cpu" => Ok(Self::Cpu),
            "cuda" => Ok(Self::Cuda),
            other => Err(format!("unknown decision device `{other}` (auto, cpu, cuda)")),
        }
    }
}

/// Default prefill chunk on CUDA.
pub const CUDA_PREFILL_CHUNK: usize = 2048;
/// Default prefill chunk on the CPU.
pub const CPU_PREFILL_CHUNK: usize = 256;

/// The served limits.
#[derive(Clone, Copy, Debug)]
pub struct ClefLimits {
    /// Most prompt tokens admitted (`--decision-max-tokens`).
    pub max_tokens: usize,
    /// Most questions per request.
    pub max_questions: usize,
    /// Most options per choice or score question.
    pub max_options: usize,
    /// Largest request body in bytes.
    pub max_body_bytes: usize,
    /// Requests that may wait behind the one running before `busy`.
    pub max_queue: usize,
    /// Tokens per prefill chunk (`--decision-chunk`); 0 picks the device's
    /// default ([`CUDA_PREFILL_CHUNK`], [`CPU_PREFILL_CHUNK`]). On the CPU, 1
    /// runs the token-at-a-time path.
    pub prefill_chunk: usize,
    /// Where the backbone runs (`--decision-device`).
    pub device: ClefDevice,
    /// CUDA projections accumulate in f16 (`--decision-accumulate f16`,
    /// the default: twice the tensor-core rate on GeForce cards) or f32.
    pub accumulate_f16: bool,
}

impl Default for ClefLimits {
    fn default() -> Self {
        Self {
            max_tokens: TRAINED_LENGTH,
            max_questions: 64,
            max_options: 255,
            max_body_bytes: 8 * 1024 * 1024,
            max_queue: 8,
            prefill_chunk: 0,
            device: ClefDevice::Auto,
            accumulate_f16: true,
        }
    }
}

/// Where the head comes from.
#[derive(Clone, Debug)]
pub enum ClefHeadSource {
    /// The head inside a Clef GGUF.
    Embedded,
    /// Cloudflare's `joint_head.safetensors` (+ `joint_head_config.json`):
    /// a directory or the safetensors file.
    Safetensors(PathBuf),
}

/// A refusal in the `{"error": {"code", "message"}}` shape.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClefRefusal {
    pub status: u16,
    pub code: &'static str,
    pub message: String,
}

impl ClefRefusal {
    fn invalid(message: impl Into<String>) -> Self {
        Self {
            status: 400,
            code: "invalid_request",
            message: message.into(),
        }
    }

    fn not_admitted(status: u16, message: impl Into<String>) -> Self {
        Self {
            status,
            code: "not_admitted",
            message: message.into(),
        }
    }

    fn internal(message: impl Into<String>) -> Self {
        Self {
            status: 500,
            code: "internal",
            message: message.into(),
        }
    }

    /// The JSON error body.
    #[must_use]
    pub fn body(&self) -> Value {
        json!({"error": {"code": self.code, "message": self.message}})
    }
}

impl std::fmt::Display for ClefRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}: {}", self.status, self.code, self.message)
    }
}

/// Whether a GGUF file is a Clef decision artifact (reads only its header).
#[must_use]
pub fn is_clef_gguf(path: &Path) -> bool {
    GgufContent::read_path(path).is_ok_and(|content| content.is_clef_decision_artifact())
}

/// A loaded Clef model.
pub struct ClefDecisionLane {
    id: String,
    artifact_path: PathBuf,
    artifact_digest: String,
    backbone: CpuGgufQwen35TextGenerationService,
    /// The trunk on CUDA; the CPU backbone then only serves embedding and
    /// output rows.
    cuda: Option<ClefCudaTrunk>,
    head: ClefHeadWeights,
    limits: ClefLimits,
    /// Calibration maps (`--decision-calibration`), one per noul question.
    calibrations: Vec<ClefCalibration>,
    /// Hidden-row export (`--decision-export-rows`).
    export: Option<RowExport>,
    waiting: AtomicUsize,
    running: std::sync::Mutex<()>,
    /// Logits of recent prompts by [`record_key`]. The trunk and head are
    /// deterministic (a repeat is bitwise identical), so a repeated prompt
    /// (the same router state, say) is answered without the device.
    logit_cache: std::sync::Mutex<std::collections::VecDeque<([u8; 32], Vec<Vec<f32>>)>>,
}

/// Prompts whose logits [`ClefDecisionLane`] keeps.
const LOGIT_CACHE_ENTRIES: usize = 256;

/// What the logits are a function of: the prompt ids and each question's
/// type and spans.
fn record_key(record: &EncodedRecord) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update((record.input_ids.len() as u64).to_le_bytes());
    for id in &record.input_ids {
        hasher.update(id.to_le_bytes());
    }
    for question in &record.questions {
        hasher.update((question.question_type as u64).to_le_bytes());
        let (start, end) = question.question_span;
        hasher.update((start as u64).to_le_bytes());
        hasher.update((end as u64).to_le_bytes());
        hasher.update((question.option_spans.len() as u64).to_le_bytes());
        for (start, end) in &question.option_spans {
            hasher.update((*start as u64).to_le_bytes());
            hasher.update((*end as u64).to_le_bytes());
        }
    }
    hasher.finalize().into()
}

impl std::fmt::Debug for ClefDecisionLane {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClefDecisionLane")
            .field("id", &self.id)
            .field("artifact", &self.artifact_path)
            .field("head", &self.head.source)
            .finish_non_exhaustive()
    }
}

fn sha256_file(path: &Path) -> Result<String, String> {
    use std::io::Read as _;
    let mut file =
        std::fs::File::open(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 8 << 20];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|error| format!("{}: {error}", path.display()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("sha256:{}", hex::encode(hasher.finalize())))
}

fn model_id_from_name(name: Option<&str>, path: &Path) -> String {
    let base = name.map(str::to_string).unwrap_or_else(|| {
        path.file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or_else(|| String::from("clef"))
    });
    base.trim()
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join("-")
}

impl ClefDecisionLane {
    /// Loads a Clef model for CPU decisions.
    pub fn load(
        gguf_path: &Path,
        head_source: ClefHeadSource,
        limits: ClefLimits,
    ) -> Result<Self, String> {
        let artifact = GgufBlobArtifact::open_path(
            gguf_path,
            LocalBlobOpenOptions::default()
                .with_integrity_policy(BlobIntegrityPolicy::LocalUnverifiedLabel),
        )
        .map_err(|error| format!("{}: {error}", gguf_path.display()))?;
        let content = artifact.content();
        let is_clef = content.is_clef_decision_artifact();
        let architecture = content
            .metadata()
            .get("general.architecture")
            .and_then(|value| match value {
                psionic_models::GgufMetadataValue::String(value) => Some(value.clone()),
                _ => None,
            })
            .unwrap_or_default();
        if architecture != "qwen35" {
            return Err(format!(
                "{}: a Clef backbone must be qwen35 (or a clef GGUF); found `{architecture}`",
                gguf_path.display()
            ));
        }
        let hidden_size = content
            .metadata()
            .get("qwen35.embedding_length")
            .and_then(psionic_models::GgufMetadataValue::as_u64)
            .ok_or_else(|| format!("{}: missing qwen35.embedding_length", gguf_path.display()))?
            as usize;
        let head = match &head_source {
            ClefHeadSource::Embedded => {
                if !is_clef {
                    return Err(format!(
                        "{}: a qwen35 GGUF has no decision head; pass the Clef head file (joint_head.safetensors)",
                        gguf_path.display()
                    ));
                }
                ClefHeadWeights::from_gguf(&artifact, hidden_size)
            }
            ClefHeadSource::Safetensors(path) => {
                ClefHeadWeights::from_safetensors(path, hidden_size)
            }
        }
        .map_err(|error| format!("{}: {error}", gguf_path.display()))?;
        let name = content
            .metadata()
            .get("general.name")
            .and_then(|value| match value {
                psionic_models::GgufMetadataValue::String(value) => Some(value.as_str()),
                _ => None,
            });
        let id = model_id_from_name(name, gguf_path);
        drop(artifact);
        let backbone = CpuGgufQwen35TextGenerationService::from_gguf_path(gguf_path)
            .map_err(|error| format!("{}: {error}", gguf_path.display()))?;
        if !backbone.has_untied_output() {
            return Err(format!(
                "{}: the Clef head reads option rows of an untied output projection, and this artifact has none",
                gguf_path.display()
            ));
        }
        let cuda = match limits.device {
            ClefDevice::Cpu => None,
            device => {
                let params = ClefCudaHeadParams {
                    hidden_norm_weight: &head.hidden_norm.weight,
                    hidden_norm_bias: &head.hidden_norm.bias,
                    memory_projection: &head.memory_projection.values,
                    width: head.config.width,
                    evidence_norms: head
                        .evidence_layers
                        .iter()
                        .map(|layer| {
                            (
                                layer.memory_norm.weight.as_slice(),
                                layer.memory_norm.bias.as_slice(),
                            )
                        })
                        .collect(),
                    norm_epsilon: HEAD_NORM_EPSILON,
                    linear_matrices: head
                        .linear_matrices()
                        .into_iter()
                        .map(|matrix| (matrix.values.as_slice(), matrix.rows, matrix.columns))
                        .collect(),
                };
                match ClefCudaTrunk::load(
                    &backbone,
                    &params,
                    limits.accumulate_f16,
                ) {
                    Ok(trunk) => Some(trunk),
                    Err(error) if device == ClefDevice::Auto => {
                        eprintln!(
                            "{}: CUDA decision trunk unavailable ({error}); deciding on the CPU",
                            gguf_path.display()
                        );
                        None
                    }
                    Err(error) => {
                        return Err(format!("{}: CUDA decision trunk: {error}", gguf_path.display()));
                    }
                }
            }
        };
        let artifact_digest = sha256_file(gguf_path)?;
        Ok(Self {
            id,
            artifact_path: gguf_path.to_path_buf(),
            artifact_digest,
            backbone,
            cuda,
            head,
            limits,
            calibrations: Vec::new(),
            export: None,
            waiting: AtomicUsize::new(0),
            running: std::sync::Mutex::new(()),
            logit_cache: std::sync::Mutex::new(std::collections::VecDeque::new()),
        })
    }

    /// Applies calibration maps. A map that names another head than this
    /// lane's is refused: its numbers describe a different model.
    pub fn with_calibrations(mut self, maps: Vec<ClefCalibration>) -> Result<Self, String> {
        for map in &maps {
            if let Some(head) = &map.head_digest {
                if head != &self.head.digest {
                    return Err(format!(
                        "calibration {} was fitted on head {head}; this lane serves head {}",
                        map.id, self.head.digest
                    ));
                }
            }
            if self.calibrations.iter().any(|m| m.instructions == map.instructions) {
                return Err(format!("two calibration maps name the question `{}`", map.instructions));
            }
            self.calibrations.push(map.clone());
        }
        Ok(self)
    }

    /// Exports every decision's pooled head inputs to `dir`.
    pub fn with_row_export(mut self, dir: &Path) -> Result<Self, String> {
        self.export = Some(RowExport::open(dir)?);
        Ok(self)
    }

    /// The model id `/v1/models` lists (from `general.name`, e.g.
    /// `clef-flash`).
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    #[must_use]
    pub fn limits(&self) -> ClefLimits {
        self.limits
    }

    /// `cuda` or `cpu`.
    #[must_use]
    pub fn backend(&self) -> &'static str {
        if self.cuda.is_some() { "cuda" } else { "cpu" }
    }

    /// The prefill chunk in effect.
    #[must_use]
    pub fn prefill_chunk(&self) -> usize {
        match (self.limits.prefill_chunk, self.cuda.is_some()) {
            (0, true) => CUDA_PREFILL_CHUNK,
            (0, false) => CPU_PREFILL_CHUNK,
            (chunk, _) => chunk,
        }
    }

    #[must_use]
    pub fn head(&self) -> &ClefHeadWeights {
        &self.head
    }

    /// Tokenizes one prompt piece (no BOS/EOS, special markers parsed).
    #[must_use]
    pub fn tokenize(&self, text: &str) -> Vec<u32> {
        self.backbone
            .runtime_tokenizer()
            .encode_with_special_tokens(text, false, false)
            .as_slice()
            .iter()
            .map(|token| token.as_u32())
            .collect()
    }

    /// Reads and validates a request body.
    pub fn parse_request(&self, body: &str) -> Result<ClefRequest, ClefRefusal> {
        let parsed = json::parse(body).map_err(|error| ClefRefusal::invalid(error.to_string()))?;
        ClefRequest::from_json(
            &parsed,
            RequestLimits {
                max_questions: self.limits.max_questions,
                max_options: self.limits.max_options,
            },
        )
        .map_err(|refusal| match refusal {
            RequestRefusal::Invalid(message) => ClefRefusal::invalid(message),
            RequestRefusal::NotAdmitted(message) => ClefRefusal::not_admitted(400, message),
        })
    }

    /// Encodes a request under this lane's token budget.
    pub fn encode(&self, request: &ClefRequest) -> Result<EncodedRecord, ClefRefusal> {
        let tokenize = |text: &str| self.tokenize(text);
        encode_record(request, &tokenize, self.limits.max_tokens).map_err(
            |BudgetRefusal {
                 prompt_tokens,
                 fixed_tokens,
                 budget,
             }| {
                let message = if fixed_tokens > budget {
                    format!(
                        "the schema alone is {fixed_tokens} Clef tokens; this server admits {budget}"
                    )
                } else {
                    format!(
                        "the prompt is {prompt_tokens} Clef tokens; this server admits {budget} (send truncation: \"state_tail\" to cut the state)"
                    )
                };
                ClefRefusal::not_admitted(413, message)
            },
        )
    }

    /// Final hidden rows for a record, streamed into the head; returns the
    /// logits per question (prompt option order).
    pub fn logits(&self, record: &EncodedRecord) -> Result<Vec<Vec<f32>>, ClefRefusal> {
        self.logits_observed(record, None, None)
    }

    /// As [`Self::logits`], also handing each hidden row to `observe` (for
    /// parity dumps).
    pub fn logits_with_rows(
        &self,
        record: &EncodedRecord,
        observe: &mut dyn FnMut(usize, &[f32]),
    ) -> Result<Vec<Vec<f32>>, ClefRefusal> {
        self.logits_observed(record, Some(observe), None)
    }

    /// The logits, with optional observers of the final hidden rows and (on
    /// CUDA) of every layer's residual rows `(layer, first_token, rows)`.
    pub fn logits_observed(
        &self,
        record: &EncodedRecord,
        observe: Option<&mut dyn FnMut(usize, &[f32])>,
        layers: Option<&mut dyn FnMut(usize, usize, &[f32])>,
    ) -> Result<Vec<Vec<f32>>, ClefRefusal> {
        self.logits_at_chunk(record, self.prefill_chunk(), observe, layers)
    }

    /// As [`Self::logits_observed`] with an explicit prefill chunk (the
    /// chunk-equivalence checks).
    pub fn logits_at_chunk(
        &self,
        record: &EncodedRecord,
        chunk: usize,
        observe: Option<&mut dyn FnMut(usize, &[f32])>,
        layers: Option<&mut dyn FnMut(usize, usize, &[f32])>,
    ) -> Result<Vec<Vec<f32>>, ClefRefusal> {
        self.logits_capturing(record, chunk, observe, layers, None)
    }

    /// As [`Self::logits_at_chunk`], also capturing the head's pooled inputs.
    fn logits_capturing(
        &self,
        record: &EncodedRecord,
        chunk: usize,
        mut observe: Option<&mut dyn FnMut(usize, &[f32])>,
        layers: Option<&mut dyn FnMut(usize, usize, &[f32])>,
        capture: Option<&mut HeadInputs>,
    ) -> Result<Vec<Vec<f32>>, ClefRefusal> {
        let tokens: Vec<TokenId> = record.input_ids.iter().map(|id| TokenId(*id)).collect();
        let lexical = |ids: &[u32]| {
            self.backbone
                .output_embedding_rows(ids)
                .map_err(|error| error.to_string())
        };
        if let Some(trunk) = &self.cuda {
            let spans = head_spans(record);
            let width = self.head.config.hidden_size;
            let mut final_rows = observe.as_mut().map(|observe| {
                move |_: usize, first: usize, rows: &[f32]| {
                    for (offset, row) in rows.chunks(width).enumerate() {
                        observe(first + offset, row);
                    }
                }
            });
            // The head's lexical vectors (each option's mean output-embedding
            // row, decoded on the host) need only the token ids, so they are
            // built on a second thread while the device runs the prefill.
            // Keyed by the option span's ids (address and length), and summed
            // in the order the head sums them.
            let option_ids: Vec<&[u32]> = record
                .questions
                .iter()
                .flat_map(|question| {
                    question
                        .option_spans
                        .iter()
                        .map(|(start, end)| &record.input_ids[*start..*end])
                })
                .collect();
            let (prefill, lexical_means) = std::thread::scope(|scope| {
                let means = scope.spawn(|| -> Result<Vec<((usize, usize), Vec<f32>)>, String> {
                    option_ids
                        .iter()
                        .map(|ids| {
                            let rows = lexical(ids)?;
                            let mut mean = vec![0.0f32; self.head.config.hidden_size];
                            for row in &rows {
                                for (target, value) in mean.iter_mut().zip(row) {
                                    *target += value;
                                }
                            }
                            let count = rows.len().max(1) as f32;
                            mean.iter_mut().for_each(|value| *value /= count);
                            Ok(((ids.as_ptr() as usize, ids.len()), mean))
                        })
                        .collect()
                });
                let prefill = trunk.prefill(
                    &self.backbone,
                    &tokens,
                    &spans,
                    chunk,
                    layers,
                    final_rows
                        .as_mut()
                        .map(|f| f as &mut dyn FnMut(usize, usize, &[f32])),
                );
                (prefill, means.join())
            });
            let prefill =
                prefill.map_err(|error| ClefRefusal::internal(format!("cuda backbone: {error}")))?;
            let lexical_means: std::collections::HashMap<(usize, usize), Vec<f32>> = lexical_means
                .map_err(|_| ClefRefusal::internal("the lexical thread panicked"))?
                .map_err(ClefRefusal::internal)?
                .into_iter()
                .collect();
            let span_means: Vec<Vec<f32>> = spans
                .iter()
                .zip(prefill.span_sums.chunks(width))
                .map(|((start, end), sum)| {
                    let count = end.saturating_sub(*start).max(1) as f32;
                    sum.iter().map(|value| value / count).collect()
                })
                .collect();
            if let Some(capture) = capture {
                capture.span_means.clone_from(&span_means);
                capture.last.clone_from(&prefill.last);
            }
            let head_began = Instant::now();
            let mut memory = DeviceMemory::new(trunk);
            let lexical_time = std::cell::Cell::new(0.0f64);
            let timed_lexical = |ids: &[u32]| {
                let began = Instant::now();
                // the prebuilt mean as one row: the head's mean of it is
                // the same value
                let rows = match lexical_means.get(&(ids.as_ptr() as usize, ids.len())) {
                    Some(mean) => Ok(vec![mean.clone()]),
                    None => lexical(ids),
                };
                lexical_time.set(lexical_time.get() + began.elapsed().as_secs_f64());
                rows
            };
            let logits = run_head(
                &self.head,
                record,
                &span_means,
                &prefill.last,
                &mut memory,
                &timed_lexical,
            )
            .map_err(ClefRefusal::internal);
            if std::env::var_os("PSIONIC_CLEF_PROFILE").is_some() {
                eprintln!(
                    "clef head: {:.1} ms (lexical rows {:.1} ms; {} linears {:.1} ms; {} memory attentions {:.1} ms)",
                    head_began.elapsed().as_secs_f64() * 1e3,
                    lexical_time.get() * 1e3,
                    memory.linear.0,
                    memory.linear.1 * 1e3,
                    memory.attend.0,
                    memory.attend.1 * 1e3,
                );
            }
            return logits;
        }
        let mut stream = ClefHeadStream::new(&self.head, record);
        let mut push_error = None;
        let mut sink = |index: usize, row: &[f32]| {
            if let Some(observe) = observe.as_mut() {
                observe(index, row);
            }
            if let Err(error) = stream.push(index, row) {
                push_error.get_or_insert(error);
            }
            Ok(())
        };
        match layers {
            Some(layers) => self.backbone.stream_layer_rows(
                &tokens,
                chunk,
                &mut sink,
                layers,
            ),
            None => self
                .backbone
                .stream_final_hidden_rows(&tokens, chunk, &mut sink),
        }
        .map_err(|error| ClefRefusal::internal(format!("backbone: {error}")))?;
        if let Some(error) = push_error {
            return Err(ClefRefusal::internal(error));
        }
        if let Some(capture) = capture {
            let (span_means, last) = stream.pooled();
            capture.span_means = span_means;
            capture.last = last;
        }
        stream
            .finish(record, &lexical)
            .map_err(ClefRefusal::internal)
    }

    /// Answers one request body (blocking; one decision runs at a time).
    pub fn decide(&self, body: &str) -> Result<Value, ClefRefusal> {
        let began = Instant::now();
        let request = self.parse_request(body)?;
        let record = self.encode(&request)?;
        if std::env::var_os("PSIONIC_CLEF_PROFILE").is_some() {
            eprintln!(
                "clef request: parse + encode {:.1} ms",
                began.elapsed().as_secs_f64() * 1e3
            );
        }
        let queued = self.waiting.fetch_add(1, Ordering::SeqCst);
        let _waiting = WaitingGuard(&self.waiting);
        if queued > self.limits.max_queue {
            return Err(ClefRefusal {
                status: 503,
                code: "busy",
                message: format!("{queued} decisions are already waiting"),
            });
        }
        let key = record_key(&record);
        let cached = if self.export.is_none() {
            self.logit_cache
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .iter()
                .find(|(k, _)| *k == key)
                .map(|(_, logits)| logits.clone())
        } else {
            None
        };
        let waited_began = Instant::now();
        let mut waited = 0.0;
        let mut inputs = HeadInputs::default();
        let hit = cached.is_some();
        let logits = match cached {
            Some(logits) => logits,
            None => {
                let _running = self
                    .running
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                waited = waited_began.elapsed().as_secs_f64();
                let capture = self.export.as_ref().map(|_| &mut inputs);
                let logits =
                    self.logits_capturing(&record, self.prefill_chunk(), None, None, capture)?;
                if self.export.is_none() {
                    let mut cache = self
                        .logit_cache
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    if cache.len() >= LOGIT_CACHE_ENTRIES {
                        cache.pop_front();
                    }
                    cache.push_back((key, logits.clone()));
                }
                logits
            }
        };
        if std::env::var("PSIONIC_CLEF_LOG").map_or(true, |value| value != "0") {
            // sizes and times only, never request text
            eprintln!(
                "clef decision: {} tokens, {} questions, waited {:.0} ms, ran {:.0} ms{}",
                record.input_ids.len(),
                record.questions.len(),
                waited * 1e3,
                (began.elapsed().as_secs_f64() - waited) * 1e3,
                if hit { ", cached" } else { "" }
            );
        }
        let answer = self.answer(&request, &record, &logits, began);
        if let Some(export) = &self.export {
            let questions: Vec<(String, Vec<String>, Vec<f32>)> = request
                .questions
                .iter()
                .zip(&record.questions)
                .zip(&logits)
                .map(|((question, encoded), logits)| {
                    (question.id.clone(), encoded.option_ids.clone(), logits.clone())
                })
                .collect();
            export
                .write(&request_digest(body), &inputs, &questions, &answer["psionic"])
                .map_err(ClefRefusal::internal)?;
        }
        Ok(answer)
    }

    /// The calibration map for a question, if one names it.
    fn calibration_for(&self, question: &ClefQuestion) -> Option<&ClefCalibration> {
        if question.kind != QuestionType::Noul {
            return None;
        }
        let instructions = question.instructions.as_str()?;
        self.calibrations
            .iter()
            .find(|map| map.instructions == instructions)
    }

    /// The System One answer body for computed logits.
    #[must_use]
    pub fn answer(
        &self,
        request: &ClefRequest,
        record: &EncodedRecord,
        logits: &[Vec<f32>],
        began: Instant,
    ) -> Value {
        let mut answers = serde_json::Map::new();
        let mut raw_answers = serde_json::Map::new();
        let mut calibration_digests = std::collections::BTreeSet::new();
        for ((question, encoded), question_logits) in
            request.questions.iter().zip(&record.questions).zip(logits)
        {
            let probabilities = probabilities(question_logits);
            let by_id: std::collections::HashMap<&str, f64> = encoded
                .option_ids
                .iter()
                .map(String::as_str)
                .zip(probabilities.iter().copied())
                .collect();
            let mut answer = answer_for(question, &by_id);
            if let Some(map) = self.calibration_for(question) {
                let raw = answer["noul"].as_f64().unwrap_or(0.0);
                answer["noul"] = json!(map.map.apply(raw));
                raw_answers.insert(question.id.clone(), json!(raw));
                calibration_digests.insert(map.digest.clone());
            }
            answers.insert(question.id.clone(), answer);
        }
        let prompt_tokens = record.input_ids.len();
        let mut psionic = json!({
            "artifact": self.artifact_path.file_name().map(|name| name.to_string_lossy().into_owned()),
            "artifact_digest": self.artifact_digest,
            "head_source": self.head.source,
            "head_digest": self.head.digest,
            "backend": self.backend(),
            "execution_mode": "native",
            "prefill_chunk": self.prefill_chunk(),
            "accumulate": if self.cuda.is_some() && self.limits.accumulate_f16 { "f16" } else { "f32" },
            "prompt_tokens": prompt_tokens,
            "truncated_state_tokens": record.truncated_state_tokens,
            "trained_length": TRAINED_LENGTH,
            "max_tokens": self.limits.max_tokens,
            "latency_ms": began.elapsed().as_millis() as u64,
        });
        if prompt_tokens > TRAINED_LENGTH {
            psionic["out_of_training_distribution"] = json!(true);
        }
        if !calibration_digests.is_empty() {
            let digests: Vec<&String> = calibration_digests.iter().collect();
            psionic["calibration_digest"] = if digests.len() == 1 {
                json!(digests[0])
            } else {
                json!(digests)
            };
            psionic["raw"] = Value::Object(raw_answers);
        }
        json!({
            "model": if request.model.is_empty() { self.id.clone() } else { request.model.clone() },
            "answers": Value::Object(answers),
            "usage": {"input_tokens": prompt_tokens, "output_tokens": 0, "cached_input_tokens": 0},
            "psionic": psionic,
        })
    }

    /// The `/v1/models` entry.
    #[must_use]
    pub fn model_entry(&self) -> Value {
        json!({
            "id": self.id,
            "object": "model",
            "owned_by": "psionic",
            "capabilities": ["decision"],
            "psionic": {
                "artifact": self.artifact_path.file_name().map(|name| name.to_string_lossy().into_owned()),
                "artifact_digest": self.artifact_digest,
                "head_source": self.head.source,
                "head_digest": self.head.digest,
                "backend": self.backend(),
                "device": self.cuda.as_ref().map(ClefCudaTrunk::device_name),
                "prefill_chunk": self.prefill_chunk(),
                "max_tokens": self.limits.max_tokens,
                "max_questions": self.limits.max_questions,
                "max_options": self.limits.max_options,
                "trained_length": TRAINED_LENGTH,
                "calibrations": self.calibrations.iter().map(|map| json!({
                    "id": map.id,
                    "digest": map.digest,
                    "instructions": map.instructions,
                })).collect::<Vec<_>>(),
            },
        })
    }
}

/// The head's memory attention against the rows the CUDA trunk left on the
/// device.
/// The head's device calls, with time spent in each kind
/// (`PSIONIC_CLEF_PROFILE`).
struct DeviceMemory<'a> {
    trunk: &'a ClefCudaTrunk,
    attend: (usize, f64),
    linear: (usize, f64),
}

impl<'a> DeviceMemory<'a> {
    fn new(trunk: &'a ClefCudaTrunk) -> Self {
        Self {
            trunk,
            attend: (0, 0.0),
            linear: (0, 0.0),
        }
    }
}

impl MemoryAttention for DeviceMemory<'_> {
    fn attend(
        &mut self,
        view: MemoryView,
        queries: &[f32],
        rows: usize,
        scale: f32,
    ) -> Result<Vec<f32>, String> {
        let evidence = match view {
            MemoryView::Evidence(layer) => Some(layer),
            MemoryView::Raw => None,
        };
        let began = Instant::now();
        let out = self.trunk.attend_memory(evidence, queries, rows, scale);
        self.attend.0 += 1;
        self.attend.1 += began.elapsed().as_secs_f64();
        out
    }

    fn linear(
        &mut self,
        matrix: &head::Matrix,
        input: &[f32],
        n: usize,
        bias: Option<&[f32]>,
    ) -> Result<Vec<f32>, String> {
        let began = Instant::now();
        let out = self.linear_inner(matrix, input, n, bias);
        self.linear.0 += 1;
        self.linear.1 += began.elapsed().as_secs_f64();
        out
    }
}

impl DeviceMemory<'_> {
    fn linear_inner(
        &mut self,
        matrix: &head::Matrix,
        input: &[f32],
        n: usize,
        bias: Option<&[f32]>,
    ) -> Result<Vec<f32>, String> {
        let Some(mut out) = self.trunk.head_linear(&matrix.values, input, n)? else {
            return Ok(matrix.apply_rows(input, n, bias));
        };
        if let Some(bias) = bias {
            for row in out.chunks_mut(matrix.rows) {
                for (value, bias) in row.iter_mut().zip(bias) {
                    *value += bias;
                }
            }
        }
        Ok(out)
    }
}

struct WaitingGuard<'a>(&'a AtomicUsize);

impl Drop for WaitingGuard<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

/// One question's answer, following the reference `systemone_answer`
/// (probabilities are not rounded).
fn answer_for(question: &ClefQuestion, by_id: &std::collections::HashMap<&str, f64>) -> Value {
    let p = |key: &str| by_id.get(key).copied().unwrap_or(0.0);
    match question.kind {
        QuestionType::Noul => json!({"type": "noul", "noul": p("true")}),
        QuestionType::Choice => {
            // Python's max() keeps the first of equal values, in request order.
            let mut choice = question.request_order[0].as_str();
            for option in &question.request_order {
                if p(option) > p(choice) {
                    choice = option.as_str();
                }
            }
            let mut probabilities = serde_json::Map::new();
            for option in &question.request_order {
                probabilities.insert(option.clone(), json!(p(option)));
            }
            json!({
                "type": "choice",
                "choice": choice,
                "confidence": p(choice),
                "probabilities": Value::Object(probabilities),
            })
        }
        QuestionType::Score => {
            let mut score = 0.0;
            let mut confidence = 0.0f64;
            let mut probabilities = serde_json::Map::new();
            let mut legend = serde_json::Map::new();
            for (index, level) in question.request_order.iter().enumerate() {
                score += index as f64 * p(level);
                confidence = confidence.max(p(level));
                probabilities.insert(level.clone(), json!(p(level)));
                legend.insert(level.clone(), question.legend[index].to_serde());
            }
            json!({
                "type": "score",
                "score": score,
                "confidence": confidence,
                "legend": Value::Object(legend),
                "probabilities": Value::Object(probabilities),
            })
        }
    }
}

/// The lanes a server answers decisions with.
#[derive(Clone, Debug)]
pub struct ClefLanes {
    lanes: Arc<Vec<Arc<ClefDecisionLane>>>,
}

impl ClefLanes {
    #[must_use]
    pub fn new(lanes: Vec<ClefDecisionLane>) -> Self {
        Self {
            lanes: Arc::new(lanes.into_iter().map(Arc::new).collect()),
        }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.lanes.is_empty()
    }

    /// The lane a request's `model` names. With one lane loaded, any model
    /// name is answered by it (the answer echoes the requested name).
    fn select(&self, body: &str) -> Result<Arc<ClefDecisionLane>, ClefRefusal> {
        if self.lanes.len() == 1 {
            return Ok(Arc::clone(&self.lanes[0]));
        }
        let model = serde_json::from_str::<Value>(body)
            .ok()
            .and_then(|value| {
                value
                    .get("model")
                    .and_then(Value::as_str)
                    .map(str::to_string)
            })
            .unwrap_or_default();
        self.lanes
            .iter()
            .find(|lane| lane.id == model)
            .cloned()
            .ok_or_else(|| ClefRefusal {
                status: 404,
                code: "model_not_found",
                message: format!("no decision model `{model}` is loaded here"),
            })
    }

    fn max_body_bytes(&self) -> usize {
        self.lanes
            .iter()
            .map(|lane| lane.limits.max_body_bytes)
            .max()
            .unwrap_or(ClefLimits::default().max_body_bytes)
    }
}

fn refusal_response(refusal: &ClefRefusal) -> Response {
    let status = StatusCode::from_u16(refusal.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    (status, Json(refusal.body())).into_response()
}

async fn systemone(
    State(lanes): State<ClefLanes>,
    body: Result<String, axum::extract::rejection::StringRejection>,
) -> Response {
    let body = match body {
        Ok(body) => body,
        Err(rejection) if rejection.status() == StatusCode::PAYLOAD_TOO_LARGE => {
            return refusal_response(&ClefRefusal::not_admitted(
                413,
                "the request body is larger than this server takes",
            ));
        }
        Err(rejection) => {
            return refusal_response(&ClefRefusal::invalid(rejection.body_text()));
        }
    };
    let lane = match lanes.select(&body) {
        Ok(lane) => lane,
        Err(refusal) => return refusal_response(&refusal),
    };
    match tokio::task::spawn_blocking(move || lane.decide(&body)).await {
        Ok(Ok(answer)) => (StatusCode::OK, Json(answer)).into_response(),
        Ok(Err(refusal)) => refusal_response(&refusal),
        Err(error) => refusal_response(&ClefRefusal::internal(format!(
            "the decision task failed: {error}"
        ))),
    }
}

async fn list_models(State(lanes): State<ClefLanes>) -> Response {
    let data: Vec<Value> = lanes.lanes.iter().map(|lane| lane.model_entry()).collect();
    (
        StatusCode::OK,
        Json(json!({"object": "list", "data": data})),
    )
        .into_response()
}

async fn health() -> Response {
    (StatusCode::OK, Json(json!({"status": "ok"}))).into_response()
}

/// `POST /v1/systemone` alone, to merge into another router.
pub fn systemone_router(lanes: ClefLanes) -> Router {
    let limit = lanes.max_body_bytes();
    Router::new()
        .route("/v1/systemone", post(systemone))
        .layer(DefaultBodyLimit::max(limit))
        .with_state(lanes)
}

/// A server with only decision models: `/health`, `/v1/models`,
/// `/v1/systemone`.
pub fn decision_router(lanes: ClefLanes) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/v1/models", get(list_models))
        .with_state(lanes.clone())
        .merge(systemone_router(lanes))
}

/// Serves a router with the server's runtime telemetry.
pub async fn serve(listener: tokio::net::TcpListener, router: Router) -> std::io::Result<()> {
    crate::tokio_runtime_telemetry_axum::serve_with_runtime_telemetry(listener, router).await
}

#[cfg(test)]
mod tests;
