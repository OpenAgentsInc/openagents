//! The HTTP surface: TypeSafe-compatible `POST /v1/systemone` and
//! `GET /v1/models`, the separate-eval and debug routes the reference
//! carries, and deterministic refusals.
//!
//! The served model identifies itself as kev; the `jev-latest` alias keeps
//! the wire contract reachable for callers that name TypeSafe's model id,
//! matching the reference server's alias list.
//!
//! A refusal answers with the envelope every System One door shares —
//! `{"detail": …, "error": {"code", "message", "question"}}` — at the
//! status its [`RefusalCode`] class carries. `detail` keeps the FastAPI
//! reference's shape for older readers; `error.code` is the stable label
//! `gym::eval::classify` reads, so a request the door declines is recorded
//! as the door's answer rather than a harness failure.

use std::sync::Arc;
use std::time::Instant;

use tokio::sync::{OwnedSemaphorePermit, Semaphore};

use axum::Router;
use axum::body::Bytes;
use axum::extract::State;
use axum::extract::rejection::BytesRejection;
use axum::http::StatusCode;
use axum::response::Json;
use axum::routing::{get, post};
use indexmap::IndexMap;
use serde_json::{Value, json};
use tokenizers::Tokenizer;

use crate::api::{Answer, Question, Record, SystemOneRequest, to_answers, to_record};
use crate::decision::DecisionModel;
use crate::error::{Bound, Error, MAX_OPTIONS, RefusalCode};

/// The unit the working-memory budget is counted in.
pub const MIB: usize = 1024 * 1024;

/// The state budget serving admits; the reference serves with the training
/// bounds relaxed to the branch ceiling.
pub const INFER_MAX_STATE: usize = 8192;
/// The branch budget serving admits.
pub const INFER_MAX_BRANCH: usize = 8192;

/// One loaded checkpoint: its weights plus the card fields `/v1/models`
/// reports.
pub struct Variant {
    /// The loaded decision model.
    pub model: DecisionModel,
    /// The id requests name, such as `kev-0.5b`.
    pub model_id: String,
    /// The artifact location the listing reports as `run`.
    pub run: String,
    /// The base model id the listing reports.
    pub base: String,
    /// The base checkpoint revision the artifact was trained against, when
    /// `head_meta.json` names one.
    ///
    /// This is what `/v1/models` publishes as `base_model_signature`, and it
    /// is the field a recorded result row uses to say which checkpoint
    /// answered. An artifact that names no revision publishes an empty
    /// string rather than a signature nobody can check.
    pub base_revision: String,
    /// The adapter rank `/api/info` reports.
    pub lora: usize,
}

impl Variant {
    /// The working memory one forward of `tokens` packed tokens needs on this
    /// variant, from its model card: the backbone's head count, widths, and
    /// compute dtype.
    ///
    /// The estimate is the larger of two peaks. Building the mask holds the
    /// `f32` allow matrix and its tensor copy, then the copy in the compute
    /// dtype. The forward holds that mask, three `[heads, tokens, tokens]`
    /// score tensors inside one attention layer (scaled, masked, and
    /// normalized), and the per-token activations: the residual stream and
    /// its normed copy, the query, key, and value projections after
    /// grouped-query expansion, the attention output, and the two SwiGLU
    /// intermediates. Weights are not counted; they are resident before any
    /// request arrives.
    #[must_use]
    pub fn forward_bytes(&self, tokens: usize) -> usize {
        let config = self.model.backbone.config();
        let dtype = self.model.backbone.dtype().size_in_bytes();
        let square = tokens.saturating_mul(tokens);
        let mask_build = square
            .saturating_mul(2 * std::mem::size_of::<f32>())
            .saturating_add(square.saturating_mul(dtype));
        let scores = square
            .saturating_mul(config.num_attention_heads)
            .saturating_mul(3)
            .saturating_mul(dtype);
        let attention_width = config.num_attention_heads.saturating_mul(config.head_dim());
        let per_token = config
            .hidden_size
            .saturating_mul(2)
            .saturating_add(attention_width.saturating_mul(4))
            .saturating_add(config.intermediate_size.saturating_mul(2));
        let activations = tokens.saturating_mul(per_token).saturating_mul(dtype);
        let forward = square
            .saturating_mul(dtype)
            .saturating_add(scores)
            .saturating_add(activations);
        mask_build.max(forward)
    }
}

