mod types;
mod chat;
mod tools;
mod streaming;

pub use types::*;
pub use tools::create_tool;

use reqwest::Client;
use tokio::sync::mpsc;

#[derive(Debug, Clone)]
pub struct DeepSeekService {
    client: Client,
    api_key: String,
    base_url: String,
}

impl DeepSeekService {
    pub fn new(api_key: String) -> Self {
        Self {
            client: Client::new(),
            api_key,
            base_url: "https://api.deepseek.com".to_string(),
        }
    }

    pub fn with_base_url(api_key: String, base_url: String) -> Self {
        Self {
            client: Client::new(),
            api_key,
            base_url,
        }
    }

    pub async fn chat_stream(
        &self,
        prompt: String,
        use_reasoner: bool,
    ) -> mpsc::Receiver<StreamUpdate> {
        self.chat_stream_with_history(Vec::new(), prompt, use_reasoner).await
    }

    pub async fn chat_stream_with_history(
        &self,
        history: Vec<ChatMessage>,
        prompt: String,
        use_reasoner: bool,
    ) -> mpsc::Receiver<StreamUpdate> {
        streaming::chat_stream_with_history(
            &self.client,
            &self.api_key,
            &self.base_url,
            history,
            prompt,
            use_reasoner,
        )
        .await
    }
}