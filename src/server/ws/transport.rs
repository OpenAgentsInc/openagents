use axum::extract::ws::{Message, WebSocket};
use futures::{sink::SinkExt, stream::StreamExt};
use std::collections::HashMap;
use std::error::Error;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::{error, info};
use uuid::Uuid;

use super::handlers::{chat::ChatHandler, MessageHandler};
use super::types::ChatMessage;
use crate::server::services::{
    deepseek::{DeepSeekService, Tool},
    github_issue::GitHubService,
    model_router::ModelRouter,
};

pub struct WebSocketState {
    connections: Arc<RwLock<HashMap<String, mpsc::UnboundedSender<Message>>>>,
    pub model_router: Arc<ModelRouter>,
    github_service: Arc<GitHubService>,
}

impl WebSocketState {
    pub fn new(
        tool_model: Arc<DeepSeekService>,
        chat_model: Arc<DeepSeekService>,
        github_service: Arc<GitHubService>,
        tools: Vec<Tool>,
    ) -> Arc<Self> {
        let model_router = Arc::new(ModelRouter::new(tool_model, chat_model, tools));
        Arc::new(Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
            model_router,
            github_service,
        })
    }

    pub fn create_handlers(ws_state: Arc<WebSocketState>) -> Arc<ChatHandler> {
        Arc::new(ChatHandler::new(
            ws_state.clone(),
            ws_state.github_service.clone(),
        ))
    }

    pub async fn handle_socket(self: Arc<Self>, socket: WebSocket, chat_handler: Arc<ChatHandler>) {
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
                if let Message::Text(text) = message {
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
                                    content: content_str.to_string(),
                                };
                                info!("Created chat message: {:?}", chat_msg);
                                if let Err(e) = chat_handler
                                    .handle_message(chat_msg, receive_conn_id.clone())
                                    .await
                                {
                                    error!("Error handling chat message: {}", e);
                                }
                            }
                        } else if let Some(message_type) = data.get("type") {
                            match message_type.as_str() {
                                Some("chat") => {
                                    info!("Processing chat message");
                                    if let Some(message) = data.get("message") {
                                        if let Ok(chat_msg) =
                                            serde_json::from_value(message.clone())
                                        {
                                            info!("Parsed chat message: {:?}", chat_msg);
                                            if let Err(e) = chat_handler
                                                .handle_message(chat_msg, receive_conn_id.clone())
                                                .await
                                            {
                                                error!("Error handling chat message: {}", e);
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

    pub async fn send_to(
        &self,
        conn_id: &str,
        msg: &str,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        info!("Sending message to {}: {}", conn_id, msg);
        if let Some(tx) = self.connections.read().await.get(conn_id) {
            tx.send(Message::Text(msg.to_string().into()))?;
            info!("Message sent successfully");
        } else {
            error!("Connection {} not found", conn_id);
        }
        Ok(())
    }

    // Test helper method - but we'll expose it always since it's useful for testing
    pub async fn add_test_connection(self: &Arc<Self>, conn_id: &str) -> mpsc::UnboundedReceiver<Message> {
        let (tx, rx) = mpsc::unbounded_channel();
        let mut conns = self.connections.write().await;
        conns.insert(conn_id.to_string(), tx);
        rx
    }
}