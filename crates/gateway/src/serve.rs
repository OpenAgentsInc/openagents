//! The serving path: one admission sequence for every decision call.
//!
//! `POST /v1/systemone` here runs the same steps for every caller:
//!
//! 1. **Authenticate.** `Authorization: Bearer oak_<id>.<secret>`
//!    resolves to a tenant through `tenancy::keys`; no header is an
//!    anonymous call, which the shared lane alone may carry.
//! 2. **Authorize.** The request's `model` field names a door; the
//!    registry's `authorize` returns the admission snapshot the call is
//!    served under — a registry update mid-flight cannot relabel it.
//! 3. **Bound.** The door's declared rate window and concurrency, and
//!    the process's forward bound, each refuse before the reservation
//!    is taken — a congestion refusal never holds quota.
//! 4. **Reserve.** `tenancy::quota` writes the `reserved` event before
//!    anything is dispatched — a crash after this point leaves a held
//!    reservation recovery will orphan, never an unaccounted spend.
//! 5. **Verify.** The backend's `GET /v1/models` card is checked against
//!    the bound identity. A mismatch is refused before the request is
//!    forwarded — the caller is never billed an answer the wrong
//!    artifact produced.
//! 6. **Forward, settle, receipt.** The backend's status maps to a typed
//!    outcome — answered, refused, unavailable — the reservation settles
//!    once, and a sealed `ExecutionReceipt` lands in `receipts.jsonl`
//!    beside the registry.

use std::collections::{HashMap, VecDeque};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use axum::Json;
use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use receipts::execution::{
    ExecutionReceipt, Outcome, Registry as ReceiptRegistry, Served, Timing, digest_request,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tokio::sync::{Mutex, Semaphore};

use tenancy::{Published, Registry, keys, quota};

use crate::config::Config;

/// The file every attempt's sealed receipt appends to.
const RECEIPTS: &str = "receipts.jsonl";

/// The span a door's rate window covers.
const RATE_WINDOW: Duration = Duration::from_secs(60);

/// What the gateway itself cannot do.
#[derive(Debug)]
pub enum Trouble {
    /// The filesystem refused.
    Io(std::io::Error),
    /// The ledger is held by another writer.
    Ledger(quota::LedgerTrouble),
    /// The registry did not open.
    Registry(tenancy::Trouble),
}

impl std::fmt::Display for Trouble {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "{error}"),
            Self::Ledger(trouble) => write!(f, "{trouble}"),
            Self::Registry(trouble) => write!(f, "{trouble}"),
        }
    }
}

impl std::error::Error for Trouble {}

impl From<std::io::Error> for Trouble {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<quota::LedgerTrouble> for Trouble {
    fn from(trouble: quota::LedgerTrouble) -> Self {
        Self::Ledger(trouble)
    }
}

impl From<tenancy::Trouble> for Trouble {
    fn from(trouble: tenancy::Trouble) -> Self {
        Self::Registry(trouble)
    }
}

/// A door's live bounds: the concurrency permit pool the binding
/// declares and the minute of dispatch times its rate limit watches.
struct DoorBounds {
    /// The concurrency the binding declared, so an update can rebuild.
    concurrency: u64,
    /// The permit pool, when the binding declares a bound.
    slots: Option<Arc<Semaphore>>,
    /// The declared calls-per-minute, so an update can clear the window.
    rate: u64,
    /// Dispatch times inside the window.
    window: VecDeque<Instant>,
}

/// The serving state: config, the durable ledger and receipt log, and
/// the in-memory bounds.
pub struct ServeState {
    /// The registry directory — manifests, keys, ledger, receipts.
    dir: PathBuf,
    config: Config,
    client: reqwest::Client,
    ledger: Mutex<quota::Ledger>,
    receipts: Mutex<std::fs::File>,
    /// The process-wide forward bound.
    in_flight: Semaphore,
    /// Per-door bounds, keyed by door name.
    doors: Mutex<HashMap<String, DoorBounds>>,
    /// Attempt ids minted within this process.
    attempt_ids: AtomicU64,
}

impl ServeState {
    /// Open the registry directory and take the ledger's writer lock.
    ///
    /// A second gateway on the same directory is refused — two writers
    /// on one ledger would race reservations, and refusing is cheaper
    /// than reconciling them.
    pub fn open(config: Config) -> Result<Arc<Self>, Trouble> {
        // Open the registry once at startup so a broken install fails
        // the process, not the first request.
        Registry::open(&config.registry)?;
        let ledger = quota::Ledger::open(&config.registry)?;
        let receipts = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(config.registry.join(RECEIPTS))?;
        let client = reqwest::Client::builder()
            .timeout(Duration::from_millis(config.forward_timeout_ms))
            .build()
            .map_err(|error| Trouble::Io(std::io::Error::other(error.to_string())))?;
        Ok(Arc::new(Self {
            dir: config.registry.clone(),
            in_flight: Semaphore::new(config.max_in_flight),
            config,
            client,
            ledger: Mutex::new(ledger),
            receipts: Mutex::new(receipts),
            doors: Mutex::new(HashMap::new()),
            attempt_ids: AtomicU64::new(0),
        }))
    }

