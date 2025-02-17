use axum::extract::ws::{Message, WebSocket};
use axum::extract::Request;
use axum_extra::extract::cookie::CookieJar;
use futures::{sink::SinkExt, stream::StreamExt};
use std::collections::HashMap;
use std::error::Error;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex, RwLock};
use tracing::{debug, error, info, warn};
use url::form_urlencoded;
use uuid::Uuid;

use super::handlers::{chat::ChatHandler, solver_json::SolverJsonHandler, MessageHandler};
use super::types::{ChatMessage, ConnectionState, WebSocketError};
use crate::server::services::{
    deepseek::{DeepSeekService, Tool},
    github_issue::GitHubService,
    model_router::ModelRouter,
    solver::SolverService,
};

pub struct WebSocketState {
    connections: Arc<RwLock<HashMap<String, ConnectionState>>>,
    pub model_router: Arc<ModelRouter>,
    github_service: Arc<GitHubService>,
    solver_service: Arc<SolverService>,
}

impl WebSocketState {
    pub fn new(
        tool_model: Arc<DeepSeekService>,
        chat_model: Arc<DeepSeekService>,
        github_service: Arc<GitHubService>,
        solver_service: Arc<SolverService>,
        tools: Vec<Tool>,
    ) -> Self {
        let model_router = Arc::new(ModelRouter::new(tool_model, chat_model, tools));
        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
            model_router,
            github_service,
            solver_service,
        }
    }

    pub fn create_handlers(&self) -> (Arc<ChatHandler>, Arc<SolverJsonHandler>) {
        (
            Arc::new(ChatHandler::new(
                Arc::new(self.clone()),
                self.github_service.clone(),
            )),
            Arc::new(SolverJsonHandler::new(
                Arc::new(self.clone()),
                Arc::new(Mutex::new((*self.solver_service).clone())),
            )),
        )
    }

    pub async fn validate_session(
        &self,
        jar: &CookieJar,
        request: Request<axum::body::Body>,
    ) -> Result<i32, WebSocketError> {
        // First try to get session from cookie
        if let Some(session_cookie) = jar.get("session") {
            let session_id = session_cookie.value();
            // Try parsing as numeric session ID
            if let Ok(session_id) = session_id.parse::<i32>() {
                debug!("Found numeric session cookie with id: {}", session_id);
                return Ok(session_id);
            }
            // Try parsing as GitHub session ID
            if session_id.starts_with("github_") {
                if let Ok(github_id) = session_id.trim_start_matches("github_").parse::<i32>() {
                    debug!("Found GitHub session cookie with id: {}", github_id);
                    return Ok(github_id);
                }
            }
        }

        // If no cookie found, try to get session from query params
        let uri = request.uri();
        if let Some(query_params) = uri.query() {
            let params: HashMap<String, String> = form_urlencoded::parse(query_params.as_bytes())
                .into_owned()
                .collect();

            if let Some(session_token) = params.get("session") {
                // Try parsing as numeric session ID
                if let Ok(session_id) = session_token.parse::<i32>() {
                    debug!(
                        "Found numeric session token in query params with id: {}",
                        session_id
                    );
                    return Ok(session_id);
                }
                // Try parsing as GitHub session ID
                if session_token.starts_with("github_") {
                    if let Ok(github_id) =
                        session_token.trim_start_matches("github_").parse::<i32>()
                    {
                        debug!(
                            "Found GitHub session token in query params with id: {}",
                            github_id
                        );
                        return Ok(github_id);
                    }
                }
            }
        }

        warn!("No valid session token found in cookie or query params");
        Err(WebSocketError::NoSession)
    }

    pub async fn handle_socket(
        self: Arc<Self>,
        socket: WebSocket,
        chat_handler: Arc<ChatHandler>,
        user_id: i32,
    ) {
        let (mut sender, mut receiver) = socket.split();
        let (tx, mut rx) = mpsc::unbounded_channel();

        // Generate unique connection ID
        let conn_id = Uuid::new_v4().to_string();
        info!("New WebSocket connection established: {}", conn_id);

        // Store connection with user ID
        self.add_connection(&conn_id, user_id, tx.clone()).await;
        info!("Connection stored for user {}: {}", user_id, conn_id);

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
            ws_state.remove_connection(&send_conn_id).await;
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
        for conn in conns.values() {
            conn.tx.send(Message::Text(msg.to_string()))?;
        }
        Ok(())
    }

    pub async fn send_to(
        &self,
        conn_id: &str,
        msg: &str,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        info!("Sending message to {}: {}", conn_id, msg);
        if let Some(conn) = self.connections.read().await.get(conn_id) {
            conn.tx.send(Message::Text(msg.to_string()))?;
            info!("Message sent successfully");
        } else {
            error!("Connection {} not found", conn_id);
        }
        Ok(())
    }

    pub async fn get_user_id(&self, conn_id: &str) -> Option<i32> {
        self.connections
            .read()
            .await
            .get(conn_id)
            .map(|conn| conn.user_id)
    }

    pub async fn add_connection(
        &self,
        conn_id: &str,
        user_id: i32,
        tx: mpsc::UnboundedSender<Message>,
    ) {
        let mut conns = self.connections.write().await;
        conns.insert(
            conn_id.to_string(),
            ConnectionState {
                user_id,
                tx: tx.clone(),
            },
        );
    }

    pub async fn remove_connection(&self, conn_id: &str) {
        let mut conns = self.connections.write().await;
        conns.remove(conn_id);
    }

    // Test helper method
    pub async fn add_test_connection(
        self: &Arc<Self>,
        conn_id: &str,
        user_id: i32,
    ) -> mpsc::UnboundedReceiver<Message> {
        let (tx, rx) = mpsc::unbounded_channel();
        let mut conns = self.connections.write().await;
        conns.insert(
            conn_id.to_string(),
            ConnectionState {
                user_id,
                tx: tx.clone(),
            },
        );
        rx
    }

    pub async fn get_tx(
        &self,
        conn_id: &str,
    ) -> Result<mpsc::UnboundedSender<Message>, Box<dyn Error + Send + Sync>> {
        if let Some(conn) = self.connections.read().await.get(conn_id) {
            Ok(conn.tx.clone())
        } else {
            Err(Box::new(WebSocketError::ConnectionError(format!(
                "Connection {} not found",
                conn_id
            ))))
        }
    }
}

impl Clone for WebSocketState {
    fn clone(&self) -> Self {
        Self {
            connections: self.connections.clone(),
            model_router: self.model_router.clone(),
            github_service: self.github_service.clone(),
            solver_service: self.solver_service.clone(),
        }
    }
}
