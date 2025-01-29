use crate::server::services::StreamUpdate;
use crate::server::ws::transport::WebSocketState;
use crate::server::ws::types::ChatMessage;
use anyhow::Result;
use async_trait::async_trait;
use serde_json::json;
use std::sync::Arc;
use tokio::sync::mpsc;

pub struct ChatHandler {
    state: Arc<WebSocketState>,
    tx: mpsc::Sender<ChatMessage>,
}

impl ChatHandler {
    pub fn new(state: Arc<WebSocketState>, tx: mpsc::Sender<ChatMessage>) -> Self {
        Self { state, tx }
    }

    pub async fn process_message(&self, msg: ChatMessage, conn_id: String) -> Result<()> {
        match msg {
            ChatMessage::UserMessage { content } => {
                let mut stream = self.state.model_router.chat_stream(content).await;

                while let Some(update) = stream.recv().await {
                    match update {
                        StreamUpdate::Content(content) => {
                            self.tx
                                .send(ChatMessage::AIMessage {
                                    content,
                                    status: "streaming".to_string(),
                                })
                                .await?;
                        }
                        StreamUpdate::ReasoningContent(content) => {
                            self.tx
                                .send(ChatMessage::AIMessage {
                                    content,
                                    status: "thinking".to_string(),
                                })
                                .await?;
                        }
                        StreamUpdate::Error(error) => {
                            self.tx
                                .send(ChatMessage::SystemMessage {
                                    content: error,
                                    status: "error".to_string(),
                                })
                                .await?;
                        }
                        StreamUpdate::Done => {
                            self.tx
                                .send(ChatMessage::SystemMessage {
                                    content: "Done".to_string(),
                                    status: "complete".to_string(),
                                })
                                .await?;
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }

        Ok(())
    }
}

#[async_trait]
impl super::MessageHandler for ChatHandler {
    type Message = ChatMessage;

    async fn handle_message(&self, msg: Self::Message, conn_id: String) -> Result<()> {
        self.process_message(msg, conn_id).await
    }

    async fn broadcast(&self, msg: Self::Message) -> Result<()> {
        self.tx.send(msg).await?;
        Ok(())
    }
}