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
use tokio::sync::{Mutex, OwnedSemaphorePermit, Semaphore};

use tenancy::{Admission, Capacity, Published, Registry, keys, lane_name, quota};

use crate::classify::{self, Mode, Request as ClassifyRequest};
use crate::config::{Config, Door};

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
    in_flight: Arc<Semaphore>,
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
            in_flight: Arc::new(Semaphore::new(config.max_in_flight)),
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
        .route("/v1/classify", post(classify))
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
#[derive(Clone, Default)]
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
    /// The call resolved — the backend's own bytes, or a response the
    /// facade assembled from per-item forwards.
    Forwarded {
        status: StatusCode,
        body: Bytes,
        /// The receipt-level outcome.
        outcome: Outcome,
        /// The `x-outcome` wire label — the outcome's name except where
        /// a facade reports an explicit `mixed`.
        label: &'static str,
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
    conclude(&state, &naming, started, verdict).await
}

/// `POST /v1/classify`: the classification facade — the same admission
/// path, then one `systemone` forward per input.
async fn classify(
    State(state): State<Arc<ServeState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let started = Instant::now();

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

    let verdict = classify_admitted(&state, &headers, &body, &naming).await;
    conclude(&state, &naming, started, verdict).await
}

/// The shared tail every attempt ends with: a sealed receipt, then the
/// response carrying the request/attempt identity and the receipt's
/// digest.
async fn conclude(
    state: &Arc<ServeState>,
    naming: &Naming<'_>,
    started: Instant,
    verdict: Verdict,
) -> Response {
    let (status, outcome, label, body_out, cause, ctx) = match verdict {
        Verdict::Forwarded {
            status,
            body,
            outcome,
            label,
            cause,
            ctx,
        } => (status, outcome, label, body, cause, ctx),
        Verdict::Refused {
            status,
            code,
            message,
            outcome,
            ctx,
        } => (
            status,
            outcome,
            outcome_label(outcome),
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
        write_receipt(state, naming, outcome, cause.as_deref(), started, &ctx).await;
    respond(
        status,
        body_out,
        naming,
        label,
        receipt_digest.as_deref(),
    )
}

/// The permits a bound attempt holds until it resolves.
struct Permits {
    /// The door's own forward slot, when the binding declares one.
    _door: Option<OwnedSemaphorePermit>,
    /// The process-wide forward slot.
    _host: OwnedSemaphorePermit,
}

/// Step 1 of every route: authenticate the caller and open the
/// receipt's context.
fn authenticated(
    state: &ServeState,
    headers: &HeaderMap,
) -> Result<(Registry, Caller, Context), Verdict> {
    let (registry, caller) = authenticate(state, headers).map_err(|(status, code, message)| {
        Verdict::Refused {
            status,
            code,
            message,
            outcome: Outcome::Refused,
            ctx: Context::default(),
        }
    })?;
    let ctx = Context {
        tenant_ref: caller.tenant.as_ref().map(|_| caller.key.clone()),
        ..Context::default()
    };
    Ok((registry, caller, ctx))
}

/// Step 2 of every route: authorize the named door and find its
/// configured backend.
fn authorized(
    state: &ServeState,
    registry: &Registry,
    caller: &Caller,
    door: &str,
    ctx: &mut Context,
) -> Result<(Admission, Door), Verdict> {
    let admission = registry
        .authorize(caller.tenant.as_deref(), door)
        .map_err(|refusal| Verdict::Refused {
            status: StatusCode::FORBIDDEN,
            code: "door_not_bound",
            message: refusal.to_string(),
            outcome: Outcome::Refused,
            ctx: ctx.clone(),
        })?;
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
    let Some(backend) = state.config.doors.get(door) else {
        return Err(Verdict::Refused {
            status: StatusCode::SERVICE_UNAVAILABLE,
            code: "door_unavailable",
            message: format!("door `{door}` is bound but no backend is configured for it"),
            outcome: Outcome::Refused,
            ctx: ctx.clone(),
        });
    };
    Ok((admission, backend.clone()))
}

/// Step 3 of every route: the door's declared rate window and
/// concurrency, then the process-wide forward bound — each refusing
/// before any reservation exists, so a congestion refusal never holds
/// quota.
async fn bounded(
    state: &ServeState,
    door: &str,
    capacity: &Capacity,
    ctx: &Context,
) -> Result<Permits, Verdict> {
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
                return Err(Verdict::Refused {
                    status: StatusCode::TOO_MANY_REQUESTS,
                    code: "rate_limited",
                    message: format!(
                        "door `{door}` admits {} calls a minute under this binding",
                        bounds.rate
                    ),
                    outcome: Outcome::Refused,
                    ctx: ctx.clone(),
                });
            }
            bounds.window.push_back(Instant::now());
        }
    }
    let door_permit = match door_permit(state, door).await {
        Some(permit) => Some(permit),
        None => {
            return Err(Verdict::Refused {
                status: StatusCode::TOO_MANY_REQUESTS,
                code: "busy",
                message: format!("door `{door}`'s forward slots are full; retry shortly"),
                outcome: Outcome::Refused,
                ctx: ctx.clone(),
            });
        }
    };
    let Ok(host_permit) = state.in_flight.clone().try_acquire_owned() else {
        return Err(Verdict::Refused {
            status: StatusCode::TOO_MANY_REQUESTS,
            code: "overloaded",
            message: "the gateway's forward bound is full; retry shortly".to_string(),
            outcome: Outcome::Refused,
            ctx: ctx.clone(),
        });
    };
    Ok(Permits {
        _door: door_permit.flatten(),
        _host: host_permit,
    })
}

