use std::{collections::HashMap, sync::Arc};
use tokio::sync::broadcast;
use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    response::IntoResponse,
    extract::State,
};
use futures::{sink::SinkExt, stream::StreamExt};
use serde_json::Value;
use tracing::{error, info};

use super::{
    db::Database,
    event::Event,
    subscription::Subscription,
};

/// Shared state for the WebSocket relay
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
}

/// Handler for WebSocket upgrade
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<RelayState>>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

/// Main WebSocket connection handler
async fn handle_socket(mut socket: WebSocket, state: Arc<RelayState>) {
    let mut event_rx = state.event_tx.subscribe();

    // Split tasks between sender and receiver
    let (mut sender, mut receiver) = socket.split();

    // Spawn task to forward broadcast events to this client
    let send_task = tokio::spawn(async move {
        while let Ok(event) = event_rx.recv().await {
            // TODO: Check subscriptions and forward relevant events
            if let Ok(event_json) = serde_json::to_string(&event) {
                // For now, broadcast to all - will filter based on subscriptions later
                let msg = Message::Text(format!(r#"["EVENT", "{}", {}]"#, "all", event_json).into());
                if sender.send(msg).await.is_err() {
                    break;
                }
            }
        }
    });

    // Handle incoming messages
    let recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            match msg {
                Message::Text(text) => {
                    if let Ok(value) = serde_json::from_str::<Value>(&text) {
                        // TODO: Handle client messages (EVENT, REQ, CLOSE)
                        info!("Received message: {}", value);
                    }
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