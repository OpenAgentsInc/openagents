#![cfg_attr(
    test,
    allow(
        clippy::all,
        clippy::expect_used,
        clippy::panic,
        clippy::panic_in_result_fn,
        clippy::pedantic,
        clippy::unwrap_used
    )
)]

use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::sync::Arc;

use axum::body::{Body, to_bytes};
use axum::extract::FromRequestParts;
use axum::extract::Request;
use axum::extract::State;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::http::{HeaderMap, StatusCode, Uri, header, header::HOST};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{any, get};
use axum::{Json, Router};
use futures_util::{SinkExt, StreamExt, TryStreamExt};
use nostr::{Event, nip01::classify_kind, nip42::validate_auth_event};
use nostr_rs_relay_upstream as _;
use reqwest::Url;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::sync::{RwLock, mpsc};

mod managed_groups;
pub mod durable;

use managed_groups::{ManagedGroupsState, event_group_id};

const ENV_LISTEN_ADDR: &str = "NEXUS_RELAY_LISTEN_ADDR";
const ENV_MAX_STORED_EVENTS: &str = "NEXUS_RELAY_MAX_STORED_EVENTS";
const ENV_CONTROL_BASE_URL: &str = "NEXUS_RELAY_CONTROL_BASE_URL";
const DEFAULT_LISTEN_ADDR: &str = "127.0.0.1:42110";
const DEFAULT_MAX_STORED_EVENTS: usize = 5_000;
const DEFAULT_CONTROL_BASE_URL: &str = "http://127.0.0.1:42020";
const MAX_PROXY_REQUEST_BYTES: usize = 8 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct RelayServiceConfig {
    pub listen_addr: SocketAddr,
    pub max_stored_events: usize,
    pub control_base_url: String,
    pub relay_identity: RelayIdentity,
}