    /// Release a reservation whose work was never dispatched — the
    /// `unattempted` settlement returns the units to the budget.
    async fn release(&self, request: &str, attempt: u32) {
        let mut ledger = self.ledger.lock().await;
        ledger
            .settle(
                request,
                attempt,
                quota::Outcome::Unattempted,
                &quota::Units::none(),
            )
            .ok();
    }

    /// Mint an attempt id within this process.
    fn mint(&self) -> u64 {
        self.attempt_ids.fetch_add(1, Ordering::Relaxed)
    }
}

/// Build the axum router over the state.
pub fn router(state: Arc<ServeState>) -> axum::Router {
    let body_max = state.config.max_body_bytes;
    axum::Router::new()
        .route("/v1/systemone", post(systemone))
        .route("/v1/models", get(models))
        .route("/healthz", get(healthz))
        .layer(DefaultBodyLimit::max(body_max))
        .with_state(state)
}

/// `GET /healthz`: the process is up — readiness for a load balancer.
/// This says nothing about backend health; a door's card check is the
/// identity check, run per request.
async fn healthz() -> impl IntoResponse {
    Json(json!({"status": "ok"}))
}

/// `GET /v1/models`: the doors the caller's tenant may name, with the
/// identity each is bound to.
///
/// This is the registry's claim — which doors exist for this caller and
/// what they are pinned to — not a statement about which backends are
/// currently reachable or what weights a remote host actually loaded.
async fn models(
    State(state): State<Arc<ServeState>>,
    headers: HeaderMap,
) -> Result<Json<Value>, Response> {
    let (registry, caller) = match authenticate(&state, &headers) {
        Ok(parts) => parts,
        Err((status, code, message)) => return Err(gateway_error(status.as_u16(), code, &message)),
    };
    let manifest = registry.manifest();
    let names: Vec<String> = match caller.tenant.as_deref() {
        Some(tenant) => registry.visible_doors(tenant),
        None => manifest.shared.keys().cloned().collect(),
    };
    let cards: Vec<Value> = names
        .iter()
        .filter_map(|door| {
            let binding = match caller.tenant.as_deref() {
                Some(tenant) => manifest
                    .tenants
                    .get(tenant)
                    .and_then(|record| record.doors.get(door))
                    .or_else(|| manifest.shared.get(door)),
                None => manifest.shared.get(door),
            }?;
            Some(json!({
                "id": door,
                "model": binding.artifact.model,
                "artifact_signature": binding.artifact.artifact_signature,
                "lane": tenancy::lane_name(binding.lane),
            }))
        })
        .collect();
    Ok(Json(json!({"models": cards})))
}

/// Who the call is, once authentication has run.
struct Caller {
    /// The tenant the key resolved to, or `None` for anonymous.
    tenant: Option<String>,
    /// The credential id the call authenticated under — a reference,
    /// never the secret.
    key: String,
}

/// Resolve the `Authorization` header and reopen the registry for this
/// request — one fresh read per call, so an update lands on the next
/// request rather than on the next restart.
fn authenticate(
    state: &ServeState,
    headers: &HeaderMap,
) -> Result<(Registry, Caller), (StatusCode, &'static str, String)> {
    let registry = Registry::open(&state.dir).map_err(|trouble| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "registry_unavailable" as &'static str,
            trouble.to_string(),
        )
    })?;
    let Some(header) = headers.get("authorization") else {
        return Ok((
            registry,
            Caller {
                tenant: None,
                key: "anonymous".to_string(),
            },
        ));
    };
    let header = header.to_str().map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            "malformed" as &'static str,
            "the Authorization header is not text".to_string(),
        )
    })?;
    let token = header.strip_prefix("Bearer ").ok_or_else(|| {
        (
            StatusCode::UNAUTHORIZED,
            "unauthenticated" as &'static str,
            "the credential is not a `Bearer oak_<id>.<secret>` token".to_string(),
        )
    })?;
    let authenticated =
        keys::authenticate(&state.dir, registry.manifest(), token).map_err(|refusal| {
            (
                StatusCode::UNAUTHORIZED,
                "unauthenticated" as &'static str,
                format!("the credential was refused: {refusal}"),
            )
        })?;
    Ok((
        registry,
        Caller {
            tenant: Some(authenticated.tenant),
            key: authenticated.key_id,
        },
    ))
}

