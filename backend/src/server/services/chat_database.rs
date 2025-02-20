use crate::server::models::chat::{
    Conversation, CreateConversationRequest, CreateMessageRequest, Message,
};
use anyhow::{Context, Result};
use sqlx::PgPool;
use uuid::Uuid;

pub struct ChatDatabaseService {
    pool: PgPool,
}

impl ChatDatabaseService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create_conversation(
        &self,
        request: &CreateConversationRequest,
    ) -> Result<Conversation> {
        let conversation = sqlx::query_as!(
            Conversation,
            r#"
            INSERT INTO conversations (user_id, title, created_at, updated_at)
            VALUES ($1, $2, NOW(), NOW())
            RETURNING id, user_id, title, created_at as "created_at: _", updated_at as "updated_at: _"
            "#,
            request.user_id,
            request.title
        )
        .fetch_one(&self.pool)
        .await
        .context("Failed to create conversation")?;

        Ok(conversation)
    }

    pub async fn create_message(&self, request: &CreateMessageRequest) -> Result<Message> {
        let message = sqlx::query_as!(
            Message,
            r#"
            INSERT INTO messages (conversation_id, user_id, role, content, metadata, tool_calls, created_at)
            VALUES ($1, $2, $3, $4, $5, $6, NOW())
            RETURNING id, conversation_id, user_id, role, content, created_at as "created_at: _", metadata, tool_calls
            "#,
            request.conversation_id,
            request.user_id,
            request.role,
            request.content,
            request.metadata,
            request.tool_calls
        )
        .fetch_one(&self.pool)
        .await
        .context("Failed to create message")?;

        Ok(message)
    }

    pub async fn get_conversation(&self, id: Uuid) -> Result<Conversation> {
        let conversation = sqlx::query_as!(
            Conversation,
            r#"
            SELECT id, user_id, title, created_at as "created_at: _", updated_at as "updated_at: _"
            FROM conversations
            WHERE id = $1
            "#,
            id
        )
        .fetch_one(&self.pool)
        .await
        .context("Conversation not found")?;

        Ok(conversation)
    }

    pub async fn get_conversation_messages(&self, conversation_id: Uuid) -> Result<Vec<Message>> {
        let messages = sqlx::query_as!(
            Message,
            r#"
            SELECT id, conversation_id, user_id, role, content, created_at as "created_at: _", metadata, tool_calls
            FROM messages
            WHERE conversation_id = $1
            ORDER BY created_at ASC
            "#,
            conversation_id
        )
        .fetch_all(&self.pool)
        .await
        .context("Failed to fetch conversation messages")?;

        Ok(messages)
    }

    pub async fn list_user_conversations(&self, user_id: &str) -> Result<Vec<Conversation>> {
        let conversations = sqlx::query_as!(
            Conversation,
            r#"
            SELECT id, user_id, title, created_at as "created_at: _", updated_at as "updated_at: _"
            FROM conversations
            WHERE user_id = $1
            ORDER BY updated_at DESC
            "#,
            user_id
        )
        .fetch_all(&self.pool)
        .await
        .context("Failed to fetch user conversations")?;

        Ok(conversations)
    }

    pub async fn delete_conversation(&self, id: Uuid) -> Result<()> {
        // Messages will be deleted automatically due to ON DELETE CASCADE
        sqlx::query!(
            r#"
            DELETE FROM conversations
            WHERE id = $1
            "#,
            id
        )
        .execute(&self.pool)
        .await
        .context("Failed to delete conversation")?;

        Ok(())
    }
}