impl RelayServiceConfig {
    pub fn from_env() -> Result<Self, String> {
        let listen_addr = std::env::var(ENV_LISTEN_ADDR)
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| DEFAULT_LISTEN_ADDR.to_string())
            .parse::<SocketAddr>()
            .map_err(|error| format!("invalid {ENV_LISTEN_ADDR}: {error}"))?;
        let max_stored_events = std::env::var(ENV_MAX_STORED_EVENTS)
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .map_or(Ok(DEFAULT_MAX_STORED_EVENTS), |value| {
                value
                    .parse::<usize>()
                    .map_err(|error| format!("invalid {ENV_MAX_STORED_EVENTS}: {error}"))
            })?;
        if max_stored_events == 0 {
            return Err(format!("{ENV_MAX_STORED_EVENTS} must be greater than zero"));
        }
        let control_base_url = std::env::var(ENV_CONTROL_BASE_URL)
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| DEFAULT_CONTROL_BASE_URL.to_string());
        let relay_identity = load_relay_identity()?;

        Ok(Self {
            listen_addr,
            max_stored_events,
            control_base_url,
            relay_identity,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HealthResponse {
    pub ok: bool,
    pub service: String,
    pub stored_events: usize,
    pub connected_clients: usize,
}

#[derive(Clone)]
struct AppState {
    config: RelayServiceConfig,
    relay_identity: RelayIdentity,
    control_http_client: reqwest::Client,
    store: Arc<RwLock<RelayStore>>,
}

#[derive(Default)]
struct RelayStore {
    next_client_id: u64,
    events: Vec<Event>,
    clients: HashMap<u64, ConnectedClient>,
    managed_groups: ManagedGroupsState,
}

struct ConnectedClient {
    sender: mpsc::UnboundedSender<String>,
    subscriptions: HashMap<String, Vec<Value>>,
    auth_challenge: String,
    authenticated_pubkey: Option<String>,
    relay_url: String,
}

#[derive(Debug, Clone)]
pub struct RelayIdentity {
    pub public_key_hex: String,
    pub secret_key: [u8; 32],
}

pub fn build_router(config: RelayServiceConfig) -> Router {
    let state = AppState {
        relay_identity: config.relay_identity.clone(),
        control_http_client: reqwest::Client::new(),
        config,
        store: Arc::new(RwLock::new(RelayStore::default())),
    };
    Router::new()
        .route("/", any(relay_root))
        .route("/ws", any(relay_websocket))
        .route("/api/{*path}", any(proxy_control_request))
        .route("/v1/{*path}", any(proxy_control_request))
        .route("/healthz", get(healthz))
        .with_state(state)
}

pub async fn run_server(config: RelayServiceConfig) -> Result<(), anyhow::Error> {
    let listener = tokio::net::TcpListener::bind(config.listen_addr).await?;
    let local_addr = listener.local_addr()?;
    tracing::info!("nexus-relay listening on {}", local_addr);
    axum::serve(listener, build_router(config)).await?;
    Ok(())
}

async fn healthz(State(state): State<AppState>) -> impl IntoResponse {
    let store = state.store.read().await;
    Json(HealthResponse {
        ok: true,
        service: "nexus-relay".to_string(),
        stored_events: store.events.len(),
        connected_clients: store.clients.len(),
    })
}

async fn relay_root(State(state): State<AppState>, request: Request) -> Response {
    if is_websocket_upgrade_request(request.headers()) {
        return upgrade_websocket(state, request).await;
    }

    Html(render_homepage(&state)).into_response()
}

async fn relay_websocket(State(state): State<AppState>, request: Request) -> Response {
    upgrade_websocket(state, request).await
}

async fn upgrade_websocket(state: AppState, request: Request) -> Response {
    let relay_url = relay_url_from_headers(request.headers());
    let (mut parts, _body) = request.into_parts();
    match WebSocketUpgrade::from_request_parts(&mut parts, &state).await {
        Ok(ws) => ws
            .on_upgrade(move |socket| handle_socket(state, socket, relay_url))
            .into_response(),
        Err(rejection) => rejection.into_response(),
    }
}

async fn proxy_control_request(State(state): State<AppState>, request: Request) -> Response {
    let method = request.method().clone();
    let uri = request.uri().clone();
    let headers = request.headers().clone();
    let body_bytes = match to_bytes(request.into_body(), MAX_PROXY_REQUEST_BYTES).await {
        Ok(body) => body,
        Err(error) => {
            return (
                StatusCode::BAD_REQUEST,
                format!("failed to read proxied request body: {error}"),
            )
                .into_response();
        }
    };
    let target = match proxy_target_url(state.config.control_base_url.as_str(), &uri) {
        Ok(url) => url,
        Err(error) => return (StatusCode::BAD_GATEWAY, error).into_response(),
    };

    let mut upstream_request = state.control_http_client.request(method.clone(), target);
    upstream_request = copy_proxy_request_headers(upstream_request, &headers);
    if body_bytes.is_empty() {
        upstream_request = upstream_request.body(Vec::new());
    } else {
        upstream_request = upstream_request.body(body_bytes.to_vec());
    }

    let upstream = match upstream_request.send().await {
        Ok(response) => response,
        Err(error) => {
            return (
                StatusCode::BAD_GATEWAY,
                format!("failed to reach nexus-control upstream: {error}"),
            )
                .into_response();
        }
    };

    let status = upstream.status();
    let upstream_headers = upstream.headers().clone();
    let body_stream = upstream.bytes_stream().map_err(std::io::Error::other);
    let mut response = match Response::builder()
        .status(status)
        .body(Body::from_stream(body_stream))
    {
        Ok(response) => response,
        Err(error) => {
            let mut response = Response::new(Body::from(format!(
                "failed to build proxied response: {error}"
            )));
            *response.status_mut() = StatusCode::BAD_GATEWAY;
            return response;
        }
    };
    copy_proxy_response_headers(response.headers_mut(), &upstream_headers);
    response
}

async fn handle_socket(state: AppState, socket: WebSocket, relay_url: String) {
    let (mut sink, mut stream) = socket.split();
    let (outgoing_tx, mut outgoing_rx) = mpsc::unbounded_channel::<String>();
    let auth_challenge = generate_auth_challenge();
    let client_id = {
        let mut store = state.store.write().await;
        store.next_client_id = store.next_client_id.saturating_add(1);
        let client_id = store.next_client_id;
        store.clients.insert(
            client_id,
            ConnectedClient {
                sender: outgoing_tx,
                subscriptions: HashMap::new(),
                auth_challenge: auth_challenge.clone(),
                authenticated_pubkey: None,
                relay_url: relay_url.clone(),
            },
        );
        client_id
    };

    let send_task = tokio::spawn(async move {
        while let Some(payload) = outgoing_rx.recv().await {
            if sink.send(Message::Text(payload.into())).await.is_err() {
                break;
            }
        }
    });
    let _ = send_auth_challenge(&state, client_id).await;

    while let Some(frame) = stream.next().await {
        let Ok(frame) = frame else {
            break;
        };
        match frame {
            Message::Text(text) => {
                if let Err(error) = handle_text_frame(&state, client_id, text.as_ref()).await {
                    let _ = send_notice(&state, client_id, error).await;
                }
            }
            Message::Close(_) => break,
            Message::Ping(_) | Message::Pong(_) | Message::Binary(_) => {}
        }
    }

    {
        let mut store = state.store.write().await;
        store.clients.remove(&client_id);
    }
    send_task.abort();
}

async fn handle_text_frame(state: &AppState, client_id: u64, raw: &str) -> Result<(), String> {
    let frame: Value =
        serde_json::from_str(raw).map_err(|error| format!("invalid_json:{error}"))?;
    let array = frame
        .as_array()
        .ok_or_else(|| "invalid_frame:expected_array".to_string())?;
    let Some(kind) = array.first().and_then(Value::as_str) else {
        return Err("invalid_frame:missing_kind".to_string());
    };

    match kind {
        "REQ" => handle_req_frame(state, client_id, array.as_slice()).await,
        "EVENT" => handle_event_frame(state, client_id, array.as_slice()).await,
        "CLOSE" => handle_close_frame(state, client_id, array.as_slice()).await,
        "AUTH" => handle_auth_frame(state, client_id, array.as_slice()).await,
        _ => send_notice(state, client_id, format!("unsupported:{kind}")).await,
    }
}

async fn handle_req_frame(state: &AppState, client_id: u64, frame: &[Value]) -> Result<(), String> {
    if frame.len() < 2 {
        return Err("invalid_req:missing_subscription_id".to_string());
    }
    let subscription_id = frame[1]
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "invalid_req:missing_subscription_id".to_string())?
        .to_string();
    let filters = frame.iter().skip(2).cloned().collect::<Vec<_>>();
    let access_message = {
        let store = state.store.read().await;
        let client = store
            .clients
            .get(&client_id)
            .ok_or_else(|| "client_not_found".to_string())?;
        store
            .managed_groups
            .subscription_auth_message(filters.as_slice(), client.authenticated_pubkey.as_deref())
    };
    if let Some(message) = access_message {
        if nostr::nip42::is_auth_required_error(message.as_str()) {
            send_auth_challenge(state, client_id).await?;
        }
        return send_notice(state, client_id, message).await;
    }
    {
        let mut store = state.store.write().await;
        let Some(client) = store.clients.get_mut(&client_id) else {
            return Err("client_not_found".to_string());
        };
        client
            .subscriptions
            .insert(subscription_id.clone(), filters.clone());
    }

    let matching_events = {
        let store = state.store.read().await;
        let client = store
            .clients
            .get(&client_id)
            .ok_or_else(|| "client_not_found".to_string())?;
        snapshot_events(
            store.events.as_slice(),
            filters.as_slice(),
            &store.managed_groups,
            client.authenticated_pubkey.as_deref(),
        )
    };
    for event in matching_events {
        let payload = json!(["EVENT", subscription_id, event]).to_string();
        send_text(state, client_id, payload).await?;
    }
    send_text(
        state,
        client_id,
        json!(["EOSE", subscription_id]).to_string(),
    )
    .await
}

async fn handle_event_frame(
    state: &AppState,
    client_id: u64,
    frame: &[Value],
) -> Result<(), String> {
    if frame.len() < 2 {
        return Err("invalid_event:missing_event".to_string());
    }
    let event: Event = serde_json::from_value(frame[1].clone())
        .map_err(|error| format!("invalid_event:{error}"))?;

    let (accepted, message, deliveries) = {
        let mut store = state.store.write().await;
        if store.events.iter().any(|existing| existing.id == event.id) {
            (false, "duplicate".to_string(), Vec::new())
        } else {
            let authenticated_pubkey = store
                .clients
                .get(&client_id)
                .and_then(|client| client.authenticated_pubkey.clone());
            if let Some(message) = store
                .managed_groups
                .publish_auth_message(&event, authenticated_pubkey.as_deref())
            {
                let needs_auth = nostr::nip42::is_auth_required_error(message.as_str());
                drop(store);
                if needs_auth {
                    send_auth_challenge(state, client_id).await?;
                }
                send_text(
                    state,
                    client_id,
                    json!(["OK", event.id, false, message]).to_string(),
                )
                .await?;
                return Ok(());
            }
            let accepted_events = match store
                .managed_groups
                .apply_event(&state.relay_identity, &event)
            {
                Ok(Some(outcome)) => {
                    apply_managed_group_prunes(&mut store.events, &outcome);
                    outcome.accepted_events
                }
                Ok(None) => vec![event.clone()],
                Err(error) => {
                    let message = if matches!(
                        error.as_str(),
                        "managed_group:membership_required" | "managed_group:admin_required"
                    ) {
                        nostr::nip42::create_restricted_message(
                            error.trim_start_matches("managed_group:"),
                        )
                    } else {
                        error
                    };
                    drop(store);
                    send_text(
                        state,
                        client_id,
                        json!(["OK", event.id, false, message]).to_string(),
                    )
                    .await?;
                    return Ok(());
                }
            };

            for accepted_event in accepted_events.iter().cloned() {
                upsert_event(
                    &mut store.events,
                    accepted_event,
                    state.config.max_stored_events,
                );
            }

            let mut deliveries = Vec::new();
            for (target_client_id, client) in &store.clients {
                for accepted_event in &accepted_events {
                    if !store
                        .managed_groups
                        .can_read_event(accepted_event, client.authenticated_pubkey.as_deref())
                    {
                        continue;
                    }
                    for (subscription_id, filters) in &client.subscriptions {
                        if event_matches_filters(accepted_event, filters.as_slice()) {
                            deliveries.push((
                                *target_client_id,
                                subscription_id.clone(),
                                accepted_event.clone(),
                            ));
                        }
                    }
                }
            }
            (true, "accepted".to_string(), deliveries)
        }
    };

    for (target_client_id, subscription_id, delivery_event) in deliveries {
        let payload = json!(["EVENT", subscription_id, delivery_event]).to_string();
        send_text(state, target_client_id, payload).await?;
    }

    send_text(
        state,
        client_id,
        json!(["OK", event.id, accepted, message]).to_string(),
    )
    .await
}

async fn handle_auth_frame(
    state: &AppState,
    client_id: u64,
    frame: &[Value],
) -> Result<(), String> {
    if frame.len() < 2 {
        return Err("invalid_auth:missing_event".to_string());
    }
    let auth_event: Event = serde_json::from_value(frame[1].clone())
        .map_err(|error| format!("invalid_auth:{error}"))?;

    let (challenge, relay_url) = {
        let store = state.store.read().await;
        let client = store
            .clients
            .get(&client_id)
            .ok_or_else(|| "client_not_found".to_string())?;
        (client.auth_challenge.clone(), client.relay_url.clone())
    };
    let is_valid = nostr::verify_event(&auth_event)
        .map_err(|error| format!("invalid_auth_signature:{error}"))?;
    if !is_valid {
        return send_notice(
            state,
            client_id,
            nostr::nip42::create_restricted_message("invalid auth signature"),
        )
        .await;
    }
    if let Err(error) =
        validate_auth_event(&auth_event, relay_url.as_str(), challenge.as_str(), None)
    {
        return send_notice(
            state,
            client_id,
            nostr::nip42::create_restricted_message(&error.to_string()),
        )
        .await;
    }

    let mut store = state.store.write().await;
    let client = store
        .clients
        .get_mut(&client_id)
        .ok_or_else(|| "client_not_found".to_string())?;
    client.authenticated_pubkey = Some(auth_event.pubkey);
    Ok(())
}

async fn handle_close_frame(
    state: &AppState,
    client_id: u64,
    frame: &[Value],
) -> Result<(), String> {
    if frame.len() < 2 {
        return Err("invalid_close:missing_subscription_id".to_string());
    }
    let subscription_id = frame[1]
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "invalid_close:missing_subscription_id".to_string())?;
    let mut store = state.store.write().await;
    let Some(client) = store.clients.get_mut(&client_id) else {
        return Err("client_not_found".to_string());
    };
    client.subscriptions.remove(subscription_id);
    Ok(())
}

