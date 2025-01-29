use crate::server::services::StreamUpdate;
use crate::server::ws::transport::WebSocketState;
use crate::server::ws::types::Message;
use anyhow::Result;
use async_trait::async_trait;
use serde_json::json;
use std::sync::Arc;
use tokio::sync::mpsc;

pub struct ChatHandler {
    state: Arc<WebSocketState>,
    tx: mpsc::Sender<Message>,
}

impl ChatHandler {
    pub fn new(state: Arc<WebSocketState>, tx: mpsc::Sender<Message>) -> Self {
        Self { state, tx }
    }

    pub async fn process_message(&self, msg: Message) -> Result<()> {
        match msg {
            Message::Text(text) => {
                let mut stream = self.state.chat_model.chat_stream(text, false).await;

                while let Some(update) = stream.recv().await {
                    match update {
                        StreamUpdate::Content(content) => {
                            self.tx
                                .send(Message::Text(json!({ "content": content }).to_string()))
                                .await?;
                        }
                        StreamUpdate::ReasoningContent(content) => {
                            self.tx
                                .send(Message::Text(
                                    json!({ "reasoning_content": content }).to_string(),
                                ))
                                .await?;
                        }
                        StreamUpdate::Error(error) => {
                            self.tx
                                .send(Message::Text(json!({ "error": error }).to_string()))
                                .await?;
                        }
                        StreamUpdate::Done => {
                            self.tx
                                .send(Message::Text(json!({ "done": true }).to_string()))
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
    async fn handle_message(&self, msg: Message) -> Result<()> {
        self.process_message(msg).await
    }

    async fn broadcast(&self, msg: Message) -> Result<()> {
        self.tx.send(msg).await?;
        Ok(())
    }
}