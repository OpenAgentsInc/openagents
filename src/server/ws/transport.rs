use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use axum::extract::ws::{Message, WebSocket};
use futures::{sink::SinkExt, stream::StreamExt};
use serde_json::json;
use uuid::Uuid;
use std::error::Error;
use tracing::info;

use super::types::WebSocketMessage;
use super::handlers::{MessageHandler, chat::ChatHandler, solver::SolverHandler};

pub struct WebSocketState {
    connections: Arc<RwLock<HashMap<String, mpsc::UnboundedSender<Message>>>>,
    chat_handler: Arc<ChatHandler>,
    solver_handler: Arc<SolverHandler>,
}

impl WebSocketState {
    pub fn new(chat_handler: Arc<ChatHandler>, solver_handler: Arc<SolverHandler>) -> Self {
        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
            chat_handler,
            solver_handler,
        }
    }

    pub async fn handle_socket(self: Arc<Self>, socket: WebSocket) {
        let (mut sender, mut receiver) = socket.split();
        let (tx, mut rx) = mpsc::unbounded_channel();

        // Generate unique connection ID
        let conn_id = Arc::new(Uuid::new_v4().to_string());

        // Store connection
        {
            let mut conns = self.connections.write().await;
            conns.insert(conn_id.to_string(), tx.clone());
        }

        // Handle outgoing messages
        let ws_state = self.clone();
        let conn_id_clone = conn_id.clone();
        let send_task = tokio::spawn(async move {
            while let Some(message) = rx.recv().await {
                if sender.send(message).await.is_err() {
                    break;
                }
            }
            // Connection closed, remove from state
            let mut conns = ws_state.connections.write().await;
            conns.remove(&conn_id_clone.to_string());
        });

        // Handle incoming messages
        let ws_state = self.clone();
        let receive_task = tokio::spawn(async move {
            while let Some(Ok(message)) = receiver.next().await {
                match message {
                    Message::Text(text) => {
                        info!("Raw WebSocket message received: {}", text);
                        
                        // Parse the message
                        if let Ok(data) = serde_json::from_str::<serde_json::Value>(&text) {
                            if let Some(message_type) = data.get("type") {
                                match message_type.as_str() {
                                    Some("chat") => {
                                        if let Some(message) = data.get("message") {
                                            if let Ok(chat_msg) = serde_json::from_value(message.clone()) {
                                                if let Err(e) = ws_state.chat_handler.handle_message(chat_msg, conn_id.to_string()).await {
                                                    eprintln!("Error handling chat message: {}", e);
                                                }
                                            }
                                        }
                                    }
                                    Some("solver") => {
                                        if let Some(message) = data.get("message") {
                                            if let Ok(solver_msg) = serde_json::from_value(message.clone()) {
                                                if let Err(e) = ws_state.solver_handler.handle_message(solver_msg, conn_id.to_string()).await {
                                                    eprintln!("Error handling solver message: {}", e);
                                                }
                                            }
                                        }
                                    }
                                    _ => {
                                        eprintln!("Unknown message type");
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        });

        // Wait for either task to finish
        tokio::select! {
            _ = send_task => {},
            _ = receive_task => {},
        }
    }

    pub async fn broadcast(&self, msg: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
        let conns = self.connections.read().await;
        for tx in conns.values() {
            tx.send(Message::Text(msg.to_string().into()))?;
        }
        Ok(())
    }

    pub async fn send_to(&self, conn_id: &str, msg: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
        if let Some(tx) = self.connections.read().await.get(conn_id) {
            tx.send(Message::Text(msg.to_string().into()))?;
        }
        Ok(())
    }
}