async fn send_text(state: &AppState, client_id: u64, payload: String) -> Result<(), String> {
    let sender = {
        let store = state.store.read().await;
        store
            .clients
            .get(&client_id)
            .map(|client| client.sender.clone())
    }
    .ok_or_else(|| "client_not_found".to_string())?;

    sender
        .send(payload)
        .map_err(|_| "client_send_failed".to_string())
}

async fn send_notice(state: &AppState, client_id: u64, message: String) -> Result<(), String> {
    send_text(state, client_id, json!(["NOTICE", message]).to_string()).await
}

async fn send_auth_challenge(state: &AppState, client_id: u64) -> Result<(), String> {
    let challenge = {
        let store = state.store.read().await;
        store
            .clients
            .get(&client_id)
            .map(|client| client.auth_challenge.clone())
    }
    .ok_or_else(|| "client_not_found".to_string())?;

    send_text(state, client_id, json!(["AUTH", challenge]).to_string()).await
}

fn snapshot_events(
    events: &[Event],
    filters: &[Value],
    managed_groups: &ManagedGroupsState,
    authenticated_pubkey: Option<&str>,
) -> Vec<Event> {
    let mut delivered_ids = HashSet::<String>::new();
    let mut matched = Vec::<Event>::new();

    for filter in filters {
        let limit = filter_limit(filter).unwrap_or(events.len());
        let filter_matches = events
            .iter()
            .filter(|event| {
                event_matches_filter(event, filter)
                    && managed_groups.can_read_event(event, authenticated_pubkey)
            })
            .cloned()
            .collect::<Vec<_>>();
        let start = filter_matches.len().saturating_sub(limit);
        for event in filter_matches.into_iter().skip(start) {
            if delivered_ids.insert(event.id.clone()) {
                matched.push(event);
            }
        }
    }

    matched
}