/// The working memory the host can lend to forwards, measured now.
///
/// On Linux this is `MemAvailable` from `/proc/meminfo`; on macOS it is the
/// free and inactive pages `vm_stat` reports, at the page size it names.
/// Measure it after the weights are loaded so what they hold is already
/// subtracted. `None` when the host offers neither reading.
#[must_use]
pub fn host_memory_budget() -> Option<usize> {
    if cfg!(target_os = "linux") {
        let meminfo = std::fs::read_to_string("/proc/meminfo").ok()?;
        return meminfo_available(&meminfo);
    }
    if cfg!(target_os = "macos") {
        let output = std::process::Command::new("vm_stat").output().ok()?;
        return vm_stat_available(&String::from_utf8_lossy(&output.stdout));
    }
    None
}

/// `MemAvailable` in bytes from the text of `/proc/meminfo`.
fn meminfo_available(meminfo: &str) -> Option<usize> {
    let line = meminfo
        .lines()
        .find_map(|line| line.strip_prefix("MemAvailable:"))?;
    let kib: usize = line.trim().trim_end_matches("kB").trim().parse().ok()?;
    Some(kib.saturating_mul(1024))
}

/// Free plus inactive pages in bytes from the text of `vm_stat`.
fn vm_stat_available(report: &str) -> Option<usize> {
    let page_size: usize = report
        .lines()
        .next()?
        .split("page size of")
        .nth(1)?
        .trim()
        .split(' ')
        .next()?
        .parse()
        .ok()?;
    let pages = |label: &str| -> Option<usize> {
        report
            .lines()
            .find_map(|line| line.strip_prefix(label))?
            .trim()
            .trim_end_matches('.')
            .parse()
            .ok()
    };
    let free = pages("Pages free:")?;
    let inactive = pages("Pages inactive:")?;
    Some(free.saturating_add(inactive).saturating_mul(page_size))
}

#[cfg(test)]
mod host_memory_tests {
    use super::{meminfo_available, vm_stat_available};

    #[test]
    fn meminfo_reads_mem_available() {
        let text = "MemTotal:       32878592 kB\nMemFree:        23775372 kB\nMemAvailable:   31567072 kB\n";
        assert_eq!(meminfo_available(text), Some(31_567_072 * 1024));
        assert_eq!(meminfo_available("MemTotal: 1 kB\n"), None);
        assert_eq!(meminfo_available("MemAvailable: lots\n"), None);
    }

    #[test]
    fn vm_stat_reads_free_and_inactive_pages() {
        let text = "Mach Virtual Memory Statistics: (page size of 16384 bytes)\n\
                    Pages free:                               12345.\n\
                    Pages active:                            100000.\n\
                    Pages inactive:                           54321.\n";
        assert_eq!(vm_stat_available(text), Some((12_345 + 54_321) * 16_384));
        assert_eq!(vm_stat_available("Pages free: 1.\n"), None);
    }
}

/// What one request may cost before the door evaluates it.
///
/// The bounds are checked in this order, cheapest first: question and option
/// counts and the delimiter floor from the request body, then the packed token
/// count and its mask bytes from the encoding, then a forward slot.
#[derive(Clone, Debug)]
pub struct Admission {
    /// Questions one request may carry.
    pub max_questions: usize,
    /// Options summed over every question in one request.
    pub max_total_options: usize,
    /// Packed tokens, state plus every branch, one forward may hold.
    pub max_total_tokens: usize,
    /// Bytes the `f32` attention mask of one packed sequence may need.
    pub max_attention_bytes: usize,
    /// Forwards in flight at once across every loaded variant, the host's
    /// compute bound.
    pub concurrency: usize,
    /// Working memory in bytes every forward in flight may hold together.
    ///
    /// Each variant's share is what one of its forwards needs at
    /// `max_total_tokens`, so the forwards a variant may run at once is this
    /// budget divided by [`Variant::forward_bytes`]. The default is zero,
    /// which [`ServeState::new`] refuses: `kev-serve` fills it from
    /// [`host_memory_budget`] after the weights load, and a test states it.
    pub memory_budget_bytes: usize,
}

