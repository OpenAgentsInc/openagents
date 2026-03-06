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

use axum::extract::State;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use futures_util::{SinkExt, StreamExt};
use nostr::Event;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::sync::{RwLock, mpsc};

const ENV_LISTEN_ADDR: &str = "NEXUS_RELAY_LISTEN_ADDR";
const ENV_MAX_STORED_EVENTS: &str = "NEXUS_RELAY_MAX_STORED_EVENTS";
const DEFAULT_LISTEN_ADDR: &str = "127.0.0.1:42110";
const DEFAULT_MAX_STORED_EVENTS: usize = 5_000;

#[derive(Debug, Clone)]
pub struct RelayServiceConfig {
    pub listen_addr: SocketAddr,
    pub max_stored_events: usize,
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

        Ok(Self {
            listen_addr,
            max_stored_events,
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
    store: Arc<RwLock<RelayStore>>,
}

#[derive(Default)]
struct RelayStore {
    next_client_id: u64,
    events: Vec<Event>,
    clients: HashMap<u64, ConnectedClient>,
}

struct ConnectedClient {
    sender: mpsc::UnboundedSender<String>,
    subscriptions: HashMap<String, Vec<Value>>,
}

pub fn build_router(config: RelayServiceConfig) -> Router {
    let state = AppState {
        config,
        store: Arc::new(RwLock::new(RelayStore::default())),
    };
    Router::new()
        .route("/", get(relay_websocket))
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

async fn relay_websocket(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(state, socket))
}

async fn handle_socket(state: AppState, socket: WebSocket) {
    let (mut sink, mut stream) = socket.split();
    let (outgoing_tx, mut outgoing_rx) = mpsc::unbounded_channel::<String>();
    let client_id = {
        let mut store = state.store.write().await;
        store.next_client_id = store.next_client_id.saturating_add(1);
        let client_id = store.next_client_id;
        store.clients.insert(
            client_id,
            ConnectedClient {
                sender: outgoing_tx,
                subscriptions: HashMap::new(),
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
        "AUTH" => send_notice(state, client_id, "unsupported:auth_not_enabled".to_string()).await,
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
        snapshot_events(store.events.as_slice(), filters.as_slice())
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

    let (is_new, deliveries) = {
        let mut store = state.store.write().await;
        let is_new = !store.events.iter().any(|existing| existing.id == event.id);
        if is_new {
            store.events.push(event.clone());
            if store.events.len() > state.config.max_stored_events {
                let overflow = store
                    .events
                    .len()
                    .saturating_sub(state.config.max_stored_events);
                store.events.drain(0..overflow);
            }
        }

        let deliveries = store
            .clients
            .iter()
            .flat_map(|(target_client_id, client)| {
                let event = event.clone();
                client
                    .subscriptions
                    .iter()
                    .filter(move |(_, filters)| event_matches_filters(&event, filters.as_slice()))
                    .map(move |(subscription_id, _)| (*target_client_id, subscription_id.clone()))
            })
            .collect::<Vec<_>>();
        (is_new, deliveries)
    };

    for (target_client_id, subscription_id) in deliveries {
        let payload = json!(["EVENT", subscription_id, event]).to_string();
        send_text(state, target_client_id, payload).await?;
    }

    let accepted = is_new;
    let message = if accepted { "accepted" } else { "duplicate" };
    send_text(
        state,
        client_id,
        json!(["OK", event.id, accepted, message]).to_string(),
    )
    .await
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

fn snapshot_events(events: &[Event], filters: &[Value]) -> Vec<Event> {
    let mut delivered_ids = HashSet::<String>::new();
    let mut matched = Vec::<Event>::new();

    for filter in filters {
        let limit = filter_limit(filter).unwrap_or(events.len());
        let filter_matches = events
            .iter()
            .filter(|event| event_matches_filter(event, filter))
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
    if let Some(tag_values) = object.get("#e").and_then(Value::as_array)
        && !event_has_tag(event, "e", tag_values.as_slice())
    {
        return false;
    }
    if let Some(tag_values) = object.get("#p").and_then(Value::as_array)
        && !event_has_tag(event, "p", tag_values.as_slice())
    {
        return false;
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

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use futures_util::{SinkExt, StreamExt};
    use nostr::Event;
    use serde_json::json;
    use tokio::time::{Duration, timeout};
    use tokio_tungstenite::{connect_async, tungstenite::Message as WsMessage};

    use super::{RelayServiceConfig, build_router, event_matches_filter, run_server};

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
    async fn relay_replays_stored_events_and_broadcasts_new_events() -> Result<()> {
        let config = RelayServiceConfig {
            listen_addr: "127.0.0.1:0".parse()?,
            max_stored_events: 128,
        };
        let listener = tokio::net::TcpListener::bind(config.listen_addr).await?;
        let addr = listener.local_addr()?;
        let server = tokio::spawn(async move {
            axum::serve(listener, build_router(config))
                .await
                .map_err(anyhow::Error::from)
        });

        let relay_url = format!("ws://{addr}/");
        let (mut publisher, _) = connect_async(relay_url.as_str()).await?;
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
        let config = RelayServiceConfig {
            listen_addr: "127.0.0.1:0".parse()?,
            max_stored_events: 32,
        };
        let server = tokio::spawn(async move { run_server(config).await });
        tokio::time::sleep(Duration::from_millis(25)).await;
        server.abort();
        Ok(())
    }
}
