use axum::extract::ws::{Message, WebSocket};
use futures::StreamExt;
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
    ws: WebSocket,
    state: AppState,
    user_id: String,
}

impl ChatHandler {
    pub fn new(ws: WebSocket, state: AppState, user_id: String) -> Self {
        Self { ws, state, user_id }
    }

    pub async fn process_message(
        &mut self,
        msg: Message,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        match msg {
            Message::Text(text) => {
                let chat_msg: ChatMessage = serde_json::from_str(&text)?;
                self.handle_message(chat_msg).await?;
            }
            Message::Close(_) => {
                info!("WebSocket connection closed");
            }
            _ => {}
        }
        Ok(())
    }

    pub async fn handle_message(
        &mut self,
        msg: ChatMessage,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        match msg {
            ChatMessage::Subscribe { scope, .. } => {
                self.broadcast(ChatResponse::Subscribed {
                    scope,
                    last_sync_id: 0, // TODO: Implement sync ID tracking
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
                let chat_db = ChatDatabaseService::new(self.state.pool.clone());

                // Handle conversation creation or lookup
                let conversation = match conversation_id {
                    Some(id) => {
                        let conv = chat_db.get_conversation(id).await?;
                        // Verify user has access
                        if conv.user_id != self.user_id {
                            return Err("Unauthorized access to conversation".into());
                        }
                        conv
                    }
                    None => {
                        chat_db
                            .create_conversation(&CreateConversationRequest {
                                id: Some(id),
                                user_id: self.user_id.clone(),
                                title: Some(format!("Chat: {}", content)),
                            })
                            .await?
                    }
                };

                // Create user message
                let _user_message = chat_db
                    .create_message(&CreateMessageRequest {
                        conversation_id: conversation.id,
                        user_id: self.user_id.clone(),
                        role: "user".to_string(),
                        content: content.clone(),
                        reasoning: None,
                        metadata: repos.clone().map(|r| json!({ "repos": r })),
                        tool_calls: None,
                    })
                    .await?;

                // Get conversation history if this is a follow-up
                let mut messages = if conversation_id.is_some() {
                    chat_db
                        .get_conversation_messages(conversation.id)
                        .await?
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

                // Start Groq stream
                let mut stream = self
                    .state
                    .groq
                    .chat_with_history_stream(messages, use_reasoning.unwrap_or(false))
                    .await?;

                // Stream updates
                let mut content = String::new();
                let mut reasoning = String::new();

                while let Some(update) = stream.next().await {
                    match update {
                        Ok(delta) => {
                            let delta: ChatDelta = serde_json::from_str(&delta)?;
                            if let Some(c) = delta.content {
                                content.push_str(&c);
                            }
                            if let Some(r) = delta.reasoning {
                                reasoning.push_str(&r);
                            }

                            self.broadcast(ChatResponse::Update {
                                message_id: id,
                                delta: ChatDelta {
                                    content: Some(content.clone()),
                                    reasoning: Some(reasoning.clone()),
                                },
                            })
                            .await?;
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

                // Save final message
                let _ai_message = chat_db
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
                    .await?;

                // Send completion
                self.broadcast(ChatResponse::Complete {
                    message_id: id,
                    conversation_id: conversation.id,
                })
                .await?;
            }
        }

        Ok(())
    }

    async fn broadcast(
        &mut self,
        response: ChatResponse,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let msg = serde_json::to_string(&response)?;
        self.ws.send(Message::Text(msg)).await?;
        Ok(())
    }
}
