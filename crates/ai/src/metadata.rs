use crate::provider::Provider;
use serde::{Deserialize, Serialize};

/// Pricing information for a model (in USD per million tokens)
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Pricing {
    /// Cost per million input tokens
    pub input_per_mtok: f64,
    /// Cost per million output tokens
    pub output_per_mtok: f64,
    /// Cost per million tokens for cache writes (if supported)
    pub cache_write_per_mtok: f64,
    /// Cost per million tokens for cache reads (if supported)
    pub cache_read_per_mtok: f64,
}

impl Pricing {
    pub const fn new(input: f64, output: f64) -> Self {
        Self {
            input_per_mtok: input,
            output_per_mtok: output,
            cache_write_per_mtok: 0.0,
            cache_read_per_mtok: 0.0,
        }
    }

    pub const fn with_cache(input: f64, output: f64, write: f64, read: f64) -> Self {
        Self {
            input_per_mtok: input,
            output_per_mtok: output,
            cache_write_per_mtok: write,
            cache_read_per_mtok: read,
        }
    }

    pub const fn free() -> Self {
        Self {
            input_per_mtok: 0.0,
            output_per_mtok: 0.0,
            cache_write_per_mtok: 0.0,
            cache_read_per_mtok: 0.0,
        }
    }
}

/// Model capabilities
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Capabilities {
    /// Supports tool/function calling
    pub tool_use: bool,
    /// Supports vision/image input
    pub vision: bool,
    /// Supports streaming responses
    pub streaming: bool,
    /// Supports extended thinking mode
    pub thinking_mode: bool,
    /// Supports prompt caching
    pub caching: bool,
}

impl Capabilities {
    pub const fn all() -> Self {
        Self {
            tool_use: true,
            vision: true,
            streaming: true,
            thinking_mode: true,
            caching: true,
        }
    }

    pub const fn tools_only() -> Self {
        Self {
            tool_use: true,
            vision: false,
            streaming: true,
            thinking_mode: false,
            caching: false,
        }
    }

    pub const fn minimal() -> Self {
        Self {
            tool_use: false,
            vision: false,
            streaming: true,
            thinking_mode: false,
            caching: false,
        }
    }

    pub const fn with_thinking() -> Self {
        Self {
            tool_use: true,
            vision: false,
            streaming: true,
            thinking_mode: true,
            caching: true,
        }
    }
}

/// Complete metadata for a model
#[derive(Debug, Clone)]
pub struct ModelMetadata {
    /// API identifier (e.g., "claude-haiku-4-5-20251001")
    pub id: &'static str,
    /// Human-readable name (e.g., "Claude Haiku 4.5")
    pub display_name: &'static str,
    /// Provider that serves this model
    pub provider: Provider,
    /// Maximum context window in tokens
    pub context_window: u32,
    /// Maximum output tokens
    pub max_output_tokens: u32,
    /// Pricing information
    pub pricing: Pricing,
    /// Model capabilities
    pub capabilities: Capabilities,
}

impl ModelMetadata {
    pub const fn new(
        id: &'static str,
        display_name: &'static str,
        provider: Provider,
        context_window: u32,
        max_output_tokens: u32,
        pricing: Pricing,
        capabilities: Capabilities,
    ) -> Self {
        Self {
            id,
            display_name,
            provider,
            context_window,
            max_output_tokens,
            pricing,
            capabilities,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pricing_new() {
        let pricing = Pricing::new(3.0, 15.0);
        assert_eq!(pricing.input_per_mtok, 3.0);
        assert_eq!(pricing.output_per_mtok, 15.0);
        assert_eq!(pricing.cache_write_per_mtok, 0.0);
    }

    #[test]
    fn test_pricing_with_cache() {
        let pricing = Pricing::with_cache(3.0, 15.0, 0.9, 0.3);
        assert_eq!(pricing.cache_write_per_mtok, 0.9);
        assert_eq!(pricing.cache_read_per_mtok, 0.3);
    }

    #[test]
    fn test_capabilities_all() {
        let caps = Capabilities::all();
        assert!(caps.tool_use);
        assert!(caps.vision);
        assert!(caps.streaming);
        assert!(caps.thinking_mode);
        assert!(caps.caching);
    }
}
