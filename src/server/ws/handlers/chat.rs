use super::MessageHandler;
use crate::server::services::github_issue::GitHubService;
use crate::server::services::gemini::{service::GeminiService, StreamUpdate};
use crate::server::ws::{transport::WebSocketState, types::ChatMessage};
use async_trait::async_trait;
use serde_json::json;
use std::error::Error;
use std::sync::Arc;
use tracing::{error, info};
use regex::Regex;

pub struct ChatHandler {
    ws_state: Arc<WebSocketState>,
    github_service: Arc<GitHubService>,
}

impl ChatHandler {
    pub fn new(ws_state: Arc<WebSocketState>, github_service: Arc<GitHubService>) -> Self {
        Self {
            ws_state,
            github_service,
        }
    }

    async fn process_message(
        &self,
        content: String,
        conn_id: &str,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        info!("Processing message: {}", content);

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

        // Check if it's a GitHub issue URL
        let re = Regex::new(r"https://github\.com/([^/]+)/([^/]+)/issues/(\d+)").unwrap();
        if let Some(captures) = re.captures(&content) {
            let owner = captures.get(1).unwrap().as_str();
            let repo = captures.get(2).unwrap().as_str();
            let issue_number = captures.get(3).unwrap().as_str().parse::<i32>()?;

            // Send status update
            let status_json = json!({
                "type": "chat",
                "content": format!("Fetching GitHub issue #{} from {}/{}", issue_number, owner, repo),
                "sender": "ai",
                "status": "tool_calls"
            });
            self.ws_state
                .send_to(conn_id, &status_json.to_string())
                .await?;

            // Fetch the issue
            let issue = self
                .github_service
                .get_issue(owner, repo, issue_number)
                .await?;

            // Initialize Gemini service
            let gemini = match GeminiService::new() {
                Ok(service) => service,
                Err(e) => {
                    let error_json = json!({
                        "type": "chat",
                        "content": format!("Error initializing Gemini service: {}", e),
                        "sender": "system",
                        "status": "error"
                    });
                    self.ws_state
                        .send_to(conn_id, &error_json.to_string())
                        .await?;
                    return Ok(());
                }
            };

            // Get repository context (this would be expanded in a real implementation)
            let repo_context = "Repository context would be here";

            // Stream the file analysis
            let mut stream = gemini.analyze_files_stream(
                &issue.body.unwrap_or_default(),
                &vec!["src/lib.rs".to_string()], // This would be replaced with actual file list
                repo_context,
            ).await;

            let mut full_response = String::new();
            
            // Process the streaming response
            while let Some(update) = stream.recv().await {
                match update {
                    StreamUpdate::Content(content) => {
                        let response_json = json!({
                            "type": "chat",
                            "content": &content,
                            "sender": "ai",
                            "status": "streaming"
                        });
                        self.ws_state
                            .send_to(conn_id, &response_json.to_string())
                            .await?;
                        full_response.push_str(&content);
                    }
                    StreamUpdate::Done => {
                        let final_json = json!({
                            "type": "chat",
                            "content": full_response,
                            "sender": "ai",
                            "status": "complete"
                        });
                        self.ws_state
                            .send_to(conn_id, &final_json.to_string())
                            .await?;
                    }
                }
            }
        } else {
            // For non-GitHub issue messages, use regular chat
            let mut stream = self.ws_state.model_router.chat_stream(content).await;
            let mut full_response = String::new();

            while let Some(update) = stream.recv().await {
                match update {
                    crate::server::services::deepseek::StreamUpdate::Content(content) => {
                        full_response.push_str(&content);
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
                    _ => {}
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