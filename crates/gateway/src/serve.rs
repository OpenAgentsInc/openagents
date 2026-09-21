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
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
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
use tokio::sync::{Mutex, OwnedSemaphorePermit, Semaphore, watch};

use tenancy::{Admission, Capacity, Published, Registry, keys, lane_name, quota};

use crate::classify::{self, Mode, RankOrder, Request as ClassifyRequest};
use crate::config::{Config, Door};
use crate::money;

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
    /// The spending ledger did not open — a lock, a damaged log, or a
    /// path that is not a private regular file.
    Money(String),
}

impl std::fmt::Display for Trouble {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "{error}"),
            Self::Ledger(trouble) => write!(f, "{trouble}"),
            Self::Registry(trouble) => write!(f, "{trouble}"),
            Self::Money(message) => write!(f, "{message}"),
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
    /// The workspace spending ledger — present only when the operator
    /// opted in to monetary admission, and locked for the process's
    /// lifetime when it is.
    money: Option<Mutex<tenancy::money::Ledger>>,
    receipts: Mutex<std::fs::File>,
    /// The process-wide forward bound.
    in_flight: Arc<Semaphore>,
    classify_inputs: Arc<Semaphore>,
    tenant_classify_inputs: Mutex<HashMap<Option<String>, Arc<Semaphore>>>,
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
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_millis(config.forward_timeout_ms))
            .build()
            .map_err(|error| Trouble::Io(std::io::Error::other(error.to_string())))?;
        // Monetary admission opens its ledger at startup — a spending
        // store that cannot open fails the process, not the first call.
        let money = config
            .money
            .as_ref()
            .map(|money| tenancy::money::Ledger::open(&money.ledger))
            .transpose()
            .map_err(Trouble::Money)?
            .map(Mutex::new);
        Ok(Arc::new(Self {
            dir: config.registry.clone(),
            in_flight: Arc::new(Semaphore::new(config.max_in_flight)),
            classify_inputs: Arc::new(Semaphore::new(config.max_classify_inputs as usize)),
            tenant_classify_inputs: Mutex::new(HashMap::new()),
            config,
            client,
            ledger: Mutex::new(ledger),
            money,
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
    let router = axum::Router::new()
        .route("/v1/systemone", post(systemone))
        .route("/v1/classify", post(classify))
        .route("/v1/models", get(models))
        .route("/healthz", get(healthz));
    // The balance read exists only under monetary admission — absent the
    // mode there is no ledger behind it and no route at all.
    let router = if state.config.money.is_some() {
        router.route("/v1/balance", get(balance))
    } else {
        router
    };
    router
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
                "admission": {
                    "scope": binding.scope,
                    "record": binding.promotion,
                    "registry_digest": manifest.digest,
                    "registry_sequence": manifest.sequence,
                },
                "classification": classification_card(&state, door, binding),
            }))
        })
        .collect();
    Ok(Json(json!({"models": cards})))
}