impl Default for Admission {
    /// The token and attention-byte defaults coincide at
    /// [`INFER_MAX_BRANCH`], whose mask needs 256 MiB. The memory budget
    /// has no default worth the name; the caller measures or states it.
    fn default() -> Self {
        Self {
            max_questions: 64,
            max_total_options: 1_024,
            max_total_tokens: INFER_MAX_BRANCH,
            max_attention_bytes: Self::attention_bytes(INFER_MAX_BRANCH),
            concurrency: 2,
            memory_budget_bytes: 0,
        }
    }
}

impl Admission {
    /// The `f32` attention mask one packed sequence needs.
    #[must_use]
    pub const fn attention_bytes(tokens: usize) -> usize {
        tokens.saturating_mul(tokens).saturating_mul(4)
    }

    /// The delimiter-token floor for a request shape, not an estimate.
    ///
    /// The floor counts the `<|fim_prefix|>` opener, then
    /// `<|fim_middle|>` and `<|fim_suffix|>` for each question, and
    /// `<|box_start|>` and `<|box_end|>` for each option.
    #[must_use]
    pub const fn token_floor(questions: usize, options: usize) -> usize {
        1usize
            .saturating_add(questions.saturating_mul(2))
            .saturating_add(options.saturating_mul(2))
    }

    /// Admit a request's shape: its question and option counts.
    ///
    /// # Errors
    ///
    /// Returns [`Error::TooManyQuestions`], [`Error::TooManyOptions`],
    /// [`Error::TooManyTotalOptions`], or [`Error::SequenceFloorTooLong`]
    /// when the request shape exceeds a configured bound.
    pub fn admit_shape(&self, request: &SystemOneRequest) -> Result<(), Error> {
        let count = request.questions.len();
        if count > self.max_questions {
            return Err(Error::TooManyQuestions {
                count,
                max: self.max_questions,
            });
        }
        let mut options = 0usize;
        for (id, question) in &request.questions {
            let count = option_count(question);
            if count > MAX_OPTIONS {
                return Err(Error::TooManyOptions {
                    id: id.clone(),
                    count,
                });
            }
            options += count;
        }
        if options > self.max_total_options {
            return Err(Error::TooManyTotalOptions {
                count: options,
                max: self.max_total_options,
            });
        }
        let floor = Self::token_floor(count, options);
        if floor > self.max_total_tokens {
            return Err(Error::SequenceFloorTooLong {
                questions: count,
                options,
                floor,
                max: self.max_total_tokens,
            });
        }
        Ok(())
    }

    /// Admit a packed sequence before its mask exists.
    ///
    /// # Errors
    ///
    /// Returns [`Error::TooManyTokens`] when the packed sequence or its
    /// attention mask exceeds a configured bound.
    pub fn admit_tokens(&self, tokens: usize) -> Result<(), Error> {
        let attention_bytes = Self::attention_bytes(tokens);
        if tokens > self.max_total_tokens || attention_bytes > self.max_attention_bytes {
            return Err(Error::TooManyTokens {
                tokens,
                max: self.max_total_tokens,
                attention_bytes,
            });
        }
        Ok(())
    }
}

/// How many options a question carries: a `choice` question's criteria
/// keys, a `score` question's levels, and none for a `noul`. A shape the
/// contract rejects is refused by [`to_record`] afterwards.
fn option_count(question: &Question) -> usize {
    match question {
        Question::Choice { criteria, .. } => criteria.len(),
        Question::Score { criteria, .. } => criteria.len(),
        Question::Noul { .. } | Question::Other => 0,
    }
}

