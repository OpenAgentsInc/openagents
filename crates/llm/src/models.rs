//! Model registry with pricing and capabilities

use crate::ModelCapabilities;

/// LLM Provider identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Provider {
    Anthropic,
    OpenAI,
    Google,
    Ollama,
    OpenRouter,
}

impl Provider {
    /// Get the provider name as a string
    pub fn name(&self) -> &'static str {
        match self {
            Provider::Anthropic => "anthropic",
            Provider::OpenAI => "openai",
            Provider::Google => "google",
            Provider::Ollama => "ollama",
            Provider::OpenRouter => "openrouter",
        }
    }

    /// Parse provider from string
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "anthropic" | "claude" => Some(Provider::Anthropic),
            "openai" | "gpt" => Some(Provider::OpenAI),
            "google" | "gemini" => Some(Provider::Google),
            "ollama" => Some(Provider::Ollama),
            "openrouter" => Some(Provider::OpenRouter),
            _ => None,
        }
    }

    /// Get the default API base URL for this provider
    pub fn default_base_url(&self) -> &'static str {
        match self {
            Provider::Anthropic => "https://api.anthropic.com/v1",
            Provider::OpenAI => "https://api.openai.com/v1",
            Provider::Google => "https://generativelanguage.googleapis.com/v1beta",
            Provider::Ollama => "http://localhost:11434/api",
            Provider::OpenRouter => "https://openrouter.ai/api/v1",
        }
    }

    /// Get the environment variable name for the API key
    pub fn api_key_env_var(&self) -> &'static str {
        match self {
            Provider::Anthropic => "ANTHROPIC_API_KEY",
            Provider::OpenAI => "OPENAI_API_KEY",
            Provider::Google => "GOOGLE_API_KEY",
            Provider::Ollama => "OLLAMA_API_KEY",
            Provider::OpenRouter => "OPENROUTER_API_KEY",
        }
    }
}

/// Cost per million tokens (in USD)
#[derive(Debug, Clone, Copy, Default)]
pub struct ModelCost {
    /// Input tokens cost per million
    pub input: f64,
    /// Output tokens cost per million
    pub output: f64,
    /// Cache creation cost per million (if supported)
    pub cache_write: f64,
    /// Cache read cost per million (if supported)
    pub cache_read: f64,
}

impl ModelCost {
    pub const fn new(input: f64, output: f64) -> Self {
        Self {
            input,
            output,
            cache_write: 0.0,
            cache_read: 0.0,
        }
    }

    pub const fn with_cache(input: f64, output: f64, cache_write: f64, cache_read: f64) -> Self {
        Self {
            input,
            output,
            cache_write,
            cache_read,
        }
    }
}

/// Complete model information
#[derive(Debug, Clone)]
pub struct Model {
    /// Model identifier (as used in API calls)
    pub id: &'static str,
    /// Human-readable name
    pub name: &'static str,
    /// Provider
    pub provider: Provider,
    /// Context window size (tokens)
    pub context_window: u32,
    /// Maximum output tokens
    pub max_output_tokens: u32,
    /// Cost per million tokens
    pub cost: ModelCost,
    /// Model capabilities
    pub capabilities: ModelCapabilities,
}

impl Model {
    /// Calculate cost for a given usage
    pub fn calculate_cost(
        &self,
        input_tokens: u32,
        output_tokens: u32,
        cache_creation_tokens: u32,
        cache_read_tokens: u32,
    ) -> f64 {
        let input_cost = (input_tokens as f64 / 1_000_000.0) * self.cost.input;
        let output_cost = (output_tokens as f64 / 1_000_000.0) * self.cost.output;
        let cache_write_cost = (cache_creation_tokens as f64 / 1_000_000.0) * self.cost.cache_write;
        let cache_read_cost = (cache_read_tokens as f64 / 1_000_000.0) * self.cost.cache_read;

        input_cost + output_cost + cache_write_cost + cache_read_cost
    }
}

/// Model registry with all known models
pub struct ModelRegistry;

