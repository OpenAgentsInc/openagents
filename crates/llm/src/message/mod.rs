//! Message types for LLM requests and responses.
//!
//! This module provides types for constructing completion requests and handling
//! responses across different providers.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A completion request to send to an LLM provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionRequest {
    /// Model to use (e.g., "claude-sonnet-4-5-20250929").
    pub model: String,

    /// Conversation messages.
    pub messages: Vec<Message>,

    /// System prompt (separate for some providers).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,

    /// Available tools.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<Tool>,

    /// Tool choice strategy.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<ToolChoice>,

    /// Maximum output tokens.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,

    /// Temperature (0.0 - 2.0).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,

    /// Top P sampling.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,

    /// Stop sequences.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stop: Vec<String>,

    /// Response format for structured output (JSON mode, JSON schema).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_format: Option<ResponseFormat>,

    /// Provider-specific options.
    #[serde(default, skip_serializing_if = "ProviderOptions::is_empty")]
    pub provider_options: ProviderOptions,
}

impl CompletionRequest {
    /// Create a new completion request.
    pub fn new(model: impl Into<String>) -> Self {
        Self {
            model: model.into(),
            messages: Vec::new(),
            system: None,
            tools: Vec::new(),
            tool_choice: None,
            max_tokens: None,
            temperature: None,
            top_p: None,
            stop: Vec::new(),
            response_format: None,
            provider_options: ProviderOptions::default(),
        }
    }

    /// Set the system prompt.
    pub fn system(mut self, system: impl Into<String>) -> Self {
        self.system = Some(system.into());
        self
    }

    /// Add a message.
    pub fn message(mut self, message: Message) -> Self {
        self.messages.push(message);
        self
    }

    /// Add messages.
    pub fn messages(mut self, messages: impl IntoIterator<Item = Message>) -> Self {
        self.messages.extend(messages);
        self
    }

    /// Add a tool.
    pub fn tool(mut self, tool: Tool) -> Self {
        self.tools.push(tool);
        self
    }

    /// Add tools.
    pub fn tools(mut self, tools: impl IntoIterator<Item = Tool>) -> Self {
        self.tools.extend(tools);
        self
    }

    /// Set max tokens.
    pub fn max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = Some(max_tokens);
        self
    }

    /// Set temperature.
    pub fn temperature(mut self, temperature: f32) -> Self {
        self.temperature = Some(temperature);
        self
    }

    /// Set response format for structured output.
    pub fn response_format(mut self, format: ResponseFormat) -> Self {
        self.response_format = Some(format);
        self
    }
}

/// A message in the conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// Message role.
    pub role: Role,
    /// Message content blocks.
    pub content: Vec<ContentBlock>,
}

impl Message {
    /// Create a user message with text content.
    pub fn user(text: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: vec![ContentBlock::Text { text: text.into() }],
        }
    }

    /// Create an assistant message with text content.
    pub fn assistant(text: impl Into<String>) -> Self {
        Self {
            role: Role::Assistant,
            content: vec![ContentBlock::Text { text: text.into() }],
        }
    }

    /// Create a system message with text content.
    pub fn system(text: impl Into<String>) -> Self {
        Self {
            role: Role::System,
            content: vec![ContentBlock::Text { text: text.into() }],
        }
    }

    /// Create a tool result message.
    pub fn tool_result(tool_use_id: impl Into<String>, content: ToolResultContent) -> Self {
        Self {
            role: Role::Tool,
            content: vec![ContentBlock::ToolResult {
                tool_use_id: tool_use_id.into(),
                content,
                is_error: false,
            }],
        }
    }

    /// Create a tool error result message.
    pub fn tool_error(tool_use_id: impl Into<String>, error: impl Into<String>) -> Self {
        Self {
            role: Role::Tool,
            content: vec![ContentBlock::ToolResult {
                tool_use_id: tool_use_id.into(),
                content: ToolResultContent::Text(error.into()),
                is_error: true,
            }],
        }
    }

    /// Add a content block.
    pub fn with_content(mut self, block: ContentBlock) -> Self {
        self.content.push(block);
        self
    }
}

/// Message role.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    /// System message.
    System,
    /// User message.
    User,
    /// Assistant message.
    Assistant,
    /// Tool result message.
    Tool,
}