/// One variant's share of the host: what a forward of it costs and how many
/// may run at once.
pub struct VariantSlots {
    /// Working memory one forward at `Admission::max_total_tokens` needs,
    /// rounded up to whole MiB.
    pub forward_mib: usize,
    /// Forwards of this variant that fit the memory budget at once, no more
    /// than the host's `concurrency`.
    pub limit: usize,
    /// The permits `limit` counts.
    pub slots: Arc<Semaphore>,
}

/// The permits one forward holds until it ends: a host slot, a variant slot,
/// and its share of the memory budget. The permit rides with the blocking
/// task, so a caller that stops waiting releases it when the forward it
/// started ends, not before.
pub struct Permit {
    _host: OwnedSemaphorePermit,
    _variant: OwnedSemaphorePermit,
    _memory: OwnedSemaphorePermit,
}

/// Everything one running server knows beyond the weights.
pub struct ServeState {
    /// Every loaded variant; requests pick one by `model` id.
    pub variants: Vec<Variant>,
    /// The variant `kev-latest` and an absent `model` field resolve to.
    pub default: usize,
    /// Wire-compatible aliases the listing advertises on the default.
    pub aliases: Vec<String>,
    /// The device name `/api/info` reports.
    pub device: String,
    /// The bounds one request is admitted against.
    pub admission: Admission,
    /// The forward slots `admission.concurrency` counts.
    pub slots: Arc<Semaphore>,
    /// Each variant's slots, indexed like `variants`.
    pub variant_slots: Vec<VariantSlots>,
    /// The memory budget in MiB, one permit each.
    pub memory: Arc<Semaphore>,
    /// The permits `memory` started with.
    pub memory_mib: usize,
}

impl ServeState {
    /// A server over `variants`, with `default` naming the one aliases and an
    /// absent `model` field resolve to.
    ///
    /// # Errors
    ///
    /// [`Error::Artifact`] when `variants` is empty, `default` names no
    /// variant, an alias collides with a variant id, `admission` holds a
    /// zero bound, or one forward of a variant at `max_total_tokens` needs
    /// more than the memory budget. In that last case lower the token bound
    /// or serve the variant on a host with more memory; admitting it would
    /// exhaust the process on the first full-length request.
    pub fn new(
        variants: Vec<Variant>,
        default: usize,
        aliases: Vec<String>,
        device: String,
        admission: Admission,
    ) -> Result<Self, Error> {
        if variants.is_empty() {
            return Err(Error::Artifact("no variants loaded".to_string()));
        }
        if default >= variants.len() {
            return Err(Error::Artifact(format!(
                "default variant index {default} names none of {} loaded variants",
                variants.len()
            )));
        }
        if let Some(alias) = aliases
            .iter()
            .find(|alias| variants.iter().any(|v| &v.model_id == *alias))
        {
            return Err(Error::Artifact(format!(
                "alias `{alias}` is also a variant id"
            )));
        }
        if admission.max_questions == 0
            || admission.max_total_options == 0
            || admission.max_total_tokens == 0
            || admission.max_attention_bytes == 0
            || admission.concurrency == 0
            || admission.memory_budget_bytes == 0
        {
            return Err(Error::Artifact(
                "admission bounds must be at least one".to_string(),
            ));
        }
        let memory_mib = admission.memory_budget_bytes.div_ceil(MIB);
        if memory_mib > Semaphore::MAX_PERMITS || u32::try_from(memory_mib).is_err() {
            return Err(Error::Artifact(format!(
                "memory budget of {memory_mib} MiB exceeds what the door can count"
            )));
        }
        let mut variant_slots = Vec::with_capacity(variants.len());
        for variant in &variants {
            let forward_mib = variant
                .forward_bytes(admission.max_total_tokens)
                .div_ceil(MIB)
                .max(1);
            if forward_mib > memory_mib {
                return Err(Error::Artifact(format!(
                    "`{}`: one forward at {} tokens needs {forward_mib} MiB; the memory budget is {memory_mib} MiB",
                    variant.model_id, admission.max_total_tokens
                )));
            }
            let limit = (memory_mib / forward_mib).min(admission.concurrency);
            variant_slots.push(VariantSlots {
                forward_mib,
                limit,
                slots: Arc::new(Semaphore::new(limit)),
            });
        }
        let slots = Arc::new(Semaphore::new(admission.concurrency));
        let memory = Arc::new(Semaphore::new(memory_mib));
        Ok(Self {
            variants,
            default,
            aliases,
            device,
            admission,
            slots,
            variant_slots,
            memory,
            memory_mib,
        })
    }

