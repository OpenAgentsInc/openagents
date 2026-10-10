//! The stateful inference routes (`docs/inference/gateway.md`, section 3,
//! P2), over [`inference::session::Sessions`]:
//!
//! - `GET /v1/responses` upgraded to a WebSocket: the Open Responses
//!   WebSocket transport. Each `response.create` message runs one
//!   response, one at a time; events come back one JSON message each;
//!   failures come back as the spec's `error` envelope. The connection
//!   remembers its most recent response, so a `store: false` response can
//!   be continued with `previous_response_id` on the same socket and
//!   nowhere else. A connection lasts at most 60 minutes.
//! - `GET /v1/responses/{id}` and `DELETE /v1/responses/{id}`: the
//!   caller's stored response (`store: true`), read or deleted at once.
//!   Another tenant's id is `404`.
//! - `POST /v1/responses/compact`: compaction.
//!
//! `POST /v1/responses` itself (in [`crate::inference_routes`]) runs
//! through the same layer, so `store` and `previous_response_id` work
//! there too. Admission is the same as for the other inference routes.

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use axum::body::Bytes;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path as UrlPath, State};
use axum::http::{HeaderMap, HeaderValue};
use axum::response::{IntoResponse, Response};
use axum::routing::{MethodRouter, get, post};
use inference::error::{ApiError, ErrorType};
use inference::run::{Caller, Gateway};
use inference::seal::Sealer;
use inference::session::{CompactRequest, Local, Owner, Sessions};
use inference::store::{DirStore, is_response_id};
use inference::ws;
use serde_json::json;

use crate::inference_routes::{admit, cost_header, error, request_id};
use crate::serve::ServeState;

pub const COMPACT: &str = "/v1/responses/compact";
pub const STORED: &str = "/v1/responses/{id}";

pub fn routes() -> Vec<(&'static str, MethodRouter<Arc<ServeState>>)> {
    vec![(COMPACT, post(compact)), (STORED, get(read).delete(remove))]
}

/// The stateful layer for a gateway: the sealing key from the store
/// config's variable or `<registry>/inference/seal.key`, stored responses
/// under `<registry>/inference/responses` when `store` is configured, and
/// web search when an Exa key is set.
///
/// # Errors
///
/// A sentence when the sealing key cannot be read or made, or the store's
/// folder cannot be made.
pub fn sessions(
    config: &crate::config::Inference,
    gateway: Arc<Gateway>,
    registry: &Path,
) -> Result<Sessions, String> {
    let store_config = config.store.clone().unwrap_or_default();
    let key_file = registry.join("inference").join("seal.key");
    let sealer = Sealer::from_env_or_file(&store_config.key_env, &key_file)?;
    let mut sessions = Sessions::new(gateway, Arc::new(sealer));
    if let Some(store) = &config.store {
        let sealer = Sealer::from_env_or_file(&store.key_env, &key_file)?;
        let dir = registry.join("inference").join("responses");
        sessions = sessions.with_store(
            Arc::new(DirStore::open(&dir, sealer)?),
            store.retention_ms(),
        );
    }
    if let Some(search) = inference::hosted::Exa::from_env() {
        sessions = sessions.with_search(Arc::new(search));
    }
    Ok(sessions)
}

/// When expired stored responses were last swept, in Unix seconds.
static SWEPT: AtomicU64 = AtomicU64::new(0);

/// The stateful layer, sweeping expired stored responses at most hourly.
pub(crate) fn engine(state: &ServeState) -> Result<&Arc<Sessions>, ApiError> {
    let sessions = state
        .sessions
        .as_ref()
        .ok_or_else(|| ApiError::new(ErrorType::NotFound, "Inference is not set up here."))?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|span| span.as_secs())
        .unwrap_or_default();
    let last = SWEPT.load(Ordering::Relaxed);
    if sessions.stores()
        && now.saturating_sub(last) >= 3_600
        && SWEPT
            .compare_exchange(last, now, Ordering::Relaxed, Ordering::Relaxed)
            .is_ok()
    {
        let sessions = sessions.clone();
        tokio::task::spawn_blocking(move || sessions.sweep());
    }
    Ok(sessions)
}

/// Who owns what the caller stores: the workspace its key acts in
/// ([`Caller::owner`]), or its tenant when it reaches none (#11186).
/// Zero retention stays the tenant's setting.
pub(crate) fn owner(state: &ServeState, caller: &Caller) -> Owner {
    let tenant = caller.tenant.clone().unwrap_or_default();
    let zero_retention = state
        .config
        .inference
        .as_ref()
        .is_some_and(|config| config.zero_retention_tenants.contains(&tenant));
    Owner {
        tenant: caller.owner.clone().unwrap_or(tenant),
        zero_retention,
    }
}

fn not_found(id: &str) -> ApiError {
    let shown: String = id.chars().take(80).collect();
    ApiError {
        param: Some("id".into()),
        ..ApiError::new(
            ErrorType::NotFound,
            format!("No stored response with id '{shown}'."),
        )
    }
}

fn with_request_id(mut response: Response, id: &str) -> Response {
    if let Ok(value) = HeaderValue::from_str(id) {
        response.headers_mut().insert("x-request-id", value);
    }
    response
}

