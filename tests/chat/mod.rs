use openagents::server::services::deepseek::{
    ChatMessage as DeepSeekMessage,
    StreamUpdate,
    DeepSeekService,
};
use openagents::server::services::github_issue::GitHubService;
use openagents::server::ws::{
    handlers::chat::ChatHandler,
    transport::WebSocketState,
    types::ChatMessage,
};
use std::sync::Arc;
use tokio::sync::mpsc;

// Rest of the test file...