    /// The permits one forward of variant `index` holds, or [`Error::Busy`]
    /// naming the bound that is saturated. Nothing waits: a refusal is
    /// immediate, so a caller that gives up leaves no queue entry behind.
    ///
    /// # Errors
    ///
    /// [`Error::Busy`] when the host's slots, the variant's slots, or the
    /// memory budget cannot take one more forward.
    ///
    /// # Panics
    ///
    /// When `index` names no loaded variant; [`ServeState::select_index`]
    /// is where an index comes from.
    pub fn permit(&self, index: usize) -> Result<Permit, Error> {
        let host = self
            .slots
            .clone()
            .try_acquire_owned()
            .map_err(|_| Error::Busy {
                bound: Bound::Host,
                in_flight: self.admission.concurrency,
                limit: self.admission.concurrency,
            })?;
        let share = &self.variant_slots[index];
        let variant_permit = share
            .slots
            .clone()
            .try_acquire_owned()
            .map_err(|_| Error::Busy {
                bound: Bound::Variant(self.variants[index].model_id.clone()),
                in_flight: share.limit,
                limit: share.limit,
            })?;
        let busy_memory = || Error::Busy {
            bound: Bound::Memory,
            in_flight: self.memory_in_use_mib(),
            limit: self.memory_mib,
        };
        let mib = u32::try_from(share.forward_mib).map_err(|_| busy_memory())?;
        let memory = self
            .memory
            .clone()
            .try_acquire_many_owned(mib)
            .map_err(|_| busy_memory())?;
        Ok(Permit {
            _host: host,
            _variant: variant_permit,
            _memory: memory,
        })
    }

    /// Forwards of variant `index` in flight now, for operators and tests.
    #[must_use]
    pub fn in_flight(&self, index: usize) -> usize {
        let share = &self.variant_slots[index];
        share.limit - share.slots.available_permits()
    }

    /// MiB of the memory budget held by forwards in flight now.
    #[must_use]
    pub fn memory_in_use_mib(&self) -> usize {
        self.memory_mib - self.memory.available_permits()
    }

    /// Resolve a request's `model` field to a loaded variant: an exact id,
    /// `kev-latest`, any alias the listing advertises, or an absent field.
    /// The aliases the listing publishes and the aliases this resolves are
    /// one list, so a client that keeps its default model reaches the door.
    pub fn select(&self, model: &str) -> Result<&Variant, Error> {
        self.select_index(model).map(|index| &self.variants[index])
    }

    /// The index into `variants` a `model` field resolves to, the same way
    /// [`ServeState::select`] does.
    ///
    /// # Errors
    ///
    /// [`Error::UnknownModel`] when `model` is neither a loaded id,
    /// `kev-latest`, an advertised alias, nor empty.
    pub fn select_index(&self, model: &str) -> Result<usize, Error> {
        if model == "kev-latest" || model.is_empty() || self.aliases.iter().any(|a| a == model) {
            return if self.default < self.variants.len() {
                Ok(self.default)
            } else {
                Err(Error::Artifact(format!(
                    "default variant index {} is not loaded",
                    self.default
                )))
            };
        }
        self.variants
            .iter()
            .position(|v| v.model_id == model)
            .ok_or_else(|| Error::UnknownModel {
                model: model.to_string(),
                known: self
                    .variants
                    .iter()
                    .map(|v| v.model_id.clone())
                    .chain(std::iter::once("kev-latest".to_string()))
                    .chain(self.aliases.iter().cloned())
                    .collect(),
            })
    }

