use super::MessageHandler;
use crate::server::services::github_issue::GitHubService;
use crate::server::ws::{transport::WebSocketState, types::ChatMessage};
use async_trait::async_trait;
use serde_json::json;
use std::error::Error;
use std::sync::Arc;
use tracing::{error, info};

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

        // Get routing decision
        let (decision, tool_calls) = self
            .ws_state
            .model_router
            .route_message(content.clone())
            .await?;

        // Send routing status
        let routing_json = json!({
            "type": "chat",
            "content": decision.reasoning,
            "sender": "ai",
            "status": "thinking"
        });
        self.ws_state
            .send_to(conn_id, &routing_json.to_string())
            .await?;

        if decision.needs_tool {
            // Handle tool execution
            if let Some(tool_calls) = tool_calls {
                for tool_call in tool_calls {
                    if tool_call.function.name == "read_github_issue" {
                        // Parse tool call arguments
                        let args: serde_json::Value =
                            serde_json::from_str(&tool_call.function.arguments)?;
                        let owner = args["owner"].as_str().unwrap_or("OpenAgentsInc");
                        let repo = args["repo"].as_str().unwrap_or("openagents");
                        let issue_number = args["issue_number"].as_i64().unwrap_or(0) as i32;

                        // Send tool call status
                        let tool_call_json = json!({
                            "type": "chat",
                            "content": format!("Fetching GitHub issue #{} from {}/{}", issue_number, owner, repo),
                            "sender": "ai",
                            "status": "tool_calls"
                        });
                        self.ws_state
                            .send_to(conn_id, &tool_call_json.to_string())
                            .await?;

                        // Fetch the issue
                        let issue = self
                            .github_service
                            .get_issue(owner, repo, issue_number)
                            .await?;

                        // Create messages for tool response
                        let user_message = crate::server::services::deepseek::ChatMessage {
                            role: "user".to_string(),
                            content: content.clone(),
                            tool_call_id: None,
                            tool_calls: None,
                        };

                        let assistant_message =
                            crate::server::services::deepseek::AssistantMessage {
                                role: "assistant".to_string(),
                                content: format!(
                                    "Let me fetch GitHub issue #{} for you.",
                                    issue_number
                                ),
                                tool_call_id: None,
                                tool_calls: Some(vec![tool_call.clone()]),
                            };

                        let issue_message = crate::server::services::deepseek::ChatMessage {
                            role: "tool".to_string(),
                            content: serde_json::to_string(&issue)?,
                            tool_call_id: Some(tool_call.id),
                            tool_calls: None,
                        };

                        // Get final response with tool results
                        let messages = vec![
                            user_message,
                            crate::server::services::deepseek::ChatMessage::from(assistant_message),
                        ];

                        let (final_content, _, _) = self
                            .ws_state
                            .model_router
                            .handle_tool_response(messages, issue_message)
                            .await?;

                        // Send final response
                        let final_json = json!({
                            "type": "chat",
                            "content": final_content,
                            "sender": "ai",
                            "status": "complete"
                        });
                        self.ws_state
                            .send_to(conn_id, &final_json.to_string())
                            .await?;
                    }
                }
            } else {
                // If no tool calls but tool was needed, send error
                let error_json = json!({
                    "type": "chat",
                    "content": "I determined a tool was needed but couldn't execute it properly.",
                    "sender": "system",
                    "status": "error"
                });
                self.ws_state
                    .send_to(conn_id, &error_json.to_string())
                    .await?;
            }
        } else {
            // Use regular chat stream for non-tool messages
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
