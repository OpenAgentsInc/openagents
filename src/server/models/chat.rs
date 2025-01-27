use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::types::time::OffsetDateTime;
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct Conversation {
    pub id: Uuid,
    pub user_id: String,
    pub title: Option<String>,
    pub created_at: Option<OffsetDateTime>,
    pub updated_at: Option<OffsetDateTime>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct Message {
    pub id: Uuid,
    pub conversation_id: Uuid,
    pub role: String,
    pub content: String,
    pub created_at: Option<OffsetDateTime>,
    pub metadata: Option<Value>,
    pub tool_calls: Option<Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateConversationRequest {
    pub user_id: String,
    pub title: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CreateMessageRequest {
    pub conversation_id: Uuid,
    pub role: String,
    pub content: String,
    pub metadata: Option<Value>,
    pub tool_calls: Option<Value>,
}

impl Conversation {
    pub fn new(user_id: String, title: Option<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            user_id,
            title,
            created_at: Some(OffsetDateTime::now_utc()),
            updated_at: Some(OffsetDateTime::now_utc()),
        }
    }
}

impl Message {
    pub fn new(
        conversation_id: Uuid,
        role: String,
        content: String,
        metadata: Option<Value>,
        tool_calls: Option<Value>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            conversation_id,
            role,
            content,
            created_at: Some(OffsetDateTime::now_utc()),
            metadata,
            tool_calls,
        }
    }
}