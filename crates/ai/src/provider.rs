use serde::{Deserialize, Serialize};

/// AI provider identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    Anthropic,
    Grok,
    OpenAI,
}

impl Provider {
    /// Get the provider name as a string
    pub fn name(&self) -> &'static str {
        match self {
            Provider::Anthropic => "anthropic",
            Provider::Grok => "grok",
            Provider::OpenAI => "openai",
        }
    }

    /// Get the default API base URL for this provider
    pub fn api_base_url(&self) -> &'static str {
        match self {
            Provider::Anthropic => "https://api.anthropic.com/v1",
            Provider::Grok => "https://api.x.ai/v1",
            Provider::OpenAI => "https://api.openai.com/v1",
        }
    }

    /// Get the environment variable name for the API key
    pub fn api_key_env_var(&self) -> &'static str {
        match self {
            Provider::Anthropic => "ANTHROPIC_API_KEY",
            Provider::Grok => "XAI_API_KEY",
            Provider::OpenAI => "OPENAI_API_KEY",
        }
    }

    /// Parse provider from string
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "anthropic" | "claude" => Some(Provider::Anthropic),
            "grok" | "x-ai" | "xai" => Some(Provider::Grok),
            "openai" | "gpt" => Some(Provider::OpenAI),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_names() {
        assert_eq!(Provider::Anthropic.name(), "anthropic");
        assert_eq!(Provider::Grok.name(), "grok");
        assert_eq!(Provider::OpenAI.name(), "openai");
    }

    #[test]
    fn test_api_key_env_vars() {
        assert_eq!(Provider::Anthropic.api_key_env_var(), "ANTHROPIC_API_KEY");
        assert_eq!(Provider::Grok.api_key_env_var(), "XAI_API_KEY");
        assert_eq!(Provider::OpenAI.api_key_env_var(), "OPENAI_API_KEY");
    }

    #[test]
    fn test_provider_from_str() {
        assert_eq!(Provider::from_str("anthropic"), Some(Provider::Anthropic));
        assert_eq!(Provider::from_str("claude"), Some(Provider::Anthropic));
        assert_eq!(Provider::from_str("grok"), Some(Provider::Grok));
        assert_eq!(Provider::from_str("openai"), Some(Provider::OpenAI));
        assert_eq!(Provider::from_str("gpt"), Some(Provider::OpenAI));
        assert_eq!(Provider::from_str("unknown"), None);
    }
}
