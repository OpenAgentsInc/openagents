use axum::extract::ws::Message;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::sync::mpsc;
use tracing::{error, info, debug};
use uuid::Uuid;

use crate::server::{
    config::AppState,
    models::chat::{CreateConversationRequest, CreateMessageRequest},
    services::chat_database::ChatDatabaseService,
};

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum ChatMessage {
    Subscribe {
        scope: String,
        conversation_id: Option<Uuid>,
        last_sync_id: Option<i64>,
    },
    Message {
        id: Uuid,
        conversation_id: Option<Uuid>,
        content: String,
        repos: Option<Vec<String>>,
        use_reasoning: Option<bool>,
    },
}

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
pub enum ChatResponse {
    Subscribed {
        scope: String,
        last_sync_id: i64,
    },
    Update {
        message_id: Uuid,
        delta: ChatDelta,
    },
    Complete {
        message_id: Uuid,
        conversation_id: Uuid,
    },
    Error {
        message: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatDelta {
    pub content: Option<String>,
    pub reasoning: Option<String>,
}

pub struct ChatHandler {
    tx: mpsc::Sender<String>,
    state: AppState,
    user_id: String,
}

impl ChatHandler {
    pub fn new(tx: mpsc::Sender<String>, state: AppState, user_id: String) -> Self {
        info!("Creating new ChatHandler for user: {}", user_id);
        Self { tx, state, user_id }
    }

    pub async fn process_message(
        &mut self,
        msg: Message,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        match msg {
            Message::Text(text) => {
                debug!("Processing text message: {}", text);
                let chat_msg: ChatMessage = serde_json::from_str(&text)?;
                self.handle_message(chat_msg).await?;
            }
            Message::Close(_) => {
                info!("WebSocket connection closed");
            }
            _ => {
                debug!("Ignoring non-text message");
            }
        }
        Ok(())
    }

    pub async fn handle_message(
        &mut self,
        msg: ChatMessage,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("Handling chat message: {:?}", msg);
        match msg {
            ChatMessage::Subscribe { scope, .. } => {
                info!("Processing subscribe message for scope: {}", scope);
                self.broadcast(ChatResponse::Subscribed {
                    scope,
                    last_sync_id: 0,
                })
                .await?;
            }

            ChatMessage::Message {
                id,
                conversation_id,
                content,
                repos,
                use_reasoning,
            } => {
                info!("Processing chat message: id={}, conv={:?}", id, conversation_id);
                let chat_db = ChatDatabaseService::new(self.state.pool.clone());

                // Handle conversation creation or lookup
                let conversation = match conversation_id {
                    Some(id) => {
                        info!("Looking up existing conversation: {}", id);
                        let conv = chat_db.get_conversation(id).await.map_err(|e| {
                            error!("Failed to get conversation: {:?}", e);
                            if e.to_string().contains("Conversation not found") {
                                format!("Conversation {} not found", id)
                            } else {
                                format!("Failed to get conversation: {}", e)
                            }
                        })?;
                        // Verify user has access
                        if conv.user_id != self.user_id {
                            error!("Unauthorized access attempt to conversation {} by user {}", id, self.user_id);
                            return Err("Unauthorized access to conversation".into());
                        }
                        conv
                    }
                    None => {
                        info!("Creating new conversation for message: {}", id);
                        chat_db
                            .create_conversation(&CreateConversationRequest {
                                id: Some(id),
                                user_id: self.user_id.clone(),
                                title: Some(format!("Chat: {}", content)),
                            })
                            .await
                            .map_err(|e| {
                                error!("Failed to create conversation: {:?}", e);
                                format!("Failed to create conversation: {}", e)
                            })?
                    }
                };

                info!("Using conversation: {}", conversation.id);

                // Create user message
                let user_message = chat_db
                    .create_message(&CreateMessageRequest {
                        conversation_id: conversation.id,
                        user_id: self.user_id.clone(),
                        role: "user".to_string(),
                        content: content.clone(),
                        reasoning: None,
                        metadata: repos.clone().map(|r| json!({ "repos": r })),
                        tool_calls: None,
                    })
                    .await
                    .map_err(|e| {
                        error!("Failed to create user message: {:?}", e);
                        format!("Failed to create user message: {}", e)
                    })?;

                info!("Created user message: {}", user_message.id);

                // Get conversation history if this is a follow-up
                let mut messages = if conversation_id.is_some() {
                    info!("Loading conversation history for {}", conversation.id);
                    chat_db
                        .get_conversation_messages(conversation.id)
                        .await
                        .map_err(|e| {
                            error!("Failed to get conversation history: {:?}", e);
                            format!("Failed to get conversation history: {}", e)
                        })?
                        .iter()
                        .map(|msg| {
                            json!({
                                "role": msg.role,
                                "content": msg.content
                            })
                        })
                        .collect::<Vec<_>>()
                } else {
                    vec![json!({
                        "role": "user",
                        "content": content
                    })]
                };

                // Add current message if this is a follow-up
                if conversation_id.is_some() {
                    messages.push(json!({
                        "role": "user",
                        "content": content
                    }));
                }

                info!("Starting Groq stream with {} messages", messages.len());

                // Start Groq stream
                let mut stream = self
                    .state
                    .groq
                    .chat_with_history_stream(messages, use_reasoning.unwrap_or(false))
                    .await
                    .map_err(|e| {
                        error!("Failed to start Groq stream: {:?}", e);
                        format!("Failed to start AI response: {}", e)
                    })?;

                // Stream updates
                let mut content = String::new();
                let mut reasoning = String::new();

                info!("Processing stream updates");
                while let Some(update) = stream.next().await {
                    match update {
                        Ok(delta) => {
                            debug!("Received delta: {}", delta);
                            let delta: ChatDelta = serde_json::from_str(&delta).map_err(|e| {
                                error!("Failed to parse delta: {:?}", e);
                                format!("Failed to parse AI response: {}", e)
                            })?;

                            // Send incremental update
                            self.broadcast(ChatResponse::Update {
                                message_id: id,
                                delta: delta.clone(),
                            })
                            .await?;

                            // Accumulate content
                            if let Some(c) = delta.content {
                                content.push_str(&c);
                                debug!("Updated content length: {}", content.len());
                            }
                            if let Some(r) = delta.reasoning {
                                reasoning.push_str(&r);
                                debug!("Updated reasoning length: {}", reasoning.len());
                            }
                        }
                        Err(e) => {
                            error!("Stream error: {:?}", e);
                            self.broadcast(ChatResponse::Error {
                                message: e.to_string(),
                            })
                            .await?;
                            return Err(e.into());
                        }
                    }
                }

                info!("Stream completed, saving final message");

                // Save final message
                let ai_message = chat_db
                    .create_message(&CreateMessageRequest {
                        conversation_id: conversation.id,
                        user_id: self.user_id.clone(),
                        role: "assistant".to_string(),
                        content,
                        reasoning: if reasoning.is_empty() {
                            None
                        } else {
                            Some(json!(reasoning))
                        },
                        metadata: repos.map(|r| json!({ "repos": r })),
                        tool_calls: None,
                    })
                    .await
                    .map_err(|e| {
                        error!("Failed to save AI message: {:?}", e);
                        format!("Failed to save AI response: {}", e)
                    })?;

                info!("Created AI message: {}", ai_message.id);

                // Send completion
                self.broadcast(ChatResponse::Complete {
                    message_id: id,
                    conversation_id: conversation.id,
                })
                .await?;

                info!("Message handling completed for {}", id);
            }
        }

        Ok(())
    }

    async fn broadcast(
        &mut self,
        response: ChatResponse,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let msg = serde_json::to_string(&response)?;
        debug!("Broadcasting response: {}", msg);
        self.tx.send(msg).await.map_err(|e| {
            error!("Failed to send message: {:?}", e);
            format!("Failed to send message: {}", e)
        })?;
        Ok(())
    }
}