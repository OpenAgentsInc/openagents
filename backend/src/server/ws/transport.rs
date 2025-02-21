use std::{collections::HashMap, sync::Arc};

use axum::extract::ws::{Message, WebSocket};
use futures::{stream::SplitSink, SinkExt};
use serde_json::json;
use tokio::sync::{mpsc, RwLock};
use tracing::{error, info};

use crate::server::{
    config::AppState,
    services::github_issue::GitHubService,
    ws::{
        handlers::{chat::ChatHandler, solver::SolverHandler, solver_json::SolverJsonHandler},
        types::ChatMessage,
    },
};

pub type WebSocketSender = SplitSink<WebSocket, Message>;

#[derive(Clone)]
pub struct WebSocketState {
    pub connections: Arc<RwLock<HashMap<String, mpsc::Sender<String>>>>,
    pub github_service: Arc<GitHubService>,
    pub model_router: Arc<crate::server::services::model_router::ModelRouter>,
}

impl WebSocketState {
    pub fn new(
        github_service: Arc<GitHubService>,
        model_router: Arc<crate::server::services::model_router::ModelRouter>,
    ) -> Self {
        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
            github_service,
            model_router,
        }
    }

    pub async fn send_to(&self, conn_id: &str, msg: &str) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(tx) = self.connections.read().await.get(conn_id) {
            tx.send(msg.to_string()).await?;
        }
        Ok(())
    }

    pub async fn broadcast(&self, msg: &str) -> Result<(), Box<dyn std::error::Error>> {
        for tx in self.connections.read().await.values() {
            tx.send(msg.to_string()).await?;
        }
        Ok(())
    }

    pub async fn add_connection(
        &self,
        conn_id: String,
        tx: mpsc::Sender<String>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.connections.write().await.insert(conn_id, tx);
        Ok(())
    }

    pub async fn remove_connection(&self, conn_id: &str) -> Result<(), Box<dyn std::error::Error>> {
        self.connections.write().await.remove(conn_id);
        Ok(())
    }
}

pub struct WebSocketTransport {
    pub state: Arc<WebSocketState>,
    pub app_state: AppState,
}

impl WebSocketTransport {
    pub fn new(state: Arc<WebSocketState>, app_state: AppState) -> Self {
        Self { state, app_state }
    }

    pub async fn handle_socket(
        &self,
        socket: WebSocket,
        user_id: String,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let (sender, mut receiver) = socket.split();
        let (tx, mut rx) = mpsc::channel::<String>(32);

        let receive_conn_id = uuid::Uuid::new_v4().to_string();
        let send_conn_id = receive_conn_id.clone();

        // Add connection to state
        self.state
            .add_connection(receive_conn_id.clone(), tx)
            .await?;

        // Create handlers
        let mut chat_handler = ChatHandler::new(socket, self.app_state.clone(), user_id);

        // Handle incoming messages
        let receive_handle = tokio::spawn(async move {
            while let Some(Ok(msg)) = receiver.next().await {
                match msg {
                    Message::Text(text) => {
                        info!("Received message: {}", text);
                        if let Ok(chat_msg) = serde_json::from_str::<ChatMessage>(&text) {
                            if let Err(e) = chat_handler.handle_message(chat_msg).await {
                                error!("Error handling chat message: {:?}", e);
                            }
                        }
                    }
                    Message::Close(_) => {
                        info!("Client disconnected");
                        break;
                    }
                    _ => {}
                }
            }
        });

        // Handle outgoing messages
        let send_handle = tokio::spawn(async move {
            let mut sender = sender;
            while let Some(msg) = rx.recv().await {
                if sender.send(Message::Text(msg)).await.is_err() {
                    break;
                }
            }
        });

        // Wait for either task to finish
        tokio::select! {
            _ = receive_handle => info!("Receive task completed"),
            _ = send_handle => info!("Send task completed"),
        }

        // Clean up
        self.state.remove_connection(&send_conn_id).await?;

        Ok(())
    }
}