/// Step 4 of every route: the durable reservation. From here the
/// attempt settles or it orphans, and either way the ledger accounts
/// for it.
async fn reserved(
    state: &ServeState,
    registry: &Registry,
    caller: &Caller,
    naming: &Naming<'_>,
    units: &quota::Units,
    ctx: &mut Context,
) -> Result<(), Verdict> {
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
                units,
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
        return Err(Verdict::Refused {
            status: StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
            code,
            message: refusal.to_string(),
            outcome: Outcome::Refused,
            ctx: ctx.clone(),
        });
    }
    ctx.usage = Some(format!("{}#{}", naming.request, naming.attempt));
    Ok(())
}

/// Step 5 of every route: the backend's published identity against the
/// binding — before a byte of the request is forwarded. A failed check
/// releases the reservation rather than charging it.
async fn verified(
    state: &ServeState,
    endpoint: &str,
    admission: &Admission,
    naming: &Naming<'_>,
    ctx: &mut Context,
) -> Result<(), Verdict> {
    let published =
        match published_identity(state, endpoint, &admission.binding.artifact.model).await {
            Ok(published) => published,
            Err(message) => {
                state.release(naming.request, naming.attempt).await;
                return Err(Verdict::Refused {
                    status: StatusCode::SERVICE_UNAVAILABLE,
                    code: "door_unavailable",
                    message,
                    outcome: Outcome::Unattempted,
                    ctx: ctx.clone(),
                });
            }
        };
    if let Err(fault) = admission.verify(&published) {
        state.release(naming.request, naming.attempt).await;
        return Err(Verdict::Refused {
            status: StatusCode::SERVICE_UNAVAILABLE,
            code: "identity_mismatch",
            message: fault.to_string(),
            outcome: Outcome::Unattempted,
            ctx: ctx.clone(),
        });
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
    Ok(())
}

/// Settle the attempt's reservation from its recorded outcome.
async fn settled(state: &ServeState, naming: &Naming<'_>, outcome: Outcome, units: &quota::Units) {
    let settle_outcome = match outcome {
        Outcome::Answered => quota::Outcome::Answered,
        Outcome::Refused => quota::Outcome::Refused,
        Outcome::Unavailable => quota::Outcome::Unavailable,
        Outcome::Unattempted => quota::Outcome::Unattempted,
        Outcome::Unknown => quota::Outcome::Unknown,
    };
    let mut ledger = state.ledger.lock().await;
    ledger
        .settle(naming.request, naming.attempt, settle_outcome, units)
        .ok();
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
    // 1. Authenticate.
    let (registry, caller, mut ctx) = match authenticated(state, headers) {
        Ok(parts) => parts,
        Err(verdict) => return verdict,
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
    // Shape bounds, before a door is consulted: the question count and
    // the option total a `choice`/`score` request would pay to read out.
    if let Some(questions) = envelope.get("questions").and_then(Value::as_object) {
        let options: u64 = questions
            .values()
            .map(|question| {
                question
                    .get("options")
                    .and_then(Value::as_array)
                    .map_or(0, |options| options.len() as u64)
            })
            .sum();
        if questions.len() as u64 > state.config.max_questions {
            return Verdict::Refused {
                status: StatusCode::UNPROCESSABLE_ENTITY,
                code: "too_many_questions",
                message: format!(
                    "the request carries {} questions; this gateway admits {}",
                    questions.len(),
                    state.config.max_questions
                ),
                outcome: Outcome::Refused,
                ctx,
            };
        }
        if options > state.config.max_options {
            return Verdict::Refused {
                status: StatusCode::UNPROCESSABLE_ENTITY,
                code: "too_many_options",
                message: format!(
                    "the request carries {options} options; this gateway admits {}",
                    state.config.max_options
                ),
                outcome: Outcome::Refused,
                ctx,
            };
        }
    }
    // 2. Authorize.
    let (admission, backend) = match authorized(state, &registry, &caller, door, &mut ctx) {
        Ok(parts) => parts,
        Err(verdict) => return verdict,
    };
    let endpoint = backend.endpoint.clone();

    // 3–5. Bound, reserve, verify.
    let capacity = admission.binding.capacity.clone().unwrap_or_default();
    let _permits = match bounded(state, door, &capacity, &ctx).await {
        Ok(permits) => permits,
        Err(verdict) => return verdict,
    };
    let units = units_of(envelope, body.len());
    if let Err(verdict) = reserved(state, &registry, &caller, naming, &units, &mut ctx).await {
        return verdict;
    }
    if let Err(verdict) = verified(state, &endpoint, &admission, naming, &mut ctx).await {
        return verdict;
    }

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
    settled(state, naming, outcome, &units).await;
    Verdict::Forwarded {
        status,
        body: body_out,
        outcome,
        label: outcome_label(outcome),
        cause,
        ctx,
    }
}

/// The classify call's passage: the same admission sequence, then a
/// `systemone` forward per input and an assembled per-item result.
async fn classify_admitted(
    state: &ServeState,
    headers: &HeaderMap,
    body: &Bytes,
    naming: &Naming<'_>,
) -> Verdict {
    let started = Instant::now();
    // 1. Authenticate, then the typed envelope — a malformed envelope
    // is refused before a door is consulted.
    let (registry, caller, mut ctx) = match authenticated(state, headers) {
        Ok(parts) => parts,
        Err(verdict) => return verdict,
    };
    let request = match ClassifyRequest::parse(body) {
        Ok(request) => request,
        Err(refusal) => {
            let status = match refusal {
                classify::Refusal::Malformed(_) => StatusCode::BAD_REQUEST,
                _ => StatusCode::UNPROCESSABLE_ENTITY,
            };
            return Verdict::Refused {
                status,
                code: refusal.code(),
                message: refusal.to_string(),
                outcome: Outcome::Refused,
                ctx,
            };
        }
    };

    // 2. Authorize the named door. The capacity the caller names is
    // the binding's lane — anything else is an unsupported
    // combination, not an option with no effect.
    let (admission, backend) =
        match authorized(state, &registry, &caller, &request.model, &mut ctx) {
            Ok(parts) => parts,
            Err(verdict) => return verdict,
        };
    if request.capacity != lane_name(admission.binding.lane) {
        return Verdict::Refused {
            status: StatusCode::UNPROCESSABLE_ENTITY,
            code: "unsupported_capacity",
            message: format!(
                "door `{}` binds the `{}` lane; the request names `{}`",
                request.model,
                lane_name(admission.binding.lane),
                request.capacity
            ),
            outcome: Outcome::Refused,
            ctx,
        };
    }

    // The facade serves only bounds the door declares — an undeclared
    // limit is unsupported, never the product maximum.
    let limits = match backend.classify {
        Some(limits) => limits,
        None => {
            return Verdict::Refused {
                status: StatusCode::UNPROCESSABLE_ENTITY,
                code: "unsupported_limits",
                message: format!(
                    "door `{}` declares no classify bounds — the facade does not \
                     infer support it was not told about",
                    request.model
                ),
                outcome: Outcome::Refused,
                ctx,
            };
        }
    };
    let plan = match request.plan(&limits) {
        Ok(plan) => plan,
        Err(refusal) => {
            return Verdict::Refused {
                status: StatusCode::UNPROCESSABLE_ENTITY,
                code: refusal.code(),
                message: refusal.to_string(),
                outcome: Outcome::Refused,
                ctx,
            };
        }
    };
    let endpoint = backend.endpoint.clone();

    // 3–5. Bound, reserve, verify — the reservation's units are the
    // plan's: one question per judgment, the readout width of every
    // categorical set, and the envelope's own bytes.
    let capacity = admission.binding.capacity.clone().unwrap_or_default();
    let _permits = match bounded(state, &request.model, &capacity, &ctx).await {
        Ok(permits) => permits,
        Err(verdict) => return verdict,
    };
    let options: u64 = plan
        .units
        .iter()
        .filter(|unit| unit.mode == Mode::SingleLabel)
        .map(|unit| plan.inputs * unit.labels.len() as u64)
        .sum();
    let units = quota::Units {
        questions: plan.judgments,
        input_bytes: body.len() as u64,
        options,
    };
    if let Err(verdict) = reserved(state, &registry, &caller, naming, &units, &mut ctx).await {
        return verdict;
    }
    if let Err(verdict) = verified(state, &endpoint, &admission, naming, &mut ctx).await {
        return verdict;
    }

    // 6. Fan out — one `systemone` forward per input, in request
    // order, holding the door's single permit for the whole call.
    let artifact = admission.binding.artifact.model.clone();
    let mut items = Vec::with_capacity(request.inputs.len());
    let mut counts = Counts::default();
    let mut forwards = 0_u64;
    let mut input_tokens: Option<u64> = None;
    let mut output_tokens: Option<u64> = None;
    let mut halted = false;
    for input in &request.inputs {
        if halted {
            // A door that stopped answering mid-call leaves the rest
            // unattempted — named, never dropped.
            items.push(unattempted_item(input, &plan));
            counts.unattempted += plan.units.len() as u64;
            continue;
        }
        let (forward_body, asked) = forward_body(&request, &plan, input, &artifact);
        let item_started = Instant::now();
        forwards += 1;
        match forward(state, &endpoint, &forward_body).await {
            Forwarded::Served { body, .. } => {
                let (item, usage) = served_item(
                    input,
                    &plan,
                    &asked,
                    &body,
                    item_started.elapsed(),
                    &mut counts,
                );
                if let Some(usage) = usage {
                    if let Some(tokens) = usage.get("input_tokens").and_then(Value::as_u64) {
                        *input_tokens.get_or_insert(0) += tokens;
                    }
                    if let Some(tokens) = usage.get("output_tokens").and_then(Value::as_u64) {
                        *output_tokens.get_or_insert(0) += tokens;
                    }
                }
                items.push(item);
            }
            Forwarded::Refused { cause, .. } => {
                items.push(failed_item(
                    input,
                    &plan,
                    "refused",
                    &cause,
                    Some(item_started.elapsed()),
                ));
                counts.refused += plan.units.len() as u64;
            }
            Forwarded::Unavailable { message } => {
                items.push(failed_item(
                    input,
                    &plan,
                    "unavailable",
                    &message,
                    Some(item_started.elapsed()),
                ));
                counts.unavailable += plan.units.len() as u64;
                halted = true;
            }
        }
    }

    // Aggregate the per-unit outcomes into the call's own.
    let total = plan.inputs * plan.units.len() as u64;
    let (label, status, outcome, cause) = if counts.answered == total {
        ("answered", StatusCode::OK, Outcome::Answered, None)
    } else if counts.answered > 0 {
        (
            "mixed",
            StatusCode::OK,
            Outcome::Answered,
            Some("mixed".to_string()),
        )
    } else if counts.unavailable + counts.unattempted > 0 {
        (
            "unavailable",
            StatusCode::SERVICE_UNAVAILABLE,
            Outcome::Unavailable,
            Some("unavailable".to_string()),
        )
    } else {
        (
            "refused",
            StatusCode::UNPROCESSABLE_ENTITY,
            Outcome::Refused,
            items
                .iter()
                .find_map(|item| item.get("cause").and_then(Value::as_str))
                .map(str::to_string),
        )
    };
    let mut usage = json!({"forwards": forwards});
    if let Some(tokens) = input_tokens {
        usage["input_tokens"] = json!(tokens);
    }
    if let Some(tokens) = output_tokens {
        usage["output_tokens"] = json!(tokens);
    }
    let response = json!({
        "v": classify::SCHEMA,
        "model": request.model,
        "capacity": request.capacity,
        "policy": request.policy,
        "served": serde_json::to_value(&ctx.served).unwrap_or_default(),
        "outcome": label,
        "outcomes": {
            "answered": counts.answered,
            "refused": counts.refused,
            "unavailable": counts.unavailable,
            "unattempted": counts.unattempted,
        },
        "results": items,
        "usage": usage,
        "timing": {"latency_ms": started.elapsed().as_millis() as u64},
    });
    let body_out = Bytes::from(serde_json::to_vec(&response).unwrap_or_default());
    ctx.result_digest = Some(digest_bytes(&body_out));
    settled(state, naming, outcome, &units).await;
    Verdict::Forwarded {
        status,
        body: body_out,
        outcome,
        label,
        cause,
        ctx,
    }
}

/// The per-unit outcome tally a classify call aggregates.
#[derive(Default)]
struct Counts {
    /// Units answered.
    answered: u64,
    /// Units refused.
    refused: u64,
    /// Units unavailable.
    unavailable: u64,
    /// Units never dispatched.
    unattempted: u64,
}

/// The questions one input's forward asks, and the map from question
/// id back to (unit, label) the answers are read through.
fn forward_body(
    request: &ClassifyRequest,
    plan: &classify::Plan,
    input: &classify::Input,
    model: &str,
) -> (Bytes, Vec<(usize, String, Option<String>)>) {
    let mut questions = serde_json::Map::new();
    let mut asked = Vec::new();
    let mut next = 0_u64;
    for (unit_index, unit) in plan.units.iter().enumerate() {
        match unit.mode {
            Mode::SingleLabel => {
                let qid = format!("q{next}");
                next += 1;
                let mut criteria = serde_json::Map::new();
                for label in &unit.labels {
                    criteria.insert(
                        label.id.clone(),
                        label
                            .description
                            .clone()
                            .map(Value::String)
                            .unwrap_or(Value::Null),
                    );
                }
                let instructions = instructions_for(
                    request,
                    unit,
                    "Pick exactly one of the options that best describes the input.",
                );
                questions.insert(
                    qid.clone(),
                    json!({"type": "choice", "instructions": instructions,
                           "criteria": criteria}),
                );
                asked.push((unit_index, qid, None));
            }
            Mode::MultiLabel => {
                for label in &unit.labels {
                    let qid = format!("q{next}");
                    next += 1;
                    let framing = match &label.description {
                        Some(description) => format!(
                            "Decide whether the label `{}` applies to the input. \
                             The label means: {description}",
                            label.id
                        ),
                        None => format!(
                            "Decide whether the label `{}` applies to the input.",
                            label.id
                        ),
                    };
                    let instructions = instructions_for(request, unit, &framing);
                    questions.insert(
                        qid.clone(),
                        json!({"type": "noul", "instructions": instructions}),
                    );
                    asked.push((unit_index, qid, Some(label.id.clone())));
                }
            }
        }
    }
    let state = match (&input.text, &input.record) {
        (Some(text), None) => Value::String(text.clone()),
        (None, Some(record)) => Value::Object(record.clone()),
        // plan() already proved exactly one content form per input.
        _ => Value::Null,
    };
    let body = serde_json::to_vec(&json!({
        "model": model,
        "state": state,
        "questions": questions,
    }))
    .unwrap_or_default();
    (Bytes::from(body), asked)
}

/// The instructions one question carries: the request's own, the
/// dimension's, then the judgment's framing — in that order.
fn instructions_for(
    request: &ClassifyRequest,
    unit: &classify::Unit,
    framing: &str,
) -> String {
    let dimension = unit
        .dimension
        .as_deref()
        .and_then(|id| {
            request
                .dimensions
                .iter()
                .find(|dimension| dimension.id == id)
        })
        .and_then(|dimension| dimension.instructions.as_deref());
    [request.instructions.as_deref(), dimension, Some(framing)]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// One input's assembled result when its forward answered: each unit's
/// outcome, raw answers, and policy-selected output.
fn served_item(
    input: &classify::Input,
    plan: &classify::Plan,
    asked: &[(usize, String, Option<String>)],
    body: &Bytes,
    latency: Duration,
    counts: &mut Counts,
) -> (Value, Option<Value>) {
    let parsed = serde_json::from_slice::<Value>(body).ok();
    let answers = parsed
        .as_ref()
        .and_then(|body| body.get("answers"))
        .and_then(Value::as_object)
        .cloned();
    let model = parsed
        .as_ref()
        .and_then(|body| body.get("model"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let usage = parsed
        .as_ref()
        .and_then(|body| body.get("usage"))
        .cloned();
    let mut units = Vec::with_capacity(plan.units.len());
    let mut answered = 0_u64;
    for (unit_index, unit) in plan.units.iter().enumerate() {
        let result = match answers.as_ref() {
            Some(answers) => unit_result(unit_index, unit, plan, asked, answers),
            None => unit_failure(unit, "unavailable", "the door's answer did not parse"),
        };
        if result.get("outcome").and_then(Value::as_str) == Some("answered") {
            counts.answered += 1;
            answered += 1;
        } else {
            counts.unavailable += 1;
        }
        units.push(result);
    }
    let input_outcome = if answered == units.len() as u64 {
        "answered"
    } else if answered > 0 {
        "mixed"
    } else {
        "unavailable"
    };
    let mut item = json!({
        "input": input.id,
        "outcome": input_outcome,
        "units": units,
        "latency_ms": latency.as_millis() as u64,
    });
    if let Some(model) = model {
        item["model"] = json!(model);
    }
    (item, usage)
}

/// One unit's result inside an answered forward: the raw answer and
/// the policy's selected output, or an unavailable outcome when the
/// answer does not hold to the primitive's contract.
fn unit_result(
    unit_index: usize,
    unit: &classify::Unit,
    plan: &classify::Plan,
    asked: &[(usize, String, Option<String>)],
    answers: &serde_json::Map<String, Value>,
) -> Value {
    let mut base = json!({
        "mode": match unit.mode {
            Mode::SingleLabel => "single-label",
            Mode::MultiLabel => "multi-label",
        },
    });
    if let Some(dimension) = &unit.dimension {
        base["dimension"] = json!(dimension);
    }
    let mut asked = asked
        .iter()
        .filter(|(index, _, _)| *index == unit_index);
    match unit.mode {
        Mode::SingleLabel => {
            let Some(rule) = plan.policy.select.single_label.as_ref() else {
                return unit_failure(unit, "unavailable", "no declared selection rule");
            };
            let Some(answer) = asked.next().and_then(|(_, qid, _)| answers.get(qid)) else {
                return unit_failure(unit, "unavailable", "the door answered no choice");
            };
            let Some(probabilities) = answer.get("probabilities").and_then(Value::as_object) else {
                return unit_failure(unit, "unavailable", "the choice answer names no distribution");
            };
            let mut pairs = Vec::with_capacity(unit.labels.len());
            for label in &unit.labels {
                match probabilities.get(&label.id).and_then(Value::as_f64) {
                    Some(probability) if (0.0..=1.0).contains(&probability) => {
                        pairs.push((label.id.clone(), probability));
                    }
                    _ => {
                        return unit_failure(
                            unit,
                            "unavailable",
                            "the distribution does not cover the label set",
                        );
                    }
                }
            }
            base["outcome"] = json!("answered");
            base["raw"] = answer.clone();
            base["selected"] = rule.select(&pairs);
            base
        }
        Mode::MultiLabel => {
            let Some(rule) = plan.policy.select.multi_label.as_ref() else {
                return unit_failure(unit, "unavailable", "no declared selection rule");
            };
            let mut raw = serde_json::Map::new();
            let mut pairs = Vec::with_capacity(unit.labels.len());
            for (index, qid, label) in asked {
                debug_assert_eq!(*index, unit_index);
                let Some(label) = label else { continue };
                let Some(answer) = answers.get(qid) else {
                    return unit_failure(unit, "unavailable", "the door answered no noul");
                };
                let Some(probability) = answer.get("noul").and_then(Value::as_f64) else {
                    return unit_failure(unit, "unavailable", "a noul answer holds no probability");
                };
                if !(0.0..=1.0).contains(&probability) {
                    return unit_failure(unit, "unavailable", "a noul answer is not a probability");
                }
                raw.insert(label.clone(), answer.clone());
                pairs.push((label.clone(), probability));
            }
            base["outcome"] = json!("answered");
            base["raw"] = Value::Object(raw);
            base["selected"] = rule.select(&pairs);
            base
        }
    }
}

/// One unit's result when it carries no usable answer.
fn unit_failure(unit: &classify::Unit, outcome: &str, cause: &str) -> Value {
    let mut base = json!({
        "mode": match unit.mode {
            Mode::SingleLabel => "single-label",
            Mode::MultiLabel => "multi-label",
        },
        "outcome": outcome,
        "cause": cause,
        "selected": Value::Null,
    });
    if let Some(dimension) = &unit.dimension {
        base["dimension"] = json!(dimension);
    }
    base
}

/// One input's result when its forward failed before any unit could
/// answer — every unit reports the same outcome and cause.
fn failed_item(
    input: &classify::Input,
    plan: &classify::Plan,
    outcome: &str,
    cause: &str,
    latency: Option<Duration>,
) -> Value {
    let mut item = json!({
        "input": input.id,
        "outcome": outcome,
        "cause": cause,
        "units": plan.units.iter().map(|unit| unit_failure(unit, outcome, cause)).collect::<Vec<_>>(),
    });
    if let Some(latency) = latency {
        item["latency_ms"] = json!(latency.as_millis() as u64);
    }
    item
}

/// One input's result when the call stopped before its forward —
/// dispatched nothing, charged nothing past the reservation.
fn unattempted_item(input: &classify::Input, plan: &classify::Plan) -> Value {
    failed_item(
        input,
        plan,
        "unattempted",
        "the call stopped before this input's forward",
        None,
    )
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
    label: &str,
    receipt_digest: Option<&str>,
) -> Response {
    let mut response = Response::builder()
        .status(status)
        .header("content-type", "application/json")
        .header("x-request-id", naming.request)
        .header("x-attempt", naming.attempt.to_string())
        .header("x-outcome", label);
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