/// What an attempt needs for its receipt — the identities it ran under.
#[derive(Default)]
struct Context {
    /// The credential reference — the key id, or absent for anonymous.
    tenant_ref: Option<String>,
    /// The registry revision the call was admitted under.
    registry: Option<ReceiptRegistry>,
    /// The identity the caller asked for — the bound expectation.
    requested: Served,
    /// The identity the backend published, when it was reached.
    served: Served,
    /// Digest of the response body, when one came back.
    result_digest: Option<String>,
    /// The reservation the attempt settled against, when it held one.
    usage: Option<String>,
}

/// What the admission path produced.
enum Verdict {
    /// The backend answered — its status and body pass through verbatim.
    Forwarded {
        status: StatusCode,
        body: Bytes,
        outcome: Outcome,
        cause: Option<String>,
        ctx: Context,
    },
    /// The gateway's own typed refusal.
    Refused {
        status: StatusCode,
        code: &'static str,
        message: String,
        outcome: Outcome,
        ctx: Context,
    },
}

/// What one attempt is called — the identity the response headers and
/// the receipt both carry.
struct Naming<'a> {
    /// The caller's idempotency key, or a minted request id.
    request: &'a str,
    /// The claimed attempt number, one-based.
    attempt: u32,
    /// This dispatch's own id.
    attempt_id: String,
    /// The canonical envelope's digest.
    request_digest: &'a str,
}

