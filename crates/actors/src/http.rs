//! HTTP adapters. The host supplies authentication and current grant validation.
//!
//! Streamed messages contain caller-authorized views, never the stored event
//! payloads. The adapter reacquires authorization on every poll and reconnect.
use crate::{postgres::PgStore, *};
use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{
        IntoResponse, Response, Sse,
        sse::{Event as SseEvent, KeepAlive},
    },
    routing::{get, post},
};
use futures_util::{future::BoxFuture, stream};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{convert::Infallible, sync::Arc, time::Duration};

/// Authentication runs in the host. Request JSON can never supply a Caller.
pub trait Authenticator: Send + Sync + 'static {
    fn authenticate<'a>(
        &'a self,
        headers: &'a HeaderMap,
        workspace: &'a str,
    ) -> BoxFuture<'a, Result<Caller>>;
    /// Resolve current authority from the recorded principal before queued work runs.
    fn revalidate<'a>(&'a self, caller: &'a Caller) -> BoxFuture<'a, Result<Caller>>;
}
#[derive(Clone)]
struct Api {
    store: PgStore,
    auth: Arc<dyn Authenticator>,
    streams: Arc<tokio::sync::Semaphore>,
}
impl Api {
    fn stream_permit(&self) -> Result<tokio::sync::OwnedSemaphorePermit> {
        self.streams.clone().try_acquire_owned().map_err(|_| {
            ActorError::retry("capacity", "Too many streams are open. Try again shortly.")
        })
    }
    async fn caller(&self, headers: &HeaderMap, workspace: &str) -> Result<Caller> {
        let caller = self.auth.authenticate(headers, workspace).await?;
        if caller.workspace_id != workspace {
            return Err(ActorError::new(
                "not_found",
                "The requested item was not found.",
            ));
        }
        Ok(caller)
    }
}
/// Configure the same authority refresher on the store passed to the runtime.
/// Use the returned store for both `Runtime` and `router`.
pub fn with_authenticator(store: PgStore, auth: Arc<dyn Authenticator>) -> PgStore {
    store.with_revalidator(Arc::new(move |caller| {
        let auth = auth.clone();
        Box::pin(async move { auth.revalidate(&caller).await })
    }))
}
/// Mount this router behind the host's TLS, origin/CSRF policy, and request limits.
/// Cookie-authenticated hosts must enforce CSRF before allowing mutations.
pub fn router(store: PgStore, auth: Arc<dyn Authenticator>) -> Router {
    let store = with_authenticator(store, auth.clone());
    Router::new()
        .route("/v1/actors/contract.json", get(contract))
        .route("/v1/w/{workspace}/feed", get(feed))
        .route(
            "/v1/w/{workspace}/actors/{actor_type}/{key}/actions/{message}",
            post(action),
        )
        .route(
            "/v1/w/{workspace}/actors/{actor_type}/{key}/inbox/{message}",
            post(enqueue),
        )
        .route(
            "/v1/w/{workspace}/actors/{actor_type}/{key}/inbox-status/{seq}",
            get(inbox),
        )
        .route(
            "/v1/w/{workspace}/actors/{actor_type}/{key}/view",
            get(view),
        )
        .route(
            "/v1/w/{workspace}/actors/{actor_type}/{key}/events",
            get(events),
        )
        .route("/v1/w/{workspace}/work/{queue}/claim", post(claim))
        .route(
            "/v1/w/{workspace}/work/{queue}/{uid}/{item}/heartbeat",
            post(heartbeat),
        )
        .route(
            "/v1/w/{workspace}/work/{queue}/{uid}/{item}/finish",
            post(finish),
        )
        .route(
            "/v1/w/{workspace}/work/{queue}/{uid}/{item}/release",
            post(release),
        )
        .layer(DefaultBodyLimit::max(128 * 1024))
        .with_state(Api {
            store,
            auth,
            streams: Arc::new(tokio::sync::Semaphore::new(64)),
        })
}

