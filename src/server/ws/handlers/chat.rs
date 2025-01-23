use crate::server::{
    services::{deepseek::StreamUpdate, ChatService},
    ws::types::{ChatRequest, ChatResponse},
};
use futures::StreamExt;
use std::sync::Arc;
use tokio::sync::mpsc::Sender;
use tokio_tungstenite::tungstenite::Message;
use tracing::error;

pub async fn handle_chat(service: Arc<ChatService>, tx: Sender<Message>, request: ChatRequest) {
    let mut stream = service.chat(request.message).await;

    let mut current_content = String::new();
    let mut current_reasoning = String::new();

    while let Some(update) = stream.recv().await {
        match update {
            StreamUpdate::Content(text) => {
                current_content.push_str(&text);
                if let Err(e) = tx
                    .send(Message::Text(
                        serde_json::to_string(&ChatResponse {
                            kind: "content".to_string(),
                            content: current_content.clone(),
                        })
                        .unwrap_or_default(),
                    ))
                    .await
                {
                    error!("Failed to send content: {}", e);
                }
            }
            StreamUpdate::Reasoning(text) => {
                current_reasoning.push_str(&text);
                if let Err(e) = tx
                    .send(Message::Text(
                        serde_json::to_string(&ChatResponse {
                            kind: "reasoning".to_string(),
                            content: current_reasoning.clone(),
                        })
                        .unwrap_or_default(),
                    ))
                    .await
                {
                    error!("Failed to send reasoning: {}", e);
                }
            }
            StreamUpdate::Done => {
                if let Err(e) = tx
                    .send(Message::Text(
                        serde_json::to_string(&ChatResponse {
                            kind: "done".to_string(),
                            content: String::new(),
                        })
                        .unwrap_or_default(),
                    ))
                    .await
                {
                    error!("Failed to send done message: {}", e);
                }
            }
            StreamUpdate::ToolCalls(tool_calls) => {
                if let Err(e) = tx
                    .send(Message::Text(
                        serde_json::to_string(&ChatResponse {
                            kind: "tool_calls".to_string(),
                            content: serde_json::to_string(&tool_calls).unwrap_or_default(),
                        })
                        .unwrap_or_default(),
                    ))
                    .await
                {
                    error!("Failed to send tool calls: {}", e);
                }
            }
        }
    }
}