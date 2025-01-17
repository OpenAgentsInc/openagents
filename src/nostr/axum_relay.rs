use std::{collections::HashMap, sync::Arc, time::{Duration, Instant}};
use tokio::sync::{broadcast, mpsc};
use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    response::IntoResponse,
    extract::State,
};
use futures::{sink::SinkExt, stream::StreamExt};
use serde_json::Value;
use tracing::error;
use bytes::Bytes;
use anyhow::Error;

use super::{
    db::{Database, EventFilter},
    event::Event,
    subscription::Subscription,
};

const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(30);
const CLIENT_TIMEOUT: Duration = Duration::from_secs(60);

/// Shared state for the WebSocket relay
#[derive(Clone)]
pub struct RelayState {
    subscriptions: Arc<tokio::sync::RwLock<HashMap<String, Subscription>>>,
    event_tx: broadcast::Sender<Event>,
    db: Arc<Database>,
}

impl RelayState {
    pub fn new(event_tx: broadcast::Sender<Event>, db: Arc<Database>) -> Self {
        Self {
            subscriptions: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
            event_tx,
            db,
        }
    }

    pub async fn add_subscription(&self, id: String, sub: Subscription) {
        let mut subs = self.subscriptions.write().await;
        subs.insert(id, sub);
    }

    pub async fn remove_subscription(&self, id: &str) {
        let mut subs = self.subscriptions.write().await;
        subs.remove(id);
    }

    pub async fn save_event(&self, event: &Event) -> Result<(), Error> {
        self.db.save_event(event).await.map_err(Error::from)
    }

    pub async fn get_subscription(&self, id: &str) -> Option<Subscription> {
        let subs = self.subscriptions.read().await;
        subs.get(id).cloned()
    }

    pub async fn get_events_by_filter(&self, filter: EventFilter) -> Result<Vec<Event>, Error> {
        self.db.get_events_by_filter(filter).await.map_err(Error::from)
    }
}

/// Handler for WebSocket upgrade
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<RelayState>>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