fn event_matches_filters(event: &Event, filters: &[Value]) -> bool {
    if filters.is_empty() {
        return true;
    }
    filters
        .iter()
        .any(|filter| event_matches_filter(event, filter))
}

fn event_matches_filter(event: &Event, filter: &Value) -> bool {
    let Some(object) = filter.as_object() else {
        return false;
    };

    if let Some(kinds) = object.get("kinds").and_then(Value::as_array)
        && !kinds
            .iter()
            .filter_map(Value::as_u64)
            .any(|kind| kind == event.kind as u64)
    {
        return false;
    }
    if let Some(authors) = object.get("authors").and_then(Value::as_array)
        && !authors
            .iter()
            .filter_map(Value::as_str)
            .any(|author| event.pubkey.starts_with(author))
    {
        return false;
    }
    if let Some(ids) = object.get("ids").and_then(Value::as_array)
        && !ids
            .iter()
            .filter_map(Value::as_str)
            .any(|candidate| event.id.starts_with(candidate))
    {
        return false;
    }
    if let Some(since) = object.get("since").and_then(Value::as_u64)
        && event.created_at < since
    {
        return false;
    }
    if let Some(until) = object.get("until").and_then(Value::as_u64)
        && event.created_at > until
    {
        return false;
    }
    for (key, tag_values) in object
        .iter()
        .filter(|(key, value)| key.starts_with('#') && value.is_array())
    {
        let Some(tag_values) = tag_values.as_array() else {
            return false;
        };
        if !event_has_tag(event, &key[1..], tag_values.as_slice()) {
            return false;
        }
    }

    true
}

fn event_has_tag(event: &Event, name: &str, values: &[Value]) -> bool {
    let accepted_values = values.iter().filter_map(Value::as_str).collect::<Vec<_>>();
    event.tags.iter().any(|tag| {
        tag.first().is_some_and(|candidate| candidate == name)
            && tag
                .get(1)
                .is_some_and(|candidate| accepted_values.contains(&candidate.as_str()))
    })
}

fn filter_limit(filter: &Value) -> Option<usize> {
    filter
        .get("limit")
        .and_then(Value::as_u64)
        .map(|value| value as usize)
}

fn apply_managed_group_prunes(
    events: &mut Vec<Event>,
    outcome: &managed_groups::ManagedGroupOutcome,
) {
    if !outcome.removed_event_ids.is_empty() {
        events.retain(|event| !outcome.removed_event_ids.iter().any(|id| id == &event.id));
    }
    if let Some(group_id) = &outcome.pruned_group_id {
        events.retain(|event| event_group_id(event) != Some(group_id.as_str()));
    }
}

fn upsert_event(events: &mut Vec<Event>, event: Event, max_stored_events: usize) {
    match classify_kind(event.kind) {
        nostr::KindClassification::Replaceable => {
            if let Some(index) = events
                .iter()
                .position(|existing| existing.kind == event.kind && existing.pubkey == event.pubkey)
            {
                events.remove(index);
            }
        }
        nostr::KindClassification::Addressable => {
            let event_d = tag_value(event.tags.as_slice(), "d");
            if let Some(index) = events.iter().position(|existing| {
                existing.kind == event.kind
                    && existing.pubkey == event.pubkey
                    && tag_value(existing.tags.as_slice(), "d") == event_d
            }) {
                events.remove(index);
            }
        }
        _ => {}
    }

    events.push(event);
    if events.len() > max_stored_events {
        let overflow = events.len().saturating_sub(max_stored_events);
        events.drain(0..overflow);
    }
}

fn tag_value<'a>(tags: &'a [Vec<String>], name: &str) -> Option<&'a str> {
    tags.iter()
        .find(|tag| tag.first().is_some_and(|candidate| candidate == name))
        .and_then(|tag| tag.get(1))
        .map(String::as_str)
}

fn relay_url_from_headers(headers: &HeaderMap) -> String {
    let host = headers
        .get(HOST)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("127.0.0.1");
    let scheme = headers
        .get("x-forwarded-proto")
        .and_then(|value| value.to_str().ok())
        .map(|value| {
            if value.eq_ignore_ascii_case("https") {
                "wss"
            } else {
                "ws"
            }
        })
        .unwrap_or("ws");

    format!("{scheme}://{host}/")
}

