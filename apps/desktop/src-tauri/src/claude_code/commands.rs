use std::env;
use log::info;
use serde_json::Value;

use crate::error::CommandResult;
use crate::claude_code::{
    EnhancedConvexClient, ConvexDatabase, SessionRepository, MessageRepository, 
    AuthService, AuthContext
};
use crate::claude_code::database::{
    CreateSessionRequest, UpdateSessionRequest, CreateMessageRequest, UpdateMessageRequest
};
use crate::claude_code::models::{ConvexSession, ConvexMessage};

/// Parse and validate JSON metadata with robust error handling
fn parse_metadata(metadata: Option<String>) -> Result<Option<Value>, String> {
    match metadata {
        Some(meta) => {
            if meta.trim().is_empty() {
                Ok(None)
            } else {
                serde_json::from_str(&meta)
                    .map(Some)
                    .map_err(|e| {
                        format!("Invalid metadata JSON: {}. Please ensure the JSON is properly formatted.", e)
                    })
            }
        }
        None => Ok(None),
    }
}

/// Helper function to create a Convex client
async fn create_convex_client(auth_token: Option<String>) -> Result<EnhancedConvexClient, String> {
    // Load environment variables
    dotenvy::dotenv().ok();
    
    let convex_url = env::var("VITE_CONVEX_URL")
        .or_else(|_| env::var("CONVEX_URL"))
        .map_err(|_| "Convex URL not configured".to_string())?;
    
    EnhancedConvexClient::new(&convex_url, auth_token)
        .await
        .map_err(|e| format!("Failed to create Convex client: {}", e))
}

/// Test Convex connection
#[tauri::command]
pub async fn test_convex_connection(
    auth_token: Option<String>,
) -> Result<CommandResult<String>, String> {
    info!("Testing Convex connection");
    
    let _client = create_convex_client(auth_token).await?;
    
    info!("Convex connection test successful");
    Ok(CommandResult::success("Convex connection successful".to_string()))
}

/// Get sessions with optional filtering
#[tauri::command]
pub async fn get_sessions(
    auth_token: Option<String>,
    limit: Option<usize>,
    user_id: Option<String>,
) -> Result<CommandResult<Vec<ConvexSession>>, String> {
    info!("Getting sessions with limit: {:?}, user_id: {:?}", limit, user_id);
    
    let mut client = create_convex_client(auth_token).await?;
    let sessions = client.get_sessions(limit, user_id)
        .await
        .map_err(|e| format!("Failed to get sessions: {}", e))?;
    
    info!("Retrieved {} sessions", sessions.len());
    Ok(CommandResult::success(sessions))
}

/// Create a new Convex session
#[tauri::command]
pub async fn create_convex_session(
    auth_token: Option<String>,
    title: Option<String>,
    user_id: String,
    metadata: Option<String>,
) -> Result<CommandResult<String>, String> {
    info!("Creating session with title: {:?}, user_id: {}", title, user_id);
    
    let mut client = create_convex_client(auth_token).await?;
    
    let metadata_value = parse_metadata(metadata)?;
    
    let request = CreateSessionRequest {
        title,
        user_id,
        metadata: metadata_value,
    };
    
    let session_id = client.create_session(request)
        .await
        .map_err(|e| format!("Failed to create session: {}", e))?;
    
    info!("Created session with ID: {}", session_id);
    Ok(CommandResult::success(session_id))
}

/// Update an existing session
#[tauri::command]
pub async fn update_session(
    auth_token: Option<String>,
    session_id: String,
    title: Option<String>,
    metadata: Option<String>,
    status: Option<String>,
) -> Result<CommandResult<()>, String> {
    info!("Updating session: {}", session_id);
    
    let mut client = create_convex_client(auth_token).await?;
    
    let metadata_value = parse_metadata(metadata)?;
    
    let updates = UpdateSessionRequest {
        title,
        metadata: metadata_value,
        status,
    };
    
    client.update_session(&session_id, updates)
        .await
        .map_err(|e| format!("Failed to update session: {}", e))?;
    
    info!("Updated session: {}", session_id);
    Ok(CommandResult::success(()))
}

