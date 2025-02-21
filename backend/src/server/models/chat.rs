use crate::server::models::timestamp::Timestamp;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct Conversation {
    pub id: Uuid,
    pub user_id: String,
    pub title: Option<String>,
    pub created_at: Option<Timestamp>,
    pub updated_at: Option<Timestamp>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct Message {
    pub id: Uuid,
    pub conversation_id: Uuid,
    pub user_id: String,
    pub role: String,
    pub content: String,
    pub reasoning: Option<Value>,
    pub created_at: Option<Timestamp>,
    pub metadata: Option<Value>,
    pub tool_calls: Option<Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateConversationRequest {
    pub user_id: String,
    pub title: Option<String>,
    pub id: Option<Uuid>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CreateMessageRequest {
    pub conversation_id: Uuid,
    pub user_id: String,
    pub role: String,
    pub content: String,
    pub reasoning: Option<Value>,
    pub metadata: Option<Value>,
    pub tool_calls: Option<Value>,
}

impl Conversation {
    pub fn new(user_id: String, title: Option<String>, id: Option<Uuid>) -> Self {
        Self {
            id: id.unwrap_or_else(Uuid::new_v4),
            user_id,
            title,
            created_at: Some(Timestamp::now()),
            updated_at: Some(Timestamp::now()),
        }
    }
}

impl Message {
    pub fn new(
        conversation_id: Uuid,
        user_id: String,
        role: String,
        content: String,
        reasoning: Option<Value>,
        metadata: Option<Value>,
        tool_calls: Option<Value>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            conversation_id,
            user_id,
            role,
            content,
            reasoning,
            created_at: Some(Timestamp::now()),
            metadata,
            tool_calls,
        }
    }
}
