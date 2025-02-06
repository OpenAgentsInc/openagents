use super::MessageHandler;
use crate::server::services::github_issue::GitHubService;
use crate::server::services::gemini::{service::GeminiService, StreamUpdate};
use crate::server::ws::{transport::WebSocketState, types::ChatMessage};
use crate::repomap::generate_repo_map;
use async_trait::async_trait;
use serde_json::json;
use std::error::Error;
use std::sync::Arc;
use tracing::{debug, error, info};
use regex::Regex;
use tempfile::TempDir;
use tokio::process::Command;

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

    async fn clone_repo(&self, owner: &str, repo: &str) -> Result<TempDir, Box<dyn Error + Send + Sync>> {
        let temp_dir = tempfile::tempdir()?;
        let repo_url = format!("https://github.com/{}/{}.git", owner, repo);
        
        Command::new("git")
            .arg("clone")
            .arg(&repo_url)
            .arg(temp_dir.path())
            .output()
            .await?;

        Ok(temp_dir)
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

            // Send repo cloning status
            let clone_status = json!({
                "type": "chat",
                "content": format!("Cloning repository {}/{} for analysis...", owner, repo),
                "sender": "ai",
                "status": "tool_calls"
            });
            self.ws_state
                .send_to(conn_id, &clone_status.to_string())
                .await?;

            // Clone repo and generate map
            let temp_dir = self.clone_repo(owner, repo).await?;
            
            // Send mapping status
            let mapping_status = json!({
                "type": "chat",
                "content": "Generating repository map...",
                "sender": "ai",
                "status": "tool_calls"
            });
            self.ws_state
                .send_to(conn_id, &mapping_status.to_string())
                .await?;

            // Generate repo map
            let repo_map = generate_repo_map(temp_dir.path());
            debug!("Generated repo map:\n{}", repo_map);

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

            // Extract valid paths from repo map
            let valid_paths: Vec<String> = repo_map
                .lines()
                .filter(|line| !line.starts_with('│') && !line.is_empty())
                .map(|line| line.trim_end_matches(':').to_string())
                .collect();

            // Send analysis status
            let analysis_status = json!({
                "type": "chat",
                "content": "Analyzing repository structure...",
                "sender": "ai",
                "status": "tool_calls"
            });
            self.ws_state
                .send_to(conn_id, &analysis_status.to_string())
                .await?;

            // Stream the file analysis
            let mut stream = gemini.analyze_files_stream(
                &issue.body.unwrap_or_default(),
                &valid_paths,
                &repo_map,
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

            // Clean up
            let _ = temp_dir.close();
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