/// Delete a session
#[tauri::command]
pub async fn delete_session(
    auth_token: Option<String>,
    session_id: String,
) -> Result<CommandResult<()>, String> {
    info!("Deleting session: {}", session_id);
    
    let mut client = create_convex_client(auth_token).await?;
    
    client.delete_session(&session_id)
        .await
        .map_err(|e| format!("Failed to delete session: {}", e))?;
    
    info!("Deleted session: {}", session_id);
    Ok(CommandResult::success(()))
}

/// Get session by ID
#[tauri::command]
pub async fn get_session_by_id(
    auth_token: Option<String>,
    session_id: String,
) -> Result<CommandResult<Option<ConvexSession>>, String> {
    info!("Getting session by ID: {}", session_id);
    
    let mut client = create_convex_client(auth_token).await?;
    
    let session = client.get_session_by_id(&session_id)
        .await
        .map_err(|e| format!("Failed to get session: {}", e))?;
    
    info!("Retrieved session: {:?}", session.is_some());
    Ok(CommandResult::success(session))
}

/// Get messages for a Convex session
#[tauri::command]
pub async fn get_convex_messages(
    auth_token: Option<String>,
    session_id: String,
    limit: Option<usize>,
) -> Result<CommandResult<Vec<ConvexMessage>>, String> {
    info!("Getting messages for session: {}, limit: {:?}", session_id, limit);
    
    let mut client = create_convex_client(auth_token).await?;
    
    let messages = client.get_messages(&session_id, limit)
        .await
        .map_err(|e| format!("Failed to get messages: {}", e))?;
    
    info!("Retrieved {} messages", messages.len());
    Ok(CommandResult::success(messages))
}

/// Add a new message to a session
#[tauri::command]
pub async fn add_message(
    auth_token: Option<String>,
    session_id: String,
    content: String,
    role: String,
    metadata: Option<String>,
) -> Result<CommandResult<String>, String> {
    info!("Adding message to session: {}, role: {}", session_id, role);
    
    let mut client = create_convex_client(auth_token).await?;
    
    let metadata_value = parse_metadata(metadata)?;
    
    let request = CreateMessageRequest {
        session_id,
        content,
        role,
        metadata: metadata_value,
    };
    
    let message_id = client.add_message(request)
        .await
        .map_err(|e| format!("Failed to add message: {}", e))?;
    
    info!("Added message with ID: {}", message_id);
    Ok(CommandResult::success(message_id))
}

/// Update a message
#[tauri::command]
pub async fn update_message(
    auth_token: Option<String>,
    message_id: String,
    content: Option<String>,
    metadata: Option<String>,
    status: Option<String>,
) -> Result<CommandResult<()>, String> {
    info!("Updating message: {}", message_id);
    
    let mut client = create_convex_client(auth_token).await?;
    
    let metadata_value = parse_metadata(metadata)?;
    
    let updates = UpdateMessageRequest {
        content,
        metadata: metadata_value,
        status,
    };
    
    client.update_message(&message_id, updates)
        .await
        .map_err(|e| format!("Failed to update message: {}", e))?;
    
    info!("Updated message: {}", message_id);
    Ok(CommandResult::success(()))
}

/// Delete a message
#[tauri::command]
pub async fn delete_message(
    auth_token: Option<String>,
    message_id: String,
) -> Result<CommandResult<()>, String> {
    info!("Deleting message: {}", message_id);
    
    let mut client = create_convex_client(auth_token).await?;
    
    client.delete_message(&message_id)
        .await
        .map_err(|e| format!("Failed to delete message: {}", e))?;
    
    info!("Deleted message: {}", message_id);
    Ok(CommandResult::success(()))
}

/// Get message by ID
#[tauri::command]
pub async fn get_message_by_id(
    auth_token: Option<String>,
    message_id: String,
) -> Result<CommandResult<Option<ConvexMessage>>, String> {
    info!("Getting message by ID: {}", message_id);
    
    let mut client = create_convex_client(auth_token).await?;
    
    let message = client.get_message_by_id(&message_id)
        .await
        .map_err(|e| format!("Failed to get message: {}", e))?;
    
    info!("Retrieved message: {:?}", message.is_some());
    Ok(CommandResult::success(message))
}