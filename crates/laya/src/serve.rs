//! The HTTP surface: TypeSafe-compatible `POST /v1/systemone` and
//! `GET /v1/models`, with deterministic refusals.
//!
//! The served model identifies itself by the checkpoint's own
//! `model_name`; no upstream alias is claimed. A refusal answers with the
//! envelope every System One door shares — `{"detail": …, "error":
//! {"code", "message", "question"}}` — at the status its [`RefusalCode`]
//! class carries. `detail` keeps the FastAPI reference's shape for older
//! readers; `error.code` is the stable label `gym::eval::classify` reads.

use std::sync::Arc;

use tokio::sync::{OwnedSemaphorePermit, Semaphore};

use axum::Router;
use axum::body::Bytes;
use axum::extract::State;
use axum::extract::rejection::BytesRejection;
use axum::http::StatusCode;
use axum::response::Json;
use axum::routing::{get, post};
use serde_json::{Value, json};

use crate::api::{ChoiceCriteria, Question, SystemOneRequest};
use crate::decision::DecisionModel;
use crate::error::{Bound, Error, MAX_OPTIONS, RefusalCode};

/// The unit the working-memory budget is counted in.
pub const MIB: usize = 1024 * 1024;

/// One loaded checkpoint: its weights plus the card fields `/v1/models`
/// reports.
pub struct Variant {
    /// The loaded decision model.
    pub model: DecisionModel,
    /// The id requests name, such as `laya-typed-decisions`.
    pub model_id: String,
    /// The artifact location the listing reports as `run`.
    pub run: String,
    /// The base encoder id the listing reports.
    pub base: String,
}

impl Variant {
    /// Numerical settings recorded separately from checkpoint contents.
    #[must_use]
    pub fn execution_identity(&self) -> std::collections::BTreeMap<String, String> {
        let backend = if self.model.device.is_cpu() {
            "cpu"
        } else if self.model.device.is_metal() {
            "metal"
        } else {
            "cuda"
        };
        [
            ("backend", backend.to_string()),
            ("dtype", "f32".to_string()),
            ("max_len", self.model.max_len().to_string()),
            ("head_max_len", self.model.head_max_len().to_string()),
        ]
        .into_iter()
        .map(|(key, value)| (key.to_string(), value))
        .collect()
    }

