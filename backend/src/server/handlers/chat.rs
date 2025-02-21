use axum::{
    extract::{Json, Path, State},
    http::StatusCode,
};
use axum_extra::extract::cookie::CookieJar;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::{error, info};
use uuid::Uuid;

use crate::server::{
    config::AppState,
    handlers::oauth::session::SESSION_COOKIE_NAME,
    models::chat::{CreateConversationRequest, CreateMessageRequest, Message},
    services::{chat_database::ChatDatabaseService, gateway::Gateway},
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
    cookies: CookieJar,
    State(state): State<AppState>,
    Json(request): Json<StartRepoChatRequest>,
) -> Result<Json<StartChatResponse>, (StatusCode, String)> {
    info!("Starting repo chat with request: {:?}", request);

    // Create chat database service
    let chat_db = ChatDatabaseService::new(state.pool);

    // Get user info from session
    let user_id = if let Some(session_cookie) = cookies.get(SESSION_COOKIE_NAME) {
        session_cookie.value().to_string()
    } else {
        return Err((
            StatusCode::UNAUTHORIZED,
            "No session found. Please log in.".to_string(),
        ));
    };
    info!("Using user_id: {}", user_id);

    // Create conversation
    let conversation = chat_db
        .create_conversation(&CreateConversationRequest {
            user_id: user_id.clone(),
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
            user_id: user_id.clone(),
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

    // Get Groq response
    let (ai_response, _) = state
        .groq
        .chat(request.message.clone(), false)
        .await
        .map_err(|e| {
            error!("Failed to get Groq response: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to get AI response: {}", e),
            )
        })?;

    // Save AI response
    let ai_message = chat_db
        .create_message(&CreateMessageRequest {
            conversation_id: conversation.id,
            user_id: user_id.clone(),
            role: "assistant".to_string(),
            content: ai_response,
            metadata: Some(json!({
                "repos": request.repos
            })),
            tool_calls: None,
        })
        .await
        .map_err(|e| {
            error!("Failed to save AI response: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to save AI response: {}", e),
            )
        })?;

    info!("Created AI message with id: {}", ai_message.id);

    Ok(Json(StartChatResponse {
        id: conversation.id.to_string(),
        initial_message: message.content,
    }))
}

pub async fn send_message(
    cookies: CookieJar,
    State(state): State<AppState>,
    Json(request): Json<SendMessageRequest>,
) -> Result<Json<SendMessageResponse>, (StatusCode, String)> {
    info!("Sending message with request: {:?}", request);

    // Create chat database service
    let chat_db = ChatDatabaseService::new(state.pool);

    // Get user info from session
    let user_id = if let Some(session_cookie) = cookies.get(SESSION_COOKIE_NAME) {
        session_cookie.value().to_string()
    } else {
        return Err((
            StatusCode::UNAUTHORIZED,
            "No session found. Please log in.".to_string(),
        ));
    };
    info!("Using user_id: {}", user_id);

    // If no repos provided in the request, try to get them from the conversation's first message
    let metadata = if let Some(repos) = request.repos {
        info!("Using repos from request: {:?}", repos);
        Some(json!({ "repos": repos }))
    } else {
        // Get the first message of the conversation to find the repos
        let messages = chat_db
            .get_conversation_messages(request.conversation_id)
            .await
            .map_err(|e| {
                error!("Failed to fetch conversation messages: {:?}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Failed to fetch conversation messages: {}", e),
                )
            })?;

        // Find the first message with repos metadata
        let first_message_repos = messages.iter().find_map(|msg| {
            msg.metadata.as_ref().and_then(|meta| {
                meta.get("repos")
                    .and_then(|repos| repos.as_array())
                    .map(|repos| repos.to_owned())
            })
        });

        if let Some(repos) = first_message_repos {
            info!("Using repos from first message: {:?}", repos);
            Some(json!({ "repos": repos }))
        } else {
            info!("No repos found in request or first message");
            None
        }
    };

    // Create user message
    let message = chat_db
        .create_message(&CreateMessageRequest {
            conversation_id: request.conversation_id,
            user_id: user_id.clone(),
            role: "user".to_string(),
            content: request.message.clone(),
            metadata: metadata.clone(), // Clone here
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

    info!("Created user message with id: {}", message.id);

    // Get Groq response
    let (ai_response, _) = state
        .groq
        .chat(request.message.clone(), false)
        .await
        .map_err(|e| {
            error!("Failed to get Groq response: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to get AI response: {}", e),
            )
        })?;

    // Save AI response
    let ai_message = chat_db
        .create_message(&CreateMessageRequest {
            conversation_id: request.conversation_id,
            user_id: user_id.clone(),
            role: "assistant".to_string(),
            content: ai_response,
            metadata, // Original value used here
            tool_calls: None,
        })
        .await
        .map_err(|e| {
            error!("Failed to save AI response: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to save AI response: {}", e),
            )
        })?;

    info!("Created AI message with id: {}", ai_message.id);

    Ok(Json(SendMessageResponse {
        id: ai_message.id.to_string(),
        message: ai_message.content,
    }))
}

pub async fn get_conversation_messages(
    cookies: CookieJar,
    State(state): State<AppState>,
    Path(conversation_id): Path<Uuid>,
) -> Result<Json<Vec<Message>>, (StatusCode, String)> {
    info!("Fetching messages for conversation: {}", conversation_id);

    // Get user info from session
    let user_id = if let Some(session_cookie) = cookies.get(SESSION_COOKIE_NAME) {
        session_cookie.value().to_string()
    } else {
        return Err((
            StatusCode::UNAUTHORIZED,
            "No session found. Please log in.".to_string(),
        ));
    };
    info!("Using user_id: {}", user_id);

    // Create chat database service
    let chat_db = ChatDatabaseService::new(state.pool);

    // Get messages
    let messages = chat_db
        .get_conversation_messages(conversation_id)
        .await
        .map_err(|e| {
            error!("Failed to fetch conversation messages: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to fetch conversation messages: {}", e),
            )
        })?;

    info!("Found {} messages", messages.len());

    Ok(Json(messages))
}