impl ModelRegistry {
    /// Get a model by its ID
    pub fn get(model_id: &str) -> Option<&'static Model> {
        MODELS.iter().find(|m| m.id == model_id)
    }

    /// Get all models for a provider
    pub fn by_provider(provider: Provider) -> Vec<&'static Model> {
        MODELS.iter().filter(|m| m.provider == provider).collect()
    }

    /// Get the default model for a provider
    pub fn default_for_provider(provider: Provider) -> Option<&'static Model> {
        match provider {
            Provider::Anthropic => Self::get("claude-sonnet-4-20250514"),
            Provider::OpenAI => Self::get("gpt-4o"),
            Provider::Google => Self::get("gemini-1.5-pro"),
            Provider::Ollama => Self::get("llama3.2"),
            Provider::OpenRouter => Self::get("gpt-4o"), // Use via OpenRouter
        }
    }

    /// List all available models
    pub fn all() -> &'static [Model] {
        &MODELS
    }

    /// Calculate cost for usage with a specific model
    pub fn calculate_cost(
        model_id: &str,
        input_tokens: u32,
        output_tokens: u32,
        cache_creation_tokens: u32,
        cache_read_tokens: u32,
    ) -> Option<f64> {
        Self::get(model_id).map(|m| {
            m.calculate_cost(
                input_tokens,
                output_tokens,
                cache_creation_tokens,
                cache_read_tokens,
            )
        })
    }
}

