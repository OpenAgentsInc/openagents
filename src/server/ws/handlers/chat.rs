use super::MessageHandler;
use crate::server::services::github_issue::GitHubService;
use crate::server::services::deepseek::{
    ChatMessage as DeepSeekMessage,
    StreamUpdate,
    DeepSeekService,
};
use crate::server::ws::{transport::WebSocketState, types::ChatMessage};
use async_trait::async_trait;
use serde_json::json;
use std::error::Error;
use std::sync::Arc;
use tokio::sync::RwLock;
use std::collections::HashMap;
use tracing::{error, info};

// Rest of the file stays the same...