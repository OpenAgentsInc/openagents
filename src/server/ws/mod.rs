use std::sync::Arc;

use axum::{
    extract::ws::{Message, WebSocket},
    response::IntoResponse,
};
use futures::{sink::SinkExt, stream::StreamExt};
use tracing::info;

use crate::server::services::{
    deepseek::{DeepSeekService, Tool},
    github_issue::GitHubService,
};

use self::{handlers::chat::ChatHandler, transport::WebSocketState};

pub mod handlers;
pub mod transport;
pub mod types;

pub async fn ws_handler(socket: WebSocket) {
    info!("New WebSocket connection");

    // Create services
    let github_token = std::env::var("GITHUB_TOKEN").expect("GITHUB_TOKEN must be set");
    let deepseek_token = std::env::var("DEEPSEEK_API_KEY").expect("DEEPSEEK_API_KEY must be set");

    let tool_model = Arc::new(DeepSeekService::new(deepseek_token.clone()));
    let chat_model = Arc::new(DeepSeekService::new(deepseek_token));
    let github_service = Arc::new(GitHubService::new(github_token));

    // Create available tools
    let tools = create_tools();

    // Create WebSocket state
    let ws_state = WebSocketState::new(
        tool_model,
        chat_model,
        github_service.clone(),
        tools,
    );

    // Create chat handler
    let chat_handler = WebSocketState::create_handlers(ws_state.clone());

    // Handle socket
    ws_state.handle_socket(socket, chat_handler).await;
}

fn create_tools() -> Vec<Tool> {
    vec![
        // GitHub issue tool
        DeepSeekService::create_tool(
            "read_github_issue".to_string(),
            Some("Read a GitHub issue by number".to_string()),
            serde_json::json!({
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
        ),
        // Calculator tool
        DeepSeekService::create_tool(
            "calculate".to_string(),
            Some("Perform a calculation".to_string()),
            serde_json::json!({
                "type": "object",
                "properties": {
                    "expression": {
                        "type": "string",
                        "description": "The mathematical expression to evaluate"
                    }
                },
                "required": ["expression"]
            }),
        ),
    ]
}

pub async fn handle_socket(socket: WebSocket) -> impl IntoResponse {
    ws_handler(socket).await;
    "WebSocket connection closed"
}