async fn read(
    State(state): State<Arc<ServeState>>,
    headers: HeaderMap,
    UrlPath(id): UrlPath<String>,
) -> Response {
    let caller = match admit(&state, &headers) {
        Ok(caller) => caller,
        Err(refusal) => return error(&refusal, &request_id()),
    };
    let rid = caller.request_id.clone();
    let sessions = match engine(&state) {
        Ok(sessions) => sessions.clone(),
        Err(refusal) => return error(&refusal, &rid),
    };
    let owner = owner(&state, &caller);
    let found = is_response_id(&id)
        .then(|| sessions.get(&owner, &id))
        .flatten();
    match found {
        Some(record) => with_request_id(axum::Json(&record.response).into_response(), &rid),
        None => error(&not_found(&id), &rid),
    }
}

async fn remove(
    State(state): State<Arc<ServeState>>,
    headers: HeaderMap,
    UrlPath(id): UrlPath<String>,
) -> Response {
    let caller = match admit(&state, &headers) {
        Ok(caller) => caller,
        Err(refusal) => return error(&refusal, &request_id()),
    };
    let rid = caller.request_id.clone();
    let sessions = match engine(&state) {
        Ok(sessions) => sessions.clone(),
        Err(refusal) => return error(&refusal, &rid),
    };
    let owner = owner(&state, &caller);
    if is_response_id(&id) && sessions.delete(&owner, &id) {
        with_request_id(
            axum::Json(json!({"id": id, "object": "response.deleted", "deleted": true}))
                .into_response(),
            &rid,
        )
    } else {
        error(&not_found(&id), &rid)
    }
}

async fn compact(
    State(state): State<Arc<ServeState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let caller = match admit(&state, &headers) {
        Ok(caller) => caller,
        Err(refusal) => return error(&refusal, &request_id()),
    };
    let rid = caller.request_id.clone();
    let request: CompactRequest = match serde_json::from_slice(&body) {
        Ok(request) => request,
        Err(why) => {
            return error(
                &ApiError::invalid_request(
                    "body",
                    format!("The request body isn't a valid compaction request: {why}"),
                ),
                &rid,
            );
        }
    };
    let sessions = match engine(&state) {
        Ok(sessions) => sessions.clone(),
        Err(refusal) => return error(&refusal, &rid),
    };
    let owner = owner(&state, &caller);
    match sessions.compact(request, &owner, &caller, None).await {
        Ok(compacted) => {
            let mut response = axum::Json(&compacted).into_response();
            cost_header(&mut response, compacted.openagents.as_ref());
            with_request_id(response, &rid)
        }
        Err(refusal) => error(&refusal, &rid),
    }
}

/// `GET /v1/responses` with an upgrade: the WebSocket transport.
pub(crate) async fn socket(
    State(state): State<Arc<ServeState>>,
    headers: HeaderMap,
    upgrade: WebSocketUpgrade,
) -> Response {
    let caller = match admit(&state, &headers) {
        Ok(caller) => caller,
        Err(refusal) => return error(&refusal, &request_id()),
    };
    let sessions = match engine(&state) {
        Ok(sessions) => sessions.clone(),
        Err(refusal) => return error(&refusal, &caller.request_id),
    };
    let owner = owner(&state, &caller);
    let ours = crate::inference_routes::wants_events(&headers);
    upgrade.on_upgrade(move |socket| converse(socket, sessions, caller, owner, ours))
}

async fn send_error(socket: &mut WebSocket, refusal: &ApiError) -> bool {
    socket
        .send(Message::Text(ws::error_envelope(refusal).into()))
        .await
        .is_ok()
}

/// One connection: `response.create` messages in, one at a time.
async fn converse(
    mut socket: WebSocket,
    sessions: Arc<Sessions>,
    caller: Caller,
    owner: Owner,
    ours: bool,
) {
    use futures_util::StreamExt;
    let local = Local::new();
    let opened = Instant::now();
    loop {
        let left = ws::CONNECTION_LIMIT.saturating_sub(opened.elapsed());
        let message = tokio::select! {
            message = socket.recv() => message,
            () = tokio::time::sleep(left) => {
                let _ = send_error(&mut socket, &ws::limit_reached()).await;
                let _ = socket.send(Message::Close(None)).await;
                return;
            }
        };
        let text = match message {
            None | Some(Err(_)) | Some(Ok(Message::Close(_))) => return,
            Some(Ok(Message::Ping(_) | Message::Pong(_))) => continue,
            Some(Ok(Message::Text(text))) => text.to_string(),
            Some(Ok(Message::Binary(bytes))) => String::from_utf8_lossy(&bytes).into_owned(),
        };
        if opened.elapsed() >= ws::CONNECTION_LIMIT {
            let _ = send_error(&mut socket, &ws::limit_reached()).await;
            let _ = socket.send(Message::Close(None)).await;
            return;
        }
        let request = match ws::parse_create(&text) {
            Ok(request) => request,
            Err(refusal) => {
                if send_error(&mut socket, &refusal).await {
                    continue;
                }
                return;
            }
        };
        let turn_caller = Caller {
            request_id: request_id(),
            ..caller.clone()
        };
        match sessions
            .create(request, &owner, &turn_caller, Some(&local))
            .await
        {
            Ok(turn) => {
                let mut events = crate::inference_routes::outgoing(turn.events, ours);
                while let Some(event) = events.next().await {
                    let Ok(text) = serde_json::to_string(&event) else {
                        continue;
                    };
                    if socket.send(Message::Text(text.into())).await.is_err() {
                        return;
                    }
                }
            }
            Err(refusal) => {
                if !send_error(&mut socket, &refusal).await {
                    return;
                }
            }
        }
    }
}
