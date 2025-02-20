use axum::{
    extract::{Json, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::{error, info};
use uuid::Uuid;

use crate::server::{
    config::AppState,
    models::chat::{CreateConversationRequest, CreateMessageRequest},
    services::chat_database::ChatDatabaseService,
};

#[derive(Debug, Deserialize)]
pub struct StartRepoChatRequest {
    pub id: Uuid,
    pub message: String,
    pub repos: Vec<String>,
    pub scope: String,
}

#[derive(Debug, Deserialize)]
pub struct SendMessageRequest {
    pub conversation_id: Uuid,
    pub message: String,
    pub repos: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
pub struct StartChatResponse {
    pub id: String,
    pub initial_message: String,
}

#[derive(Debug, Serialize)]
pub struct SendMessageResponse {
    pub id: String,
    pub message: String,
}

pub async fn start_repo_chat(
    State(state): State<AppState>,
    Json(request): Json<StartRepoChatRequest>,
) -> Result<Json<StartChatResponse>, (StatusCode, String)> {
    info!("Starting repo chat with request: {:?}", request);

    // Create chat database service
    let chat_db = ChatDatabaseService::new(state.pool);

    // Get user info from session
    let user_id = "anonymous"; // TODO: Get from session
    info!("Using user_id: {}", user_id);

    // Create conversation
    let conversation = chat_db
        .create_conversation(&CreateConversationRequest {
            user_id: user_id.to_string(),
            title: Some(format!("Repo chat: {}", request.message)),
        })
        .await
        .map_err(|e| {
            error!("Failed to create conversation: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to create conversation: {}", e),
            )
        })?;

    info!("Created conversation with id: {}", conversation.id);

    // Create initial message with repos metadata
    let message = chat_db
        .create_message(&CreateMessageRequest {
            conversation_id: conversation.id,
            user_id: user_id.to_string(),
            role: "user".to_string(),
            content: request.message.clone(),
            metadata: Some(json!({
                "repos": request.repos
            })),
            tool_calls: None,
        })
        .await
        .map_err(|e| {
            error!("Failed to create message: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to create message: {}", e),
            )
        })?;

    info!("Created message with id: {}", message.id);

    Ok(Json(StartChatResponse {
        id: conversation.id.to_string(),
        initial_message: message.content,
    }))
}

pub async fn send_message(
    State(state): State<AppState>,
    Json(request): Json<SendMessageRequest>,
) -> Result<Json<SendMessageResponse>, (StatusCode, String)> {
    info!("Sending message with request: {:?}", request);

    // Create chat database service
    let chat_db = ChatDatabaseService::new(state.pool);

    // Get user info from session
    let user_id = "anonymous"; // TODO: Get from session
    info!("Using user_id: {}", user_id);

    // Create message
    let message = chat_db
        .create_message(&CreateMessageRequest {
            conversation_id: request.conversation_id,
            user_id: user_id.to_string(),
            role: "user".to_string(),
            content: request.message.clone(),
            metadata: request.repos.map(|repos| json!({ "repos": repos })),
            tool_calls: None,
        })
        .await
        .map_err(|e| {
            error!("Failed to create message: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to create message: {}", e),
            )
        })?;

    info!("Created message with id: {}", message.id);

    Ok(Json(SendMessageResponse {
        id: message.id.to_string(),
        message: message.content,
    }))
}