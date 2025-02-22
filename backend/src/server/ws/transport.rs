use std::{collections::HashMap, sync::Arc};

use axum::extract::ws::{Message, WebSocket};
use futures_util::{SinkExt, StreamExt};
use tokio::sync::{mpsc, RwLock};
use tracing::{error, info, debug};

use crate::server::{
    config::AppState,
    services::{github_issue::GitHubService, model_router::ModelRouter},
    ws::handlers::chat::{ChatHandler, ChatMessage, ChatResponse},
};

pub type WebSocketSender = mpsc::Sender<String>;

#[derive(Clone)]
pub struct WebSocketState {
    pub connections: Arc<RwLock<HashMap<String, WebSocketSender>>>,
    pub github_service: Arc<GitHubService>,
    pub model_router: Arc<ModelRouter>,
}

impl WebSocketState {
    pub fn new(github_service: Arc<GitHubService>, model_router: Arc<ModelRouter>) -> Self {
        info!("Creating new WebSocketState");
        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
            github_service,
            model_router,
        }
    }

    pub async fn send_to(
        &self,
        conn_id: &str,
        msg: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        debug!("Sending message to connection {}: {}", conn_id, msg);
        if let Some(tx) = self.connections.read().await.get(conn_id) {
            tx.send(msg.to_string()).await?;
            debug!("Message sent successfully");
        } else {
            debug!("Connection {} not found", conn_id);
        }
        Ok(())
    }

    pub async fn broadcast(
        &self,
        msg: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        debug!("Broadcasting message to all connections: {}", msg);
        let connections = self.connections.read().await;
        for (id, tx) in connections.iter() {
            debug!("Sending to connection {}", id);
            if let Err(e) = tx.send(msg.to_string()).await {
                error!("Failed to send to connection {}: {:?}", id, e);
            }
        }
        Ok(())
    }

    pub async fn add_connection(
        &self,
        conn_id: String,
        tx: WebSocketSender,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("Adding new connection: {}", conn_id);
        self.connections.write().await.insert(conn_id.clone(), tx);
        let count = self.connections.read().await.len();
        info!("Total active connections: {}", count);
        Ok(())
    }

    pub async fn remove_connection(
        &self,
        conn_id: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("Removing connection: {}", conn_id);
        self.connections.write().await.remove(conn_id);
        let count = self.connections.read().await.len();
        info!("Total active connections: {}", count);
        Ok(())
    }
}

pub struct WebSocketTransport {
    pub state: Arc<WebSocketState>,
    pub app_state: AppState,
}

impl WebSocketTransport {
    pub fn new(state: Arc<WebSocketState>, app_state: AppState) -> Self {
        info!("Creating new WebSocketTransport");
        Self { state, app_state }
    }