    /// The variant a request resolves to.
    ///
    /// # Errors
    ///
    /// [`Error::UnknownModel`] when `model` is neither a loaded id,
    /// `kev-latest`, an advertised alias, nor empty.
    pub fn pick<'a>(
        &'a self,
        request: &crate::api::SystemOneRequest,
    ) -> Result<&'a Variant, Error> {
        self.select(&request.model)
    }
}

/// The error body every refusal shares: the reference's `detail` beside the
/// typed `error` object the System One contract publishes.
///
/// `detail` is the human-readable line the FastAPI reference sent and older
/// readers look for; `error.code` is the stable refusal code
/// `gym::eval::classify` reads, `error.message` carries the same text for
/// readers of that envelope, and `error.question` names the question the
/// refusal is about when there is one.
fn refuse(
    status: StatusCode,
    code: RefusalCode,
    detail: impl Into<String>,
    question: Option<&str>,
) -> (StatusCode, Json<Value>) {
    let detail = detail.into();
    let message = detail.clone();
    (
        status,
        Json(json!({
            "detail": detail,
            "error": {
                "code": code.label(),
                "message": message,
                "question": question,
            },
        })),
    )
}

/// A request that fails before evaluation: a body that is not a request, or
/// a field the contract does not carry.
fn invalid(detail: impl Into<String>) -> (StatusCode, Json<Value>) {
    refuse(
        StatusCode::UNPROCESSABLE_ENTITY,
        RefusalCode::InvalidRequest,
        detail,
        None,
    )
}

/// The refusal one [`Error`] publishes, at the status its class answers with.
fn refused(error: &Error) -> (StatusCode, Json<Value>) {
    let code = error.refusal();
    let status = StatusCode::from_u16(code.status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    refuse(status, code, error.to_string(), error.question())
}

/// A panic in the evaluation task: the door's own runtime failed on a
/// request it accepted.
fn panicked(error: impl std::fmt::Display) -> (StatusCode, Json<Value>) {
    refuse(
        StatusCode::INTERNAL_SERVER_ERROR,
        RefusalCode::InferenceFailure,
        error.to_string(),
        None,
    )
}

/// The body limit is a capacity refusal; other body-read failures remain
/// transport errors without a refusal code.
fn received(body: Result<Bytes, BytesRejection>) -> Result<Bytes, (StatusCode, Json<Value>)> {
    body.map_err(|error| {
        let status = error.status();
        if status == StatusCode::PAYLOAD_TOO_LARGE {
            refuse(
                status,
                RefusalCode::PayloadTooLarge,
                error.body_text(),
                None,
            )
        } else {
            (status, Json(json!({ "detail": error.body_text() })))
        }
    })
}

/// `POST /v1/systemone`: typed questions in, typed answers out, one prefill.
async fn systemone(
    State(state): State<Arc<ServeState>>,
    body: Result<Bytes, BytesRejection>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let body = received(body)?;
    let request: SystemOneRequest =
        serde_json::from_slice(&body).map_err(|e| invalid(format!("request body: {e}")))?;
    state
        .admission
        .admit_shape(&request)
        .map_err(|e| refused(&e))?;
    let permit = state
        .select_index(&request.model)
        .and_then(|index| state.permit(index))
        .map_err(|e| refused(&e))?;
    let out = tokio::task::spawn_blocking(move || {
        let _permit = permit;
        evaluate(&state, &request)
    })
    .await
    .map_err(panicked)?
    .map_err(|e| refused(&e))?;
    Ok(Json(out))
}

/// `POST /v1/systemone/separate`: each question in its own pass against the
/// same state, for packed-vs-separate comparison.
async fn systemone_separate(
    State(state): State<Arc<ServeState>>,
    body: Result<Bytes, BytesRejection>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let body = received(body)?;
    let request: SystemOneRequest =
        serde_json::from_slice(&body).map_err(|e| invalid(format!("request body: {e}")))?;
    state
        .admission
        .admit_shape(&request)
        .map_err(|e| refused(&e))?;
    let permit = state
        .select_index(&request.model)
        .and_then(|index| state.permit(index))
        .map_err(|e| refused(&e))?;
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        evaluate_separate(&state, &request)
    })
    .await
    .map_err(panicked)?
    .map_err(|e| refused(&e))
    .map(Json)
}