#[derive(Debug)]
struct ApiError(ActorError);
impl From<ActorError> for ApiError {
    fn from(e: ActorError) -> Self {
        Self(e)
    }
}
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = match self.0.code.as_str() {
            "not_found" | "forbidden" | "unauthorized" => StatusCode::NOT_FOUND,
            "work/fenced" | "fenced" | "stale_claim" | "result_conflict" | "progress_conflict"
            | "conflict" | "exists" | "version_conflict" => StatusCode::CONFLICT,
            "idempotency_mismatch" | "idempotency_conflict" => StatusCode::UNPROCESSABLE_ENTITY,
            "inbox_full" | "limit" | "capacity" => StatusCode::TOO_MANY_REQUESTS,
            "busy" | "storage" | "version_ahead" => StatusCode::SERVICE_UNAVAILABLE,
            _ if self.0.retryable => StatusCode::SERVICE_UNAVAILABLE,
            _ => StatusCode::BAD_REQUEST,
        };
        let mut response=(status,Json(json!({"error":{"group":"actor","code":self.0.code,"message":self.0.message,"retryable":self.0.retryable,"metadata":{}}}))).into_response();
        if self.0.retryable {
            response
                .headers_mut()
                .insert("retry-after", axum::http::HeaderValue::from_static("1"));
        }
        response
    }
}
type ApiResult<T> = std::result::Result<Json<T>, ApiError>;
fn id(workspace: String, actor_type: String, key: String) -> ActorId {
    ActorId {
        workspace_id: workspace,
        actor_type,
        key,
    }
}
fn idem(headers: &HeaderMap) -> Result<Option<String>> {
    headers
        .get("idempotency-key")
        .map(|v| {
            v.to_str()
                .map(str::to_owned)
                .map_err(|_| ActorError::new("bad_args", "The request key is invalid."))
        })
        .transpose()
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ActionBody {
    args: Value,
    #[serde(default)]
    input: Option<Value>,
    #[serde(default)]
    expected_version: Option<u64>,
    /// The work claim this call is fenced by (an executor's call).
    #[serde(default)]
    fence: Option<WorkFence>,
}
async fn action(
    State(api): State<Api>,
    Path((ws, kind, key, name)): Path<(String, String, String, String)>,
    headers: HeaderMap,
    Json(body): Json<ActionBody>,
) -> ApiResult<ActionReply> {
    let caller = api.caller(&headers, &ws).await?;
    Ok(Json(
        api.store
            .call(
                &caller,
                ActionRequest {
                    id: id(ws, kind, key),
                    message: Envelope {
                        name,
                        args: body.args,
                        origin: Origin::Action,
                    },
                    input: body.input,
                    idempotency_key: idem(&headers)?,
                    expected_version: body.expected_version,
                    fence: body.fence,
                },
            )
            .await?,
    ))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct InboxBody {
    args: Value,
}
async fn enqueue(
    State(api): State<Api>,
    Path((ws, kind, key, name)): Path<(String, String, String, String)>,
    headers: HeaderMap,
    Json(body): Json<InboxBody>,
) -> ApiResult<InboxReceipt> {
    let caller = api.caller(&headers, &ws).await?;
    let idem = idem(&headers)?;
    Ok(Json(
        api.store
            .enqueue(
                &caller,
                &id(ws, kind, key),
                Envelope {
                    name,
                    args: body.args,
                    origin: Origin::Inbox,
                },
                idem.as_deref(),
            )
            .await?,
    ))
}
async fn inbox(
    State(api): State<Api>,
    Path((ws, kind, key, seq)): Path<(String, String, String, u64)>,
    headers: HeaderMap,
) -> ApiResult<InboxReceipt> {
    let caller = api.caller(&headers, &ws).await?;
    Ok(Json(
        api.store.inbox(&caller, &id(ws, kind, key), seq).await?,
    ))
}
async fn view(
    State(api): State<Api>,
    Path((ws, kind, key)): Path<(String, String, String)>,
    headers: HeaderMap,
) -> ApiResult<ViewReply> {
    let caller = api.caller(&headers, &ws).await?;
    Ok(Json(api.store.view(&caller, &id(ws, kind, key)).await?))
}
#[derive(Deserialize, Default)]
struct Cursor {
    after: Option<u64>,
}
async fn events(
    State(api): State<Api>,
    Path((ws, kind, key)): Path<(String, String, String)>,
    Query(query): Query<Cursor>,
    headers: HeaderMap,
) -> std::result::Result<Response, ApiError> {
    let actor = id(ws, kind, key);
    let caller = api.caller(&headers, &actor.workspace_id).await?;
    api.store.view(&caller, &actor).await?;
    let header_cursor = headers
        .get("last-event-id")
        .map(|v| {
            v.to_str()
                .ok()
                .and_then(|s| s.parse::<u64>().ok())
                .ok_or_else(|| ActorError::new("bad_args", "The stream position is invalid."))
        })
        .transpose()?;
    // A reconnect always receives a freshly authorized snapshot. A cursor is a
    // version hint, not permission to replay unfiltered stored event payloads.
    let _after = query.after.or(header_cursor);
    let permit = api.stream_permit()?;
    let state = (
        api,
        headers,
        actor,
        None::<(u64, Value)>,
        true,
        tokio::time::Instant::now(),
        permit,
    );
    let updates = stream::unfold(
        state,
        |(api, headers, actor, mut previous, first, started, permit)| async move {
            if started.elapsed() > Duration::from_secs(300) {
                return None;
            }
            if !first {
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
            let caller = match api.caller(&headers, &actor.workspace_id).await {
                Ok(c) => c,
                Err(_) => return None,
            };
            let snapshot = match api.store.view(&caller, &actor).await {
                Ok(v) => v,
                Err(_) => return None,
            };
            let current = (snapshot.version, snapshot.view.clone());
            let event = if previous.as_ref() != Some(&current) {
                previous = Some(current);
                SseEvent::default()
                    .event("view")
                    .id(snapshot.version.to_string())
                    .json_data(&snapshot)
                    .ok()?
            } else {
                SseEvent::default().comment("keep-alive")
            };
            Some((
                Ok::<_, Infallible>(event),
                (api, headers, actor, previous, false, started, permit),
            ))
        },
    );
    Ok(Sse::new(updates)
        .keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
        .into_response())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ClaimBody {
    #[serde(default)]
    target: Option<String>,
    #[serde(default = "one")]
    max: u32,
    /// Wait this long, at most 30 000 ms, for work when none is ready.
    #[serde(default)]
    wait_ms: u64,
}
fn one() -> u32 {
    1
}
#[derive(Serialize)]
struct Claims {
    items: Vec<ClaimedWork>,
}
async fn claim(
    State(api): State<Api>,
    Path((ws, queue)): Path<(String, String)>,
    headers: HeaderMap,
    Json(body): Json<ClaimBody>,
) -> ApiResult<Claims> {
    let caller = api.caller(&headers, &ws).await?;
    Ok(Json(Claims {
        items: api
            .store
            .claim_work_wait(
                &caller,
                &queue,
                body.target.as_deref(),
                body.max,
                Duration::from_millis(body.wait_ms.min(30_000)),
            )
            .await?,
    }))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HeartbeatBody {
    epoch: u64,
    #[serde(default)]
    progress: Option<Progress>,
}
async fn heartbeat(
    State(api): State<Api>,
    Path((ws, queue, uid, item)): Path<(String, String, String, String)>,
    headers: HeaderMap,
    Json(body): Json<HeartbeatBody>,
) -> ApiResult<HeartbeatReply> {
    let caller = api.caller(&headers, &ws).await?;
    require_queue(&caller, &queue)?;
    Ok(Json(
        api.store
            .heartbeat(&caller, &uid, &item, body.epoch, body.progress)
            .await?,
    ))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FinishBody {
    epoch: u64,
    outcome: Value,
}
async fn finish(
    State(api): State<Api>,
    Path((ws, queue, uid, item)): Path<(String, String, String, String)>,
    headers: HeaderMap,
    Json(body): Json<FinishBody>,
) -> ApiResult<Value> {
    let caller = api.caller(&headers, &ws).await?;
    require_queue(&caller, &queue)?;
    api.store
        .finish_work(&caller, &uid, &item, body.epoch, body.outcome)
        .await?;
    Ok(Json(json!({"ok":true})))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReleaseBody {
    epoch: u64,
    reason: String,
}
async fn release(
    State(api): State<Api>,
    Path((ws, queue, uid, item)): Path<(String, String, String, String)>,
    headers: HeaderMap,
    Json(body): Json<ReleaseBody>,
) -> ApiResult<Value> {
    let caller = api.caller(&headers, &ws).await?;
    require_queue(&caller, &queue)?;
    api.store
        .release_work(&caller, &uid, &item, body.epoch, &body.reason)
        .await?;
    Ok(Json(json!({"ok":true})))
}
fn require_queue(caller: &Caller, queue: &str) -> Result<()> {
    if caller
        .executor
        .as_ref()
        .is_some_and(|e| e.queues.iter().any(|q| q == queue))
    {
        Ok(())
    } else {
        Err(ActorError::new(
            "not_found",
            "The requested queue was not found.",
        ))
    }
}

async fn contract(State(api): State<Api>) -> Json<Value> {
    Json(api.store.registry.contract())
}
#[derive(Deserialize)]
struct Topics {
    topics: String,
}
async fn feed(
    State(api): State<Api>,
    Path(ws): Path<String>,
    Query(query): Query<Topics>,
    headers: HeaderMap,
) -> std::result::Result<Response, ApiError> {
    let caller = api.caller(&headers, &ws).await?;
    let topics = query
        .topics
        .split(',')
        .map(|topic| {
            let (kind, key) = topic
                .split_once(':')
                .ok_or_else(|| ActorError::new("bad_args", "Use type:key for each topic."))?;
            let actor = id(ws.clone(), kind.into(), key.into());
            crate::core::validate_actor_id(&actor)?;
            Ok(actor)
        })
        .collect::<Result<Vec<_>>>()?;
    if topics.is_empty() || topics.len() > 16 {
        return Err(ActorError::new("bad_args", "Choose between 1 and 16 topics.").into());
    }
    for actor in &topics {
        api.store.view(&caller, actor).await?;
    }
    let permit = api.stream_permit()?;
    let state = (
        api,
        headers,
        ws,
        topics,
        std::collections::BTreeMap::<ActorId, (u64, Value)>::new(),
        std::collections::VecDeque::<ActorId>::new(),
        tokio::time::Instant::now(),
        permit,
    );
    let updates = stream::unfold(
        state,
        |(api, headers, ws, topics, mut positions, mut pending, started, permit)| async move {
            if started.elapsed() > Duration::from_secs(300) {
                return None;
            }
            if pending.is_empty() {
                if !positions.is_empty() {
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
                pending.extend(topics.iter().cloned());
            }
            // Recheck authority for each emitted view, including views waiting in a
            // multiplexed feed. Never cache a Caller for the connection's lifetime.
            let caller = match api.caller(&headers, &ws).await {
                Ok(c) => c,
                Err(_) => return None,
            };
            let actor = pending.pop_front()?;
            let view = match api.store.view(&caller, &actor).await {
                Ok(v) => v,
                Err(_) => return None,
            };
            let current = (view.version, view.view.clone());
            let event = if positions.get(&actor) != Some(&current) {
                positions.insert(actor.clone(), current);
                SseEvent::default()
                    .event("view")
                    .id(format!(
                        "{}:{}:{}",
                        actor.actor_type, actor.key, view.version
                    ))
                    .json_data(json!({"actor":actor,"view":view}))
                    .ok()?
            } else {
                SseEvent::default().comment("keep-alive")
            };
            Some((
                Ok::<_, Infallible>(event),
                (
                    api, headers, ws, topics, positions, pending, started, permit,
                ),
            ))
        },
    );
    Ok(Sse::new(updates)
        .keep_alive(KeepAlive::default())
        .into_response())
}
