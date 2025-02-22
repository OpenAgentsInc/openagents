use axum::{
    extract::{Json, Path, State},
    http::StatusCode,
};
use axum_extra::extract::cookie::CookieJar;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::{error, info};
use uuid::Uuid;

use crate::server::{
    config::AppState,
    handlers::oauth::session::SESSION_COOKIE_NAME,
    models::chat::{CreateConversationRequest, CreateMessageRequest, Message},
    services::chat_database::ChatDatabaseService,
    ws::handlers::chat::{ChatDelta, ChatResponse},
};

#[derive(Debug, Deserialize)]
pub struct StartRepoChatRequest {
    pub id: Uuid,
    pub message: String,
    pub repos: Vec<String>,
    pub scope: String,
    pub use_reasoning: Option<bool>, // Add reasoning flag
}

#[derive(Debug, Deserialize)]
pub struct SendMessageRequest {
    pub conversation_id: Uuid,
    pub message: String,
    pub repos: Option<Vec<String>>,
    pub use_reasoning: Option<bool>, // Add reasoning flag
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

    // Create conversation with client's UUID
    let conversation = chat_db
        .create_conversation(&CreateConversationRequest {
            id: Some(request.id), // Use the client's UUID
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
    let _message = chat_db
        .create_message(&CreateMessageRequest {
            conversation_id: conversation.id,
            user_id: user_id.clone(),
            role: "user".to_string(),
            content: request.message.clone(),
            reasoning: None,
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

    info!("Created message with id: {}", _message.id);

    // Convert message to Groq format
    let messages = vec![json!({
        "role": "user",
        "content": request.message
    })];

    // Start Groq stream
    let mut stream = state
        .groq
        .chat_with_history_stream(messages, request.use_reasoning.unwrap_or(false))
        .await
        .map_err(|e| {
            error!("Failed to get Groq stream: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to get AI response: {}", e),
            )
        })?;

    // Process stream and accumulate content
    let mut content = String::new();
    let mut reasoning = String::new();

    // Broadcast updates through WebSocket
    while let Some(update) = stream.next().await {
        match update {
            Ok(delta_str) => {
                let delta: ChatDelta = serde_json::from_str(&delta_str).map_err(|e| {
                    error!("Failed to parse delta: {:?}", e);
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        format!("Failed to parse AI response: {}", e),
                    )
                })?;

                // Send update through WebSocket
                let update_response = ChatResponse::Update {
                    message_id: _message.id,
                    connection_id: None,
                    delta: delta.clone(),
                };
                let msg = serde_json::to_string(&update_response).map_err(|e| {
                    error!("Failed to serialize update: {:?}", e);
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        format!("Failed to serialize update: {}", e),
                    )
                })?;
                state.ws_state.broadcast(&msg).await.map_err(|e| {
                    error!("Failed to broadcast update: {:?}", e);
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        format!("Failed to broadcast update: {}", e),
                    )
                })?;

                // Accumulate content
                if let Some(c) = delta.content {
                    content.push_str(&c);
                }
                if let Some(r) = delta.reasoning {
                    reasoning.push_str(&r);
                }
            }
            Err(e) => {
                error!("Stream error: {:?}", e);
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Stream error: {}", e),
                ));
            }
        }
    }

    // Save final AI response with reasoning
    let ai_message = chat_db
        .create_message(&CreateMessageRequest {
            conversation_id: conversation.id,
            user_id: user_id.clone(),
            role: "assistant".to_string(),
            content,
            reasoning: if reasoning.is_empty() {
                None
            } else {
                Some(json!(reasoning))
            },
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

    // Send completion through WebSocket
    let complete_response = ChatResponse::Complete {
        message_id: _message.id,
        connection_id: None,
        conversation_id: conversation.id,
    };
    let msg = serde_json::to_string(&complete_response).map_err(|e| {
        error!("Failed to serialize completion: {:?}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to serialize completion: {}", e),
        )
    })?;
    state.ws_state.broadcast(&msg).await.map_err(|e| {
        error!("Failed to broadcast completion: {:?}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to broadcast completion: {}", e),
        )
    })?;

    Ok(Json(StartChatResponse {
        id: conversation.id.to_string(),
        initial_message: _message.content,
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

    // Get conversation and verify it exists
    let conversation = chat_db
        .get_conversation(request.conversation_id)
        .await
        .map_err(|e| {
            error!("Failed to get conversation: {:?}", e);
            if e.to_string().contains("Conversation not found") {
                (
                    StatusCode::NOT_FOUND,
                    format!("Conversation {} not found", request.conversation_id),
                )
            } else {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Failed to get conversation: {}", e),
                )
            }
        })?;

    // Verify user has access to this conversation
    if conversation.user_id != user_id {
        return Err((
            StatusCode::FORBIDDEN,
            "You do not have access to this conversation".to_string(),
        ));
    }

    // Get conversation history
    let messages = chat_db
        .get_conversation_messages(request.conversation_id)
        .await
        .map_err(|e| {
            error!("Failed to get conversation history: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to get conversation history: {}", e),
            )
        })?;

    // Create user message
    let _message = chat_db
        .create_message(&CreateMessageRequest {
            conversation_id: request.conversation_id,
            user_id: user_id.clone(),
            role: "user".to_string(),
            content: request.message.clone(),
            reasoning: None,
            metadata: request.repos.clone().map(|repos| json!({ "repos": repos })),
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

    // Convert messages to Groq format
    let mut chat_messages: Vec<serde_json::Value> = messages
        .iter()
        .map(|msg| {
            json!({
                "role": msg.role,
                "content": msg.content
            })
        })
        .collect();

    // Add current message
    chat_messages.push(json!({
        "role": "user",
        "content": request.message
    }));

    // Start Groq stream
    let mut stream = state
        .groq
        .chat_with_history_stream(chat_messages, request.use_reasoning.unwrap_or(false))
        .await
        .map_err(|e| {
            error!("Failed to get Groq stream: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to get AI response: {}", e),
            )
        })?;

    // Process stream and accumulate content
    let mut content = String::new();
    let mut reasoning = String::new();

    // Broadcast updates through WebSocket
    while let Some(update) = stream.next().await {
        match update {
            Ok(delta_str) => {
                let delta: ChatDelta = serde_json::from_str(&delta_str).map_err(|e| {
                    error!("Failed to parse delta: {:?}", e);
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        format!("Failed to parse AI response: {}", e),
                    )
                })?;

                // Send update through WebSocket
                let update_response = ChatResponse::Update {
                    message_id: _message.id,
                    connection_id: None,
                    delta: delta.clone(),
                };
                let msg = serde_json::to_string(&update_response).map_err(|e| {
                    error!("Failed to serialize update: {:?}", e);
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        format!("Failed to serialize update: {}", e),
                    )
                })?;
                state.ws_state.broadcast(&msg).await.map_err(|e| {
                    error!("Failed to broadcast update: {:?}", e);
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        format!("Failed to broadcast update: {}", e),
                    )
                })?;

                // Accumulate content
                if let Some(c) = delta.content {
                    content.push_str(&c);
                }
                if let Some(r) = delta.reasoning {
                    reasoning.push_str(&r);
                }
            }
            Err(e) => {
                error!("Stream error: {:?}", e);
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Stream error: {}", e),
                ));
            }
        }
    }

    // Save final AI response with reasoning
    let ai_message = chat_db
        .create_message(&CreateMessageRequest {
            conversation_id: request.conversation_id,
            user_id,
            role: "assistant".to_string(),
            content: content.clone(),
            reasoning: if reasoning.is_empty() {
                None
            } else {
                Some(json!(reasoning))
            },
            metadata: request.repos.clone().map(|repos| json!({ "repos": repos })),
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

    // Send completion through WebSocket
    let complete_response = ChatResponse::Complete {
        message_id: ai_message.id,
        connection_id: None,
        conversation_id: request.conversation_id,
    };
    let msg = serde_json::to_string(&complete_response).map_err(|e| {
        error!("Failed to serialize completion: {:?}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to serialize completion: {}", e),
        )
    })?;
    state.ws_state.broadcast(&msg).await.map_err(|e| {
        error!("Failed to broadcast completion: {:?}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to broadcast completion: {}", e),
        )
    })?;

    Ok(Json(SendMessageResponse {
        id: ai_message.id.to_string(),
        message: content,
    }))
}

pub async fn get_conversation_messages(
    cookies: CookieJar,
    State(state): State<AppState>,
    Path(conversation_id): Path<Uuid>,
) -> Result<Json<Vec<Message>>, (StatusCode, String)> {
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

    // Get conversation and verify it exists
    let conversation = chat_db
        .get_conversation(conversation_id)
        .await
        .map_err(|e| {
            error!("Failed to get conversation: {:?}", e);
            if e.to_string().contains("Conversation not found") {
                (
                    StatusCode::NOT_FOUND,
                    format!("Conversation {} not found", conversation_id),
                )
            } else {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Failed to get conversation: {}", e),
                )
            }
        })?;

    // Verify user has access to this conversation
    if conversation.user_id != user_id {
        return Err((
            StatusCode::FORBIDDEN,
            "You do not have access to this conversation".to_string(),
        ));
    }

    // Get messages
    let messages = chat_db
        .get_conversation_messages(conversation_id)
        .await
        .map_err(|e| {
            error!("Failed to get messages: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to get messages: {}", e),
            )
        })?;

    Ok(Json(messages))
}
