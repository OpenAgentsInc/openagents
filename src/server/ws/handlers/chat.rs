use super::MessageHandler;
use crate::server::services::github_issue::GitHubService;
use crate::server::services::deepseek::{
    ChatMessage as DeepSeekMessage,
    StreamUpdate,
    DeepSeekService,
    create_tool,
};
use crate::server::ws::{transport::WebSocketState, types::ChatMessage};
use async_trait::async_trait;
use serde_json::json;
use std::error::Error;
use std::sync::Arc;
use tokio::sync::RwLock;
use std::collections::HashMap;
use tracing::{error, info};

pub struct ChatHandler {
    ws_state: Arc<WebSocketState>,
    deepseek_service: Arc<DeepSeekService>,
    github_service: Arc<GitHubService>,
    message_history: Arc<RwLock<HashMap<String, Vec<DeepSeekMessage>>>>,
}

impl ChatHandler {
    pub fn new(
        ws_state: Arc<WebSocketState>,
        deepseek_service: Arc<DeepSeekService>,
        github_service: Arc<GitHubService>,
    ) -> Self {
        Self {
            ws_state,
            deepseek_service,
            github_service,
            message_history: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    // Add message to history
    async fn add_to_history(&self, conn_id: &str, message: DeepSeekMessage) {
        let mut history = self.message_history.write().await;
        history.entry(conn_id.to_string())
            .or_insert_with(Vec::new)
            .push(message);
    }

    // Get message history
    async fn get_history(&self, conn_id: &str) -> Vec<DeepSeekMessage> {
        let history = self.message_history.read().await;
        history.get(conn_id)
            .cloned()
            .unwrap_or_default()
    }

    // Clean up history for a connection
    pub async fn cleanup_history(&self, conn_id: &str) {
        let mut history = self.message_history.write().await;
        history.remove(conn_id);
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

        // Add user message to history
        let user_message = DeepSeekMessage {
            role: "user".to_string(),
            content: content.clone(),
            tool_call_id: None,
            tool_calls: None,
        };
        self.add_to_history(conn_id, user_message.clone()).await;

        // Check if message might be about GitHub issues
        let is_github_related = content.to_lowercase().contains("issue")
            || content.to_lowercase().contains("github")
            || content.to_lowercase().contains("#");

        if is_github_related {
            // Create GitHub issue tool
            let get_issue_tool = create_tool(
                "get_github_issue".to_string(),
                Some("Get a GitHub issue by number".to_string()),
                json!({
                    "type": "object",
                    "properties": {
                        "owner": {
                            "type": "string",
                            "description": "The owner of the repository"
                        },
                        "repo": {
                            "type": "string",
                            "description": "The name of the repository"
                        },
                        "issue_number": {
                            "type": "integer",
                            "description": "The issue number"
                        }
                    },
                    "required": ["owner", "repo", "issue_number"]
                }),
            );

            // Get message history and use it for the initial request
            let mut messages = self.get_history(conn_id).await;
            messages.push(user_message);

            // Get initial response with potential tool calls
            let (initial_content, _, tool_calls) = self
                .deepseek_service
                .chat_with_tools(content.clone(), vec![get_issue_tool.clone()], None, false)
                .await?;

            // Send initial content
            let response_json = json!({
                "type": "chat",
                "content": initial_content,
                "sender": "ai",
                "status": "streaming"
            });
            self.ws_state
                .send_to(conn_id, &response_json.to_string())
                .await?;

            // Handle tool calls if present
            if let Some(tool_calls) = tool_calls {
                for tool_call in tool_calls {
                    if tool_call.function.name == "get_github_issue" {
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

                        let assistant_message = DeepSeekMessage {
                            role: "assistant".to_string(),
                            content: format!(
                                "Let me fetch GitHub issue #{} for you.",
                                issue_number
                            ),
                            tool_call_id: None,
                            tool_calls: Some(vec![tool_call.clone()]),
                        };

                        let issue_message = DeepSeekMessage {
                            role: "tool".to_string(),
                            content: serde_json::to_string(&issue)?,
                            tool_call_id: Some(tool_call.id.clone()),
                            tool_calls: None,
                        };

                        // Add messages to history
                        self.add_to_history(conn_id, assistant_message.clone()).await;
                        self.add_to_history(conn_id, issue_message.clone()).await;

                        // Get final response with tool results
                        let mut messages = self.get_history(conn_id).await;
                        messages.push(issue_message.clone());

                        let (final_content, _, _) = self
                            .deepseek_service
                            .chat_with_tool_response(
                                messages,
                                issue_message,
                                vec![get_issue_tool.clone()],
                                false,
                            )
                            .await?;

                        // Add final response to history
                        let final_message = DeepSeekMessage {
                            role: "assistant".to_string(),
                            content: final_content.clone(),
                            tool_call_id: None,
                            tool_calls: None,
                        };
                        self.add_to_history(conn_id, final_message).await;

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
                // If no tool calls, send complete status
                let complete_json = json!({
                    "type": "chat",
                    "content": initial_content,
                    "sender": "ai",
                    "status": "complete"
                });
                self.ws_state
                    .send_to(conn_id, &complete_json.to_string())
                    .await?;

                // Add assistant response to history
                let assistant_message = DeepSeekMessage {
                    role: "assistant".to_string(),
                    content: initial_content,
                    tool_call_id: None,
                    tool_calls: None,
                };
                self.add_to_history(conn_id, assistant_message).await;
            }
        } else {
            // Get message history and use it for the request
            let messages = self.get_history(conn_id).await;

            // Use regular chat_stream for non-GitHub related queries
            let mut stream = self.deepseek_service.chat_stream_with_history(messages, content, true).await;
            let mut full_response = String::new();

            while let Some(update) = stream.recv().await {
                match update {
                    StreamUpdate::Content(content) => {
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
                    StreamUpdate::Reasoning(reasoning) => {
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
                    StreamUpdate::Done => {
                        let response_json = json!({
                            "type": "chat",
                            "content": full_response,
                            "sender": "ai",
                            "status": "complete"
                        });
                        self.ws_state
                            .send_to(conn_id, &response_json.to_string())
                            .await?;

                        // Add assistant response to history
                        let assistant_message = DeepSeekMessage {
                            role: "assistant".to_string(),
                            content: full_response.clone(),
                            tool_call_id: None,
                            tool_calls: None,
                        };
                        self.add_to_history(conn_id, assistant_message).await;
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