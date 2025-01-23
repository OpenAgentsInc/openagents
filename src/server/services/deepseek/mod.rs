mod types;
mod chat;
mod tools;
mod streaming;

pub use types::*;
pub use chat::ChatRequest;
pub use tools::create_tool;
pub use streaming::chat_stream_with_history;

use reqwest::Client;

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
}