fn render_homepage(state: &AppState) -> String {
    format!(
        "<!DOCTYPE html><html><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><title>Nexus Relay</title><style>body{{margin:0;font-family:ui-monospace,SFMono-Regular,Menlo,monospace;background:#030712;color:#d1d5db}}main{{max-width:880px;margin:0 auto;padding:56px 24px}}h1{{margin:0 0 12px;font-size:36px;color:#f9fafb}}p{{line-height:1.6;color:#9ca3af}}code{{background:#111827;padding:2px 6px;border-radius:4px;color:#93c5fd}}.panel{{margin-top:24px;padding:18px 20px;border:1px solid #1f2937;border-radius:12px;background:#0b1220}}.label{{display:block;font-size:12px;letter-spacing:.08em;text-transform:uppercase;color:#6b7280;margin-bottom:6px}}</style></head><body><main><h1>OpenAgents Nexus Relay</h1><p>This host runs the current-repo Nexus relay surface and forwards hosted authority API traffic to <code>{control_base}</code>.</p><div class=\"panel\"><span class=\"label\">Relay websocket</span><code>/</code> and <code>/ws</code></div><div class=\"panel\"><span class=\"label\">Hosted API</span><code>/api/*</code> and <code>/v1/*</code> are forwarded to Nexus Control</div><div class=\"panel\"><span class=\"label\">Relay pubkey</span><code>{pubkey}</code></div></main></body></html>",
        control_base = state.config.control_base_url,
        pubkey = state.relay_identity.public_key_hex
    )
}

fn is_websocket_upgrade_request(headers: &HeaderMap) -> bool {
    header_contains(headers, header::CONNECTION, "upgrade")
        && header_eq(headers, header::UPGRADE, "websocket")
        && header_eq(headers, header::SEC_WEBSOCKET_VERSION, "13")
}

fn header_eq(headers: &HeaderMap, key: header::HeaderName, value: &'static str) -> bool {
    headers
        .get(key)
        .is_some_and(|header| header.as_bytes().eq_ignore_ascii_case(value.as_bytes()))
}

fn header_contains(headers: &HeaderMap, key: header::HeaderName, value: &'static str) -> bool {
    let Some(header) = headers.get(key) else {
        return false;
    };
    std::str::from_utf8(header.as_bytes())
        .map(|header| header.to_ascii_lowercase().contains(value))
        .unwrap_or(false)
}

fn proxy_target_url(base_url: &str, uri: &Uri) -> Result<Url, String> {
    let mut url =
        Url::parse(base_url).map_err(|error| format!("invalid control base url: {error}"))?;
    url.set_path(uri.path());
    url.set_query(uri.query());
    url.set_fragment(None);
    Ok(url)
}

fn copy_proxy_request_headers(
    mut builder: reqwest::RequestBuilder,
    headers: &HeaderMap,
) -> reqwest::RequestBuilder {
    for (name, value) in headers {
        if should_skip_proxy_request_header(name) {
            continue;
        }
        builder = builder.header(name, value);
    }
    builder
}

fn copy_proxy_response_headers(target: &mut HeaderMap, headers: &HeaderMap) {
    for (name, value) in headers {
        if should_skip_proxy_response_header(name) {
            continue;
        }
        target.insert(name, value.clone());
    }
}

fn should_skip_proxy_request_header(name: &header::HeaderName) -> bool {
    matches!(
        *name,
        header::HOST
            | header::CONNECTION
            | header::UPGRADE
            | header::CONTENT_LENGTH
            | header::TRANSFER_ENCODING
    )
}

fn should_skip_proxy_response_header(name: &header::HeaderName) -> bool {
    matches!(
        *name,
        header::CONNECTION | header::CONTENT_LENGTH | header::TRANSFER_ENCODING
    )
}

fn generate_auth_challenge() -> String {
    hex::encode(nostr::generate_secret_key())
}