/// `POST /v1/systemone`: the whole admission path, end to end.
async fn systemone(
    State(state): State<Arc<ServeState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let started = Instant::now();

    // A parseable envelope is the smallest thing a receipt can bind.
    let Ok(envelope) = serde_json::from_slice::<Value>(&body) else {
        return gateway_error(
            400,
            "invalid_request",
            "the body is not a JSON request envelope",
        );
    };
    let request_digest = digest_request(&envelope);
    let request = request_id(&state, &headers);
    let naming = Naming {
        request: &request,
        attempt: attempt_of(&headers),
        attempt_id: format!("{request}-{}-{}", attempt_of(&headers), state.mint()),
        request_digest: &request_digest,
    };

    let verdict = admitted(&state, &headers, &envelope, &body, &naming).await;

    let (status, outcome, body_out, cause, ctx) = match verdict {
        Verdict::Forwarded {
            status,
            body,
            outcome,
            cause,
            ctx,
        } => (status, outcome, body, cause, ctx),
        Verdict::Refused {
            status,
            code,
            message,
            outcome,
            ctx,
        } => (
            status,
            outcome,
            serde_json::to_vec(&json!({
                "error": {"code": code, "message": message,
                          "request": naming.request, "attempt": naming.attempt},
            }))
            .unwrap_or_default()
            .into(),
            Some(code.to_string()),
            ctx,
        ),
    };

    let receipt_digest =
        write_receipt(&state, &naming, outcome, cause.as_deref(), started, &ctx).await;
    respond(
        status,
        body_out,
        &naming,
        outcome,
        receipt_digest.as_deref(),
    )
}

