use crate::server::models::chat::{
    Conversation, CreateConversationRequest, CreateMessageRequest, Message,
};
use anyhow::{Context, Result};
use sqlx::PgPool;
use uuid::Uuid;

pub struct ChatDatabase {
    pool: PgPool,
}

impl ChatDatabase {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create_conversation(
        &self,
        request: CreateConversationRequest,
    ) -> Result<Conversation> {
        let conversation = sqlx::query_as!(
            Conversation,
            r#"
            INSERT INTO conversations (user_id, title)
            VALUES ($1, $2)
            RETURNING id, user_id, title, created_at, updated_at
            "#,
            request.user_id,
            request.title
        )
        .fetch_one(&self.pool)
        .await
        .context("Failed to create conversation")?;

        Ok(conversation)
    }

    pub async fn add_message(&self, request: CreateMessageRequest) -> Result<Message> {
        // First verify the conversation exists
        self.get_conversation(request.conversation_id)
            .await
            .context("Conversation not found")?;

        let message = sqlx::query_as!(
            Message,
            r#"
            INSERT INTO messages (conversation_id, role, content, metadata, tool_calls)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING id, conversation_id, role, content, created_at, metadata, tool_calls
            "#,
            request.conversation_id,
            request.role,
            request.content,
            request.metadata,
            request.tool_calls
        )
        .fetch_one(&self.pool)
        .await
        .context("Failed to create message")?;

        // Update conversation updated_at timestamp
        sqlx::query!(
            r#"
            UPDATE conversations 
            SET updated_at = NOW() 
            WHERE id = $1
            "#,
            request.conversation_id
        )
        .execute(&self.pool)
        .await
        .context("Failed to update conversation timestamp")?;

        Ok(message)
    }

    pub async fn get_conversation(&self, id: Uuid) -> Result<Conversation> {
        let conversation = sqlx::query_as!(
            Conversation,
            r#"
            SELECT id, user_id, title, created_at, updated_at
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
            SELECT id, conversation_id, role, content, created_at, metadata, tool_calls
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
            SELECT id, user_id, title, created_at, updated_at
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