/// `GET /v1/models`: every loaded variant, with kev's own artifact fields
/// beside the card fields the TypeSafe client reads.
async fn models(State(state): State<Arc<ServeState>>) -> Json<Value> {
    let cards: Vec<Value> = state
        .variants
        .iter()
        .enumerate()
        .map(|(i, v)| {
            json!({
                "id": v.model_id,
                "name": v.model_id,
                "description": format!("kev decision model on {}, served by the openagents Rust port", v.base),
                "release_date": "2026-09-19",
                "base_model_signature": v.base_revision,
                "aliases": if i == state.default { state.aliases.clone() } else { Vec::<String>::new() },
                "run": v.run,
                "base": v.base,
            })
        })
        .collect();
    Json(json!({ "models": cards }))
}

/// `GET /api/info`: what is loaded and what each variant may run at once,
/// for operators.
async fn info(State(state): State<Arc<ServeState>>) -> Json<Value> {
    Json(json!({
        "device": state.device,
        "default": state.variants[state.default].model_id,
        "admission": {
            "max_total_tokens": state.admission.max_total_tokens,
            "concurrency": state.admission.concurrency,
            "memory_budget_mib": state.memory_mib,
            "memory_in_use_mib": state.memory_in_use_mib(),
        },
        "variants": state
            .variants
            .iter()
            .zip(&state.variant_slots)
            .enumerate()
            .map(|(i, (v, share))| json!({
                "model_id": v.model_id,
                "run": v.run,
                "base": v.base,
                "base_revision": v.base_revision,
                "lora": v.lora,
                "option_isolation": v.model.option_isolation,
                "forward_mib": share.forward_mib,
                "concurrency": share.limit,
                "in_flight": state.in_flight(i),
            }))
            .collect::<Vec<_>>(),
    }))
}

/// `POST /api/predict`: a rendered record in, raw option distributions out.
/// The optional `model` field selects a variant the same way
/// `/v1/systemone` does.
async fn predict(
    State(state): State<Arc<ServeState>>,
    body: Result<Bytes, BytesRejection>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let body = received(body)?;
    let body: Value =
        serde_json::from_slice(&body).map_err(|e| invalid(format!("request body: {e}")))?;
    let model_field = body["model"].as_str().unwrap_or("kev-latest").to_string();
    let record: Record =
        serde_json::from_value(body.clone()).map_err(|e| invalid(format!("request body: {e}")))?;
    let permit = state
        .select_index(&model_field)
        .and_then(|index| state.permit(index))
        .map_err(|e| refused(&e))?;
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        let variant = state.select(&model_field)?;
        let enc = variant
            .model
            .encode(&record, INFER_MAX_STATE, INFER_MAX_BRANCH)?;
        let tokens = enc.ids.len();
        state.admission.admit_tokens(tokens)?;
        let state_tokens = enc.seg.iter().filter(|s| **s == 0).count();
        let start = Instant::now();
        let probs = variant.model.probs(&enc)?;
        Ok::<_, Error>(json!({
            "model": variant.model_id,
            "probs": probs,
            "tokens": tokens,
            "state_tokens": state_tokens,
            "latency_ms": start.elapsed().as_secs_f64() * 1000.0,
        }))
    })
    .await
    .map_err(panicked)?
    .map_err(|e| refused(&e))
    .map(Json)
}