async fn handle_client_message(
    msg: &str,
    state: &Arc<RelayState>,
    tx: &mpsc::Sender<Message>,
) {
    let parsed: Result<Value, _> = serde_json::from_str(msg);

    match parsed {
        Ok(value) => {
            if let Some(array) = value.as_array() {
                if array.is_empty() {
                    return;
                }

                match array[0].as_str() {
                    Some("EVENT") => {
                        if let Ok(event) = serde_json::from_value::<Event>(array[1].clone()) {
                            handle_event(event, state, tx).await;
                        }
                    }
                    Some("REQ") => {
                        if let Ok(sub) = serde_json::from_value(value.clone()) {
                            handle_subscription(sub, state, tx).await;
                        }
                    }
                    Some("CLOSE") => {
                        if array.len() >= 2 {
                            if let Some(sub_id) = array[1].as_str() {
                                state.remove_subscription(sub_id).await;
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        Err(_) => {
            let _ = tx
                .send(Message::Text(r#"["NOTICE", "Invalid message format"]"#.into()))
                .await;
        }
    }
}

async fn handle_event(
    event: Event,
    state: &Arc<RelayState>,
    tx: &mpsc::Sender<Message>,
) {
    match event.validate() {
        Ok(()) => {
            // Save event to database
            let event_clone = event.clone();
            if let Err(e) = state.save_event(&event_clone).await {
                error!("Failed to save event: {}", e);
            }

            // Build index and broadcast valid event
            let mut event_to_broadcast = event.clone();
            event_to_broadcast.build_index();
            if let Err(e) = state.event_tx.send(event_to_broadcast) {
                let _ = tx
                    .send(Message::Text(
                        format!(r#"["NOTICE", "Error broadcasting event: {}"]"#, e).into(),
                    ))
                    .await;
                return;
            }

            // Send OK message
            let _ = tx
                .send(Message::Text(
                    format!(r#"["OK", "{}", "{}"]"#, event.id, true).into(),
                ))
                .await;
        }
        Err(e) => {
            let _ = tx
                .send(Message::Text(
                    format!(r#"["OK", "{}", "{}", "{}"]"#, event.id, false, e).into(),
                ))
                .await;
        }
    }
}

async fn handle_subscription(
    sub: Subscription,
    state: &Arc<RelayState>,
    tx: &mpsc::Sender<Message>,
) {
    let sub_id = sub.id.clone();

    // Extract filter parameters from subscription
    let ids = sub.filters.iter().filter_map(|f| f.ids.clone()).next();
    let authors = sub.filters.iter().filter_map(|f| f.authors.clone()).next();
    let kinds = sub.filters.iter().filter_map(|f| f.kinds.clone()).next();
    let since = sub.filters.iter().filter_map(|f| f.since).next();
    let until = sub.filters.iter().filter_map(|f| f.until).next();
    let limit = sub.filters.iter().filter_map(|f| f.limit).next();

    // Collect tag filters
    let tag_filters = sub
        .filters
        .iter()
        .flat_map(|f| f.tags.iter())
        .filter(|(k, _)| k.starts_with('#'))
        .map(|(k, v)| (k.chars().nth(1).unwrap(), v.clone().into_iter().collect()))
        .collect::<Vec<_>>();

    let filter = EventFilter {
        ids,
        authors,
        kinds,
        since,
        until,
        limit,
        tag_filters,
    };

    match state.get_events_by_filter(filter).await {
        Ok(events) => {
            for event in events {
                if let Ok(event_json) = serde_json::to_string(&event) {
                    let _ = tx
                        .send(Message::Text(
                            format!(r#"["EVENT", "{}", {}]"#, sub_id, event_json).into(),
                        ))
                        .await;
                }
            }
        }
        Err(e) => {
            error!("Failed to query events: {}", e);
        }
    }

    state.add_subscription(sub_id.clone(), sub).await;

    // Send EOSE
    let _ = tx
        .send(Message::Text(format!(r#"["EOSE", "{}"]"#, sub_id).into()))
        .await;
}

/// Main WebSocket connection handler
async fn handle_socket(socket: WebSocket, state: Arc<RelayState>) {
    let (tx, mut rx) = mpsc::channel(32);
    let (mut sender, mut receiver) = socket.split();
    let state_clone = state.clone();
    let tx_clone = tx.clone();

    // Spawn task to forward broadcast events to this client
    let send_task = tokio::spawn(async move {
        let last_active = Instant::now();
        let mut heartbeat_interval = tokio::time::interval(HEARTBEAT_INTERVAL);

        loop {
            tokio::select! {
                Some(msg) = rx.recv() => {
                    if sender.send(msg).await.is_err() {
                        break;
                    }
                }
                Ok(event) = state_clone.event_tx.subscribe().recv() => {
                    let subs = state_clone.subscriptions.read().await;
                    for (sub_id, sub) in subs.iter() {
                        if sub.interested_in_event(&event) {
                            if let Ok(event_json) = serde_json::to_string(&event) {
                                let msg = Message::Text(
                                    format!(r#"["EVENT", "{}", {}]"#, sub_id, event_json).into()
                                );
                                if sender.send(msg).await.is_err() {
                                    return;
                                }
                            }
                        }
                    }
                }
                _ = heartbeat_interval.tick() => {
                    if Instant::now().duration_since(last_active) > CLIENT_TIMEOUT {
                        return;
                    }
                    if sender.send(Message::Ping(Bytes::new())).await.is_err() {
                        return;
                    }
                }
            }
        }
    });

    // Handle incoming messages
    let recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            match msg {
                Message::Text(text) => {
                    handle_client_message(&text, &state, &tx_clone).await;
                }
                Message::Ping(bytes) => {
                    let _ = tx_clone.send(Message::Pong(bytes)).await;
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
    });

    // Wait for either task to finish
    tokio::select! {
        _ = send_task => {
            error!("Send task completed");
        }
        _ = recv_task => {
            error!("Receive task completed");
        }
    }
}