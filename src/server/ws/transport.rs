use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use axum::extract::ws::{Message, WebSocket};
use futures::{sink::SinkExt, stream::StreamExt};
use serde_json::json;
use uuid::Uuid;
use std::error::Error;
use tracing::{info, error};

use super::types::{WebSocketMessage, ChatMessage};
use super::handlers::{MessageHandler, chat::ChatHandler, solver::SolverHandler};

pub struct WebSocketState {
    connections: Arc<RwLock<HashMap<String, mpsc::UnboundedSender<Message>>>>,
}

impl WebSocketState {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    pub fn create_handlers(ws_state: Arc<WebSocketState>) -> (Arc<ChatHandler>, Arc<SolverHandler>) {
        let chat_handler = Arc::new(ChatHandler::new(ws_state.clone()));
        let solver_handler = Arc::new(SolverHandler::new());
        (chat_handler, solver_handler)
    }

    pub async fn handle_socket(
        self: Arc<Self>,
        socket: WebSocket,
        chat_handler: Arc<ChatHandler>,
        solver_handler: Arc<SolverHandler>
    ) {
        let (mut sender, mut receiver) = socket.split();
        let (tx, mut rx) = mpsc::unbounded_channel();

        // Generate unique connection ID
        let conn_id = Uuid::new_v4().to_string();
        info!("New WebSocket connection established: {}", conn_id);

        // Store connection
        {
            let mut conns = self.connections.write().await;
            conns.insert(conn_id.clone(), tx.clone());
            info!("Connection stored: {}", conn_id);
        }

        // Handle outgoing messages
        let ws_state = self.clone();
        let send_conn_id = conn_id.clone();
        let send_task = tokio::spawn(async move {
            while let Some(message) = rx.recv().await {
                info!("Sending message to client {}: {:?}", send_conn_id, message);
                if sender.send(message).await.is_err() {
                    error!("Failed to send message to client {}", send_conn_id);
                    break;
                }
            }
            // Connection closed, remove from state
            let mut conns = ws_state.connections.write().await;
            conns.remove(&send_conn_id);
            info!("Connection removed: {}", send_conn_id);
        });

        // Handle incoming messages
        let receive_conn_id = conn_id.clone();
        let receive_task = tokio::spawn(async move {
            while let Some(Ok(message)) = receiver.next().await {
                match message {
                    Message::Text(text) => {
                        info!("Raw WebSocket message received: {}", text);
                        
                        // Parse the message
                        if let Ok(data) = serde_json::from_str::<serde_json::Value>(&text) {
                            info!("Parsed message: {:?}", data);
                            
                            // Try to extract content directly if message type is missing
                            if let Some(content) = data.get("content") {
                                info!("Found direct content: {:?}", content);
                                if let Some(content_str) = content.as_str() {
                                    // Create a chat message manually
                                    let chat_msg = ChatMessage::UserMessage {
                                        content: content_str.to_string()
                                    };
                                    info!("Created chat message: {:?}", chat_msg);
                                    if let Err(e) = chat_handler.handle_message(chat_msg, receive_conn_id.clone()).await {
                                        error!("Error handling chat message: {}", e);
                                    }
                                }
                            } else if let Some(message_type) = data.get("type") {
                                match message_type.as_str() {
                                    Some("chat") => {
                                        info!("Processing chat message");
                                        if let Some(message) = data.get("message") {
                                            if let Ok(chat_msg) = serde_json::from_value(message.clone()) {
                                                info!("Parsed chat message: {:?}", chat_msg);
                                                if let Err(e) = chat_handler.handle_message(chat_msg, receive_conn_id.clone()).await {
                                                    error!("Error handling chat message: {}", e);
                                                }
                                            }
                                        }
                                    }
                                    Some("solver") => {
                                        info!("Processing solver message");
                                        if let Some(message) = data.get("message") {
                                            if let Ok(solver_msg) = serde_json::from_value(message.clone()) {
                                                if let Err(e) = solver_handler.handle_message(solver_msg, receive_conn_id.clone()).await {
                                                    error!("Error handling solver message: {}", e);
                                                }
                                            }
                                        }
                                    }
                                    _ => {
                                        error!("Unknown message type");
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
        let final_conn_id = conn_id.clone();
        tokio::select! {
            _ = send_task => {
                info!("Send task completed for {}", final_conn_id);
            },
            _ = receive_task => {
                info!("Receive task completed for {}", final_conn_id);
            },
        }
    }

    pub async fn broadcast(&self, msg: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
        info!("Broadcasting message: {}", msg);
        let conns = self.connections.read().await;
        for tx in conns.values() {
            tx.send(Message::Text(msg.to_string().into()))?;
        }
        Ok(())
    }

    pub async fn send_to(&self, conn_id: &str, msg: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
        info!("Sending message to {}: {}", conn_id, msg);
        if let Some(tx) = self.connections.read().await.get(conn_id) {
            tx.send(Message::Text(msg.to_string().into()))?;
            info!("Message sent successfully");
        } else {
            error!("Connection {} not found", conn_id);
        }
        Ok(())
    }
}