/// Static model definitions
static MODELS: &[Model] = &[
    // Anthropic Claude models
    Model {
        id: "claude-opus-4-20250514",
        name: "Claude Opus 4",
        provider: Provider::Anthropic,
        context_window: 200000,
        max_output_tokens: 32000,
        cost: ModelCost::with_cache(15.0, 75.0, 18.75, 1.875),
        capabilities: ModelCapabilities {
            tool_use: true,
            vision: true,
            streaming: true,
            thinking: true,
            input_cost_per_mtok: Some(15.0),
            output_cost_per_mtok: Some(75.0),
        },
    },
    Model {
        id: "claude-sonnet-4-20250514",
        name: "Claude Sonnet 4",
        provider: Provider::Anthropic,
        context_window: 200000,
        max_output_tokens: 64000,
        cost: ModelCost::with_cache(3.0, 15.0, 3.75, 0.375),
        capabilities: ModelCapabilities {
            tool_use: true,
            vision: true,
            streaming: true,
            thinking: true,
            input_cost_per_mtok: Some(3.0),
            output_cost_per_mtok: Some(15.0),
        },
    },
    Model {
        id: "claude-3-5-haiku-20241022",
        name: "Claude 3.5 Haiku",
        provider: Provider::Anthropic,
        context_window: 200000,
        max_output_tokens: 8192,
        cost: ModelCost::with_cache(0.80, 4.0, 1.0, 0.1),
        capabilities: ModelCapabilities {
            tool_use: true,
            vision: true,
            streaming: true,
            thinking: false,
            input_cost_per_mtok: Some(0.80),
            output_cost_per_mtok: Some(4.0),
        },
    },
    // OpenAI models
    Model {
        id: "gpt-4o",
        name: "GPT-4o",
        provider: Provider::OpenAI,
        context_window: 128000,
        max_output_tokens: 16384,
        cost: ModelCost::new(2.50, 10.0),
        capabilities: ModelCapabilities {
            tool_use: true,
            vision: true,
            streaming: true,
            thinking: false,
            input_cost_per_mtok: Some(2.50),
            output_cost_per_mtok: Some(10.0),
        },
    },
    Model {
        id: "gpt-4o-mini",
        name: "GPT-4o Mini",
        provider: Provider::OpenAI,
        context_window: 128000,
        max_output_tokens: 16384,
        cost: ModelCost::new(0.15, 0.60),
        capabilities: ModelCapabilities {
            tool_use: true,
            vision: true,
            streaming: true,
            thinking: false,
            input_cost_per_mtok: Some(0.15),
            output_cost_per_mtok: Some(0.60),
        },
    },
    Model {
        id: "o1",
        name: "o1",
        provider: Provider::OpenAI,
        context_window: 200000,
        max_output_tokens: 100000,
        cost: ModelCost::new(15.0, 60.0),
        capabilities: ModelCapabilities {
            tool_use: true,
            vision: true,
            streaming: true,
            thinking: true,
            input_cost_per_mtok: Some(15.0),
            output_cost_per_mtok: Some(60.0),
        },
    },
    Model {
        id: "o1-mini",
        name: "o1 Mini",
        provider: Provider::OpenAI,
        context_window: 128000,
        max_output_tokens: 65536,
        cost: ModelCost::new(3.0, 12.0),
        capabilities: ModelCapabilities {
            tool_use: true,
            vision: false,
            streaming: true,
            thinking: true,
            input_cost_per_mtok: Some(3.0),
            output_cost_per_mtok: Some(12.0),
        },
    },
    Model {
        id: "gpt-4-turbo",
        name: "GPT-4 Turbo",
        provider: Provider::OpenAI,
        context_window: 128000,
        max_output_tokens: 4096,
        cost: ModelCost::new(10.0, 30.0),
        capabilities: ModelCapabilities {
            tool_use: true,
            vision: true,
            streaming: true,
            thinking: false,
            input_cost_per_mtok: Some(10.0),
            output_cost_per_mtok: Some(30.0),
        },
    },
    // Google Gemini models
    Model {
        id: "gemini-1.5-pro",
        name: "Gemini 1.5 Pro",
        provider: Provider::Google,
        context_window: 2000000,
        max_output_tokens: 8192,
        cost: ModelCost::new(1.25, 5.0),
        capabilities: ModelCapabilities {
            tool_use: true,
            vision: true,
            streaming: true,
            thinking: false,
            input_cost_per_mtok: Some(1.25),
            output_cost_per_mtok: Some(5.0),
        },
    },
    Model {
        id: "gemini-1.5-flash",
        name: "Gemini 1.5 Flash",
        provider: Provider::Google,
        context_window: 1000000,
        max_output_tokens: 8192,
        cost: ModelCost::new(0.075, 0.30),
        capabilities: ModelCapabilities {
            tool_use: true,
            vision: true,
            streaming: true,
            thinking: false,
            input_cost_per_mtok: Some(0.075),
            output_cost_per_mtok: Some(0.30),
        },
    },
    // Ollama local models (no cost)
    Model {
        id: "llama3.2",
        name: "Llama 3.2",
        provider: Provider::Ollama,
        context_window: 128000,
        max_output_tokens: 8192,
        cost: ModelCost::new(0.0, 0.0),
        capabilities: ModelCapabilities {
            tool_use: true,
            vision: false,
            streaming: true,
            thinking: false,
            input_cost_per_mtok: None,
            output_cost_per_mtok: None,
        },
    },
    Model {
        id: "qwen2.5-coder",
        name: "Qwen 2.5 Coder",
        provider: Provider::Ollama,
        context_window: 32768,
        max_output_tokens: 8192,
        cost: ModelCost::new(0.0, 0.0),
        capabilities: ModelCapabilities {
            tool_use: true,
            vision: false,
            streaming: true,
            thinking: false,
            input_cost_per_mtok: None,
            output_cost_per_mtok: None,
        },
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_model() {
        let model = ModelRegistry::get("gpt-4o");
        assert!(model.is_some());
        let model = model.unwrap();
        assert_eq!(model.provider, Provider::OpenAI);
        assert_eq!(model.context_window, 128000);
    }

    #[test]
    fn test_get_unknown_model() {
        let model = ModelRegistry::get("gpt-5");
        assert!(model.is_none());
    }

    #[test]
    fn test_by_provider() {
        let anthropic_models = ModelRegistry::by_provider(Provider::Anthropic);
        assert!(!anthropic_models.is_empty());
        for model in anthropic_models {
            assert_eq!(model.provider, Provider::Anthropic);
        }
    }

    #[test]
    fn test_default_for_provider() {
        let default = ModelRegistry::default_for_provider(Provider::OpenAI);
        assert!(default.is_some());
        assert_eq!(default.unwrap().id, "gpt-4o");
    }

    #[test]
    fn test_calculate_cost() {
        let cost = ModelRegistry::calculate_cost(
            "gpt-4o",
            1_000_000, // 1M input tokens
            100_000,   // 100k output tokens
            0,
            0,
        );
        assert!(cost.is_some());
        let cost = cost.unwrap();
        // $2.50 per M input + $10 per M output * 0.1 = $2.50 + $1.00 = $3.50
        assert!((cost - 3.50).abs() < 0.01);
    }

    #[test]
    fn test_calculate_cost_with_cache() {
        let cost = ModelRegistry::calculate_cost(
            "claude-sonnet-4-20250514",
            500_000,   // 500k input tokens
            100_000,   // 100k output tokens
            100_000,   // 100k cache creation
            200_000,   // 200k cache read
        );
        assert!(cost.is_some());
        let cost = cost.unwrap();
        // $3 * 0.5 + $15 * 0.1 + $3.75 * 0.1 + $0.375 * 0.2
        // = $1.50 + $1.50 + $0.375 + $0.075 = $3.45
        assert!((cost - 3.45).abs() < 0.01);
    }

    #[test]
    fn test_provider_from_str() {
        assert_eq!(Provider::from_str("anthropic"), Some(Provider::Anthropic));
        assert_eq!(Provider::from_str("OpenAI"), Some(Provider::OpenAI));
        assert_eq!(Provider::from_str("CLAUDE"), Some(Provider::Anthropic));
        assert_eq!(Provider::from_str("unknown"), None);
    }

    #[test]
    fn test_model_all() {
        let all = ModelRegistry::all();
        assert!(all.len() >= 10);
    }
}
