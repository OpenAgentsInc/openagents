use crate::server::services::StreamUpdate;
use anyhow::Result;
use tokio::sync::mpsc;

#[derive(Debug, Clone)]
pub struct GatewayMetadata {
    pub name: String,
    pub openai_compatible: bool,
    pub supported_features: Vec<String>,
    pub default_model: String,
    pub available_models: Vec<String>,
}

#[async_trait::async_trait]
pub trait Gateway: Send + Sync {
    fn metadata(&self) -> GatewayMetadata;
    async fn chat(&self, prompt: String, use_reasoner: bool) -> Result<(String, Option<String>)>;
    async fn chat_stream(&self, prompt: String, use_reasoner: bool) -> mpsc::Receiver<StreamUpdate>;
}