mod routing;
mod chat;
mod tool_execution;

use dotenvy::dotenv;
use openagents::server::services::{
    deepseek::{DeepSeekService, Tool},
    model_router::ModelRouter,
};
use serde_json::json;
use std::sync::Arc;
use tracing::{Level};
use tracing_subscriber;
use wiremock::MockServer;

// Helper function to initialize logging once
pub(crate) fn init_logging() {
    let _ = tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .try_init();
}

// Helper function to create test tools
pub(crate) fn create_test_tools() -> Vec<Tool> {
    vec![
        // GitHub issue tool
        DeepSeekService::create_tool(
            "read_github_issue".to_string(),
            Some("Read a GitHub issue by number".to_string()),
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
        ),
        // Calculator tool
        DeepSeekService::create_tool(
            "calculate".to_string(),
            Some("Perform a calculation".to_string()),
            json!({
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

// Helper function to create a mocked model router
pub(crate) async fn create_mock_router() -> (ModelRouter, MockServer) {
    // Create mock server
    let mock_server = MockServer::start().await;

    // Create DeepSeek services with mock server
    let tool_model = Arc::new(DeepSeekService::with_base_url("test_key".to_string(), mock_server.uri()));
    let chat_model = Arc::new(DeepSeekService::with_base_url("test_key".to_string(), mock_server.uri()));

    // Create model router
    let router = ModelRouter::new(tool_model, chat_model, create_test_tools());

    (router, mock_server)
}