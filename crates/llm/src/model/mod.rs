//! Model information and pricing.
//!
//! This module provides types for model metadata, capabilities, and pricing
//! for cost calculation.

use serde::{Deserialize, Serialize};

/// Information about an LLM model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    /// Model identifier (e.g., "claude-sonnet-4-5-20250929").
    pub id: String,

    /// Provider ID (e.g., "anthropic").
    pub provider: String,

    /// Display name.
    pub name: String,

    /// Model family (e.g., "claude-4.5", "gpt-5").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub family: Option<String>,

    /// Model capabilities.
    pub capabilities: ModelCapabilities,

    /// Pricing per million tokens.
    pub pricing: ModelPricing,

    /// Token limits.
    pub limits: ModelLimits,

    /// Model status.
    pub status: ModelStatus,
}

impl ModelInfo {
    /// Create a new model info builder.
    pub fn builder(id: impl Into<String>, provider: impl Into<String>) -> ModelInfoBuilder {
        ModelInfoBuilder::new(id, provider)
    }
}

/// Builder for ModelInfo.
pub struct ModelInfoBuilder {
    id: String,
    provider: String,
    name: Option<String>,
    family: Option<String>,
    capabilities: ModelCapabilities,
    pricing: ModelPricing,
    limits: ModelLimits,
    status: ModelStatus,
}

impl ModelInfoBuilder {
    fn new(id: impl Into<String>, provider: impl Into<String>) -> Self {
        let id = id.into();
        Self {
            name: Some(id.clone()),
            id,
            provider: provider.into(),
            family: None,
            capabilities: ModelCapabilities::default(),
            pricing: ModelPricing::default(),
            limits: ModelLimits::default(),
            status: ModelStatus::Active,
        }
    }

    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn family(mut self, family: impl Into<String>) -> Self {
        self.family = Some(family.into());
        self
    }

    pub fn capabilities(mut self, capabilities: ModelCapabilities) -> Self {
        self.capabilities = capabilities;
        self
    }

    pub fn pricing(mut self, pricing: ModelPricing) -> Self {
        self.pricing = pricing;
        self
    }

    pub fn limits(mut self, limits: ModelLimits) -> Self {
        self.limits = limits;
        self
    }

    pub fn status(mut self, status: ModelStatus) -> Self {
        self.status = status;
        self
    }

    pub fn build(self) -> ModelInfo {
        ModelInfo {
            id: self.id,
            provider: self.provider,
            name: self.name.unwrap_or_default(),
            family: self.family,
            capabilities: self.capabilities,
            pricing: self.pricing,
            limits: self.limits,
            status: self.status,
        }
    }
}

/// Model capabilities.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ModelCapabilities {
    /// Supports temperature parameter.
    #[serde(default)]
    pub temperature: bool,

    /// Supports reasoning/thinking tokens.
    #[serde(default)]
    pub reasoning: bool,

    /// Supports tool/function calling.
    #[serde(default)]
    pub tool_calling: bool,

    /// Supports vision (image input).
    #[serde(default)]
    pub vision: bool,

    /// Supports PDF input.
    #[serde(default)]
    pub pdf: bool,

    /// Supports audio input.
    #[serde(default)]
    pub audio: bool,

    /// Supports video input.
    #[serde(default)]
    pub video: bool,

    /// Supports streaming.
    #[serde(default)]
    pub streaming: bool,

    /// Supports prompt caching.
    #[serde(default)]
    pub caching: bool,

    /// Supports interleaved thinking.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interleaved_thinking: Option<bool>,

    /// Supports fine-grained tool streaming.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fine_grained_tool_streaming: Option<bool>,
}

impl ModelCapabilities {
    /// Full-featured model capabilities.
    pub fn full() -> Self {
        Self {
            temperature: true,
            reasoning: true,
            tool_calling: true,
            vision: true,
            pdf: true,
            audio: false,
            video: false,
            streaming: true,
            caching: true,
            interleaved_thinking: Some(true),
            fine_grained_tool_streaming: Some(true),
        }
    }

