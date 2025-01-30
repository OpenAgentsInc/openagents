use std::sync::OnceLock;

static OLLAMA_CONFIG: OnceLock<OllamaConfig> = OnceLock::new();

#[derive(Debug, Clone)]
pub struct OllamaConfig {
    pub base_url: String,
    pub model: String,
}

impl Default for OllamaConfig {
    fn default() -> Self {
        Self {
            base_url: "http://localhost:11434".to_string(),
            model: "deepseek-r1:14b".to_string(),
        }
    }
}

impl OllamaConfig {
    pub fn global() -> &'static OllamaConfig {
        OLLAMA_CONFIG.get_or_init(|| {
            // Load from environment
            dotenvy::dotenv().ok();

            Self {
                base_url: std::env::var("OLLAMA_URL")
                    .unwrap_or_else(|_| "http://localhost:11434".to_string()),
                model: std::env::var("OLLAMA_MODEL")
                    .unwrap_or_else(|_| "deepseek-r1:14b".to_string()),
            }
        })
    }
}