    pub async fn handle_socket(
        &self,
        socket: WebSocket,
        user_id: String,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("Handling new WebSocket connection for user: {}", user_id);
        
        let (sender, mut receiver) = socket.split();
        let (tx, mut rx) = mpsc::channel::<String>(32);

        let conn_id = uuid::Uuid::new_v4().to_string();
        info!("Created connection ID: {}", conn_id);

        // Add connection to state with error handling
        if let Err(e) = self.state.add_connection(conn_id.clone(), tx.clone()).await {
            error!("Failed to add connection: {:?}", e);
            // Send error to client before returning
            let error_msg = serde_json::to_string(&ChatResponse::Error {
                message: "Failed to initialize connection".to_string(),
            })?;
            tx.send(error_msg).await?;
            return Err(e);
        }

        let processor = MessageProcessor::new(self.app_state.clone(), user_id.clone());
        info!("Created message processor for user: {}", user_id);

        // Clone connection ID for each task
        let receive_conn_id = conn_id.clone();
        let send_conn_id = conn_id.clone();
        let cleanup_state = self.state.clone();
        let cleanup_conn_id = conn_id.clone();

        // Set up cleanup function
        let cleanup = {
            let cleanup_conn_id = conn_id;
            let cleanup_state = cleanup_state;
            move || async move {
                info!("Running cleanup for connection: {}", cleanup_conn_id);
                if let Err(e) = cleanup_state.remove_connection(&cleanup_conn_id).await {
                    error!("Failed to clean up connection: {:?}", e);
                }
                info!("Cleanup completed for connection: {}", cleanup_conn_id);
            }
        };

        // Create a channel to signal when the send task is done
        let (send_done_tx, mut send_done_rx) = mpsc::channel::<()>(1);
        let send_done_tx_clone = send_done_tx.clone();

        // Handle incoming messages
        let receive_handle = tokio::spawn(async move {
            info!("Starting receive task for connection: {}", receive_conn_id);
            while let Some(Ok(msg)) = receiver.next().await {
                match msg {
                    Message::Text(ref text) => {
                        info!("Received message on {}: {}", receive_conn_id, text);
                        if let Err(e) = processor.process_message(text, &tx).await {
                            error!("Error processing message on {}: {:?}", receive_conn_id, e);
                            // Send error to client
                            let error_msg = serde_json::to_string(&ChatResponse::Error {
                                message: format!("Message processing error: {}", e),
                            }).unwrap_or_else(|e| format!("{{\"type\":\"Error\",\"message\":\"{}\"}}", e));
                            if let Err(e) = tx.send(error_msg).await {
                                error!("Failed to send error message: {:?}", e);
                            }
                            break;
                        }
                    }
                    Message::Close(_) => {
                        info!("Client requested close on {}", receive_conn_id);
                        // Signal send task to complete
                        if let Err(e) = send_done_tx.send(()).await {
                            error!("Failed to signal send task completion: {:?}", e);
                        }
                        break;
                    }
                    _ => {
                        debug!("Ignoring non-text message on {}", receive_conn_id);
                    }
                }
            }
            info!("Receive task ending for {}", receive_conn_id);
        });

        // Handle outgoing messages
        let send_handle = tokio::spawn(async move {
            info!("Starting send task for connection: {}", send_conn_id);
            let mut sender = sender;
            
            loop {
                tokio::select! {
                    Some(msg) = rx.recv() => {
                        debug!("Sending message on {}: {}", send_conn_id, msg);
                        if let Err(e) = sender.send(Message::Text(msg)).await {
                            error!("Error sending message on {}: {:?}", send_conn_id, e);
                            break;
                        }
                    }
                    _ = send_done_rx.recv() => {
                        info!("Received completion signal for {}", send_conn_id);
                        break;
                    }
                }
            }

            info!("Send task ending for {}", send_conn_id);
            // Signal that we're done sending
            if let Err(e) = send_done_tx_clone.send(()).await {
                error!("Failed to signal send task completion: {:?}", e);
            }
        });

        // Wait for BOTH tasks to finish
        let (receive_result, send_result) = tokio::join!(receive_handle, send_handle);
        
        // Check results
        if let Err(e) = receive_result {
            error!("Receive task error: {:?}", e);
        }
        if let Err(e) = send_result {
            error!("Send task error: {:?}", e);
        }

        // Clean up only after both tasks are done
        cleanup().await;

        Ok(())
    }
}

pub struct MessageProcessor {
    app_state: AppState,
    user_id: String,
}

impl MessageProcessor {
    pub fn new(app_state: AppState, user_id: String) -> Self {
        Self { app_state, user_id }
    }

    pub async fn process_message(
        &self,
        text: &str,
        tx: &mpsc::Sender<String>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        debug!("Processing message: {}", text);
        
        match serde_json::from_str::<ChatMessage>(text) {
            Ok(chat_msg) => {
                info!("Parsed chat message: {:?}", chat_msg);
                match chat_msg {
                    ChatMessage::Subscribe { scope, .. } => {
                        info!("Processing subscribe message for scope: {}", scope);
                        let response = ChatResponse::Subscribed {
                            scope,
                            last_sync_id: 0,
                        };
                        let msg = serde_json::to_string(&response)?;
                        debug!("Sending subscribe response: {}", msg);
                        tx.send(msg).await?;
                    }
                    ChatMessage::Message {
                        id,
                        conversation_id,
                        content,
                        repos,
                        use_reasoning,
                    } => {
                        info!("Processing chat message: id={}, conv={:?}", id, conversation_id);
                        let mut chat_handler =
                            ChatHandler::new(tx.clone(), self.app_state.clone(), self.user_id.clone());
                        chat_handler
                            .handle_message(ChatMessage::Message {
                                id,
                                conversation_id,
                                content,
                                repos,
                                use_reasoning,
                            })
                            .await?;
                    }
                }
            }
            Err(e) => {
                error!("Failed to parse message '{}': {:?}", text, e);
                let error_msg = serde_json::to_string(&ChatResponse::Error {
                    message: format!("Invalid message format: {}", e),
                })?;
                tx.send(error_msg).await?;
                return Err(e.into());
            }
        }
        Ok(())
    }
}