/// Content block within a message.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    /// Text content.
    Text {
        /// The text content.
        text: String,
    },

    /// Image content.
    Image {
        /// Image source.
        source: ImageSource,
        /// Media type (e.g., "image/png").
        #[serde(skip_serializing_if = "Option::is_none")]
        media_type: Option<String>,
    },

    /// Tool use (assistant requesting a tool call).
    ToolUse {
        /// Unique tool call ID.
        id: String,
        /// Tool name.
        name: String,
        /// Tool input arguments.
        input: serde_json::Value,
    },

    /// Tool result (user providing tool output).
    ToolResult {
        /// Tool call ID this result corresponds to.
        tool_use_id: String,
        /// Result content.
        content: ToolResultContent,
        /// Whether this is an error result.
        #[serde(default)]
        is_error: bool,
    },

    /// Reasoning/thinking content (for extended thinking models).
    Reasoning {
        /// Reasoning text.
        text: String,
        /// Provider-specific metadata.
        #[serde(skip_serializing_if = "Option::is_none")]
        provider_metadata: Option<serde_json::Value>,
    },
}

/// Image source for image content blocks.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ImageSource {
    /// Base64-encoded image data.
    Base64 {
        /// Base64-encoded image data.
        data: String,
        /// Media type (e.g., "image/png").
        media_type: String,
    },
    /// Image URL.
    Url {
        /// Image URL.
        url: String,
    },
}

/// Tool result content.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ToolResultContent {
    /// Simple text result.
    Text(String),
    /// Multiple content blocks (for images, etc.).
    Blocks(Vec<ContentBlock>),
}

impl From<String> for ToolResultContent {
    fn from(s: String) -> Self {
        Self::Text(s)
    }
}

impl From<&str> for ToolResultContent {
    fn from(s: &str) -> Self {
        Self::Text(s.to_string())
    }
}

/// Tool definition for function calling.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    /// Tool name (function name).
    pub name: String,
    /// Tool description.
    pub description: String,
    /// JSON Schema for input parameters.
    pub input_schema: serde_json::Value,
}

impl Tool {
    /// Create a new tool definition.
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        input_schema: serde_json::Value,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            input_schema,
        }
    }
}

/// Tool choice strategy.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolChoice {
    /// Let the model decide whether to use tools.
    Auto,
    /// Don't use tools.
    None,
    /// Must use a tool.
    Required,
    /// Use a specific tool.
    Tool {
        /// Name of the tool to use.
        name: String,
    },
}

/// Provider-specific options.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProviderOptions {
    /// Anthropic-specific options.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub anthropic: Option<AnthropicOptions>,

    /// OpenAI-specific options.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub openai: Option<OpenAIOptions>,

    /// Google-specific options.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub google: Option<GoogleOptions>,

    /// AWS Bedrock-specific options.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bedrock: Option<BedrockOptions>,

    /// Generic key-value options.
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

impl ProviderOptions {
    /// Check if all options are empty.
    pub fn is_empty(&self) -> bool {
        self.anthropic.is_none()
            && self.openai.is_none()
            && self.google.is_none()
            && self.bedrock.is_none()
            && self.extra.is_empty()
    }
}

/// Anthropic-specific options.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnthropicOptions {
    /// Enable extended thinking.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking: Option<ThinkingConfig>,

    /// Beta features to enable.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub betas: Vec<String>,

    /// Cache control settings.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_control: Option<CacheControl>,
}

/// Extended thinking configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThinkingConfig {
    /// Configuration type ("enabled").
    #[serde(rename = "type")]
    pub config_type: String,
    /// Budget tokens for thinking.
    pub budget_tokens: u32,
}

impl ThinkingConfig {
    /// Create enabled thinking config with a token budget.
    pub fn enabled(budget_tokens: u32) -> Self {
        Self {
            config_type: "enabled".to_string(),
            budget_tokens,
        }
    }
}

/// Cache control settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheControl {
    /// Cache type.
    #[serde(rename = "type")]
    pub cache_type: String,
}

/// OpenAI-specific options.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIOptions {
    /// Reasoning effort (for o-series models).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_effort: Option<String>,

    /// Reasoning summary.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_summary: Option<String>,

    /// Prompt cache key for session continuity.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_cache_key: Option<String>,

    /// Service tier.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_tier: Option<String>,
}

/// Google-specific options.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoogleOptions {
    /// Thinking configuration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking_config: Option<GoogleThinkingConfig>,
}

/// Google thinking configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoogleThinkingConfig {
    /// Include thoughts in response.
    pub include_thoughts: bool,
    /// Thinking level.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking_level: Option<String>,
}

/// AWS Bedrock-specific options.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BedrockOptions {
    /// AWS region.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub region: Option<String>,

    /// Model profile ARN.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile_arn: Option<String>,
}

/// Response format for structured output.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ResponseFormat {
    /// Plain text response (default).
    Text,
    /// JSON object response (model must output valid JSON).
    JsonObject,
    /// JSON schema response (model must match the provided schema).
    JsonSchema {
        /// The JSON schema the response must conform to.
        schema: serde_json::Value,
        /// Optional name for the schema (some providers require this).
        #[serde(skip_serializing_if = "Option::is_none")]
        name: Option<String>,
    },
}
