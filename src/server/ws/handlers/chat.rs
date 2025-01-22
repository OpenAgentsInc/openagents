use super::MessageHandler;
use crate::server::ws::{transport::WebSocketState, types::ChatMessage};
use async_trait::async_trait;
use serde_json::json;
use std::error::Error;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{error, info};

pub struct ChatHandler {
    ws_state: Arc<WebSocketState>,
}

impl ChatHandler {
    pub fn new(ws_state: Arc<WebSocketState>) -> Self {
        Self { ws_state }
    }

    async fn process_message(
        &self,
        content: String,
    ) -> Result<String, Box<dyn Error + Send + Sync>> {
        info!("Processing message: {}", content);

        // Simulate processing delay
        sleep(Duration::from_secs(1)).await;

        // TODO: Implement actual chat processing
        // This should integrate with your agent/AI service
        let response = format!("You said: {}", content);
        info!("Generated response: {}", response);
        Ok(response)
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
                let response = self.process_message(content).await?;

                // Format response as expected by the client
                let response_json = json!({
                    "type": "chat",
                    "content": response,
                    "sender": "ai"
                });

                info!("Sending response: {}", response_json);
                // Send response back through WebSocket
                self.ws_state
                    .send_to(&conn_id, &response_json.to_string())
                    .await?;
            }
            _ => {
                error!("Unhandled message type: {:?}", msg);
            }
        }
        Ok(())
    }

    async fn broadcast(&self, _msg: Self::Message) -> Result<(), Box<dyn Error + Send + Sync>> {
        // Implement if needed
        Ok(())
    }
}
