use std::{collections::HashMap, sync::Arc};

use axum::extract::ws::{Message, WebSocket};
use futures_util::{SinkExt, StreamExt};
use tokio::sync::{mpsc, RwLock, Mutex};
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
                connection_id: Some(conn_id.clone()),
            })?;
            tx.send(error_msg).await?;
            return Err(e);
        }

        let processor = MessageProcessor::new(self.app_state.clone(), user_id.clone(), conn_id.clone());
        info!("Created message processor for user: {}", user_id);

        // Clone connection ID for each task
        let receive_conn_id = conn_id.clone();
        let send_conn_id = conn_id.clone();
        let cleanup_state = self.state.clone();
        let cleanup_conn_id = conn_id.clone();

        // Create a mutex to synchronize closing
        let is_closing = Arc::new(Mutex::new(false));
        let is_closing_send = is_closing.clone();
        let is_closing_receive = is_closing.clone();

        // Create a channel for pending messages
        let (pending_tx, mut pending_rx) = mpsc::channel::<String>(32);
        let pending_tx = Arc::new(pending_tx);
        let pending_tx_receive = pending_tx.clone();

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

        // Handle incoming messages
        let receive_handle = tokio::spawn(async move {
            info!("Starting receive task for connection: {}", receive_conn_id);
            while let Some(Ok(msg)) = receiver.next().await {
                match msg {
                    Message::Text(ref text) => {
                        info!("Received message on {}: {}", receive_conn_id, text);
                        if let Err(e) = processor.process_message(text, &pending_tx_receive).await {
                            error!("Error processing message on {}: {:?}", receive_conn_id, e);
                            // Send error to client
                            let error_msg = serde_json::to_string(&ChatResponse::Error {
                                message: format!("Message processing error: {}", e),
                                connection_id: Some(receive_conn_id.clone()),
                            }).unwrap_or_else(|e| format!("{{\"type\":\"Error\",\"message\":\"{}\",\"connection_id\":\"{}\"}}", e, receive_conn_id));
                            if let Err(e) = pending_tx_receive.send(error_msg).await {
                                error!("Failed to send error message: {:?}", e);
                            }
                            break;
                        }
                    }
                    Message::Close(_) => {
                        info!("Client requested close on {}", receive_conn_id);
                        // Set closing flag
                        let mut is_closing = is_closing_receive.lock().await;
                        *is_closing = true;
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
                        // Queue message in pending
                        if let Err(e) = pending_tx.send(msg).await {
                            error!("Failed to queue message: {:?}", e);
                            break;
                        }
                    }
                    Some(msg) = pending_rx.recv() => {
                        debug!("Processing pending message on {}: {}", send_conn_id, msg);
                        if let Err(e) = sender.send(Message::Text(msg)).await {
                            error!("Error sending message on {}: {:?}", send_conn_id, e);
                            break;
                        }
                    }
                    else => {
                        // Check if we're closing and all messages are sent
                        let is_closing = is_closing_send.lock().await;
                        if *is_closing && rx.is_empty() && pending_rx.is_empty() {
                            info!("All messages sent, closing connection {}", send_conn_id);
                            break;
                        }
                    }
                }
            }

            info!("Send task ending for {}", send_conn_id);
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
    connection_id: String,
}

impl MessageProcessor {
    pub fn new(app_state: AppState, user_id: String, connection_id: String) -> Self {
        Self { app_state, user_id, connection_id }
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
                            connection_id: Some(self.connection_id.clone()),
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
                        ..
                    } => {
                        info!("Processing chat message: id={}, conv={:?}", id, conversation_id);
                        let mut chat_handler =
                            ChatHandler::new(tx.clone(), self.app_state.clone(), self.user_id.clone());
                        chat_handler
                            .handle_message(ChatMessage::Message {
                                id,
                                connection_id: Some(self.connection_id.clone()),
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
                    connection_id: Some(self.connection_id.clone()),
                })?;
                tx.send(error_msg).await?;
                return Err(e.into());
            }
        }
        Ok(())
    }
}