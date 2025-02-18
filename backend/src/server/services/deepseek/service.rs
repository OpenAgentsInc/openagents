use reqwest::{Client, ClientBuilder};
use std::time::Duration;

use super::types::{FunctionDefinition, Tool};

#[derive(Debug, Clone)]
pub struct DeepSeekService {
    pub(crate) client: Client,
    pub(crate) api_key: String,
    pub(crate) base_url: String,
}

impl DeepSeekService {
    pub fn new(api_key: String) -> Self {
        let client = ClientBuilder::new()
            .timeout(Duration::from_secs(180)) // 3 minutes timeout
            .build()
            .expect("Failed to create HTTP client");

        let base_url = std::env::var("DEEPSEEK_API_URL")
            .unwrap_or_else(|_| "https://api.deepseek.com/v1".to_string());

        Self {
            client,
            api_key,
            base_url,
        }
    }

    pub fn with_base_url(api_key: String, base_url: String) -> Self {
        let client = ClientBuilder::new()
            .timeout(Duration::from_secs(180)) // 3 minutes timeout
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            api_key,
            base_url,
        }
    }

    pub fn create_tool(
        name: String,
        description: Option<String>,
        parameters: serde_json::Value,
    ) -> Tool {
        Tool {
            tool_type: "function".to_string(),
            function: FunctionDefinition {
                name,
                description,
                parameters,
            },
        }
    }
}
