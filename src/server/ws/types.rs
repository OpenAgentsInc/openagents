use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc::UnboundedSender, RwLock};
use axum::extract::ws::Message as WsMessage;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Message {
    #[serde(rename = "chat")]
    Chat {
        content: String,
    },
    #[serde(rename = "tool")]
    Tool {
        name: String,
        arguments: Value,
    },
}

pub struct WebSocketState {
    connections: Arc<RwLock<HashMap<String, UnboundedSender<WsMessage>>>>,
}

impl WebSocketState {
    pub fn new() -> Self {
        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn broadcast(&self, msg: Message) {
        let connections = self.connections.read().await;
        let msg_str = serde_json::to_string(&msg).unwrap();
        for tx in connections.values() {
            let _ = tx.send(WsMessage::Text(msg_str.clone()));
        }
    }

    pub async fn add_connection(&self, id: String, tx: UnboundedSender<WsMessage>) {
        let mut connections = self.connections.write().await;
        connections.insert(id, tx);
    }

    pub async fn remove_connection(&self, id: &str) {
        let mut connections = self.connections.write().await;
        connections.remove(id);
    }
}

impl Default for WebSocketState {
    fn default() -> Self {
        Self::new()
    }
}