    /// The working memory one forward of `rows` sequences at `max_len`
    /// tokens needs on this variant: the encoder's per-layer attention
    /// scores (`heads × rows × tokens²`, with roughly three live buffers
    /// for the scaled, masked, and softmaxed forms) plus the residual
    /// stream. Weights are not counted; they are resident before any
    /// request arrives.
    #[must_use]
    pub fn forward_bytes(&self, rows: usize) -> usize {
        let tokens = self.model.max_len();
        let hidden = self.model.hidden_size();
        let heads = self.model.attention_heads();
        let square = rows.saturating_mul(tokens).saturating_mul(tokens);
        let scores = square
            .saturating_mul(heads)
            .saturating_mul(3)
            .saturating_mul(4);
        let activations = rows
            .saturating_mul(tokens)
            .saturating_mul(hidden)
            .saturating_mul(8);
        scores.saturating_add(activations)
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

/// What one request may cost before the door evaluates it.
///
/// The bounds are checked cheapest first: question and option counts from
/// the request body, then the encoded marker fit, then a forward slot.
#[derive(Clone, Debug)]
pub struct Admission {
    /// Questions one request may carry; each is one forward row.
    pub max_questions: usize,
    /// Options summed over every question in one request.
    pub max_total_options: usize,
    /// Forwards in flight at once across every loaded variant, the host's
    /// compute bound.
    pub concurrency: usize,
    /// Working memory in bytes every forward in flight may hold together.
    ///
    /// Each variant's share is what one of its forwards needs at
    /// `max_questions` rows, so the forwards a variant may run at once is
    /// this budget divided by [`Variant::forward_bytes`]. The default is
    /// zero, which [`ServeState::new`] refuses: `laya-serve` fills it from
    /// [`host_memory_budget`] after the weights load, and a test states
    /// it.
    pub memory_budget_bytes: usize,
}

impl Default for Admission {
    fn default() -> Self {
        Self {
            max_questions: 64,
            max_total_options: 1_024,
            concurrency: 2,
            memory_budget_bytes: 0,
        }
    }
}

impl Admission {
    /// Admit a request's shape: its question and option counts.
    ///
    /// # Errors
    ///
    /// Returns [`Error::TooManyQuestions`], [`Error::TooManyOptions`], or
    /// [`Error::TooManyTotalOptions`] when the request shape exceeds a
    /// configured bound.
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
        Ok(())
    }
}

/// How many options a question carries: a `choice` question's criteria,
/// a `score` question's levels, and two for a `noul` — the marker count
/// the model scores. A shape the contract rejects is refused by
/// [`SystemOneRequest::validate`] afterwards.
fn option_count(question: &Question) -> usize {
    match question {
        Question::Choice { criteria, .. } => match criteria {
            ChoiceCriteria::Map(map) => map.len(),
            ChoiceCriteria::List(list) => list.len(),
        },
        Question::Score { criteria, .. } => criteria.len(),
        Question::Noul { .. } => 2,
        Question::Other => 0,
    }
}

/// One variant's share of the host: what a forward of it costs and how
/// many may run at once.
pub struct VariantSlots {
    /// Working memory one forward at `Admission::max_questions` rows
    /// needs, rounded up to whole MiB.
    pub forward_mib: usize,
    /// Forwards of this variant that fit the memory budget at once, no
    /// more than the host's `concurrency`.
    pub limit: usize,
    /// The permits `limit` counts.
    pub slots: Arc<Semaphore>,
}

/// The permits one forward holds until it ends: a host slot, a variant
/// slot, and its share of the memory budget. The permit rides with the
/// blocking task, so a caller that stops waiting releases it when the
/// forward it started ends, not before.
pub struct Permit {
    _host: OwnedSemaphorePermit,
    _variant: OwnedSemaphorePermit,
    _memory: OwnedSemaphorePermit,
}

/// Everything one running server knows beyond the weights.
pub struct ServeState {
    /// Every loaded variant; requests pick one by `model` id.
    pub variants: Vec<Variant>,
    /// The variant an absent `model` field resolves to.
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
    /// A server over `variants`, with `default` naming the one aliases and
    /// an absent `model` field resolve to.
    ///
    /// # Errors
    ///
    /// [`Error::Artifact`] when `variants` is empty, `default` names no
    /// variant, an alias collides with a variant id, `admission` holds a
    /// zero bound, or one forward of a variant at `max_questions` rows
    /// needs more than the memory budget. In that last case lower the
    /// question bound or serve the variant on a host with more memory;
    /// admitting it would exhaust the process on the first full request.
    pub fn new(
        variants: Vec<Variant>,
        default: usize,
        aliases: Vec<String>,
        device: String,
        admission: Admission,
    ) -> Result<Self, Error> {
        if variants.is_empty() {
            return Err(Error::Artifact("no model variants are loaded".to_string()));
        }
        if default >= variants.len() {
            return Err(Error::Artifact(format!(
                "the default variant index {default} is out of range; {} variants are loaded",
                variants.len()
            )));
        }
        if let Some(alias) = aliases
            .iter()
            .find(|alias| variants.iter().any(|v| &v.model_id == *alias))
        {
            return Err(Error::Artifact(format!(
                "alias `{alias}` is already the name of a loaded variant"
            )));
        }
        if admission.max_questions == 0
            || admission.max_total_options == 0
            || admission.concurrency == 0
            || admission.memory_budget_bytes == 0
        {
            return Err(Error::Artifact(
                "every admission limit must be at least 1".to_string(),
            ));
        }
        let memory_mib = admission.memory_budget_bytes.div_ceil(MIB);
        if memory_mib > Semaphore::MAX_PERMITS || u32::try_from(memory_mib).is_err() {
            return Err(Error::Artifact(format!(
                "the memory budget of {memory_mib} MiB is larger than this server can track; pass a smaller --memory-budget-mib"
            )));
        }
        let mut variant_slots = Vec::with_capacity(variants.len());
        for variant in &variants {
            let forward_mib = variant
                .forward_bytes(admission.max_questions)
                .div_ceil(MIB)
                .max(1);
            if forward_mib > memory_mib {
                return Err(Error::Artifact(format!(
                    "`{}`: one inference pass at {} questions needs {forward_mib} MiB, more than the {memory_mib} MiB memory budget; raise --memory-budget-mib or lower --max-questions",
                    variant.model_id, admission.max_questions
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

    /// The permits one forward of variant `index` holds, or
    /// [`Error::Busy`] naming the bound that is saturated. Nothing waits:
    /// a refusal is immediate, so a caller that gives up leaves no queue
    /// entry behind.
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
            bound: Bound::Host,
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

    /// Resolve a request's `model` field to a loaded variant: an exact
    /// id, any alias the listing advertises, or an absent field.
    ///
    /// The aliases the listing publishes and the aliases this resolves
    /// are one list, so a client that keeps its default model reaches
    /// the door.
    pub fn select(&self, model: &str) -> Result<&Variant, Error> {
        self.select_index(model).map(|index| &self.variants[index])
    }

    /// The index into `variants` a `model` field resolves to, the same
    /// way [`ServeState::select`] does.
    ///
    /// # Errors
    ///
    /// [`Error::UnknownModel`] when `model` is neither a loaded id, an
    /// advertised alias, nor empty.
    pub fn select_index(&self, model: &str) -> Result<usize, Error> {
        if model.is_empty() || self.aliases.iter().any(|a| a == model) {
            return if self.default < self.variants.len() {
                Ok(self.default)
            } else {
                Err(Error::Artifact(format!(
                    "the default variant index {} is out of range",
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
                    .chain(self.aliases.iter().cloned())
                    .collect(),
            })
    }
}

/// The error body every refusal shares: the reference's `detail` beside
/// the typed `error` object the System One contract publishes.
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

/// A request that fails before evaluation: a body that is not a request,
/// or a field the contract does not carry.
fn invalid(detail: impl Into<String>) -> (StatusCode, Json<Value>) {
    refuse(
        StatusCode::UNPROCESSABLE_ENTITY,
        RefusalCode::InvalidRequest,
        detail,
        None,
    )
}

/// The refusal one [`Error`] publishes, at the status its class answers
/// with.
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

/// `POST /v1/systemone`: typed questions in, typed answers out, one
/// forward.
async fn systemone(
    State(state): State<Arc<ServeState>>,
    body: Result<Bytes, BytesRejection>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let body = received(body)?;
    let request: SystemOneRequest = serde_json::from_slice(&body)
        .map_err(|e| invalid(format!("the request body is not valid: {e}")))?;
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

/// The answer one admitted request gets: the checkpoint's `system_one`
/// output verbatim.
fn evaluate(state: &ServeState, request: &SystemOneRequest) -> Result<Value, Error> {
    let variant = state.select(&request.model)?;
    variant.model.system_one(request)
}

/// `GET /v1/models`: every loaded variant, with laya's own artifact
/// fields beside the card fields the TypeSafe client reads.
async fn models(State(state): State<Arc<ServeState>>) -> Json<Value> {
    let cards: Vec<Value> = state
        .variants
        .iter()
        .enumerate()
        .map(|(i, v)| {
            json!({
                "id": v.model_id,
                "name": v.model_id,
                "description": format!("{} decision model on {}, served by the openagents Rust port", v.model.model_name, v.base),
                "base_model_signature": v.base,
                "artifact_identity": v.model.artifacts,
                "execution": v.execution_identity(),
                // laya batches the questions of one request into one
                // forward; it does not batch separate requests. A batch
                // of inputs is the caller's loop of bounded calls.
                "batching": {"kind": "caller-loop"},
                "limits": {
                    "context_tokens": v.model.max_len(),
                    "head_tokens": v.model.head_max_len(),
                    "questions_per_call": state.admission.max_questions,
                    "options_per_call": state.admission.max_total_options,
                    "concurrent_calls": state.admission.concurrency,
                },
                "aliases": if i == state.default { state.aliases.clone() } else { Vec::<String>::new() },
                "run": v.run,
                "base": v.base,
            })
        })
        .collect();
    Json(json!({ "models": cards }))
}

/// `GET /api/info`: what is loaded and what each variant may run at
/// once, for operators.
async fn info(State(state): State<Arc<ServeState>>) -> Json<Value> {
    Json(json!({
        "device": state.device,
        "default": state.variants[state.default].model_id,
        "admission": {
            "max_questions": state.admission.max_questions,
            "max_total_options": state.admission.max_total_options,
            "concurrency": state.admission.concurrency,
            "memory_budget_mib": state.memory_mib,
            "memory_in_use_mib": state.memory_in_use_mib(),
        },
        "variants": state
            .variants
            .iter()
            .zip(&state.variant_slots)
            .map(|(v, share)| json!({
                "model_id": v.model_id,
                "model_name": v.model.model_name,
                "run": v.run,
                "base": v.base,
                "encoder": v.model.encoder_id,
                "max_len": v.model.max_len(),
                "head_max_len": v.model.head_max_len(),
                "forward_mib": share.forward_mib,
                "limit": share.limit,
                "in_flight": share.limit - share.slots.available_permits(),
                "artifact_digest": v.model.artifacts.digest,
            }))
            .collect::<Vec<_>>(),
    }))
}

/// `GET /healthz`: process liveness only.
async fn healthz() -> &'static str {
    "ok"
}

/// The request-size cap for `/v1/systemone`: a request is text plus a
/// state document; 8 MiB is far past any state the encoder's `max_len`
/// can use, and a larger body can only cost the door a read it must
/// refuse anyway.
pub const BODY_LIMIT: usize = 8 * MIB;

/// The routes one laya door serves.
pub fn router(state: Arc<ServeState>) -> Router {
    Router::new()
        .route("/v1/systemone", post(systemone))
        .route("/v1/models", get(models))
        .route("/api/info", get(info))
        .route("/healthz", get(healthz))
        .layer(axum::extract::DefaultBodyLimit::max(BODY_LIMIT))
        .layer(tower_http::cors::CorsLayer::permissive())
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn request(questions: Value) -> SystemOneRequest {
        serde_json::from_value(json!({
            "state": "s", "model": "m", "questions": questions
        }))
        .unwrap()
    }

    #[test]
    fn admission_bounds_the_request_shape() {
        let admission = Admission {
            max_questions: 2,
            max_total_options: 4,
            concurrency: 1,
            memory_budget_bytes: 1,
        };
        let questions = |n: usize| {
            (0..n)
                .map(|i| {
                    (
                        format!("q{i}"),
                        json!({"type": "noul", "instructions": "x"}),
                    )
                })
                .collect::<serde_json::Map<String, Value>>()
        };
        // Three nouls carry six options total: both bounds answer.
        let many = request(Value::Object(questions(3)));
        assert!(matches!(
            admission.admit_shape(&many),
            Err(Error::TooManyQuestions { count: 3, max: 2 })
        ));
        let wide = request(json!({
            "a": {"type": "noul"}, "b": {"type": "noul"}, "c": {"type": "noul"}
        }));
        assert!(matches!(
            admission.admit_shape(&wide),
            Err(Error::TooManyQuestions { .. })
        ));
        let fits = request(json!({
            "a": {"type": "choice", "criteria": ["x", "y"]},
            "b": {"type": "noul"}
        }));
        admission.admit_shape(&fits).unwrap();
        // A question over the model's own ceiling answers before the
        // configured totals are even counted.
        let huge = request(json!({
            "a": {"type": "choice", "criteria": (0..=MAX_OPTIONS).map(|i| i.to_string()).collect::<Vec<_>>()}
        }));
        assert!(matches!(
            admission.admit_shape(&huge),
            Err(Error::TooManyOptions { .. })
        ));
    }

    #[test]
    fn memory_parsers_read_the_host_reports() {
        let meminfo = "MemTotal:       33554432 kB\nMemAvailable:   1048576 kB\n";
        assert_eq!(meminfo_available(meminfo), Some(1048576 * 1024));
        assert_eq!(meminfo_available("MemTotal: 100 kB\n"), None);
        let vm_stat = "Mach Virtual Memory Statistics: (page size of 16384 bytes)\n\
                       Pages free:                              1000.\n\
                       Pages inactive:                          2000.\n";
        assert_eq!(vm_stat_available(vm_stat), Some(3000 * 16384));
        assert_eq!(vm_stat_available("Pages free: 10.\n"), None);
    }

    #[test]
    fn refusals_carry_the_shared_envelope() {
        let (status, Json(body)) = refuse(
            StatusCode::UNPROCESSABLE_ENTITY,
            RefusalCode::InvalidRequest,
            "bad shape",
            Some("q1"),
        );
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body["detail"], "bad shape");
        assert_eq!(body["error"]["code"], "invalid_request");
        assert_eq!(body["error"]["question"], "q1");
        // Every error class maps to a refusal code and a status.
        let error = Error::UnknownModel {
            model: "ghost".to_string(),
            known: vec!["english".to_string()],
        };
        assert_eq!(error.refusal(), RefusalCode::ModelUnavailable);
        assert_eq!(error.refusal().status(), 503);
    }

    #[test]
    fn serve_state_refuses_an_empty_deployment() {
        let err = ServeState::new(
            Vec::new(),
            0,
            Vec::new(),
            "cpu".to_string(),
            Admission {
                memory_budget_bytes: 1,
                ..Admission::default()
            },
        );
        assert!(matches!(err, Err(Error::Artifact(_))));
    }
}
