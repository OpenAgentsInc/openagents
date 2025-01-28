use anyhow::Result;
use tokio::sync::mpsc;

use super::StreamUpdate;

pub mod types;
pub mod streaming;

pub use self::types::GatewayMetadata;

/// Gateway trait defines the common interface that all AI providers must implement
#[async_trait::async_trait]
pub trait Gateway: Send + Sync {
    /// Get metadata about this gateway's capabilities
    fn metadata(&self) -> GatewayMetadata;

    /// Send a chat request and get a response
    async fn chat(&self, prompt: String, use_reasoner: bool) -> Result<(String, Option<String>)>;
    
    /// Send a chat request and get a streaming response
    async fn chat_stream(&self, prompt: String, use_reasoner: bool) -> mpsc::Receiver<StreamUpdate>;
}