fn load_relay_identity() -> Result<RelayIdentity, String> {
    let identity = nostr::load_or_create_identity()
        .map_err(|error| format!("failed to load relay identity: {error}"))?;
    let secret_key_bytes = hex::decode(identity.private_key_hex)
        .map_err(|error| format!("failed to decode relay private key: {error}"))?;
    let secret_key: [u8; 32] = secret_key_bytes
        .as_slice()
        .try_into()
        .map_err(|_| "relay private key must be 32 bytes".to_string())?;

    Ok(RelayIdentity {
        public_key_hex: identity.public_key_hex,
        secret_key,
    })
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use axum::http::{HeaderMap, StatusCode, header};
    use axum::routing::{get, post};
    use axum::{Json, Router};
    use futures_util::{SinkExt, StreamExt};
    use nostr::{
        Event, EventTemplate, ModerationAction, ModerationEvent, finalize_event, get_public_key_hex,
    };
    use nostr_client::{RelayAuthIdentity, RelayConfig, RelayConnection, RelayMessage};
    use serde_json::{Value, json};
    use tokio::time::{Duration, timeout};
    use tokio_tungstenite::{connect_async, tungstenite::Message as WsMessage};

    use super::{RelayServiceConfig, build_router, event_matches_filter, run_server};

    fn relay_config() -> RelayServiceConfig {
        let secret_key = [7u8; 32];
        RelayServiceConfig {
            listen_addr: "127.0.0.1:0".parse().expect("valid listen addr"),
            max_stored_events: 128,
            control_base_url: "http://127.0.0.1:42020".to_string(),
            relay_identity: super::RelayIdentity {
                public_key_hex: get_public_key_hex(&secret_key).expect("derive relay pubkey"),
                secret_key,
            },
        }
    }

    fn sign_template(template: EventTemplate, secret_key: &[u8; 32]) -> Event {
        finalize_event(&template, secret_key).expect("sign test event")
    }

    fn sign_moderation(event: ModerationEvent, secret_key: &[u8; 32], pubkey: &str) -> Event {
        let unsigned = event
            .to_unsigned_event(pubkey.to_string())
            .expect("build moderation event");
        sign_template(
            EventTemplate {
                created_at: unsigned.created_at,
                kind: unsigned.kind,
                tags: unsigned.tags,
                content: unsigned.content,
            },
            secret_key,
        )
    }

    async fn drain_initial_auth(
        socket: &mut tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
    ) -> Result<()> {
        let frame = timeout(Duration::from_secs(2), socket.next()).await;
        let frame = frame
            .ok()
            .and_then(|value| value)
            .ok_or_else(|| anyhow::anyhow!("expected initial auth challenge"))??;
        let text = match frame {
            WsMessage::Text(text) => text.to_string(),
            other => {
                return Err(anyhow::anyhow!(
                    "unexpected auth challenge frame: {other:?}"
                ));
            }
        };
        assert!(text.contains("\"AUTH\""));
        Ok(())
    }

    fn sample_event() -> Event {
        Event {
            id: "evt-1".to_string(),
            pubkey: "npub1buyer".to_string(),
            created_at: 1_700_000_000,
            kind: 5050,
            tags: vec![vec!["e".to_string(), "req-1".to_string()]],
            content: "Generate text".to_string(),
            sig: "11".repeat(64),
        }
    }

    #[test]
    fn event_filter_matches_kinds_and_tags() {
        let event = sample_event();
        assert!(event_matches_filter(
            &event,
            &json!({"kinds":[5050], "#e":["req-1"]})
        ));
        assert!(!event_matches_filter(
            &event,
            &json!({"kinds":[5051], "#e":["req-1"]})
        ));
        assert!(!event_matches_filter(
            &event,
            &json!({"kinds":[5050], "#e":["req-2"]})
        ));
    }

    #[tokio::test]
    async fn relay_root_serves_html_when_not_upgrading() -> Result<()> {
        let config = relay_config();
        let listener = tokio::net::TcpListener::bind(config.listen_addr).await?;
        let addr = listener.local_addr()?;
        let server = tokio::spawn(async move {
            axum::serve(listener, build_router(config))
                .await
                .map_err(anyhow::Error::from)
        });

        let response = reqwest::get(format!("http://{addr}/")).await?;
        assert_eq!(response.status(), StatusCode::OK);
        let body = response.text().await?;
        assert!(body.contains("OpenAgents Nexus Relay"));
        assert!(body.contains("/api/*"));

        server.abort();
        Ok(())
    }

    #[tokio::test]
    async fn relay_proxies_control_api_requests() -> Result<()> {
        let control_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let control_addr = control_listener.local_addr()?;
        let control_server = tokio::spawn(async move {
            let app = Router::new()
                .route(
                    "/api/sync/token",
                    post(
                        |headers: HeaderMap, Json(payload): Json<Value>| async move {
                            let auth = headers
                                .get(header::AUTHORIZATION)
                                .and_then(|value| value.to_str().ok())
                                .unwrap_or_default()
                                .to_string();
                            (
                                StatusCode::CREATED,
                                Json(json!({
                                    "authorization": auth,
                                    "payload": payload,
                                })),
                            )
                        },
                    ),
                )
                .route(
                    "/api/stats",
                    get(|| async { Json(json!({"ok": true, "service": "nexus-control"})) }),
                );
            axum::serve(control_listener, app)
                .await
                .map_err(anyhow::Error::from)
        });

        let mut config = relay_config();
        config.control_base_url = format!("http://{control_addr}");
        let relay_listener = tokio::net::TcpListener::bind(config.listen_addr).await?;
        let relay_addr = relay_listener.local_addr()?;
        let relay_server = tokio::spawn(async move {
            axum::serve(relay_listener, build_router(config))
                .await
                .map_err(anyhow::Error::from)
        });

        let client = reqwest::Client::new();
        let response = client
            .post(format!("http://{relay_addr}/api/sync/token"))
            .header(header::AUTHORIZATION, "Bearer audit-token")
            .json(&json!({"scope":"sync.subscribe"}))
            .send()
            .await?;
        assert_eq!(response.status(), StatusCode::CREATED);
        let body: Value = response.json().await?;
        assert_eq!(body["authorization"], "Bearer audit-token");
        assert_eq!(body["payload"]["scope"], "sync.subscribe");

        let stats = client
            .get(format!("http://{relay_addr}/api/stats"))
            .send()
            .await?;
        assert_eq!(stats.status(), StatusCode::OK);
        let stats_body: Value = stats.json().await?;
        assert_eq!(stats_body["service"], "nexus-control");

        relay_server.abort();
        control_server.abort();
        Ok(())
    }

    #[tokio::test]
    async fn relay_replays_stored_events_and_broadcasts_new_events() -> Result<()> {
        let config = relay_config();
        let listener = tokio::net::TcpListener::bind(config.listen_addr).await?;
        let addr = listener.local_addr()?;
        let server = tokio::spawn(async move {
            axum::serve(listener, build_router(config))
                .await
                .map_err(anyhow::Error::from)
        });

        let relay_url = format!("ws://{addr}/");
        let (mut publisher, _) = connect_async(relay_url.as_str()).await?;
        drain_initial_auth(&mut publisher).await?;
        let event = sample_event();
        publisher
            .send(WsMessage::Text(json!(["EVENT", event]).to_string().into()))
            .await?;
        let ok_frame = timeout(Duration::from_secs(2), publisher.next()).await;
        let ok_frame = ok_frame
            .ok()
            .and_then(|value| value)
            .ok_or_else(|| anyhow::anyhow!("expected OK frame after publishing stored event"))??;
        let ok_text = match ok_frame {
            WsMessage::Text(text) => text.to_string(),
            other => return Err(anyhow::anyhow!("unexpected frame after publish: {other:?}")),
        };
        assert!(ok_text.contains("\"OK\""));

        let (mut subscriber, _) = connect_async(relay_url.as_str()).await?;
        drain_initial_auth(&mut subscriber).await?;
        subscriber
            .send(WsMessage::Text(
                json!(["REQ", "sub-1", {"kinds":[5050], "limit": 10}])
                    .to_string()
                    .into(),
            ))
            .await?;

        let first = timeout(Duration::from_secs(2), subscriber.next()).await;
        let first = first
            .ok()
            .and_then(|value| value)
            .ok_or_else(|| anyhow::anyhow!("expected initial snapshot event"))??;
        let first_text = match first {
            WsMessage::Text(text) => text.to_string(),
            other => {
                return Err(anyhow::anyhow!(
                    "unexpected first snapshot frame: {other:?}"
                ));
            }
        };
        assert!(first_text.contains("\"EVENT\""));
        let second = timeout(Duration::from_secs(2), subscriber.next()).await;
        let second = second
            .ok()
            .and_then(|value| value)
            .ok_or_else(|| anyhow::anyhow!("expected EOSE frame"))??;
        let second_text = match second {
            WsMessage::Text(text) => text.to_string(),
            other => {
                return Err(anyhow::anyhow!(
                    "unexpected second snapshot frame: {other:?}"
                ));
            }
        };
        assert!(second_text.contains("\"EOSE\""));

        let new_event = Event {
            id: "evt-2".to_string(),
            ..sample_event()
        };
        publisher
            .send(WsMessage::Text(
                json!(["EVENT", new_event]).to_string().into(),
            ))
            .await?;
        let _ = timeout(Duration::from_secs(2), publisher.next()).await;
        let broadcast = timeout(Duration::from_secs(2), subscriber.next()).await;
        let broadcast = broadcast
            .ok()
            .and_then(|value| value)
            .ok_or_else(|| anyhow::anyhow!("expected live broadcast event"))??;
        let broadcast_text = match broadcast {
            WsMessage::Text(text) => text.to_string(),
            other => return Err(anyhow::anyhow!("unexpected broadcast frame: {other:?}")),
        };
        assert!(broadcast_text.contains("\"evt-2\""));

        server.abort();
        Ok(())
    }

    #[tokio::test]
    async fn run_server_binds_real_listener() -> Result<()> {
        let mut config = relay_config();
        config.max_stored_events = 32;
        let server = tokio::spawn(async move { run_server(config).await });
        tokio::time::sleep(Duration::from_millis(25)).await;
        server.abort();
        Ok(())
    }

    #[tokio::test]
    async fn relay_enforces_managed_group_membership_and_replays_snapshots() -> Result<()> {
        let config = relay_config();
        let listener = tokio::net::TcpListener::bind(config.listen_addr).await?;
        let addr = listener.local_addr()?;
        let server = tokio::spawn(async move {
            axum::serve(listener, build_router(config))
                .await
                .map_err(anyhow::Error::from)
        });

        let relay_url = format!("ws://{addr}/");
        let admin_secret = [1u8; 32];
        let admin_pubkey = get_public_key_hex(&admin_secret)?;
        let member_secret = [2u8; 32];
        let mut admin = connect_async(relay_url.as_str()).await?.0;
        let mut observer = connect_async(relay_url.as_str()).await?.0;
        drain_initial_auth(&mut admin).await?;
        drain_initial_auth(&mut observer).await?;

        observer
            .send(WsMessage::Text(
                json!(["REQ", "group-meta", {"kinds":[39000,39001,39002,39003], "#d":["oa-main"]}])
                    .to_string()
                    .into(),
            ))
            .await?;

        let create_group = sign_moderation(
            ModerationEvent::new("oa-main", ModerationAction::CreateGroup, 10)?,
            &admin_secret,
            admin_pubkey.as_str(),
        );
        admin
            .send(WsMessage::Text(
                json!(["EVENT", create_group]).to_string().into(),
            ))
            .await?;

        let ok_after_create = timeout(Duration::from_secs(2), admin.next()).await;
        let ok_after_create = ok_after_create
            .ok()
            .and_then(|value| value)
            .ok_or_else(|| anyhow::anyhow!("expected OK after create group"))??;
        let ok_after_create = match ok_after_create {
            WsMessage::Text(text) => text.to_string(),
            other => return Err(anyhow::anyhow!("unexpected create-group frame: {other:?}")),
        };
        assert!(ok_after_create.contains("\"accepted\""));

        let mut snapshot_kinds = Vec::new();
        while snapshot_kinds.len() < 4 {
            let frame = timeout(Duration::from_secs(2), observer.next()).await;
            let frame = frame
                .ok()
                .and_then(|value| value)
                .ok_or_else(|| anyhow::anyhow!("expected snapshot event"))??;
            let text = match frame {
                WsMessage::Text(text) => text.to_string(),
                other => return Err(anyhow::anyhow!("unexpected snapshot frame: {other:?}")),
            };
            if text.contains("\"EVENT\"") {
                if text.contains("\"kind\":39000") {
                    snapshot_kinds.push(39000);
                } else if text.contains("\"kind\":39001") {
                    snapshot_kinds.push(39001);
                } else if text.contains("\"kind\":39002") {
                    snapshot_kinds.push(39002);
                } else if text.contains("\"kind\":39003") {
                    snapshot_kinds.push(39003);
                }
            }
        }
        assert_eq!(snapshot_kinds.len(), 4);

        let outsider_message = sign_template(
            EventTemplate {
                created_at: 11,
                kind: 42,
                tags: vec![
                    vec!["h".to_string(), "oa-main".to_string()],
                    vec![
                        "e".to_string(),
                        "a".repeat(64),
                        relay_url.clone(),
                        "root".to_string(),
                    ],
                ],
                content: "blocked".to_string(),
            },
            &member_secret,
        );
        admin
            .send(WsMessage::Text(
                json!(["EVENT", outsider_message]).to_string().into(),
            ))
            .await?;
        let blocked = timeout(Duration::from_secs(2), admin.next()).await;
        let blocked = blocked
            .ok()
            .and_then(|value| value)
            .ok_or_else(|| anyhow::anyhow!("expected membership rejection"))??;
        let blocked_text = match blocked {
            WsMessage::Text(text) => text.to_string(),
            other => return Err(anyhow::anyhow!("unexpected membership frame: {other:?}")),
        };
        assert!(blocked_text.contains("membership_required"));

        server.abort();
        Ok(())
    }

    #[tokio::test]
    async fn relay_requires_auth_for_restricted_group_reads_and_writes() -> Result<()> {
        let config = relay_config();
        let listener = tokio::net::TcpListener::bind(config.listen_addr).await?;
        let addr = listener.local_addr()?;
        let server = tokio::spawn(async move {
            axum::serve(listener, build_router(config))
                .await
                .map_err(anyhow::Error::from)
        });

        let relay_url = format!("ws://{addr}/");
        let admin_secret = [9u8; 32];
        let admin_pubkey = get_public_key_hex(&admin_secret)?;

        let mut admin_socket = connect_async(relay_url.as_str()).await?.0;
        drain_initial_auth(&mut admin_socket).await?;

        let create_group = sign_moderation(
            ModerationEvent::new("oa-main", ModerationAction::CreateGroup, 10)?,
            &admin_secret,
            admin_pubkey.as_str(),
        );
        admin_socket
            .send(WsMessage::Text(
                json!(["EVENT", create_group]).to_string().into(),
            ))
            .await?;
        let _ = timeout(Duration::from_secs(2), admin_socket.next()).await;

        let restrict_group = sign_moderation(
            ModerationEvent::new(
                "oa-main",
                ModerationAction::EditMetadata {
                    changes: vec![vec!["restricted".to_string()]],
                },
                11,
            )?,
            &admin_secret,
            admin_pubkey.as_str(),
        );
        admin_socket
            .send(WsMessage::Text(
                json!(["EVENT", restrict_group]).to_string().into(),
            ))
            .await?;
        let _ = timeout(Duration::from_secs(2), admin_socket.next()).await;

        let mut raw_client = connect_async(relay_url.as_str()).await?.0;
        drain_initial_auth(&mut raw_client).await?;
        raw_client
            .send(WsMessage::Text(
                json!(["REQ", "restricted-meta", {"kinds":[39000], "#d":["oa-main"]}])
                    .to_string()
                    .into(),
            ))
            .await?;
        let mut restricted_notice = String::new();
        for _ in 0..2 {
            let frame = timeout(Duration::from_secs(2), raw_client.next()).await;
            let frame = frame
                .ok()
                .and_then(|value| value)
                .ok_or_else(|| anyhow::anyhow!("expected auth-required flow"))??;
            let text = match frame {
                WsMessage::Text(text) => text.to_string(),
                other => return Err(anyhow::anyhow!("unexpected auth-required frame: {other:?}")),
            };
            if text.contains(nostr::nip42::AUTH_REQUIRED_PREFIX) {
                restricted_notice = text;
                break;
            }
        }
        assert!(restricted_notice.contains(nostr::nip42::AUTH_REQUIRED_PREFIX));

        let relay_connection = RelayConnection::with_config(
            relay_url.as_str(),
            RelayConfig {
                nip42_identity: Some(RelayAuthIdentity {
                    private_key_hex: hex::encode(admin_secret),
                }),
                ..RelayConfig::default()
            },
        )?;
        relay_connection.connect().await?;
        tokio::time::sleep(Duration::from_millis(50)).await;

        relay_connection
            .subscribe_filters(
                "restricted-meta-auth",
                vec![json!({"kinds":[39000], "#d":["oa-main"]})],
            )
            .await?;
        let mut saw_group_metadata = false;
        for _ in 0..6 {
            let Some(message) = timeout(Duration::from_secs(2), relay_connection.recv()).await??
            else {
                continue;
            };
            if let RelayMessage::Event(_, event) = message
                && event.kind == 39000
            {
                saw_group_metadata = true;
                break;
            }
        }
        assert!(saw_group_metadata);

        let channel_create = sign_template(
            EventTemplate {
                created_at: 12,
                kind: 40,
                tags: vec![
                    vec!["h".to_string(), "oa-main".to_string()],
                    vec!["oa-room-mode".to_string(), "managed-channel".to_string()],
                ],
                content: "{\"name\":\"ops\",\"about\":\"\",\"picture\":\"\"}".to_string(),
            },
            &admin_secret,
        );
        let publish = relay_connection.publish(&channel_create).await?;
        assert!(publish.accepted);
        let mut saw_publish_ok = false;
        for _ in 0..4 {
            let Some(message) = timeout(Duration::from_secs(2), relay_connection.recv()).await??
            else {
                continue;
            };
            if let RelayMessage::Ok(event_id, accepted, message_text) = message
                && event_id == channel_create.id
            {
                assert!(accepted, "{message_text}");
                saw_publish_ok = true;
                break;
            }
        }
        assert!(saw_publish_ok);

        relay_connection.disconnect().await?;
        server.abort();
        Ok(())
    }
}