/// The request's full passage through admission — one function so the
/// steps read in the order they run.
async fn admitted(
    state: &ServeState,
    headers: &HeaderMap,
    envelope: &Value,
    body: &Bytes,
    naming: &Naming<'_>,
) -> Verdict {
    // 1–2. Authenticate, then authorize the named door.
    let (registry, caller) = match authenticate(state, headers) {
        Ok(parts) => parts,
        Err((status, code, message)) => {
            return Verdict::Refused {
                status,
                code,
                message,
                outcome: Outcome::Refused,
                ctx: Context::default(),
            };
        }
    };
    let mut ctx = Context {
        tenant_ref: caller.tenant.as_ref().map(|_| caller.key.clone()),
        ..Context::default()
    };
    let door = envelope
        .get("model")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if door.is_empty() {
        return Verdict::Refused {
            status: StatusCode::UNPROCESSABLE_ENTITY,
            code: "invalid_request",
            message: "the envelope names no `model` door".to_string(),
            outcome: Outcome::Refused,
            ctx,
        };
    }
    let admission = match registry.authorize(caller.tenant.as_deref(), door) {
        Ok(admission) => admission,
        Err(refusal) => {
            return Verdict::Refused {
                status: StatusCode::FORBIDDEN,
                code: "door_not_bound",
                message: refusal.to_string(),
                outcome: Outcome::Refused,
                ctx,
            };
        }
    };
    ctx.registry = Some(ReceiptRegistry {
        digest: admission.registry_digest.clone(),
        sequence: admission.sequence,
    });
    ctx.requested = Served {
        model: admission.binding.artifact.model.clone(),
        adapter: admission.binding.artifact.adapter.clone(),
        artifact_signature: admission.binding.artifact.artifact_signature.clone(),
        execution: admission.binding.artifact.execution.clone(),
    };
    let Some(door_cfg) = state.config.doors.get(door) else {
        return Verdict::Refused {
            status: StatusCode::SERVICE_UNAVAILABLE,
            code: "door_unavailable",
            message: format!("door `{door}` is bound but no backend is configured for it"),
            outcome: Outcome::Refused,
            ctx,
        };
    };
    let endpoint = door_cfg.endpoint.clone();

    // 3. Bound: rate window and door concurrency first, then the
    // process-wide forward bound — all before the reservation exists.
    let capacity = admission.binding.capacity.clone().unwrap_or_default();
    {
        let mut doors = state.doors.lock().await;
        let bounds = doors.entry(door.to_string()).or_insert_with(|| DoorBounds {
            concurrency: capacity.concurrency.unwrap_or(0),
            slots: capacity
                .concurrency
                .map(|limit| Arc::new(Semaphore::new(limit as usize))),
            rate: capacity.requests_per_minute.unwrap_or(0),
            window: VecDeque::new(),
        });
        // A changed binding takes effect on the next call: the window
        // clears and the pool rebuilds when the declared limits moved.
        if bounds.rate != capacity.requests_per_minute.unwrap_or(0) {
            bounds.rate = capacity.requests_per_minute.unwrap_or(0);
            bounds.window.clear();
        }
        if bounds.concurrency != capacity.concurrency.unwrap_or(0) {
            bounds.concurrency = capacity.concurrency.unwrap_or(0);
            bounds.slots = capacity
                .concurrency
                .map(|limit| Arc::new(Semaphore::new(limit as usize)));
        }
        if bounds.rate > 0 {
            let horizon = Instant::now() - RATE_WINDOW;
            while bounds.window.front().is_some_and(|then| *then < horizon) {
                bounds.window.pop_front();
            }
            if bounds.window.len() as u64 >= bounds.rate {
                return Verdict::Refused {
                    status: StatusCode::TOO_MANY_REQUESTS,
                    code: "rate_limited",
                    message: format!(
                        "door `{door}` admits {} calls a minute under this binding",
                        bounds.rate
                    ),
                    outcome: Outcome::Refused,
                    ctx,
                };
            }
            bounds.window.push_back(Instant::now());
        }
    }
    let _door_permit = match door_permit(state, door).await {
        Some(permit) => Some(permit),
        None => {
            return Verdict::Refused {
                status: StatusCode::TOO_MANY_REQUESTS,
                code: "busy",
                message: format!("door `{door}`'s forward slots are full; retry shortly"),
                outcome: Outcome::Refused,
                ctx,
            };
        }
    };
    let Ok(_host_permit) = state.in_flight.try_acquire() else {
        return Verdict::Refused {
            status: StatusCode::TOO_MANY_REQUESTS,
            code: "overloaded",
            message: "the gateway's forward bound is full; retry shortly".to_string(),
            outcome: Outcome::Refused,
            ctx,
        };
    };

    // 4. Reserve. From here the attempt is durable: it settles or it
    // orphans, and either way the ledger accounts for it.
    let units = units_of(envelope, body.len());
    let reservation = {
        let mut ledger = state.ledger.lock().await;
        ledger.reserve(
            registry.manifest(),
            &quota::Call {
                tenant: caller.tenant.as_deref().unwrap_or("anonymous"),
                key: &caller.key,
                request: naming.request,
                attempt: naming.attempt,
                request_digest: naming.request_digest,
                units: &units,
                ttl_secs: state.config.reservation_ttl_secs,
            },
        )
    };
    if let Err(refusal) = reservation {
        let (status, code) = match &refusal {
            quota::Refusal::Exhausted { .. } => (429_u16, "quota_exhausted"),
            quota::Refusal::ContentConflict { .. } | quota::Refusal::Resolved { .. } => {
                (409, "idempotency_conflict")
            }
            _ => (500, "ledger_unavailable"),
        };
        return Verdict::Refused {
            status: StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
            code,
            message: refusal.to_string(),
            outcome: Outcome::Refused,
            ctx,
        };
    }
    ctx.usage = Some(format!("{}#{}", naming.request, naming.attempt));

    // 5. Verify the backend's published identity against the binding —
    // before a byte of the request is forwarded.
    let published =
        match published_identity(state, &endpoint, &admission.binding.artifact.model).await {
            Ok(published) => published,
            Err(message) => {
                state.release(naming.request, naming.attempt).await;
                return Verdict::Refused {
                    status: StatusCode::SERVICE_UNAVAILABLE,
                    code: "door_unavailable",
                    message,
                    outcome: Outcome::Unattempted,
                    ctx,
                };
            }
        };
    if let Err(fault) = admission.verify(&published) {
        state.release(naming.request, naming.attempt).await;
        return Verdict::Refused {
            status: StatusCode::SERVICE_UNAVAILABLE,
            code: "identity_mismatch",
            message: fault.to_string(),
            outcome: Outcome::Unattempted,
            ctx,
        };
    }
    ctx.served = Served {
        model: published.model.clone(),
        adapter: if published.adapter.is_empty() {
            None
        } else {
            Some(published.adapter.clone())
        },
        artifact_signature: published.artifact_signature.clone(),
        execution: published.execution.clone(),
    };

    // 6. Forward, then settle from the recorded outcome.
    let (status, outcome, body_out, cause) = match forward(state, &endpoint, body).await {
        Forwarded::Served { status, body } => (status, Outcome::Answered, body, None),
        Forwarded::Refused {
            status,
            body,
            cause,
        } => (status, Outcome::Refused, body, Some(cause)),
        Forwarded::Unavailable { message } => (
            StatusCode::SERVICE_UNAVAILABLE,
            Outcome::Unavailable,
            Bytes::from(
                serde_json::to_vec(&json!({
                    "error": {"code": "unavailable", "message": message},
                }))
                .unwrap_or_default(),
            ),
            Some("unavailable".to_string()),
        ),
    };
    ctx.result_digest = Some(digest_bytes(&body_out));
    let settle_outcome = match outcome {
        Outcome::Answered => quota::Outcome::Answered,
        Outcome::Refused => quota::Outcome::Refused,
        Outcome::Unavailable => quota::Outcome::Unavailable,
        Outcome::Unattempted => quota::Outcome::Unattempted,
        Outcome::Unknown => quota::Outcome::Unknown,
    };
    {
        let mut ledger = state.ledger.lock().await;
        ledger
            .settle(naming.request, naming.attempt, settle_outcome, &units)
            .ok();
    }
    Verdict::Forwarded {
        status,
        body: body_out,
        outcome,
        cause,
        ctx,
    }
}