/// One packed forward and the shaped response body.
fn evaluate(state: &ServeState, request: &SystemOneRequest) -> Result<Value, Error> {
    let variant = state.pick(request)?;
    let (record, meta) = to_record(request)?;
    let enc = variant
        .model
        .encode(&record, INFER_MAX_STATE, INFER_MAX_BRANCH)?;
    let tokens = enc.ids.len();
    state.admission.admit_tokens(tokens)?;
    let start = Instant::now();
    let probs = variant.model.probs(&enc)?;
    let latency = start.elapsed().as_secs_f64() * 1000.0;
    let answers = to_answers(&probs, &meta);
    Ok(response_body(
        &variant.model_id,
        &answers,
        tokens,
        output_tokens(&variant.model.tokenizer, &answers),
        latency,
    ))
}

/// One forward per question, merged back in request order.
fn evaluate_separate(state: &ServeState, request: &SystemOneRequest) -> Result<Value, Error> {
    let variant = state.pick(request)?;
    let mut answers = IndexMap::new();
    let mut tokens = 0usize;
    let mut latency = 0.0;
    for qid in request.questions.keys() {
        let mut one = request.clone();
        one.questions.retain(|id, _| id == qid);
        let (record, meta) = to_record(&one)?;
        let enc = variant
            .model
            .encode(&record, INFER_MAX_STATE, INFER_MAX_BRANCH)?;
        state.admission.admit_tokens(enc.ids.len())?;
        tokens += enc.ids.len();
        let start = Instant::now();
        let probs = variant.model.probs(&enc)?;
        latency += start.elapsed().as_secs_f64() * 1000.0;
        answers.extend(to_answers(&probs, &meta));
    }
    Ok(response_body(
        &variant.model_id,
        &answers,
        tokens,
        output_tokens(&variant.model.tokenizer, &answers),
        latency,
    ))
}

fn response_body(
    model: &str,
    answers: &IndexMap<String, Answer>,
    input_tokens: usize,
    output_tokens: usize,
    latency_ms: f64,
) -> Value {
    json!({
        "model": model,
        "answers": answers,
        "usage": {
            "input_tokens": input_tokens,
            "output_tokens": output_tokens,
        },
        "latency_ms": (latency_ms * 10.0).round() / 10.0,
    })
}

/// The billing-style output count: tokens of the serialized answers, using
/// the reference's `json.dumps` formatting so the figure is comparable.
fn output_tokens(tokenizer: &Tokenizer, answers: &IndexMap<String, Answer>) -> usize {
    let answers = serde_json::to_value(answers).unwrap_or(Value::Null);
    tokenizer
        .encode(python_json(&answers).as_str(), false)
        .map(|enc| enc.get_ids().len())
        .unwrap_or(0)
}

/// `json.dumps` with the defaults Python applies: `", "` and `": "` separators
/// and `ensure_ascii` escaping.
fn python_json(value: &Value) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => python_string(s),
        Value::Array(items) => format!(
            "[{}]",
            items.iter().map(python_json).collect::<Vec<_>>().join(", ")
        ),
        Value::Object(map) => format!(
            "{{{}}}",
            map.iter()
                .map(|(k, v)| format!("{}: {}", python_string(k), python_json(v)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

/// A JSON string literal the way `json.dumps` writes one: short escapes for
/// the common controls, `\uXXXX` for the rest, surrogate pairs for astral.
fn python_string(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            ch if (ch as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", ch as u32));
            }
            ch if ch.is_ascii() => out.push(ch),
            ch => {
                let mut buf = [0u16; 2];
                for unit in ch.encode_utf16(&mut buf) {
                    out.push_str(&format!("\\u{unit:04x}"));
                }
            }
        }
    }
    out.push('"');
    out
}

/// The routes the binary mounts.
pub fn router(state: Arc<ServeState>) -> Router {
    Router::new()
        .route("/v1/systemone", post(systemone))
        .route("/v1/systemone/separate", post(systemone_separate))
        .route("/v1/models", get(models))
        .route("/api/info", get(info))
        .route("/api/predict", post(predict))
        .with_state(state)
}