    /// Basic chat model capabilities.
    pub fn basic() -> Self {
        Self {
            temperature: true,
            reasoning: false,
            tool_calling: true,
            vision: false,
            pdf: false,
            audio: false,
            video: false,
            streaming: true,
            caching: false,
            interleaved_thinking: None,
            fine_grained_tool_streaming: None,
        }
    }
}

/// Model pricing per million tokens.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ModelPricing {
    /// Input cost per million tokens.
    pub input_per_mtok: f64,

    /// Output cost per million tokens.
    pub output_per_mtok: f64,

    /// Cache read cost per million tokens.
    #[serde(default)]
    pub cache_read_per_mtok: f64,

    /// Cache write cost per million tokens.
    #[serde(default)]
    pub cache_write_per_mtok: f64,

    /// Extended pricing for >200K context.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub over_200k: Option<Box<ModelPricing>>,
}

impl ModelPricing {
    /// Create pricing with input/output costs.
    pub fn new(input_per_mtok: f64, output_per_mtok: f64) -> Self {
        Self {
            input_per_mtok,
            output_per_mtok,
            cache_read_per_mtok: 0.0,
            cache_write_per_mtok: 0.0,
            over_200k: None,
        }
    }

    /// Add caching costs.
    pub fn with_caching(mut self, read_per_mtok: f64, write_per_mtok: f64) -> Self {
        self.cache_read_per_mtok = read_per_mtok;
        self.cache_write_per_mtok = write_per_mtok;
        self
    }

    /// Add extended pricing for >200K context.
    pub fn with_over_200k(mut self, pricing: ModelPricing) -> Self {
        self.over_200k = Some(Box::new(pricing));
        self
    }
}

/// Model token limits.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ModelLimits {
    /// Maximum context window size.
    pub context_window: u32,

    /// Maximum output tokens.
    pub max_output_tokens: u32,
}

impl ModelLimits {
    /// Create new limits.
    pub fn new(context_window: u32, max_output_tokens: u32) -> Self {
        Self {
            context_window,
            max_output_tokens,
        }
    }
}

/// Model status.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ModelStatus {
    /// Alpha/preview.
    Alpha,
    /// Beta.
    Beta,
    /// Generally available.
    #[default]
    Active,
    /// Deprecated.
    Deprecated,
}

// ============================================================================
// Built-in model definitions
// ============================================================================

/// Anthropic model definitions.
pub mod anthropic {
    use super::*;

    /// Claude Sonnet 4.5.
    pub fn claude_sonnet_4_5() -> ModelInfo {
        ModelInfo::builder("claude-sonnet-4-5-20250929", "anthropic")
            .name("Claude Sonnet 4.5")
            .family("claude-4.5")
            .capabilities(ModelCapabilities::full())
            .pricing(
                ModelPricing::new(3.0, 15.0)
                    .with_caching(0.30, 3.75)
                    .with_over_200k(ModelPricing::new(6.0, 30.0)),
            )
            .limits(ModelLimits::new(200_000, 64_000))
            .build()
    }

    /// Claude Opus 4.5.
    pub fn claude_opus_4_5() -> ModelInfo {
        ModelInfo::builder("claude-opus-4-5-20251101", "anthropic")
            .name("Claude Opus 4.5")
            .family("claude-4.5")
            .capabilities(ModelCapabilities::full())
            .pricing(
                ModelPricing::new(15.0, 75.0)
                    .with_caching(1.50, 18.75)
                    .with_over_200k(ModelPricing::new(30.0, 150.0)),
            )
            .limits(ModelLimits::new(200_000, 32_000))
            .build()
    }

