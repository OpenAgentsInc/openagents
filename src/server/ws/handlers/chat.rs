use super::MessageHandler;
use crate::server::services::DeepSeekService;
use crate::server::ws::{transport::WebSocketState, types::ChatMessage};
use async_trait::async_trait;
use serde_json::json;
use std::error::Error;
use std::sync::Arc;
use tracing::{error, info};

pub struct ChatHandler {
    ws_state: Arc<WebSocketState>,
    deepseek_service: Arc<DeepSeekService>,
}

impl ChatHandler {
    pub fn new(ws_state: Arc<WebSocketState>, deepseek_service: Arc<DeepSeekService>) -> Self {
        Self {
            ws_state,
            deepseek_service,
        }
    }

    async fn process_message(
        &self,
        content: String,
        conn_id: &str,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        info!("Processing message: {}", content);

        // Get streaming response from DeepSeek
        let mut stream = self.deepseek_service.chat_stream(content, true).await;

        // Send "typing" indicator
        let typing_json = json!({
            "type": "chat",
            "content": "...",
            "sender": "ai",
            "status": "typing"
        });
        self.ws_state
            .send_to(conn_id, &typing_json.to_string())
            .await?;

        // Accumulate the full response while sending stream updates
        let mut full_response = String::new();
        while let Some(update) = stream.recv().await {
            match update {
                crate::server::services::deepseek::StreamUpdate::Content(content) => {
                    full_response.push_str(&content);
                    
                    // Send partial response
                    let response_json = json!({
                        "type": "chat",
                        "content": &content,
                        "sender": "ai",
                        "status": "streaming"
                    });
                    self.ws_state
                        .send_to(conn_id, &response_json.to_string())
                        .await?;
                }
                crate::server::services::deepseek::StreamUpdate::Reasoning(reasoning) => {
                    // Send reasoning update
                    let reasoning_json = json!({
                        "type": "chat",
                        "content": &reasoning,
                        "sender": "ai",
                        "status": "thinking"
                    });
                    self.ws_state
                        .send_to(conn_id, &reasoning_json.to_string())
                        .await?;
                }
                crate::server::services::deepseek::StreamUpdate::Done => {
                    // Send final complete message
                    let response_json = json!({
                        "type": "chat",
                        "content": full_response,
                        "sender": "ai",
                        "status": "complete"
                    });
                    self.ws_state
                        .send_to(conn_id, &response_json.to_string())
                        .await?;
                    break;
                }
            }
        }

        Ok(())
    }
}

#[async_trait]
impl MessageHandler for ChatHandler {
    type Message = ChatMessage;

    async fn handle_message(
        &self,
        msg: Self::Message,
        conn_id: String,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        info!("Handling chat message: {:?}", msg);
        match msg {
            ChatMessage::UserMessage { content } => {
                match self.process_message(content, &conn_id).await {
                    Ok(_) => Ok(()),
                    Err(e) => {
                        error!("Error processing message: {}", e);
                        let error_json = json!({
                            "type": "chat",
                            "content": format!("Error: {}", e),
                            "sender": "system",
                            "status": "error"
                        });
                        self.ws_state
                            .send_to(&conn_id, &error_json.to_string())
                            .await?;
                        Ok(())
                    }
                }
            }
            _ => {
                error!("Unhandled message type: {:?}", msg);
                Ok(())
            }
        }
    }

    async fn broadcast(&self, _msg: Self::Message) -> Result<(), Box<dyn Error + Send + Sync>> {
        // Implement if needed
        Ok(())
    }
}