use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroqConfig {
    pub api_key: String,
    pub base_url: Option<String>,
    pub default_model: Option<String>,
    pub timeout_secs: Option<u64>,
}

impl Default for GroqConfig {
    fn default() -> Self {
        Self {
            api_key: std::env::var("GROQ_API_KEY").expect("GROQ_API_KEY must be set"),
            base_url: None,
            default_model: None,
            timeout_secs: Some(180),
        }
    }
}