/// Try the door's concurrency pool — `None` when the pool exists and is
/// full, `Some(None)` when the binding declares no bound.
async fn door_permit(
    state: &ServeState,
    door: &str,
) -> Option<Option<tokio::sync::OwnedSemaphorePermit>> {
    let slots = {
        let doors = state.doors.lock().await;
        doors.get(door).and_then(|bounds| bounds.slots.clone())
    };
    match slots {
        Some(slots) => slots.try_acquire_owned().ok().map(Some),
        None => Some(None),
    }
}

/// The units an envelope asks for: its question count, its wire size,
/// and the option total across `choice`/`score` questions.
fn units_of(envelope: &Value, body_bytes: usize) -> quota::Units {
    let questions = envelope
        .get("questions")
        .and_then(Value::as_object)
        .map_or(0, |questions| questions.len() as u64);
    let options = envelope
        .get("questions")
        .and_then(Value::as_object)
        .map(|questions| {
            questions
                .values()
                .map(|question| {
                    question
                        .get("options")
                        .and_then(Value::as_array)
                        .map_or(0, |options| options.len() as u64)
                })
                .sum()
        })
        .unwrap_or_default();
    quota::Units {
        questions,
        input_bytes: body_bytes as u64,
        options,
    }
}

/// The request's idempotency key, or a minted one. A minted id is unique
/// within the process; a cross-process collision on the ledger refuses
/// as a content conflict rather than merging.
fn request_id(state: &ServeState, headers: &HeaderMap) -> String {
    headers
        .get("idempotency-key")
        .and_then(|value| value.to_str().ok())
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| format!("req-{:x}-{}", unix_now(), state.mint()))
}

/// The attempt number a caller claimed, one-based.
fn attempt_of(headers: &HeaderMap) -> u32 {
    headers
        .get("x-attempt")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse().ok())
        .unwrap_or(1)
}