    /// Claude Haiku 4.5.
    pub fn claude_haiku_4_5() -> ModelInfo {
        ModelInfo::builder("claude-haiku-4-5-20251001", "anthropic")
            .name("Claude Haiku 4.5")
            .family("claude-4.5")
            .capabilities(ModelCapabilities {
                reasoning: false,
                interleaved_thinking: None,
                ..ModelCapabilities::full()
            })
            .pricing(ModelPricing::new(0.80, 4.0).with_caching(0.08, 1.0))
            .limits(ModelLimits::new(200_000, 8_192))
            .build()
    }

    /// All Anthropic models.
    pub fn all() -> Vec<ModelInfo> {
        vec![claude_sonnet_4_5(), claude_opus_4_5(), claude_haiku_4_5()]
    }
}

/// OpenAI model definitions.
pub mod openai {
    use super::*;

    /// GPT-4o.
    pub fn gpt_4o() -> ModelInfo {
        ModelInfo::builder("gpt-4o", "openai")
            .name("GPT-4o")
            .family("gpt-4o")
            .capabilities(ModelCapabilities {
                temperature: true,
                reasoning: false,
                tool_calling: true,
                vision: true,
                pdf: false,
                audio: true,
                video: false,
                streaming: true,
                caching: true,
                interleaved_thinking: None,
                fine_grained_tool_streaming: None,
            })
            .pricing(ModelPricing::new(2.50, 10.0).with_caching(1.25, 0.0))
            .limits(ModelLimits::new(128_000, 16_384))
            .build()
    }

    /// GPT-4o Mini.
    pub fn gpt_4o_mini() -> ModelInfo {
        ModelInfo::builder("gpt-4o-mini", "openai")
            .name("GPT-4o Mini")
            .family("gpt-4o")
            .capabilities(ModelCapabilities {
                temperature: true,
                reasoning: false,
                tool_calling: true,
                vision: true,
                pdf: false,
                audio: false,
                video: false,
                streaming: true,
                caching: true,
                interleaved_thinking: None,
                fine_grained_tool_streaming: None,
            })
            .pricing(ModelPricing::new(0.15, 0.60).with_caching(0.075, 0.0))
            .limits(ModelLimits::new(128_000, 16_384))
            .build()
    }

    /// O1 (reasoning model).
    pub fn o1() -> ModelInfo {
        ModelInfo::builder("o1", "openai")
            .name("O1")
            .family("o1")
            .capabilities(ModelCapabilities {
                temperature: false,
                reasoning: true,
                tool_calling: true,
                vision: true,
                pdf: false,
                audio: false,
                video: false,
                streaming: true,
                caching: true,
                interleaved_thinking: None,
                fine_grained_tool_streaming: None,
            })
            .pricing(ModelPricing::new(15.0, 60.0).with_caching(7.50, 0.0))
            .limits(ModelLimits::new(200_000, 100_000))
            .build()
    }

    /// All OpenAI models.
    pub fn all() -> Vec<ModelInfo> {
        vec![gpt_4o(), gpt_4o_mini(), o1()]
    }
}

/// Ollama (local) model definitions.
pub mod ollama {
    use super::*;

    /// Llama 3.2.
    pub fn llama_3_2() -> ModelInfo {
        ModelInfo::builder("llama3.2", "ollama")
            .name("Llama 3.2")
            .family("llama-3")
            .capabilities(ModelCapabilities::basic())
            .pricing(ModelPricing::new(0.0, 0.0)) // Free (local)
            .limits(ModelLimits::new(128_000, 4_096))
            .build()
    }

    /// Qwen 2.5 Coder.
    pub fn qwen_2_5_coder() -> ModelInfo {
        ModelInfo::builder("qwen2.5-coder", "ollama")
            .name("Qwen 2.5 Coder")
            .family("qwen-2.5")
            .capabilities(ModelCapabilities::basic())
            .pricing(ModelPricing::new(0.0, 0.0)) // Free (local)
            .limits(ModelLimits::new(128_000, 8_192))
            .build()
    }

    /// All Ollama models.
    pub fn all() -> Vec<ModelInfo> {
        vec![llama_3_2(), qwen_2_5_coder()]
    }
}
