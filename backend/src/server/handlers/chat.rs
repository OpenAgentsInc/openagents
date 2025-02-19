use axum::{
    extract::{Json, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::server::{
    config::AppState,
    models::chat::{Conversation, CreateConversationRequest, CreateMessageRequest, Message},
    services::chat_database::ChatDatabaseService,
};

#[derive(Debug, Deserialize)]
pub struct StartRepoChatRequest {
    pub id: Uuid,
    pub message: String,
    pub repos: Vec<String>,
    pub scope: String,
}

#[derive(Debug, Serialize)]
pub struct StartChatResponse {
    pub id: String,
    pub initial_message: String,
}

pub async fn start_repo_chat(
    State(state): State<AppState>,
    Json(request): Json<StartRepoChatRequest>,
) -> Result<Json<StartChatResponse>, (StatusCode, String)> {
    // Create chat database service
    let chat_db = ChatDatabaseService::new(state.pool);

    // Get user info from session
    let user_id = "anonymous"; // TODO: Get from session

    // Create conversation
    let conversation = chat_db
        .create_conversation(&CreateConversationRequest {
            user_id: user_id.to_string(),
            title: Some(format!("Repo chat: {}", request.message)),
        })
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to create conversation: {}", e),
            )
        })?;

    // Create initial message
    let message = chat_db
        .create_message(&CreateMessageRequest {
            conversation_id: conversation.id,
            role: "user".to_string(),
            content: request.message.clone(),
            metadata: None,
            tool_calls: None,
        })
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to create message: {}", e),
            )
        })?;

    Ok(Json(StartChatResponse {
        id: conversation.id.to_string(),
        initial_message: message.content,
    }))
}