/// The `Published` identity a backend's `GET /v1/models` reports for the
/// bound model id.
async fn published_identity(
    state: &ServeState,
    endpoint: &str,
    model: &str,
) -> Result<Published, String> {
    let response = state
        .client
        .get(format!("{endpoint}/v1/models"))
        .send()
        .await
        .map_err(|error| format!("the door's identity could not be read: {error}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "the door's identity check answered {}",
            response.status()
        ));
    }
    let body: Value = response
        .json()
        .await
        .map_err(|error| format!("the door's identity did not parse: {error}"))?;
    let cards = body
        .get("models")
        .and_then(Value::as_array)
        .ok_or_else(|| "the door's identity document carries no `models`".to_string())?;
    let card = cards
        .iter()
        .find(|card| card.get("id").and_then(Value::as_str) == Some(model))
        .ok_or_else(|| format!("the door publishes no card for `{model}`"))?;
    Ok(Published {
        model: card
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        adapter: card
            .get("adapter")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        // kev publishes the loaded-bytes digest under
        // `artifact_identity.digest`; a TypeSafe-shaped card may carry a
        // flat `artifact_signature` instead. Either counts; neither is
        // trusted beyond being the process's own claim.
        artifact_signature: card
            .get("artifact_identity")
            .and_then(|identity| identity.get("digest"))
            .or_else(|| card.get("artifact_signature"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        execution: card
            .get("execution")
            .and_then(Value::as_object)
            .map(|execution| {
                execution
                    .iter()
                    .map(|(key, value)| (key.clone(), scalar(value)))
                    .collect()
            })
            .unwrap_or_default(),
    })
}

/// A card field's comparable form: a string stays itself, a number or
/// boolean becomes its compact JSON — so a binding written by hand
/// matches the card whichever scalar shape the publisher chose.
fn scalar(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        other => serde_json::to_string(other).unwrap_or_default(),
    }
}

/// What a forward produced.
enum Forwarded {
    /// A 2xx — the door answered.
    Served { status: StatusCode, body: Bytes },
    /// A 4xx — the door declined, its typed refusal intact.
    Refused {
        status: StatusCode,
        body: Bytes,
        cause: String,
    },
    /// A 5xx, a timeout, or a transport failure — the attempt is named
    /// and its outcome is `unavailable`, never mistaken for a refusal.
    Unavailable { message: String },
}

/// Forward the request body to the backend's `systemone`, bounded by the
/// configured timeout and response cap.
async fn forward(state: &ServeState, endpoint: &str, body: &Bytes) -> Forwarded {
    let response = match state
        .client
        .post(format!("{endpoint}/v1/systemone"))
        .header("content-type", "application/json")
        .body(body.to_vec())
        .send()
        .await
    {
        Ok(response) => response,
        Err(error) => {
            return Forwarded::Unavailable {
                message: format!("the door could not be reached: {error}"),
            };
        }
    };
    let status = response.status();
    let limit = state.config.max_response_bytes;
    if response
        .content_length()
        .is_some_and(|length| length as usize > limit)
    {
        return Forwarded::Unavailable {
            message: format!("the door's answer exceeded {limit} bytes"),
        };
    }
    let body = match response.bytes().await {
        Ok(body) => body,
        Err(error) => {
            return Forwarded::Unavailable {
                message: format!("the door's answer could not be read: {error}"),
            };
        }
    };
    if body.len() > limit {
        return Forwarded::Unavailable {
            message: format!("the door's answer exceeded {limit} bytes"),
        };
    }
    if status.is_success() {
        Forwarded::Served { status, body }
    } else if status.is_client_error() {
        let cause = serde_json::from_slice::<Value>(&body)
            .ok()
            .and_then(|body| {
                body.get("error")
                    .and_then(|error| error.get("code"))
                    .or_else(|| body.get("code"))
                    .and_then(Value::as_str)
                    .map(str::to_string)
            })
            .unwrap_or_else(|| "refused".to_string());
        Forwarded::Refused {
            status,
            body,
            cause,
        }
    } else {
        Forwarded::Unavailable {
            message: format!("the door answered {status}"),
        }
    }
}