/// `GET /v1/balance`: the caller's workspace account position under
/// monetary admission — exact credited, reserved, settled, refunded,
/// available, and remaining authorized spend, plus the price versions
/// the configured doors charge under.
///
/// The read is scoped by the same authenticated membership the decision
/// path requires: the `X-Workspace-Id` header names the account, and a
/// caller can only ever read a workspace it belongs to. There is no
/// top-up or payment mutation here — account funding is an operator act
/// on the ledger itself.
async fn balance(
    State(state): State<Arc<ServeState>>,
    headers: HeaderMap,
) -> Result<Json<Value>, Response> {
    let (_registry, caller) = match authenticate(&state, &headers) {
        Ok(parts) => parts,
        Err((status, code, message)) => return Err(gateway_error(status.as_u16(), code, &message)),
    };
    let Some(workspace) = caller.workspace else {
        return Err(gateway_error(
            400,
            "workspace_required",
            "one X-Workspace-Id header is required",
        ));
    };
    let Some(ledger) = &state.money else {
        return Err(gateway_error(
            404,
            "unmetered",
            "this gateway does not run monetary admission",
        ));
    };
    let balance = ledger.lock().await.balance(&workspace).map_err(|error| {
        gateway_error(
            404,
            "account_missing",
            &format!("workspace `{workspace}` holds no monetary account: {error}"),
        )
    })?;
    let prices: serde_json::Map<String, Value> = state
        .config
        .money
        .as_ref()
        .map(|money| {
            money
                .doors
                .iter()
                .map(|(door, priced)| {
                    (
                        door.clone(),
                        json!({
                            "version": priced.price.version,
                            "policy": priced.price.policy,
                            "currency": priced.price.currency,
                        }),
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    Ok(Json(json!({
        "workspace": workspace,
        "balance": serde_json::to_value(&balance).unwrap_or_default(),
        "prices": prices,
    })))
}

/// Publish configured admission bounds without claiming a live backend probe or
/// model quality. Missing declarations never inherit product maxima.
fn classification_card(state: &ServeState, door: &str, binding: &tenancy::Binding) -> Value {
    let mut card = json!({
        "v": "openagents.classify-discovery.v1",
        "request_schema": classify::SCHEMA,
        "policy_schema": classify::POLICY_SCHEMA,
        "status": "unavailable",
    });
    let Some(backend) = state.config.doors.get(door) else {
        return card;
    };
    let Some(mut limits) = backend.classify else {
        card["status"] = json!("unsupported");
        return card;
    };
    if limits.check().is_err() {
        card["status"] = json!("invalid-limits");
        return card;
    }
    limits.max_inputs = limits
        .max_inputs
        .min(u64::from(state.config.max_classify_inputs))
        .min(u64::from(state.config.max_classify_inputs_per_tenant));
    let capacity = binding.capacity.clone().unwrap_or_default();
    let concurrency = backend
        .classify_item_concurrency
        .min(capacity.concurrency.unwrap_or(u64::MAX))
        .min(state.config.max_in_flight as u64)
        .min(limits.max_inputs);
    let mut modes = vec!["multi-label", "binary"];
    if limits.max_labels >= 2 {
        modes.push("single-label");
    }
    if limits.max_levels >= 2 {
        modes.push("score");
    }
    card["status"] = json!("configured");
    card["limits"] = json!(limits);
    card["modes"] = json!(modes);
    card["admission"] = json!({
        "max_body_bytes": state.config.max_body_bytes,
        "max_pending_inputs": state.config.max_classify_inputs,
        "max_pending_inputs_per_tenant": state.config.max_classify_inputs_per_tenant,
        "requires_workspace_membership": state.config.require_workspace_membership,
        "context_tokens": null,
    });
    card["execution"] = json!({
        "kind": "native-per-input",
        "max_item_concurrency": concurrency,
        "timeout_ms": state.config.forward_timeout_ms,
        "model_packing": false,
    });
    card
}

/// Who the call is, once authentication has run.
struct Caller {
    /// The tenant the key resolved to, or `None` for anonymous.
    tenant: Option<String>,
    /// The credential id the call authenticated under — a reference,
    /// never the secret.
    key: String,
    /// The workspace the membership check admitted — present only when
    /// `require_workspace_membership` ran, and the account monetary
    /// admission charges.
    workspace: Option<String>,
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
        if state.config.require_workspace_membership {
            return Err((
                StatusCode::UNAUTHORIZED,
                "unauthenticated",
                "workspace membership requires a bearer key".into(),
            ));
        }
        return Ok((
            registry,
            Caller {
                tenant: None,
                key: "anonymous".to_string(),
                workspace: None,
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
    let mut workspace = None;
    if state.config.require_workspace_membership {
        let mut values = headers.get_all("x-workspace-id").iter();
        let named = values
            .next()
            .and_then(|value| value.to_str().ok())
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                (
                    StatusCode::BAD_REQUEST,
                    "workspace_required",
                    "one X-Workspace-Id header is required".into(),
                )
            })?;
        if values.next().is_some() {
            return Err((
                StatusCode::BAD_REQUEST,
                "workspace_required",
                "one X-Workspace-Id header is required".into(),
            ));
        }
        let accounts = tenancy::Accounts::open(&state.dir).map_err(|_| {
            (
                StatusCode::SERVICE_UNAVAILABLE,
                "membership_unavailable",
                "the workspace membership store is unavailable".into(),
            )
        })?;
        accounts
            .authenticate_key(registry.manifest(), named, token)
            .map_err(|cause| match cause {
                tenancy::accounts::Refusal::Store(_) => (
                    StatusCode::SERVICE_UNAVAILABLE,
                    "membership_unavailable",
                    "the workspace membership store is unavailable".into(),
                ),
                tenancy::accounts::Refusal::Authentication(_) => (
                    StatusCode::UNAUTHORIZED,
                    "unauthenticated",
                    "the credential is no longer valid".into(),
                ),
                _ => (
                    StatusCode::FORBIDDEN,
                    "workspace_forbidden",
                    "the credential has no active membership in this workspace and tenant".into(),
                ),
            })?;
        workspace = Some(named.to_string());
    }
    Ok((
        registry,
        Caller {
            tenant: Some(authenticated.tenant),
            key: authenticated.key_id,
            workspace,
        },
    ))
}

/// What an attempt needs for its receipt — the identities it ran under.
type Context = Box<ReceiptContext>;

#[derive(Clone, Default)]
struct ReceiptContext {
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
    /// How the attempt's monetary hold resolved — `settled`,
    /// `outstanding`, or `released` — when monetary admission ran.
    settlement: Option<&'static str>,
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

/// A dropped HTTP handler signals cancellation without dropping durable cleanup.
struct OnDisconnect(watch::Sender<bool>);

impl Drop for OnDisconnect {
    fn drop(&mut self) {
        let _ = self.0.send(true);
    }
}

#[derive(Clone)]
struct Cancellation(watch::Receiver<bool>);

impl Cancellation {
    fn stopped(&self) -> bool {
        *self.0.borrow()
    }

    async fn wait(&self) {
        let mut receiver = self.0.clone();
        let _ = receiver.wait_for(|stopped| *stopped).await;
    }
}

async fn systemone(
    State(state): State<Arc<ServeState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    owned_request(state, headers, body, false).await
}

async fn classify(
    State(state): State<Arc<ServeState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    owned_request(state, headers, body, true).await
}

/// Connection loss cancels work, but the owned task retains its reservations
/// until settlement and receipt recording finish. It never retries a forward.
async fn owned_request(
    state: Arc<ServeState>,
    headers: HeaderMap,
    body: Bytes,
    classification: bool,
) -> Response {
    let (sender, receiver) = watch::channel(false);
    let _on_disconnect = OnDisconnect(sender);
    let cancellation = Cancellation(receiver);
    match tokio::spawn(async move {
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
        let mut verdict = if classification {
            classify_admitted(&state, &headers, &body, &naming, &cancellation).await
        } else {
            admitted(&state, &headers, &envelope, &body, &naming, &cancellation).await
        };
        if cancellation.stopped()
            && let Verdict::Forwarded { cause, .. } = &mut verdict
        {
            *cause = Some("caller_disconnected".into());
        }
        conclude(&state, &naming, started, verdict).await
    })
    .await
    {
        Ok(response) => response,
        Err(_) => gateway_error(
            503,
            "unavailable",
            "request execution stopped; completion requires reconciliation",
        ),
    }
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
        } => {
            let mut body = json!({
                "error": {"code": code, "message": message,
                          "request": naming.request, "attempt": naming.attempt},
            });
            if let Some(settlement) = ctx.settlement {
                body["settlement"] = json!(settlement);
            }
            (
                status,
                outcome,
                outcome_label(outcome),
                serde_json::to_vec(&body).unwrap_or_default().into(),
                Some(code.to_string()),
                ctx,
            )
        }
    };

    let settlement = ctx.settlement;
    let receipt_digest =
        write_receipt(state, naming, outcome, cause.as_deref(), started, &ctx).await;
    respond(
        status,
        body_out,
        naming,
        label,
        receipt_digest.as_deref(),
        settlement,
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
    let (registry, caller) =
        authenticate(state, headers).map_err(|(status, code, message)| Verdict::Refused {
            status,
            code,
            message,
            outcome: Outcome::Refused,
            ctx: Context::default(),
        })?;
    let ctx = Box::new(ReceiptContext {
        tenant_ref: caller.tenant.as_ref().map(|_| caller.key.clone()),
        ..ReceiptContext::default()
    });
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
    windowed(state, door, capacity, ctx).await?;
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

/// The door's declared rate window, counted once per call and refusing
/// when the window is full — before any reservation exists, so a
/// congestion refusal never holds quota. This is also where the door's
/// bounds entry is created or rebuilt from the binding's declared
/// capacity, which the classify scheduler's per-item forwards acquire
/// from afterwards.
async fn windowed(
    state: &ServeState,
    door: &str,
    capacity: &Capacity,
    ctx: &Context,
) -> Result<(), Verdict> {
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
    Ok(())
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
/// releases the reservation rather than charging it; a monetary hold is
/// released the same way, because work that never dispatched is the one
/// release the ledger accepts without further evidence.
async fn verified(
    state: &ServeState,
    endpoint: &str,
    admission: &Admission,
    naming: &Naming<'_>,
    ctx: &mut Context,
    hold: &Option<money::Hold>,
) -> Result<(), Verdict> {
    let published =
        match published_identity(state, endpoint, &admission.binding.artifact.model).await {
            Ok(published) => published,
            Err(message) => {
                state.release(naming.request, naming.attempt).await;
                ctx.settlement = money_release(state, hold).await;
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
        ctx.settlement = money_release(state, hold).await;
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

/// Step 4b of every route, only under monetary admission: the durable
/// worst-case spend reservation against the caller's workspace, taken
/// before any backend dispatch. A refusal here releases the call's
/// quota reservation — the hold was fresh, because a replayed
/// reservation returns standing rather than refusing.
async fn money_hold(
    state: &ServeState,
    caller: &Caller,
    door: &str,
    admission: &Admission,
    naming: &Naming<'_>,
    ctx: &Context,
) -> Result<Option<money::Hold>, Verdict> {
    let Some(config) = &state.config.money else {
        return Ok(None);
    };
    let Some(workspace) = caller.workspace.clone() else {
        return Err(Verdict::Refused {
            status: StatusCode::BAD_REQUEST,
            code: "workspace_required",
            message: "monetary admission requires an authenticated workspace".to_string(),
            outcome: Outcome::Refused,
            ctx: ctx.clone(),
        });
    };
    let Some(priced) = config.doors.get(door) else {
        state.release(naming.request, naming.attempt).await;
        return Err(Verdict::Refused {
            status: StatusCode::SERVICE_UNAVAILABLE,
            code: "unpriced",
            message: format!(
                "door `{door}` has no configured price — the gateway does not \
                 invent one to keep it serving"
            ),
            outcome: Outcome::Refused,
            ctx: ctx.clone(),
        });
    };
    let ledger = state.money.as_ref().expect("money config opens a ledger");
    let mut ledger = ledger.lock().await;
    match money::reserve(
        &mut ledger,
        &workspace,
        naming.request,
        naming.attempt,
        naming.request_digest,
        priced,
        (
            &admission.binding.artifact.model,
            lane_name(admission.binding.lane),
        ),
    ) {
        Ok(hold) => Ok(Some(hold)),
        Err(refusal) => {
            drop(ledger);
            // A duplicate belongs to the original execution. Preserve its
            // quota reservation; other refusals release the fresh reservation.
            if !matches!(refusal, money::Refusal::Duplicate) {
                state.release(naming.request, naming.attempt).await;
            }
            let (status, code) = match &refusal {
                money::Refusal::Duplicate => (StatusCode::CONFLICT, "idempotency_conflict"),
                money::Refusal::Funds(_) => (StatusCode::PAYMENT_REQUIRED, "insufficient_funds"),
                money::Refusal::Price(_) => (StatusCode::SERVICE_UNAVAILABLE, "price_invalid"),
                money::Refusal::Ledger(_) => {
                    (StatusCode::INTERNAL_SERVER_ERROR, "ledger_unavailable")
                }
            };
            Err(Verdict::Refused {
                status,
                code,
                message: refusal.to_string(),
                outcome: Outcome::Refused,
                ctx: ctx.clone(),
            })
        }
    }
}

/// Release a monetary hold whose work was never dispatched.
async fn money_release(state: &ServeState, hold: &Option<money::Hold>) -> Option<&'static str> {
    if let (Some(ledger), Some(hold)) = (&state.money, hold) {
        let mut ledger = ledger.lock().await;
        Some(money::release(&mut ledger, hold).label())
    } else {
        None
    }
}

/// Resolve the attempt's monetary hold from its outcome and observed
/// usage — settling only what the response prices, and leaving the full
/// reservation outstanding otherwise.
async fn money_settle(
    state: &ServeState,
    hold: &Option<money::Hold>,
    naming: &Naming<'_>,
    usage: Option<tenancy::money::Usage>,
) -> Option<&'static str> {
    let (Some(ledger), Some(hold)) = (&state.money, hold) else {
        return None;
    };
    let mut ledger = ledger.lock().await;
    let receipt = format!("{}#{}", naming.request, naming.attempt);
    Some(money::settle(&mut ledger, hold, usage, &receipt).label())
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

async fn cancelled_before_dispatch(
    state: &ServeState,
    naming: &Naming<'_>,
    ctx: &mut Context,
    hold: &Option<money::Hold>,
) -> Verdict {
    state.release(naming.request, naming.attempt).await;
    ctx.settlement = money_release(state, hold).await;
    Verdict::Refused {
        status: StatusCode::SERVICE_UNAVAILABLE,
        code: "cancelled",
        message: "caller disconnected before inference dispatch".into(),
        outcome: Outcome::Unattempted,
        ctx: ctx.clone(),
    }
}

async fn verified_cancellable(
    state: &ServeState,
    endpoint: &str,
    admission: &Admission,
    naming: &Naming<'_>,
    ctx: &mut Context,
    hold: &Option<money::Hold>,
    cancellation: &Cancellation,
) -> Result<(), Verdict> {
    tokio::select! {
        biased;
        _ = cancellation.wait() => Err(cancelled_before_dispatch(state, naming, ctx, hold).await),
        result = verified(state, endpoint, admission, naming, ctx, hold) => result,
    }
}

async fn forward_cancellable(
    state: &ServeState,
    endpoint: &str,
    body: &Bytes,
    cancellation: &Cancellation,
) -> Forwarded {
    tokio::select! {
        biased;
        _ = cancellation.wait() => Forwarded::Unavailable {
            message: "caller disconnected after dispatch; completion is unknown".into(),
        },
        result = forward(state, endpoint, body) => result,
    }
}

/// The request's full passage through admission — one function so the
/// steps read in the order they run.
async fn admitted(
    state: &ServeState,
    headers: &HeaderMap,
    envelope: &Value,
    body: &Bytes,
    naming: &Naming<'_>,
    cancellation: &Cancellation,
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
    let hold = match money_hold(state, &caller, door, &admission, naming, &ctx).await {
        Ok(hold) => hold,
        Err(verdict) => return verdict,
    };
    if let Err(verdict) = verified_cancellable(
        state,
        &endpoint,
        &admission,
        naming,
        &mut ctx,
        &hold,
        cancellation,
    )
    .await
    {
        return verdict;
    }

    // 6. Forward, then settle from the recorded outcome.
    if cancellation.stopped() {
        return cancelled_before_dispatch(state, naming, &mut ctx, &hold).await;
    }
    let (status, outcome, body_out, cause) =
        match forward_cancellable(state, &endpoint, body, cancellation).await {
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
    if hold.is_some() {
        // A dispatched attempt settles the usage the door reported;
        // anything unpriceable — a missing, partial, or non-count
        // report — leaves the whole hold outstanding, never zero.
        let usage = serde_json::from_slice::<Value>(&body_out)
            .ok()
            .and_then(|body| money::observed(&hold.as_ref().unwrap().price, &body));
        ctx.settlement = money_settle(state, &hold, naming, usage).await;
    }
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
    state: &Arc<ServeState>,
    headers: &HeaderMap,
    body: &Bytes,
    naming: &Naming<'_>,
    cancellation: &Cancellation,
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
    let (admission, backend) = match authorized(state, &registry, &caller, &request.model, &mut ctx)
    {
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
    // Check the complete expanded context before queueing or reserving usage.
    // Per-field bounds alone cannot bound repeated question text or JSON framing.
    for input in &request.inputs {
        let (body, _) = forward_body(&request, &plan, input, &admission.binding.artifact.model);
        if body.len() as u64 > limits.max_forward_bytes {
            return Verdict::Refused {
                status: StatusCode::UNPROCESSABLE_ENTITY,
                code: "context_limit",
                message: format!(
                    "input `{}` exceeds the door's native request byte limit of {}",
                    input.id, limits.max_forward_bytes
                ),
                outcome: Outcome::Refused,
                ctx,
            };
        }
    }
    let endpoint = backend.endpoint.clone();
    let _queued = match classify_queue(state, &caller, plan.inputs as u32, &ctx).await {
        Ok(permits) => permits,
        Err(verdict) => return verdict,
    };

    // 3–5. Bound, reserve, verify — the reservation's units are the
    // plan's: one question per judgment, the readout width of every
    // categorical set and rubric, and the envelope's own bytes. The
    // call holds no forward permits of its own: each item's forward
    // acquires the door's declared slot and the process's bound for
    // itself, so an in-flight forward is always covered work.
    let capacity = admission.binding.capacity.clone().unwrap_or_default();
    if let Err(verdict) = windowed(state, &request.model, &capacity, &ctx).await {
        return verdict;
    }
    let options: u64 = plan
        .units
        .iter()
        .map(|unit| match unit.mode {
            Mode::SingleLabel => plan.inputs * unit.labels.len() as u64,
            Mode::Score => plan.inputs * unit.levels.len() as u64,
            Mode::MultiLabel | Mode::Binary => 0,
        })
        .sum();
    let units = quota::Units {
        questions: plan.judgments,
        input_bytes: body.len() as u64,
        options,
    };
    if let Err(verdict) = reserved(state, &registry, &caller, naming, &units, &mut ctx).await {
        return verdict;
    }
    let hold = match money_hold(state, &caller, &request.model, &admission, naming, &ctx).await {
        Ok(hold) => hold,
        Err(verdict) => return verdict,
    };
    if let Err(verdict) = verified_cancellable(
        state,
        &endpoint,
        &admission,
        naming,
        &mut ctx,
        &hold,
        cancellation,
    )
    .await
    {
        return verdict;
    }

    // 6. Fan out — one `systemone` forward per input, reconstructed in
    // request order however the forwards complete. The call's fan-out
    // bound is the door's configured item concurrency — one unless the
    // operator declared more — never above the binding's declared
    // concurrency or the process's forward bound, so a configured bound
    // cannot multiply capacity the deployment did not declare.
    let artifact = admission.binding.artifact.model.clone();
    let plan = Arc::new(plan);
    let item_bound = backend
        .classify_item_concurrency
        .min(capacity.concurrency.unwrap_or(u64::MAX))
        .min(state.config.max_in_flight as u64)
        .min(plan.inputs)
        .max(1) as usize;
    let slots = Arc::new(Semaphore::new(item_bound));
    let halt = Arc::new(AtomicBool::new(false));
    let deadline = started + Duration::from_millis(state.config.forward_timeout_ms);
    let mut scheduled = tokio::task::JoinSet::new();
    for (index, input) in request.inputs.iter().enumerate() {
        let (body, asked) = forward_body(&request, &plan, input, &artifact);
        scheduled.spawn(classify_item(ItemWork {
            index,
            input: input.id.clone(),
            body,
            asked,
            state: state.clone(),
            door: request.model.clone(),
            endpoint: endpoint.clone(),
            model: artifact.clone(),
            plan: plan.clone(),
            slots: slots.clone(),
            halt: halt.clone(),
            deadline,
            attempt_id: format!("{}:{index}", naming.attempt_id),
            cancellation: cancellation.clone(),
        }));
    }
    let mut done: Vec<Option<ItemResult>> = (0..request.inputs.len()).map(|_| None).collect();
    while let Some(joined) = scheduled.join_next().await {
        if let Ok(result) = joined {
            let slot = &mut done[result.index];
            *slot = Some(result);
        }
    }

    // Reassemble in input order and aggregate the per-unit outcomes
    // into the call's own. An item whose task never reported is counted
    // as dispatched-unavailable — potentially attempted work never
    // reads as unattempted or answered.
    let mut items = Vec::with_capacity(done.len());
    let mut primaries = Vec::with_capacity(done.len());
    let mut forwards = 0_u64;
    let mut input_tokens = CompleteCounter::default();
    let mut output_tokens = CompleteCounter::default();
    // Every dispatched item's own usage report — the settlement reads.
    let mut dispatched_reports: Vec<Option<Value>> = Vec::new();
    for (index, slot) in done.into_iter().enumerate() {
        let result = slot.unwrap_or_else(|| ItemResult {
            index,
            dispatched: true,
            item: failed_item(
                &request.inputs[index].id,
                &plan,
                "unavailable",
                "the input's forward never reported",
                None,
            ),
            usage: None,
            attempt_id: format!("{}:{index}", naming.attempt_id),
        });
        if result.dispatched {
            forwards += 1;
            input_tokens.add(
                result
                    .usage
                    .as_ref()
                    .and_then(|u| u.get("input_tokens"))
                    .and_then(Value::as_u64),
            );
            output_tokens.add(
                result
                    .usage
                    .as_ref()
                    .and_then(|u| u.get("output_tokens"))
                    .and_then(Value::as_u64),
            );
            dispatched_reports.push(result.usage.clone());
        }
        primaries.push(PrimaryRecord {
            attempt_id: result.attempt_id.clone(),
            outcome: result.item["outcome"]
                .as_str()
                .unwrap_or("unavailable")
                .to_string(),
            cause: result
                .item
                .get("cause")
                .and_then(Value::as_str)
                .map(str::to_string),
            latency_ms: result.item.get("latency_ms").and_then(Value::as_u64),
        });
        items.push(result.item);
    }

    // A declared review policy runs its bounded phase over the
    // assembled primaries: declared-cause fallbacks first, then the
    // triggered re-judgments — every dispatch through the full
    // admission path under its own recorded identity.
    let mut book = ReviewBook::default();
    if let Some(review) = &plan.policy.review {
        book = review_phase(
            &PhaseContext {
                state,
                registry: &registry,
                caller: &caller,
                headers,
                request: &request,
                plan: &plan,
                artifact: &artifact,
                naming,
                cancellation,
                call_deadline: deadline,
            },
            review,
            &mut items,
            &primaries,
        )
        .await;
    }

    // The outcome tally reads the items' final state — a strict review
    // that did not answer is counted under its own outcome.
    let mut counts = Counts::default();
    for item in &items {
        for unit in item["units"].as_array().into_iter().flatten() {
            match unit.get("outcome").and_then(Value::as_str) {
                Some("answered") => counts.answered += 1,
                Some("refused") => counts.refused += 1,
                Some("unavailable") => counts.unavailable += 1,
                _ => counts.unattempted += 1,
            }
        }
    }

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
    } else if forwards == 0 {
        (
            "unattempted",
            StatusCode::SERVICE_UNAVAILABLE,
            Outcome::Unattempted,
            Some("deadline".to_string()),
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
    let mut usage = json!({"forwards": forwards, "input_tokens_complete": input_tokens.total().is_some(), "output_tokens_complete": output_tokens.total().is_some()});
    if let Some(tokens) = input_tokens.total() {
        usage["input_tokens"] = json!(tokens);
    }
    if let Some(tokens) = output_tokens.total() {
        usage["output_tokens"] = json!(tokens);
    }
    // The corpus views the binary and score units produce — built from
    // the assembled items so a failed input is named rather than
    // silently absent — and the per-unit tallies every unit reports.
    let selections = corpus_selections(&plan, &items);
    let aggregates = corpus_aggregates(&plan, &items);
    let mut response = json!({
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
        "selections": selections,
        "aggregates": aggregates,
        "usage": usage,
        "timing": {"latency_ms": started.elapsed().as_millis() as u64},
    });
    // A declared review policy reports its own accounting: the digested
    // policy document, the bound identities it dispatched under, and
    // the usage its dispatches carried separately from the primary's.
    if let Some(review) = &plan.policy.review {
        usage["review"] = json!({
            "forwards": book.review_forwards,
            "input_tokens_complete": book.review_input.total().is_some(),
            "output_tokens_complete": book.review_output.total().is_some(),
        });
        if let Some(tokens) = book.review_input.total() {
            usage["review"]["input_tokens"] = json!(tokens);
        }
        if let Some(tokens) = book.review_output.total() {
            usage["review"]["output_tokens"] = json!(tokens);
        }
        usage["fallback"] = json!({
            "forwards": book.fallback_dispatched,
            "input_tokens_complete": book.fallback_input.total().is_some(),
            "output_tokens_complete": book.fallback_output.total().is_some(),
        });
        if let Some(tokens) = book.fallback_input.total() {
            usage["fallback"]["input_tokens"] = json!(tokens);
        }
        if let Some(tokens) = book.fallback_output.total() {
            usage["fallback"]["output_tokens"] = json!(tokens);
        }
        response["usage"] = usage;
        let mut summary = json!({
            "v": classify::REVIEW_SCHEMA,
            "policy_digest": digest_request(
                &serde_json::to_value(review).unwrap_or_default()
            ),
            "reviewer": review.reviewer,
            "trigger": match review.trigger {
                classify::ReviewTrigger::Uncertain => "uncertain",
                classify::ReviewTrigger::NoMatch => "no-match",
                classify::ReviewTrigger::Always => "always",
            },
            "on_failure": match review.on_failure {
                classify::ReviewFailure::KeepOriginal => "keep-original",
                classify::ReviewFailure::Strict => "strict",
            },
            "bounds": {
                "max_items": review.max_items,
                "max_attempts": review.max_attempts,
                "latency_ms": review.latency_ms,
                "max_spend": review.max_spend,
            },
            "attempts": book.attempts,
            "reviewed": book.reviewed,
            "review_answered": book.review_answered,
            "fallback_dispatched": book.fallback_dispatched,
            "fallback_answered": book.fallback_answered,
        });
        if state.config.money.is_some() {
            summary["reserved_spend"] = json!(book.spend);
        }
        response["review"] = summary;
    }
    let body_out = Bytes::from(serde_json::to_vec(&response).unwrap_or_default());
    ctx.result_digest = Some(digest_bytes(&body_out));
    let attempted = quota::Units {
        questions: (plan.judgments / plan.inputs) * forwards,
        options: (options / plan.inputs) * forwards,
        input_bytes: units.input_bytes,
    };
    settled(state, naming, outcome, &attempted).await;
    if let Some(held) = &hold {
        ctx.settlement = if forwards == 0 {
            // Nothing dispatched — the one release the ledger accepts
            // without further evidence.
            money_release(state, &hold).await
        } else {
            // Every dispatched item must report every priced resource;
            // one silent item leaves the whole hold outstanding.
            let reports: Vec<Option<&Value>> =
                dispatched_reports.iter().map(Option::as_ref).collect();
            let usage = money::observed_total(&held.price, &reports);
            money_settle(state, &hold, naming, usage).await
        };
    }
    Verdict::Forwarded {
        status,
        body: body_out,
        outcome,
        label,
        cause,
        ctx,
    }
}

/// Reserve the entire atomic input set before quota or task creation. These
/// permits cover waiting and running inputs until the request completes.
async fn classify_queue(
    state: &ServeState,
    caller: &Caller,
    inputs: u32,
    ctx: &Context,
) -> Result<(OwnedSemaphorePermit, OwnedSemaphorePermit), Verdict> {
    let refuse = || Verdict::Refused {
        status: StatusCode::TOO_MANY_REQUESTS,
        code: "classification_queue_full",
        message: "the global or tenant classification input allowance cannot fit this request"
            .into(),
        outcome: Outcome::Refused,
        ctx: ctx.clone(),
    };
    let global = state
        .classify_inputs
        .clone()
        .try_acquire_many_owned(inputs)
        .map_err(|_| refuse())?;
    let mut tenants = state.tenant_classify_inputs.lock().await;
    // An owned permit retains its semaphore. Remove entries with no active
    // reservations so departed tenants do not accumulate process state.
    tenants.retain(|_, pool| Arc::strong_count(pool) > 1);
    let pool = tenants.entry(caller.tenant.clone()).or_insert_with(|| {
        Arc::new(Semaphore::new(
            state.config.max_classify_inputs_per_tenant as usize,
        ))
    });
    let tenant = pool
        .clone()
        .try_acquire_many_owned(inputs)
        .map_err(|_| refuse())?;
    Ok((global, tenant))
}

/// What one input's scheduled forward needs: its position and id, its
/// own question body, and every bound it runs under.
struct ItemWork {
    /// The input's position — results reassemble in request order.
    index: usize,
    /// The input's caller-chosen id.
    input: String,
    /// The serialized `systemone` envelope for this input alone.
    body: Bytes,
    /// The question ids the body asked, mapped back to (unit, label).
    asked: Asked,
    state: Arc<ServeState>,
    /// The door's name, for its declared concurrency pool.
    door: String,
    endpoint: String,
    /// The artifact id the answer must claim.
    model: String,
    plan: Arc<classify::Plan>,
    /// The call's own fan-out bound — closed when the call halts.
    slots: Arc<Semaphore>,
    /// Set when the door stops answering: queued items stop waiting.
    halt: Arc<AtomicBool>,
    /// The call's execution deadline — queue waits and forwards share it.
    deadline: Instant,
    /// This forward's recorded identity within the call's attempt —
    /// the attempt chain's name for it when a review policy runs.
    attempt_id: String,
    cancellation: Cancellation,
}

/// What one scheduled input produced.
struct ItemResult {
    /// The input's position.
    index: usize,
    /// Whether the forward was dispatched — dispatched work is charged.
    dispatched: bool,
    /// The assembled per-input result.
    item: Value,
    /// The door's own usage report, when it sent one.
    usage: Option<Value>,
    /// The forward's recorded identity within the call's attempt.
    attempt_id: String,
}

/// One input's scheduled forward: take the call's fan-out slot, then
/// wait — inside the call's deadline — for the door's declared slot and
/// the process's forward slot, so an in-flight forward always holds the
/// capacity that covers it. An item the call never dispatched reports
/// `unattempted` and spends nothing; a door that stops answering halts
/// the call rather than letting queued work pretend it ran.
async fn classify_item(work: ItemWork) -> ItemResult {
    let cancellation = work.cancellation.clone();
    let dispatched = Arc::new(AtomicBool::new(false));
    let index = work.index;
    let input = work.input.clone();
    let plan = work.plan.clone();
    let attempt_id = work.attempt_id.clone();
    tokio::select! {
        biased;
        _ = cancellation.wait() => {
            let attempted = dispatched.load(Ordering::SeqCst);
            ItemResult {
                index,
                dispatched: attempted,
                item: failed_item(&input, &plan,
                    if attempted { "unavailable" } else { "unattempted" },
                    if attempted { "caller disconnected after dispatch; completion is unknown" }
                    else { "caller disconnected before dispatch" }, None),
                usage: None,
                attempt_id,
            }
        }
        result = classify_item_running(work, dispatched.clone()) => result,
    }
}

async fn classify_item_running(work: ItemWork, dispatched: Arc<AtomicBool>) -> ItemResult {
    let ItemWork {
        index,
        input,
        body,
        asked,
        state,
        door,
        endpoint,
        model,
        plan,
        slots,
        halt,
        deadline,
        attempt_id,
        cancellation: _,
    } = work;
    let unattempted = |cause: &'static str| ItemResult {
        index,
        dispatched: false,
        item: unattempted_item(&input, &plan, cause),
        usage: None,
        attempt_id: attempt_id.clone(),
    };
    // The call's own fan-out bound. The pool closes on halt, which is
    // how a stopped call frees its queue instead of waiting it out.
    let cutoff = tokio::time::Instant::from_std(deadline);
    let Ok(Ok(_item)) = tokio::time::timeout_at(cutoff, slots.clone().acquire_owned()).await else {
        return unattempted("the call stopped before this input's forward");
    };
    let Some(_) = deadline.checked_duration_since(Instant::now()) else {
        return unattempted("the call's execution deadline passed before its forward ran");
    };
    if halt.load(Ordering::SeqCst) {
        return unattempted("the call stopped before this input's forward");
    }
    // The binding's declared concurrency, when it names one — waited
    // on, never borrowed: a busy door's items queue inside the deadline.
    let _door = match door_slots(&state, &door).await {
        Some(pool) => match tokio::time::timeout_at(cutoff, pool.acquire_owned()).await {
            Ok(Ok(permit)) => Some(permit),
            _ => {
                return unattempted(
                    "the call's execution deadline passed while the door's slots were full",
                );
            }
        },
        None => None,
    };
    let _host = match tokio::time::timeout_at(cutoff, state.in_flight.clone().acquire_owned()).await
    {
        Ok(Ok(permit)) => permit,
        _ => {
            return unattempted(
                "the call's execution deadline passed while the gateway's slots were full",
            );
        }
    };
    if halt.load(Ordering::SeqCst) {
        return unattempted("the call stopped before this input's forward");
    }
    let item_started = Instant::now();
    if Instant::now() >= deadline {
        return unattempted("the call's execution deadline passed before dispatch");
    }
    dispatched.store(true, Ordering::SeqCst);
    let forwarded = tokio::time::timeout_at(cutoff, forward(&state, &endpoint, &body))
        .await
        .unwrap_or_else(|_| Forwarded::Unavailable {
            message: "the classification call exceeded its execution deadline".to_string(),
        });
    let latency = item_started.elapsed();
    let (item, usage) = match forwarded {
        Forwarded::Served { body, .. } => {
            served_item(&input, &plan, &asked, &model, &body, latency)
        }
        Forwarded::Refused { cause, .. } => (
            failed_item(&input, &plan, "refused", &cause, Some(latency)),
            None,
        ),
        Forwarded::Unavailable { message } => {
            // A door that stops answering halts the call: queued inputs
            // report unattempted — dispatched, never invented.
            halt.store(true, Ordering::SeqCst);
            slots.close();
            (
                failed_item(&input, &plan, "unavailable", &message, Some(latency)),
                None,
            )
        }
    };
    ItemResult {
        index,
        dispatched: true,
        item,
        usage,
        attempt_id,
    }
}

/// The door's declared concurrency pool — `None` when the binding
/// declares no bound and the configured item concurrency is the door's
/// only declaration.
async fn door_slots(state: &ServeState, door: &str) -> Option<Arc<Semaphore>> {
    state
        .doors
        .lock()
        .await
        .get(door)
        .and_then(|bounds| bounds.slots.clone())
}

/// A review or fallback dispatch's identities and work: a secondary
/// call under the caller's own credentials, admitted end to end like
/// the primary's — never a bypass around authorization, bounds, quota,
/// monetary admission, identity, or the receipt.
struct SubCall {
    /// The artifact authorized when the parent call froze its policy context.
    expected: Option<tenancy::Expected>,
    /// The door this dispatch names — a bound door, authorized fresh.
    door: String,
    /// The reservation and hold request id: `{request}:{role}:{seq}` —
    /// a new logical request under the caller's pair, never the
    /// caller's own key.
    request: String,
    /// The caller's claimed attempt — the pair stays unique on
    /// `request`.
    attempt: u32,
    /// This dispatch's own recorded attempt identity.
    attempt_id: String,
    /// The sub-envelope's canonical digest.
    request_digest: String,
    /// The serialized sub-envelope.
    body: Bytes,
    /// The reservation's unit cost.
    units: quota::Units,
    /// The dispatch's deadline — the phase's, inside the call's own.
    deadline: Instant,
    cancellation: Cancellation,
}

/// Why a sub-dispatch could not dispatch — the admission step's own
/// refusal, recorded rather than collapsed into a generic failure.
struct Fail {
    /// The step's recorded outcome: refused for a declined admission,
    /// unattempted for work the deadline or identity check stopped.
    outcome: Outcome,
    /// The refusal's typed code — what a `refused` fallback's `codes`
    /// would match.
    code: String,
    /// The step's own message, kept for the record.
    message: String,
}

impl Fail {
    /// A dispatch the deadline stopped before it held anything.
    fn unattempted(message: &str) -> Self {
        Self {
            outcome: Outcome::Unattempted,
            code: "unattempted".to_string(),
            message: message.to_string(),
        }
    }
}

/// Fold an admission step's refusal verdict into the dispatch's own
/// failure record — the code and message the step produced, kept.
fn fail(verdict: Verdict) -> Fail {
    match verdict {
        Verdict::Refused {
            code,
            message,
            outcome,
            ..
        } => Fail {
            outcome,
            code: code.to_string(),
            message,
        },
        Verdict::Forwarded { .. } => Fail {
            outcome: Outcome::Unavailable,
            code: "unavailable".to_string(),
            message: "the dispatch produced no typed refusal".to_string(),
        },
    }
}

/// What a sub-dispatch produced — dispatched or not, everything the
/// review or fallback record reports.
struct DispatchOutcome {
    /// The dispatch's recorded attempt identity.
    attempt_id: String,
    /// The reservation reference it settled against, when it held one.
    usage_ref: Option<String>,
    /// The sealed receipt reference, absent when the receipt write failed.
    receipt: Option<String>,
    /// The receipt-level outcome.
    outcome: Outcome,
    /// The recorded cause: the refusal's typed code, or the failure's
    /// message.
    cause: Option<String>,
    /// The typed code the refusal carried, when one did — what a
    /// `refused` fallback's `codes` matches.
    code: Option<String>,
    /// Whether a forward reached the backend — dispatched work is
    /// charged work.
    dispatched: bool,
    /// The response body, when the door answered.
    body: Option<Bytes>,
    /// The response's own model claim.
    model: Option<String>,
    /// The response's usage report.
    usage: Option<Value>,
    /// The identity the backend published, when it was reached.
    served: Served,
    /// How long the dispatch took, admission through answer.
    latency: Duration,
    /// How the dispatch's monetary hold resolved, when it held one.
    settlement: Option<&'static str>,
}

/// One review or fallback dispatch through the full admission path:
/// authorize the door under the caller's credentials, wait inside the
/// deadline for the door's and process's slots, reserve quota and
/// monetary spend under its own request identity, verify the backend's
/// published card against the binding, forward, settle, and leave a
/// sealed receipt — every secondary call audited like the primary's.
async fn dispatch(state: &ServeState, headers: &HeaderMap, sub: &SubCall) -> DispatchOutcome {
    let started = Instant::now();
    let mut ctx: Context = Box::default();
    let naming = Naming {
        request: &sub.request,
        attempt: sub.attempt,
        attempt_id: sub.attempt_id.clone(),
        request_digest: &sub.request_digest,
    };
    let result = match authenticated(state, headers) {
        Ok((registry, caller, fresh)) => {
            ctx = fresh;
            dispatch_admitted(state, &registry, &caller, sub, &naming, &mut ctx).await
        }
        Err(verdict) => Err(fail(verdict)),
    };
    let (outcome, cause, code) = match &result {
        Ok(Forwarded::Served { .. }) => (Outcome::Answered, None, None),
        Ok(Forwarded::Refused { cause, .. }) => {
            (Outcome::Refused, Some(cause.clone()), Some(cause.clone()))
        }
        Ok(Forwarded::Unavailable { message }) => {
            (Outcome::Unavailable, Some(message.clone()), None)
        }
        Err(failed) => (
            failed.outcome,
            Some(failed.code.clone()),
            Some(failed.code.clone()),
        ),
    };
    let dispatched = result.is_ok();
    let (body, model, usage) = match &result {
        Ok(Forwarded::Served { body, .. } | Forwarded::Refused { body, .. }) => {
            let parsed = serde_json::from_slice::<Value>(body).ok();
            (
                Some(body.clone()),
                parsed
                    .as_ref()
                    .and_then(|body| body.get("model"))
                    .and_then(Value::as_str)
                    .map(str::to_string),
                parsed.as_ref().and_then(|body| body.get("usage")).cloned(),
            )
        }
        _ => (None, None, None),
    };
    ctx.result_digest = body.as_ref().map(|body| digest_bytes(body));
    let receipt = write_receipt(state, &naming, outcome, cause.as_deref(), started, &ctx).await;
    DispatchOutcome {
        receipt,
        attempt_id: sub.attempt_id.clone(),
        usage_ref: ctx.usage.clone(),
        outcome,
        cause: match result {
            Err(failed) => Some(failed.message.clone()),
            _ => cause,
        },
        code,
        dispatched,
        body,
        model,
        usage,
        served: ctx.served.clone(),
        latency: started.elapsed(),
        settlement: ctx.settlement,
    }
}

/// The sub-dispatch's passage through admission — the same steps the
/// primary route runs, in the same order, against the dispatch's own
/// reservation identity.
async fn dispatch_admitted(
    state: &ServeState,
    registry: &Registry,
    caller: &Caller,
    sub: &SubCall,
    naming: &Naming<'_>,
    ctx: &mut Context,
) -> Result<Forwarded, Fail> {
    // Authorize the named door — a reviewer or fallback the caller's
    // bindings do not name is refused here, never dispatched.
    let (admission, backend) = authorized(state, registry, caller, &sub.door, ctx).map_err(fail)?;
    if let Some(limits) = backend.classify {
        limits.check().map_err(|error| Fail {
            outcome: Outcome::Refused,
            code: error.code().to_string(),
            message: error.to_string(),
        })?;
        if sub.body.len() as u64 > limits.max_forward_bytes {
            return Err(Fail {
                outcome: Outcome::Refused,
                code: "context_limit".to_string(),
                message: "the secondary request exceeds the door's native request byte limit"
                    .to_string(),
            });
        }
    }
    if sub.expected.as_ref() != Some(&admission.binding.artifact) {
        return Err(Fail {
            outcome: Outcome::Unattempted,
            code: "identity_mismatch".to_string(),
            message: "the secondary door's artifact changed after the parent call was admitted"
                .to_string(),
        });
    }
    if Instant::now() >= sub.deadline {
        return Err(Fail::unattempted(
            "the secondary dispatch deadline passed during admission",
        ));
    }
    let capacity = admission.binding.capacity.clone().unwrap_or_default();
    windowed(state, &sub.door, &capacity, ctx)
        .await
        .map_err(fail)?;
    let cutoff = tokio::time::Instant::from_std(sub.deadline);
    // The door's declared concurrency and the process's forward bound,
    // waited on inside the phase deadline like the primary fan-out's.
    let _door = match door_slots(state, &sub.door).await {
        Some(pool) => match tokio::time::timeout_at(cutoff, pool.acquire_owned()).await {
            Ok(Ok(permit)) => Some(permit),
            _ => {
                return Err(Fail::unattempted(
                    "the phase's deadline passed while the door's slots were full",
                ));
            }
        },
        None => None,
    };
    let _host = match tokio::time::timeout_at(cutoff, state.in_flight.clone().acquire_owned()).await
    {
        Ok(Ok(permit)) => permit,
        _ => {
            return Err(Fail::unattempted(
                "the phase's deadline passed while the gateway's slots were full",
            ));
        }
    };
    reserved(state, registry, caller, naming, &sub.units, ctx)
        .await
        .map_err(fail)?;
    let hold = money_hold(state, caller, &sub.door, &admission, naming, ctx)
        .await
        .map_err(fail)?;
    match tokio::time::timeout_at(
        cutoff,
        verified_cancellable(
            state,
            &backend.endpoint,
            &admission,
            naming,
            ctx,
            &hold,
            &sub.cancellation,
        ),
    )
    .await
    {
        Ok(result) => result.map_err(fail)?,
        Err(_) => {
            // Identity reads cannot execute inference. Both reservations can
            // therefore be released when this pre-dispatch deadline expires.
            state.release(naming.request, naming.attempt).await;
            ctx.settlement = money_release(state, &hold).await;
            return Err(Fail::unattempted(
                "the secondary dispatch deadline passed during identity verification",
            ));
        }
    }
    if sub.cancellation.stopped() {
        return Err(fail(
            cancelled_before_dispatch(state, naming, ctx, &hold).await,
        ));
    }
    if Instant::now() >= sub.deadline {
        state.release(naming.request, naming.attempt).await;
        ctx.settlement = money_release(state, &hold).await;
        return Err(Fail::unattempted(
            "the secondary dispatch deadline passed before forwarding",
        ));
    }
    let forwarded = tokio::time::timeout_at(
        cutoff,
        forward_cancellable(state, &backend.endpoint, &sub.body, &sub.cancellation),
    )
    .await
    .unwrap_or_else(|_| Forwarded::Unavailable {
        message: "the dispatch exceeded the review phase's deadline".to_string(),
    });
    let outcome = match &forwarded {
        Forwarded::Served { .. } => Outcome::Answered,
        Forwarded::Refused { .. } => Outcome::Refused,
        Forwarded::Unavailable { .. } => Outcome::Unavailable,
    };
    settled(state, naming, outcome, &sub.units).await;
    if let Some(held) = &hold {
        // A dispatched secondary attempt settles the usage its own door
        // reported — anything unpriceable stays outstanding, never zero.
        let usage = match &forwarded {
            Forwarded::Served { body, .. } | Forwarded::Refused { body, .. } => {
                serde_json::from_slice::<Value>(body)
                    .ok()
                    .and_then(|body| money::observed(&held.price, &body))
            }
            Forwarded::Unavailable { .. } => None,
        };
        ctx.settlement = money_settle(state, &hold, naming, usage).await;
    }
    Ok(forwarded)
}

/// The primary forward's own record in an item's attempt chain.
struct PrimaryRecord {
    /// The item's recorded attempt identity within the call.
    attempt_id: String,
    /// The item's outcome before review or fallback ran.
    outcome: String,
    /// The failure's cause, when the item failed.
    cause: Option<String>,
    /// The forward's latency, when it dispatched.
    latency_ms: Option<u64>,
}

/// What the review phase accounted for — the response's review summary
/// and usage reads draw from it.
#[derive(Default)]
struct ReviewBook {
    /// Dispatches spent against the policy's `max_attempts`.
    attempts: u64,
    /// Units dispatched to the reviewer against `max_items`.
    reviewed: u64,
    /// Review dispatches that answered.
    review_answered: u64,
    /// Fallback dispatches that reached a backend.
    fallback_dispatched: u64,
    /// Fallback dispatches that answered.
    fallback_answered: u64,
    /// The worst-case spend the phase's holds reserved, in millionths —
    /// zero when monetary admission is not configured.
    spend: u64,
    /// Review dispatches that reached a backend.
    review_forwards: u64,
    /// Review usage accounting, complete only when every dispatched
    /// review reported the counter.
    review_input: CompleteCounter,
    /// The review forwards' output tokens.
    review_output: CompleteCounter,
    /// Fallback usage accounting.
    fallback_input: CompleteCounter,
    /// The fallback forwards' output tokens.
    fallback_output: CompleteCounter,
    /// The dispatch sequence — every secondary call's identity suffix.
    seq: u64,
}

/// The worst-case reservation a dispatch to `door` would take under
/// monetary admission — `None` when money is off or the door is
/// unpriced (the dispatch itself then refuses `unpriced`).
fn spend_quote(state: &ServeState, door: &str) -> Option<u64> {
    state
        .config
        .money
        .as_ref()?
        .doors
        .get(door)
        .and_then(|priced| priced.price.quote(&priced.maximum_usage).ok())
}

/// Why the phase may not spend another dispatch — the cause the
/// record reports. `None` means the bounds admit one more.
fn phase_stop(
    book: &ReviewBook,
    policy: &classify::Review,
    deadline: Instant,
    quote: Option<u64>,
    cancellation: &Cancellation,
) -> Option<&'static str> {
    if cancellation.stopped() {
        return Some("caller disconnected before secondary dispatch");
    }
    if book.attempts >= policy.max_attempts {
        return Some("the review policy's `max_attempts` bound is spent");
    }
    if Instant::now() >= deadline {
        return Some("the review phase's `latency_ms` bound is spent");
    }
    if policy.max_spend.is_some() && quote.is_none() {
        return Some("the review policy's `max_spend` requires a known configured price");
    }
    if let Some(quote) = quote {
        let Some(total) = book.spend.checked_add(quote) else {
            return Some("the review phase's spend accounting would overflow");
        };
        if policy.max_spend.is_some_and(|maximum| total > maximum) {
            return Some("the review policy's `max_spend` cannot cover the door's worst-case hold");
        }
    }
    None
}

/// One attempt's row in an item's `attempts` chain: who dispatched,
/// what it produced, and the identity it was recorded under.
fn attempt_record(
    role: &str,
    door: &str,
    model: &str,
    outcome: &str,
    cause: Option<&str>,
    attempt_id: &str,
    latency_ms: Option<u64>,
) -> Value {
    let mut record = json!({
        "role": role,
        "door": door,
        "model": model,
        "outcome": outcome,
        "attempt_id": attempt_id,
    });
    if let Some(cause) = cause {
        record["cause"] = json!(cause);
    }
    if let Some(latency_ms) = latency_ms {
        record["latency_ms"] = json!(latency_ms);
    }
    record
}

/// The primary's row in an item's attempt chain — the call's own
/// forward, recorded the same way the secondary dispatches are.
fn primary_attempt(request: &ClassifyRequest, artifact: &str, primary: &PrimaryRecord) -> Value {
    attempt_record(
        "primary",
        &request.model,
        artifact,
        &primary.outcome,
        primary.cause.as_deref(),
        &primary.attempt_id,
        primary.latency_ms,
    )
}

/// A dispatch's row in an item's attempt chain.
fn dispatch_attempt(role: &str, door: &str, result: &DispatchOutcome) -> Value {
    let mut record = attempt_record(
        role,
        door,
        &result.served.model,
        outcome_label(result.outcome),
        result.cause.as_deref(),
        &result.attempt_id,
        Some(result.latency.as_millis() as u64),
    );
    record["receipt"] = json!(result.receipt);
    record["served"] = json!(result.served);
    if let Some(code) = &result.code {
        record["code"] = json!(code);
    }
    if let Some(usage_ref) = &result.usage_ref {
        record["usage_ref"] = json!(usage_ref);
    }
    if let Some(settlement) = result.settlement {
        record["settlement"] = json!(settlement);
    }
    record
}

/// Push a row onto an item's `attempts` chain, opening it with the
/// primary's own record when the chain does not exist yet.
fn push_attempt(
    item: &mut Value,
    request: &ClassifyRequest,
    artifact: &str,
    primary: &PrimaryRecord,
    record: Value,
) {
    if item.get("attempts").and_then(Value::as_array).is_none() {
        item["attempts"] = json!([primary_attempt(request, artifact, primary)]);
    }
    if let Some(attempts) = item["attempts"].as_array_mut() {
        attempts.push(record);
    }
}

/// The item's outcome from its units' final outcomes — the same
/// answered-or-partial read `served_item` makes, kept uniform when a
/// strict review rewrites a unit's outcome.
fn item_outcome(units: &[Value]) -> &'static str {
    let answered = units
        .iter()
        .filter(|unit| unit.get("outcome").and_then(Value::as_str) == Some("answered"))
        .count();
    if answered == units.len() {
        return "answered";
    }
    if answered > 0 {
        return "mixed";
    }
    if units
        .iter()
        .all(|unit| unit.get("outcome").and_then(Value::as_str) == Some("refused"))
    {
        return "refused";
    }
    if units
        .iter()
        .all(|unit| unit.get("outcome").and_then(Value::as_str) == Some("unattempted"))
    {
        return "unattempted";
    }
    "unavailable"
}

/// The call context a review phase's dispatches run inside — the
/// caller's own credentials, the call's plan and primary artifact, and
/// the call's own deadline as the outer bound.
struct PhaseContext<'a> {
    state: &'a ServeState,
    registry: &'a Registry,
    caller: &'a Caller,
    headers: &'a HeaderMap,
    request: &'a ClassifyRequest,
    plan: &'a Arc<classify::Plan>,
    artifact: &'a str,
    naming: &'a Naming<'a>,
    call_deadline: Instant,
    cancellation: &'a Cancellation,
}

/// The review and fallback phase a declared review policy runs after
/// the primary fan-out: first retry the items whose declared cause a
/// fallback entry covers, then re-judge the units the declared trigger
/// names. Every dispatch passes the full admission path under its own
/// identity; every bound the policy declared stops further work
/// visibly — an exhausted budget is a recorded outcome, never a
/// silently skipped review.
async fn review_phase(
    ctx: &PhaseContext<'_>,
    policy: &classify::Review,
    items: &mut [Value],
    primaries: &[PrimaryRecord],
) -> ReviewBook {
    let PhaseContext {
        state,
        registry,
        caller,
        headers,
        request,
        plan,
        artifact,
        naming,
        call_deadline,
        cancellation,
    } = *ctx;
    let deadline = call_deadline.min(Instant::now() + Duration::from_millis(policy.latency_ms));
    let mut book = ReviewBook::default();

    // Fallback first: an item the primary never got a decided answer
    // for retries through the door its cause's entry names — one hop,
    // and only the causes the policy declared.
    for (index, item) in items.iter_mut().enumerate() {
        let class = match item["outcome"].as_str() {
            Some("unavailable") => classify::FallbackCause::Transport,
            Some("unattempted") => classify::FallbackCause::Capacity,
            Some("refused") => classify::FallbackCause::Refused,
            _ => continue,
        };
        let cause = item["cause"].as_str().unwrap_or_default().to_string();
        let Some(entry) = policy.fallback.iter().find(|entry| entry.on == class) else {
            item["fallback"] = json!({
                "on": class.name(),
                "outcome": "skipped",
                "cause": "the policy names no fallback for this cause",
            });
            continue;
        };
        if class == classify::FallbackCause::Refused
            && !entry
                .codes
                .as_ref()
                .is_some_and(|codes| codes.contains(&cause))
        {
            // A semantic refusal the entry did not enumerate is an
            // answer, not a retryable failure — the item keeps it.
            item["fallback"] = json!({
                "on": class.name(),
                "outcome": "skipped",
                "cause": "the refusal's cause is not among the entry's declared `codes`",
            });
            continue;
        }
        let quote = spend_quote(state, &entry.model);
        if let Some(stop) = phase_stop(&book, policy, deadline, quote, cancellation) {
            item["fallback"] = json!({
                "on": class.name(),
                "door": entry.model,
                "outcome": "unattempted",
                "cause": stop,
            });
            continue;
        }
        // The body names the fallback door's bound artifact — the same
        // questions and state the primary carried.
        let expected = registry
            .authorize(caller.tenant.as_deref(), &entry.model)
            .ok()
            .map(|admission| admission.binding.artifact.clone());
        let fallback_artifact = expected
            .as_ref()
            .map(|artifact| artifact.model.clone())
            .unwrap_or_else(|| entry.model.clone());
        let (envelope, asked) =
            forward_envelope(request, plan, &request.inputs[index], &fallback_artifact);
        let body = Bytes::from(serde_json::to_vec(&envelope).unwrap_or_default());
        book.seq += 1;
        let sub = SubCall {
            cancellation: cancellation.clone(),
            expected,
            door: entry.model.clone(),
            request: format!("{}:fb:{}", naming.request, book.seq),
            attempt: naming.attempt,
            attempt_id: format!("{}:fb:{}", naming.attempt_id, book.seq),
            request_digest: digest_request(&envelope),
            units: units_of(&envelope, body.len()),
            body,
            deadline,
        };
        let result = dispatch(state, headers, &sub).await;
        book.attempts += 1;
        if result.settlement.is_some() {
            book.spend += quote.unwrap_or(0);
        }
        if result.dispatched {
            book.fallback_dispatched += 1;
            book.fallback_input.add(
                result
                    .usage
                    .as_ref()
                    .and_then(|u| u.get("input_tokens"))
                    .and_then(Value::as_u64),
            );
            book.fallback_output.add(
                result
                    .usage
                    .as_ref()
                    .and_then(|u| u.get("output_tokens"))
                    .and_then(Value::as_u64),
            );
            if result.outcome == Outcome::Answered {
                book.fallback_answered += 1;
            }
        }
        push_attempt(
            item,
            request,
            artifact,
            &primaries[index],
            dispatch_attempt("fallback", &entry.model, &result),
        );
        // Rebuild the item from what the fallback produced, keeping the
        // primary's own record under `original`.
        let input = request.inputs[index].id.clone();
        let rebuilt = match result.outcome {
            Outcome::Answered => match &result.body {
                Some(body) => {
                    served_item(
                        &input,
                        plan,
                        &asked,
                        &fallback_artifact,
                        body,
                        result.latency,
                    )
                    .0
                }
                None => failed_item(
                    &input,
                    plan,
                    "unavailable",
                    "the fallback answered nothing",
                    Some(result.latency),
                ),
            },
            Outcome::Refused => failed_item(
                &input,
                plan,
                "refused",
                result.cause.as_deref().unwrap_or("refused"),
                Some(result.latency),
            ),
            Outcome::Unavailable => failed_item(
                &input,
                plan,
                "unavailable",
                result.cause.as_deref().unwrap_or("unavailable"),
                Some(result.latency),
            ),
            _ => failed_item(
                &input,
                plan,
                "unattempted",
                result.cause.as_deref().unwrap_or("unattempted"),
                Some(result.latency),
            ),
        };
        // The original item survives whole — its units and outputs are
        // the primary's record, never discarded by the retry. The
        // attempt chain carries onto the item the fallback produced.
        let original = item.clone();
        let attempts = original.get("attempts").cloned();
        *item = rebuilt;
        if let Some(attempts) = attempts {
            item["attempts"] = attempts;
        }
        item["original"] = original;
        item["fallback"] = json!({
            "on": class.name(),
            "door": entry.model,
            "model": result.served.model,
            "outcome": outcome_label(result.outcome),
            "attempt_id": result.attempt_id,
        });
        if let Some(cause) = &result.cause {
            item["fallback"]["cause"] = json!(cause);
        }
        if let Some(usage) = &result.usage {
            item["usage"] = usage.clone();
        }
        if let Some(settlement) = result.settlement {
            item["fallback"]["settlement"] = json!(settlement);
        }
    }

    // Then review: the declared trigger names the units a second model
    // re-judges — the same state and questions, an independent read.
    for (index, item) in items.iter_mut().enumerate() {
        if !matches!(item["outcome"].as_str(), Some("answered") | Some("mixed")) {
            continue;
        }
        let mut triggered = false;
        let mut unresolved = false;
        for unit_index in 0..plan.units.len() {
            let unit = &item["units"][unit_index];
            let unit_outcome = unit["outcome"].as_str().unwrap_or_default();
            let fired = match policy.trigger {
                classify::ReviewTrigger::Uncertain => unit["uncertain"].as_bool() == Some(true),
                classify::ReviewTrigger::NoMatch => unit["no_match"].as_bool() == Some(true),
                classify::ReviewTrigger::Always => {
                    matches!(unit_outcome, "answered" | "unavailable")
                }
            };
            if !fired {
                continue;
            }
            triggered = true;
            let reason = match policy.trigger {
                classify::ReviewTrigger::Uncertain => "uncertain",
                classify::ReviewTrigger::NoMatch => "no-match",
                classify::ReviewTrigger::Always => "always",
            };
            // A bound that stops the dispatch is itself recorded —
            // an exhausted budget is visible on the unit it stopped.
            let stop = if book.reviewed >= policy.max_items {
                Some("the review policy's `max_items` bound is spent")
            } else {
                phase_stop(
                    &book,
                    policy,
                    deadline,
                    spend_quote(state, &policy.reviewer),
                    cancellation,
                )
            };
            if let Some(stop) = stop {
                unresolved = true;
                let unit = &mut item["units"][unit_index];
                if policy.on_failure == classify::ReviewFailure::Strict {
                    let original = unit.clone();
                    *unit = unit_failure(&plan.units[unit_index], "unattempted", stop);
                    unit["original"] = original;
                    unit["final_source"] = json!("reviewer");
                } else {
                    unit["final_source"] = json!("primary");
                }
                unit["review"] = json!({
                    "reason": reason,
                    "outcome": "unattempted",
                    "selected": null,
                    "cause": stop,
                });
                continue;
            }
            // The body names the reviewer door's bound artifact; the
            // dispatch itself re-authorizes fresh under the caller.
            let expected = registry
                .authorize(caller.tenant.as_deref(), &policy.reviewer)
                .ok()
                .map(|admission| admission.binding.artifact.clone());
            let reviewer_artifact = expected
                .as_ref()
                .map(|artifact| artifact.model.clone())
                .unwrap_or_else(|| policy.reviewer.clone());
            let (envelope, asked) = unit_forward_envelope(
                request,
                plan,
                &request.inputs[index],
                unit_index,
                &reviewer_artifact,
            );
            let body = Bytes::from(serde_json::to_vec(&envelope).unwrap_or_default());
            book.seq += 1;
            let sub = SubCall {
                cancellation: cancellation.clone(),
                expected,
                door: policy.reviewer.clone(),
                request: format!("{}:rev:{}", naming.request, book.seq),
                attempt: naming.attempt,
                attempt_id: format!("{}:rev:{}", naming.attempt_id, book.seq),
                request_digest: digest_request(&envelope),
                units: units_of(&envelope, body.len()),
                body,
                deadline,
            };
            let result = dispatch(state, headers, &sub).await;
            book.attempts += 1;
            book.reviewed += 1;
            if result.settlement.is_some() {
                book.spend += spend_quote(state, &policy.reviewer).unwrap_or(0);
            }
            if result.dispatched {
                book.review_forwards += 1;
                book.review_input.add(
                    result
                        .usage
                        .as_ref()
                        .and_then(|u| u.get("input_tokens"))
                        .and_then(Value::as_u64),
                );
                book.review_output.add(
                    result
                        .usage
                        .as_ref()
                        .and_then(|u| u.get("output_tokens"))
                        .and_then(Value::as_u64),
                );
            }
            let mut review = json!({
                "reason": reason,
                "attempt_id": result.attempt_id,
                "latency_ms": result.latency.as_millis() as u64,
                "usage": result.usage.clone().unwrap_or(Value::Null),
            });
            if !result.served.model.is_empty() {
                review["model"] = json!(result.served.model);
                if !result.served.artifact_signature.is_empty() {
                    review["artifact"] = json!(result.served.artifact_signature);
                }
            }
            if let Some(settlement) = result.settlement {
                review["settlement"] = json!(settlement);
            }
            // Read the reviewer's answer through the same contract the
            // primary's answers are held to — a review that cannot be
            // validated is a review that did not answer.
            let resolved = if result.outcome == Outcome::Answered {
                match result
                    .body
                    .as_ref()
                    .and_then(|body| serde_json::from_slice::<Value>(body).ok())
                    .filter(|_| result.model.as_deref() == Some(reviewer_artifact.as_str()))
                    .and_then(|parsed| parsed.get("answers").and_then(Value::as_object).cloned())
                {
                    Some(answers) => Some(unit_result(
                        unit_index,
                        &plan.units[unit_index],
                        plan,
                        &asked,
                        &answers,
                    )),
                    None => Some(unit_failure(
                        &plan.units[unit_index],
                        "unavailable",
                        "the reviewer's answer did not parse or names another model",
                    )),
                }
            } else {
                None
            };
            push_attempt(
                item,
                request,
                artifact,
                &primaries[index],
                dispatch_attempt("review", &policy.reviewer, &result),
            );
            let unit = &mut item["units"][unit_index];
            let old = unit.clone();
            match resolved {
                // The reviewer answered and its answer held to the
                // unit's contract: it becomes the final selection, with
                // the primary's whole result preserved under `original`.
                Some(resolved) if resolved["outcome"].as_str() == Some("answered") => {
                    book.review_answered += 1;
                    review["outcome"] = json!("answered");
                    review["raw"] = resolved["raw"].clone();
                    review["selected"] = resolved["selected"].clone();
                    review["changed"] = json!(resolved["selected"] != old["selected"]);
                    if resolved.get("no_match").and_then(Value::as_bool) == Some(true) {
                        review["no_match"] = json!(true);
                    }
                    if resolved.get("uncertain").and_then(Value::as_bool) == Some(true) {
                        review["uncertain"] = json!(true);
                    }
                    *unit = resolved;
                    unit["original"] = old;
                    unit["review"] = review;
                    unit["final_source"] = json!("reviewer");
                }
                // The review did not produce a usable answer: a refused
                // or unreachable reviewer, an answer that fails the
                // contract, or a dispatch the bounds stopped. The
                // policy's `on_failure` decides whether the primary's
                // output stands or the review's outcome governs.
                other => {
                    unresolved = true;
                    let (outcome, cause) = match other {
                        Some(failed) => (
                            failed["outcome"]
                                .as_str()
                                .unwrap_or("unavailable")
                                .to_string(),
                            failed["cause"].as_str().map(str::to_string),
                        ),
                        None => (
                            outcome_label(result.outcome).to_string(),
                            result.cause.clone(),
                        ),
                    };
                    review["outcome"] = json!(outcome);
                    review["selected"] = Value::Null;
                    if let Some(cause) = &cause {
                        review["cause"] = json!(cause);
                    }
                    match policy.on_failure {
                        classify::ReviewFailure::KeepOriginal => {
                            unit["review"] = review;
                            unit["final_source"] = json!("primary");
                        }
                        classify::ReviewFailure::Strict => {
                            *unit = unit_failure(
                                &plan.units[unit_index],
                                &outcome,
                                cause.as_deref().unwrap_or("the review did not answer"),
                            );
                            unit["original"] = old;
                            unit["review"] = review;
                            unit["final_source"] = json!("reviewer");
                        }
                    }
                }
            }
        }
        if !triggered {
            item["review_status"] = json!("not-reviewed");
        } else if unresolved {
            item["review_status"] = json!("review-incomplete");
        } else {
            item["review_status"] = json!("reviewed");
        }
        if triggered {
            // The item's outcome reads its units' final state — a
            // strict review that did not answer can move an item off
            // `answered`.
            item["outcome"] = json!(item_outcome(
                item["units"]
                    .as_array()
                    .cloned()
                    .unwrap_or_default()
                    .as_slice()
            ));
        }
    }
    book
}

/// A total exists only when every dispatched input reports the counter.
#[derive(Default)]
struct CompleteCounter {
    value: u64,
    missing: bool,
}

impl CompleteCounter {
    fn add(&mut self, value: Option<u64>) {
        match value.and_then(|value| self.value.checked_add(value)) {
            Some(total) => self.value = total,
            None => self.missing = true,
        }
    }

    fn total(&self) -> Option<u64> {
        (!self.missing).then_some(self.value)
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

/// The (unit index, question id, label) rows a forward's answers are
/// read back through — one row per question the envelope asked.
type Asked = Vec<(usize, String, Option<String>)>;

/// One unit's questions, numbered from `first`: the question map
/// entries and the (unit, qid, label) rows the answers are read
/// through.
fn unit_questions(
    request: &ClassifyRequest,
    unit: &classify::Unit,
    unit_index: usize,
    first: u64,
) -> (serde_json::Map<String, Value>, Asked, u64) {
    let mut questions = serde_json::Map::new();
    let mut asked = Vec::new();
    let mut next = first;
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
        Mode::MultiLabel | Mode::Binary => {
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
        Mode::Score => {
            let qid = format!("q{next}");
            next += 1;
            let criteria: Vec<Value> = unit
                .levels
                .iter()
                .map(|level| Value::String(level.clone()))
                .collect();
            let instructions = instructions_for(
                request,
                unit,
                "Place the input on the rubric's ordered levels; level 0 is the \
                 first criterion.",
            );
            questions.insert(
                qid.clone(),
                json!({"type": "score", "instructions": instructions,
                       "criteria": criteria}),
            );
            asked.push((unit_index, qid, None));
        }
    }
    (questions, asked, next)
}

/// The input's `state` value: its text, its record, or null — `plan()`
/// already proved exactly one content form per input.
fn input_state(input: &classify::Input) -> Value {
    match (&input.text, &input.record) {
        (Some(text), None) => Value::String(text.clone()),
        (None, Some(record)) => Value::Object(record.clone()),
        _ => Value::Null,
    }
}

/// The `systemone` envelope one input's forward carries, and the map
/// from question id back to (unit, label) the answers are read
/// through.
fn forward_envelope(
    request: &ClassifyRequest,
    plan: &classify::Plan,
    input: &classify::Input,
    model: &str,
) -> (Value, Asked) {
    let mut questions = serde_json::Map::new();
    let mut asked = Vec::new();
    let mut next = 0_u64;
    for (unit_index, unit) in plan.units.iter().enumerate() {
        let (unit_questions, unit_asked, after) = unit_questions(request, unit, unit_index, next);
        questions.extend(unit_questions);
        asked.extend(unit_asked);
        next = after;
    }
    (
        json!({"model": model, "state": input_state(input), "questions": questions}),
        asked,
    )
}

/// The questions one input's forward asks, and the map from question
/// id back to (unit, label) the answers are read through.
fn forward_body(
    request: &ClassifyRequest,
    plan: &classify::Plan,
    input: &classify::Input,
    model: &str,
) -> (Bytes, Asked) {
    let (envelope, asked) = forward_envelope(request, plan, input, model);
    (
        Bytes::from(serde_json::to_vec(&envelope).unwrap_or_default()),
        asked,
    )
}

/// The `systemone` envelope a review forward carries for one unit of
/// one input — the same state and the same questions the primary
/// asked, nothing more: a second model's independent read, never the
/// first model's answer handed back for confirmation.
fn unit_forward_envelope(
    request: &ClassifyRequest,
    plan: &classify::Plan,
    input: &classify::Input,
    unit_index: usize,
    model: &str,
) -> (Value, Asked) {
    let (questions, asked, _) = unit_questions(request, &plan.units[unit_index], unit_index, 0);
    (
        json!({"model": model, "state": input_state(input), "questions": questions}),
        asked,
    )
}

/// The instructions one question carries: the request's own, the
/// dimension's, then the judgment's framing — in that order.
fn instructions_for(request: &ClassifyRequest, unit: &classify::Unit, framing: &str) -> String {
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

/// The corpus-level views a call reports: one entry per binary or score
/// unit. A binary unit reports the input ids its declared threshold
/// selected; a score unit reports the answered inputs ordered by their
/// weighted positions, equal scores keeping input order. Both name the
/// inputs no judgment answered — an input the call could not evaluate
/// is neither selected nor ranked, and never silently dropped.
fn corpus_selections(plan: &classify::Plan, items: &[Value]) -> Vec<Value> {
    /// One input's result for a unit, by the unit's plan position.
    fn unit_of(item: &Value, unit_index: usize) -> Option<&Value> {
        item.get("units")?.get(unit_index)
    }
    /// Whether the unit answered on this input.
    fn answered(item: &Value, unit_index: usize) -> bool {
        unit_of(item, unit_index)
            .and_then(|unit| unit.get("outcome"))
            .and_then(Value::as_str)
            == Some("answered")
    }
    let mut selections = Vec::new();
    for (unit_index, unit) in plan.units.iter().enumerate() {
        let mut entry = match unit.mode {
            Mode::Binary => {
                let label = unit.labels.first().map_or("", |label| label.id.as_str());
                let selected: Vec<Value> = items
                    .iter()
                    .filter(|item| {
                        answered(item, unit_index)
                            && unit_of(item, unit_index)
                                .and_then(|unit| unit.get("selected"))
                                .and_then(Value::as_str)
                                == Some(label)
                    })
                    .map(|item| item["input"].clone())
                    .collect();
                json!({"mode": "binary", "label": label, "selected": selected})
            }
            Mode::Score => {
                let Some(rule) = plan.policy.select.score.as_ref() else {
                    continue;
                };
                let mut scored: Vec<(&Value, f64)> = items
                    .iter()
                    .filter(|item| answered(item, unit_index))
                    .filter_map(|item| {
                        unit_of(item, unit_index)
                            .and_then(|unit| unit.get("raw"))
                            .and_then(|raw| raw.get("score"))
                            .and_then(Value::as_f64)
                            .map(|score| (item, score))
                    })
                    .collect();
                // The sort is stable, so equal positions keep input order.
                match rule.order {
                    RankOrder::Descending => {
                        scored.sort_by(|a, b| b.1.total_cmp(&a.1));
                    }
                    RankOrder::Ascending => scored.sort_by(|a, b| a.1.total_cmp(&b.1)),
                }
                if let Some(top_n) = rule.top_n {
                    scored.truncate(top_n as usize);
                }
                let ranking: Vec<Value> = scored
                    .iter()
                    .map(|(item, _)| item["input"].clone())
                    .collect();
                json!({"mode": "score", "ranking": ranking})
            }
            Mode::SingleLabel | Mode::MultiLabel => continue,
        };
        if let Some(dimension) = &unit.dimension {
            entry["dimension"] = json!(dimension);
        }
        let unevaluated: Vec<Value> = items
            .iter()
            .filter(|item| !answered(item, unit_index))
            .map(|item| item["input"].clone())
            .collect();
        entry["unevaluated"] = json!(unevaluated);
        selections.push(entry);
    }
    selections
}

/// Increment one counter in a count map.
fn bump(counts: &mut serde_json::Map<String, Value>, key: &str) {
    let next = counts.get(key).and_then(Value::as_u64).unwrap_or(0) + 1;
    counts.insert(key.to_string(), json!(next));
}

/// The per-unit tallies a call reports, one entry per unit in plan
/// order: the unit's own outcome counts, how many answered inputs each
/// label or rubric level was selected for, how many answered inputs
/// resolved to no-match, and the inputs a declared `uncertain_below`
/// cut flagged.
///
/// Every figure derives from the assembled per-item results: a count
/// is a policy selection that actually happened, so an input no
/// judgment answered is named under its outcome and never reads as a
/// zero, a label a `top_n` cap excluded is not counted, and a
/// designated no-match label counts under `no_match` when the policy
/// routed the input there rather than under its own id. A multi-label
/// input counts under every label it selected — independent Nouls are
/// not a distribution and the counts need not sum to `answered`.
fn corpus_aggregates(plan: &classify::Plan, items: &[Value]) -> Vec<Value> {
    /// The `uncertain_below` cut a unit's mode declares, if any.
    fn uncertainty_cut(plan: &classify::Plan, mode: Mode) -> Option<f64> {
        let select = &plan.policy.select;
        match mode {
            Mode::SingleLabel => select.single_label.as_ref()?.uncertain_below,
            Mode::MultiLabel => select.multi_label.as_ref()?.uncertain_below,
            Mode::Binary => select.binary.as_ref()?.uncertain_below,
            Mode::Score => select.score.as_ref()?.uncertain_below,
        }
    }
    let mut aggregates = Vec::new();
    for (unit_index, unit) in plan.units.iter().enumerate() {
        let mut counts = Counts::default();
        let mut no_match = 0_u64;
        let mut uncertain = Vec::new();
        let mut labels: serde_json::Map<String, Value> = unit
            .labels
            .iter()
            .map(|label| (label.id.clone(), json!(0)))
            .collect();
        let mut levels: serde_json::Map<String, Value> = (0..unit.levels.len())
            .map(|level| (level.to_string(), json!(0)))
            .collect();
        for item in items {
            let Some(result) = item.get("units").and_then(|units| units.get(unit_index)) else {
                continue;
            };
            match result.get("outcome").and_then(Value::as_str) {
                Some("answered") => counts.answered += 1,
                Some("refused") => counts.refused += 1,
                Some("unavailable") => counts.unavailable += 1,
                _ => counts.unattempted += 1,
            }
            if result.get("outcome").and_then(Value::as_str) != Some("answered") {
                continue;
            }
            if result.get("uncertain").and_then(Value::as_bool) == Some(true) {
                uncertain.push(item["input"].clone());
            }
            if result.get("no_match").and_then(Value::as_bool) == Some(true) {
                no_match += 1;
                continue;
            }
            match unit.mode {
                Mode::Score => {
                    if let Some(level) = result.get("selected").and_then(Value::as_u64) {
                        bump(&mut levels, &level.to_string());
                    }
                }
                Mode::MultiLabel => {
                    if let Some(selected) = result.get("selected").and_then(Value::as_array) {
                        for label in selected.iter().filter_map(Value::as_str) {
                            bump(&mut labels, label);
                        }
                    }
                }
                Mode::SingleLabel | Mode::Binary => {
                    if let Some(label) = result.get("selected").and_then(Value::as_str) {
                        bump(&mut labels, label);
                    }
                }
            }
        }
        let mut entry = json!({
            "mode": unit.mode.name(),
            "outcomes": {
                "answered": counts.answered,
                "refused": counts.refused,
                "unavailable": counts.unavailable,
                "unattempted": counts.unattempted,
            },
            "no_match": no_match,
        });
        if let Some(dimension) = &unit.dimension {
            entry["dimension"] = json!(dimension);
        }
        match unit.mode {
            Mode::Score => {
                entry["levels"] = Value::Object(levels);
            }
            _ => {
                entry["labels"] = Value::Object(labels);
            }
        }
        if uncertainty_cut(plan, unit.mode).is_some() {
            entry["uncertain"] = json!(uncertain);
        }
        aggregates.push(entry);
    }
    aggregates
}

/// One input's assembled result when its forward answered: each unit's
/// outcome, raw answers, and policy-selected output.
fn served_item(
    input: &str,
    plan: &classify::Plan,
    asked: &[(usize, String, Option<String>)],
    expected_model: &str,
    body: &Bytes,
    latency: Duration,
) -> (Value, Option<Value>) {
    let parsed = serde_json::from_slice::<Value>(body).ok();
    let answers = parsed
        .as_ref()
        .filter(|body| body.get("model").and_then(Value::as_str) == Some(expected_model))
        .and_then(|body| body.get("answers"))
        .and_then(Value::as_object)
        .cloned();
    let model = parsed
        .as_ref()
        .and_then(|body| body.get("model"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let usage = parsed.as_ref().and_then(|body| body.get("usage")).cloned();
    let mut units = Vec::with_capacity(plan.units.len());
    let mut answered = 0_u64;
    for (unit_index, unit) in plan.units.iter().enumerate() {
        let result = match answers.as_ref() {
            Some(answers) => unit_result(unit_index, unit, plan, asked, answers),
            None => unit_failure(unit, "unavailable", "the door's answer did not parse"),
        };
        if result.get("outcome").and_then(Value::as_str) == Some("answered") {
            answered += 1;
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
        "input": input,
        "outcome": input_outcome,
        "units": units,
        "latency_ms": latency.as_millis() as u64,
    });
    if let Some(model) = model {
        item["model"] = json!(model);
    }
    item["usage"] = usage.clone().unwrap_or(Value::Null);
    item["review_status"] = json!("not-reviewed");
    (item, usage)
}

/// Reuse the native SDK's answer validation, including categorical mass.
/// The local envelope supplies only the decoder context; it is never returned
/// as serving evidence and does not make a network call.
fn valid_primitive(answer: &Value, kind: &str) -> bool {
    decode_answer(answer).is_some_and(|answer| answer.kind() == kind)
}

/// One answer decoded through the native SDK, whatever its type.
fn decode_answer(answer: &Value) -> Option<jev::Answer> {
    let bytes = serde_json::to_vec(&json!({"model":"decoder-context", "answers":{"q":answer}}))
        .expect("a JSON value serializes");
    jev::SystemOneResponse::decode(jev::RawResponse {
        status: 200,
        headers: Default::default(),
        bytes,
    })
    .ok()
    .and_then(|response| response.answers.get("q").cloned())
}

/// A score answer the native SDK validates, held to exactly the rubric
/// the unit declared — the same check `check_against` runs against the
/// questions a request sent, so a legend or a distribution that names
/// other levels fails.
fn valid_score(answer: &Value, levels: usize) -> Option<jev::ScoreAnswer> {
    if answer.get("type").and_then(Value::as_str) != Some("score") {
        return None;
    }
    let bytes = serde_json::to_vec(&json!({"model":"decoder-context", "answers":{"q":answer}}))
        .expect("a JSON value serializes");
    let response = jev::SystemOneResponse::decode(jev::RawResponse {
        status: 200,
        headers: Default::default(),
        bytes,
    })
    .ok()?;
    let mut asked = jev::Questions::new();
    asked.insert(
        "q",
        jev::Score::new("the unit's rubric", vec![None; levels]),
    );
    response.check_against(&asked).ok()?;
    response.score("q").ok().cloned()
}

/// The categorical level a score answer reports: the estimator's own
/// `selected` when it sent one, else the highest level among the
/// distribution's maxima — the documented fallback. An answer with no
/// distribution selects nothing; its weighted position is still on
/// `raw`.
fn selected_level(answer: &jev::ScoreAnswer) -> Option<u64> {
    if let Some(selected) = &answer.selected {
        return selected.parse().ok();
    }
    answer
        .probabilities
        .iter()
        .max_by(|a, b| a.1.total_cmp(b.1).then(a.0.cmp(b.0)))
        .map(|(level, _)| u64::from(*level))
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
    let mut base = json!({"mode": unit.mode.name()});
    if let Some(dimension) = &unit.dimension {
        base["dimension"] = json!(dimension);
    }
    let mut asked = asked.iter().filter(|(index, _, _)| *index == unit_index);
    match unit.mode {
        Mode::SingleLabel => {
            let Some(rule) = plan.policy.select.single_label.as_ref() else {
                return unit_failure(unit, "unavailable", "no declared selection rule");
            };
            let Some(answer) = asked.next().and_then(|(_, qid, _)| answers.get(qid)) else {
                return unit_failure(unit, "unavailable", "the door answered no choice");
            };
            if !valid_primitive(answer, "choice") {
                return unit_failure(unit, "unavailable", "the answer is not a choice");
            }
            let Some(probabilities) = answer.get("probabilities").and_then(Value::as_object) else {
                return unit_failure(
                    unit,
                    "unavailable",
                    "the choice answer names no distribution",
                );
            };
            if probabilities.len() != unit.labels.len() {
                return unit_failure(
                    unit,
                    "unavailable",
                    "the distribution has a different label set",
                );
            }
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
            let selection = rule.resolve(&pairs);
            base["outcome"] = json!("answered");
            base["raw"] = answer.clone();
            base["selected"] = selection.output;
            if selection.no_match {
                base["no_match"] = json!(true);
            }
            if rule.uncertain_below.is_some_and(|cut| {
                pairs
                    .iter()
                    .map(|(_, probability)| *probability)
                    .fold(f64::NEG_INFINITY, f64::max)
                    < cut
            }) {
                base["uncertain"] = json!(true);
            }
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
                if !valid_primitive(answer, "noul") {
                    return unit_failure(unit, "unavailable", "the answer is not a noul");
                }
                let Some(probability) = answer.get("noul").and_then(Value::as_f64) else {
                    return unit_failure(unit, "unavailable", "a noul answer holds no probability");
                };
                if !(0.0..=1.0).contains(&probability) {
                    return unit_failure(unit, "unavailable", "a noul answer is not a probability");
                }
                raw.insert(label.clone(), answer.clone());
                pairs.push((label.clone(), probability));
            }
            let selected = rule.select(&pairs);
            // Each label's Noul stands alone — an empty selection is
            // the no-match outcome, and a weak label anywhere flags the
            // input for review when the policy declares a cut.
            if selected.is_null() || selected.as_array().is_some_and(|labels| labels.is_empty()) {
                base["no_match"] = json!(true);
            }
            if rule.uncertain_below.is_some_and(|cut| {
                pairs
                    .iter()
                    .any(|(_, probability)| probability.max(1.0 - *probability) < cut)
            }) {
                base["uncertain"] = json!(true);
            }
            base["outcome"] = json!("answered");
            base["raw"] = Value::Object(raw);
            base["selected"] = selected;
            base
        }
        Mode::Binary => {
            let Some(rule) = plan.policy.select.binary.as_ref() else {
                return unit_failure(unit, "unavailable", "no declared selection rule");
            };
            let Some(answer) = asked.next().and_then(|(_, qid, _)| answers.get(qid)) else {
                return unit_failure(unit, "unavailable", "the door answered no noul");
            };
            if !valid_primitive(answer, "noul") {
                return unit_failure(unit, "unavailable", "the answer is not a noul");
            }
            let Some(probability) = answer.get("noul").and_then(Value::as_f64) else {
                return unit_failure(unit, "unavailable", "a noul answer holds no probability");
            };
            if !(0.0..=1.0).contains(&probability) {
                return unit_failure(unit, "unavailable", "a noul answer is not a probability");
            }
            let selected = if rule.selects(probability) {
                json!(unit.labels[0].id)
            } else {
                base["no_match"] = json!(true);
                Value::Null
            };
            if rule
                .uncertain_below
                .is_some_and(|cut| probability.max(1.0 - probability) < cut)
            {
                base["uncertain"] = json!(true);
            }
            base["outcome"] = json!("answered");
            base["raw"] = answer.clone();
            base["selected"] = selected;
            base
        }
        Mode::Score => {
            let Some(answer) = asked.next().and_then(|(_, qid, _)| answers.get(qid)) else {
                return unit_failure(unit, "unavailable", "the door answered no score");
            };
            let Some(scored) = valid_score(answer, unit.levels.len()) else {
                return unit_failure(
                    unit,
                    "unavailable",
                    "the answer is not a score on this rubric",
                );
            };
            let selected = selected_level(&scored).map_or(Value::Null, |level| json!(level));
            if selected.is_null() {
                base["no_match"] = json!(true);
            }
            if let Some(cut) = plan
                .policy
                .select
                .score
                .as_ref()
                .and_then(|rule| rule.uncertain_below)
            {
                // An answer with no distribution has no top level to
                // compare, so the declared cut cannot flag it.
                let top = scored
                    .probabilities
                    .values()
                    .copied()
                    .fold(f64::NEG_INFINITY, f64::max);
                if !scored.probabilities.is_empty() && top < cut {
                    base["uncertain"] = json!(true);
                }
            }
            base["outcome"] = json!("answered");
            base["raw"] = answer.clone();
            base["selected"] = selected;
            base
        }
    }
}

/// One unit's result when it carries no usable answer.
fn unit_failure(unit: &classify::Unit, outcome: &str, cause: &str) -> Value {
    let mut base = json!({
        "mode": unit.mode.name(),
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
    input: &str,
    plan: &classify::Plan,
    outcome: &str,
    cause: &str,
    latency: Option<Duration>,
) -> Value {
    let mut item = json!({
        "input": input,
        "outcome": outcome,
        "cause": cause,
        "units": plan.units.iter().map(|unit| unit_failure(unit, outcome, cause)).collect::<Vec<_>>(),
    });
    if let Some(latency) = latency {
        item["latency_ms"] = json!(latency.as_millis() as u64);
    }
    item
}

/// One input's result when the call never dispatched its forward —
/// nothing spent past the reservation, and the cause says which bound
/// stopped it.
fn unattempted_item(input: &str, plan: &classify::Plan, cause: &str) -> Value {
    failed_item(input, plan, "unattempted", cause, None)
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
    let bytes = backend_response_bytes(response, state.config.max_response_bytes).await?;
    let body: Value = serde_json::from_slice(&bytes)
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

/// Bound both model-card and inference bodies before parsing or retaining them.
async fn backend_response_bytes(
    mut response: reqwest::Response,
    limit: usize,
) -> Result<Bytes, String> {
    if response
        .content_length()
        .is_some_and(|length| length > limit as u64)
    {
        return Err(format!("the door's response exceeded {limit} bytes"));
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|error| format!("the door's response could not be read: {error}"))?
    {
        if chunk.len() > limit.saturating_sub(bytes.len()) {
            return Err(format!("the door's response exceeded {limit} bytes"));
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(Bytes::from(bytes))
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
    if status.is_redirection() {
        return Forwarded::Unavailable {
            message: "the configured door redirected inference; no redirect was followed".into(),
        };
    }
    let body = match backend_response_bytes(response, state.config.max_response_bytes).await {
        Ok(body) => body,
        Err(message) => return Forwarded::Unavailable { message },
    };
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
    settlement: Option<&str>,
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
    if let Some(settlement) = settlement {
        response = response.header("x-settlement", settlement);
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

#[cfg(test)]
mod classification_accounting_tests {
    use super::{CompleteCounter, corpus_aggregates, valid_primitive};
    use crate::classify;
    use serde_json::{Value, json};

    #[test]
    fn choice_mass_and_answer_types_follow_the_native_contract() {
        assert!(!valid_primitive(
            &json!({"type":"choice","choice":"a","confidence":0.8,"probabilities":{"a":0.9,"b":0.9}}),
            "choice"
        ));
        assert!(valid_primitive(
            &json!({"type":"choice","choice":"a","confidence":0.8,"probabilities":{"a":0.8,"b":0.2}}),
            "choice"
        ));
        assert!(!valid_primitive(
            &json!({"type":"noul","noul":0.8}),
            "choice"
        ));
        assert!(!valid_primitive(&json!({"type":"noul","noul":1.1}), "noul"));
    }

    #[test]
    fn missing_or_overflowed_usage_never_becomes_a_complete_total() {
        for entries in [vec![Some(2), None, Some(3)], vec![Some(u64::MAX), Some(1)]] {
            let mut counter = CompleteCounter::default();
            for entry in entries {
                counter.add(entry);
            }
            assert_eq!(counter.total(), None);
        }
        let mut counter = CompleteCounter::default();
        counter.add(Some(0));
        counter.add(Some(3));
        assert_eq!(counter.total(), Some(3));
    }

    /// Plan a classification envelope against the product limits.
    fn plan(value: &Value) -> classify::Plan {
        classify::Request::parse(&serde_json::to_vec(value).unwrap())
            .unwrap()
            .plan(&classify::BackendLimits::product())
            .unwrap()
    }

    /// One assembled item result, as `classify_admitted` builds it.
    fn item(input: &str, outcome: &str, units: Vec<Value>) -> Value {
        json!({"input": input, "outcome": outcome, "units": units})
    }

    /// One answered unit result carrying the given selection.
    fn answered_unit(mode: &str, selected: Value) -> Value {
        json!({"mode": mode, "outcome": "answered", "selected": selected})
    }

    #[test]
    fn aggregates_count_overlapping_labels_and_name_unevaluated_work() {
        // Two inputs, one selecting both labels — the counts overlap
        // rather than summing to the answered count, because each
        // label's Noul stands alone.
        let plan = plan(&json!({
            "v": classify::SCHEMA, "model": "m", "capacity": "c",
            "policy": {"v": classify::POLICY_SCHEMA, "name": "p",
                "select": {"multi_label": {"threshold": 0.5, "ties": "include-all",
                                         "no_match": "empty", "uncertain_below": 0.7}}},
            "inputs": [{"id": "a", "text": "x"}, {"id": "b", "text": "x"},
                       {"id": "c", "text": "x"}, {"id": "d", "text": "x"}],
            "mode": "multi-label",
            "labels": [{"id": "x"}, {"id": "y"}],
        }));
        let items = vec![
            item(
                "a",
                "answered",
                vec![answered_unit("multi-label", json!(["x", "y"]))],
            ),
            item(
                "b",
                "answered",
                vec![{
                    let mut unit = answered_unit("multi-label", json!(["x"]));
                    unit["uncertain"] = json!(true);
                    unit
                }],
            ),
            item(
                "c",
                "answered",
                vec![{
                    let mut unit = answered_unit("multi-label", json!([]));
                    unit["no_match"] = json!(true);
                    unit
                }],
            ),
            item(
                "d",
                "refused",
                vec![json!({"mode": "multi-label", "outcome": "refused", "selected": null})],
            ),
        ];
        let aggregates = corpus_aggregates(&plan, &items);
        assert_eq!(
            Value::Array(aggregates),
            json!([{
                "mode": "multi-label",
                "outcomes": {"answered": 3, "refused": 1, "unavailable": 0, "unattempted": 0},
                "no_match": 1,
                "labels": {"x": 2, "y": 1},
                "uncertain": ["b"],
            }])
        );
    }

    #[test]
    fn aggregates_tell_a_routed_no_match_from_a_genuine_label() {
        // The designated no-match label counts under `no_match` only
        // when the policy routed the input there; winning the
        // distribution outright is a selection like any other.
        let plan = plan(&json!({
            "v": classify::SCHEMA, "model": "m", "capacity": "c",
            "policy": {"v": classify::POLICY_SCHEMA, "name": "p",
                "select": {"single_label": {"ties": "first-declared",
                            "min_probability": 0.6,
                            "no_match": {"kind": "label", "label": "other"}}}},
            "inputs": [{"id": "a", "text": "x"}, {"id": "b", "text": "x"}, {"id": "c", "text": "x"}],
            "mode": "single-label",
            "labels": [{"id": "a"}, {"id": "other"}],
        }));
        let items = vec![
            item(
                "a",
                "answered",
                vec![{
                    let mut unit = answered_unit("single-label", json!("other"));
                    unit["no_match"] = json!(true);
                    unit
                }],
            ),
            item(
                "b",
                "answered",
                vec![answered_unit("single-label", json!("other"))],
            ),
            item(
                "c",
                "answered",
                vec![answered_unit("single-label", json!("a"))],
            ),
        ];
        let aggregates = corpus_aggregates(&plan, &items);
        assert_eq!(aggregates[0]["labels"], json!({"a": 1, "other": 1}));
        assert_eq!(aggregates[0]["no_match"], 1);
        // No review cut was declared, so no uncertainty list appears.
        assert!(aggregates[0].get("uncertain").is_none());
    }

    #[test]
    fn aggregates_tally_levels_and_never_count_unattempted_work() {
        // A score unit's `levels` counts each answered input's
        // categorical level; inputs no judgment answered are named
        // under their outcome, never folded into a zero.
        let plan = plan(&json!({
            "v": classify::SCHEMA, "model": "m", "capacity": "c",
            "policy": {"v": classify::POLICY_SCHEMA, "name": "p",
                "select": {"score": {"order": "descending", "top_n": 1,
                            "uncertain_below": 0.9}}},
            "inputs": [{"id": "a", "text": "x"}, {"id": "b", "text": "x"}, {"id": "c", "text": "x"}],
            "mode": "score",
            "levels": ["low", "high"],
        }));
        let items = vec![
            item("a", "answered", vec![answered_unit("score", json!(1))]),
            item(
                "b",
                "answered",
                vec![{
                    let mut unit = answered_unit("score", json!(0));
                    unit["uncertain"] = json!(true);
                    unit
                }],
            ),
            item(
                "c",
                "unattempted",
                vec![json!({"mode": "score", "outcome": "unattempted", "selected": null})],
            ),
        ];
        let aggregates = corpus_aggregates(&plan, &items);
        assert_eq!(
            Value::Array(aggregates),
            json!([{
                "mode": "score",
                "outcomes": {"answered": 2, "refused": 0, "unavailable": 0, "unattempted": 1},
                "no_match": 0,
                "levels": {"0": 1, "1": 1},
                "uncertain": ["b"],
            }])
        );
    }

    #[test]
    fn aggregates_report_binary_rejection_and_each_dimension_on_its_own() {
        // A binary unit counts the inputs its threshold declined as
        // `no_match`; a dimensional request tallies each unit against
        // its own label set.
        let plan = plan(&json!({
            "v": classify::SCHEMA, "model": "m", "capacity": "c",
            "policy": {"v": classify::POLICY_SCHEMA, "name": "p",
                "select": {"binary": {"threshold": 0.5},
                           "single_label": {"ties": "first-declared",
                              "no_match": {"kind": "null"}}}},
            "inputs": [{"id": "a", "text": "x"}, {"id": "b", "text": "x"}],
            "dimensions": [
                {"id": "gate", "mode": "binary", "labels": [{"id": "keep"}]},
                {"id": "topic", "mode": "single-label",
                 "labels": [{"id": "t1"}, {"id": "t2"}]},
            ],
        }));
        let items = vec![
            item(
                "a",
                "answered",
                vec![
                    answered_unit("binary", json!("keep")),
                    answered_unit("single-label", json!("t2")),
                ],
            ),
            item(
                "b",
                "answered",
                vec![
                    {
                        let mut gate = answered_unit("binary", Value::Null);
                        gate["no_match"] = json!(true);
                        gate
                    },
                    {
                        let mut topic = answered_unit("single-label", Value::Null);
                        topic["no_match"] = json!(true);
                        topic
                    },
                ],
            ),
        ];
        let aggregates = corpus_aggregates(&plan, &items);
        assert_eq!(
            Value::Array(aggregates),
            json!([
                {"dimension": "gate", "mode": "binary",
                 "outcomes": {"answered": 2, "refused": 0, "unavailable": 0, "unattempted": 0},
                 "no_match": 1, "labels": {"keep": 1}},
                {"dimension": "topic", "mode": "single-label",
                 "outcomes": {"answered": 2, "refused": 0, "unavailable": 0, "unattempted": 0},
                 "no_match": 1, "labels": {"t1": 0, "t2": 1}},
            ])
        );
    }
}