/// Write the attempt's receipt — one sealed document per line, beside
/// the registry it was admitted under.
async fn write_receipt(
    state: &ServeState,
    naming: &Naming<'_>,
    outcome: Outcome,
    cause: Option<&str>,
    started: Instant,
    ctx: &Context,
) -> Option<String> {
    let mut receipt = ExecutionReceipt::for_attempt(
        "http",
        naming.request,
        naming.attempt,
        naming.request_digest,
    );
    receipt.attempt_id = naming.attempt_id.clone();
    receipt.tenant = ctx.tenant_ref.clone();
    receipt.registry = ctx.registry.clone();
    receipt.requested = ctx.requested.clone();
    receipt.served = ctx.served.clone();
    receipt.outcome = outcome;
    receipt.cause = cause.map(str::to_string);
    receipt.timing = Timing {
        queued_ms: None,
        latency_ms: Some(started.elapsed().as_millis() as u64),
        resolved_at: Some(now_utc()),
    };
    receipt.result_digest = ctx.result_digest.clone();
    receipt.usage = ctx.usage.clone();
    receipt.seal();
    let line = serde_json::to_string(&receipt).ok()?;
    let mut file = state.receipts.lock().await;
    writeln!(file, "{line}").ok()?;
    file.sync_all().ok()?;
    Some(receipt.digest)
}

/// Build the response: the backend's own bytes for forwarded answers and
/// refusals, the typed error for the gateway's own refusals — every path
/// carries the request/attempt identity and the receipt digest.
fn respond(
    status: StatusCode,
    body: Bytes,
    naming: &Naming<'_>,
    outcome: Outcome,
    receipt_digest: Option<&str>,
) -> Response {
    let mut response = Response::builder()
        .status(status)
        .header("content-type", "application/json")
        .header("x-request-id", naming.request)
        .header("x-attempt", naming.attempt.to_string())
        .header("x-outcome", outcome_label(outcome));
    if let Some(digest) = receipt_digest {
        response = response.header("x-receipt", digest);
    }
    if status == StatusCode::TOO_MANY_REQUESTS {
        response = response.header("retry-after", "1");
    }
    response
        .body(axum::body::Body::from(body))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

/// An outcome's wire label.
fn outcome_label(outcome: Outcome) -> &'static str {
    match outcome {
        Outcome::Answered => "answered",
        Outcome::Refused => "refused",
        Outcome::Unavailable => "unavailable",
        Outcome::Unattempted => "unattempted",
        Outcome::Unknown => "unknown",
    }
}

/// The gateway's own typed refusal body.
fn gateway_error(status: u16, code: &'static str, message: &str) -> Response {
    Response::builder()
        .status(status)
        .header("content-type", "application/json")
        .body(
            serde_json::to_vec(&json!({"error": {"code": code, "message": message}}))
                .unwrap_or_default()
                .into(),
        )
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

/// SHA-256 of response bytes, `sha256:`-prefixed.
fn digest_bytes(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

/// The current UTC time, as RFC 3339 — the same Howard Hinnant civil
/// calendar the registry stamps its revisions with.
fn now_utc() -> String {
    let seconds = unix_now();
    let days = seconds / 86400;
    let day_seconds = seconds % 86400;
    let (year, month, day) = civil_from_days(days as i64);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        day_seconds / 3600,
        (day_seconds % 3600) / 60,
        day_seconds % 60
    )
}

/// Days since the epoch to a calendar date, by Howard Hinnant's algorithm.
fn civil_from_days(days: i64) -> (i64, u64, u64) {
    let days = days + 719_468;
    let era = if days >= 0 { days } else { days - 146_096 } / 146_097;
    let day_of_era = (days - era * 146_097) as u64;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36524 - day_of_era / 146_096) / 365;
    let year = year_of_era as i64 + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = if month_prime < 10 {
        month_prime + 3
    } else {
        month_prime - 9
    };
    (if month <= 2 { year + 1 } else { year }, month, day)
}

/// Unix seconds now.
fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|span| span.as_secs())
        .unwrap_or_default()
}
