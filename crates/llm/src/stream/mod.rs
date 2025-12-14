//! Streaming types for LLM responses.
//!
//! This module provides types for handling streaming responses from LLM providers,
//! including text deltas, tool calls, reasoning tokens, and usage tracking.

mod sse;

pub use sse::SseStream;

use futures::Stream;
use serde::{Deserialize, Serialize};
use std::pin::Pin;

/// Type alias for the completion stream returned by providers.
pub type CompletionStream =
    Pin<Box<dyn Stream<Item = Result<StreamEvent, crate::ProviderError>> + Send>>;

/// Events emitted during streaming - mirrors Vercel AI SDK's fullStream pattern.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StreamEvent {
    /// Stream started.
    Start {
        /// Model being used.
        model: String,
        /// Provider identifier.
        provider: String,
    },

    /// Text content started.
    TextStart {
        /// Content block ID.
        id: String,
    },

    /// Text content delta.
    TextDelta {
        /// Content block ID.
        id: String,
        /// Text delta.
        delta: String,
    },

    /// Text content ended.
    TextEnd {
        /// Content block ID.
        id: String,
    },

    /// Reasoning/thinking started (Anthropic extended thinking, OpenAI o-series).
    ReasoningStart {
        /// Reasoning block ID.
        id: String,
        /// Provider-specific metadata.
        #[serde(skip_serializing_if = "Option::is_none")]
        provider_metadata: Option<serde_json::Value>,
    },

    /// Reasoning delta.
    ReasoningDelta {
        /// Reasoning block ID.
        id: String,
        /// Reasoning text delta.
        delta: String,
        /// Provider-specific metadata.
        #[serde(skip_serializing_if = "Option::is_none")]
        provider_metadata: Option<serde_json::Value>,
    },

    /// Reasoning ended.
    ReasoningEnd {
        /// Reasoning block ID.
        id: String,
    },

    /// Tool call input streaming started.
    ToolInputStart {
        /// Tool call ID.
        id: String,
        /// Tool name being called.
        tool_name: String,
    },

    /// Tool call input delta (streaming JSON).
    ToolInputDelta {
        /// Tool call ID.
        id: String,
        /// Partial JSON input delta.
        delta: String,
    },

    /// Tool call input complete.
    ToolInputEnd {
        /// Tool call ID.
        id: String,
    },

    /// Complete tool call (ready for execution).
    ToolCall {
        /// Unique tool call ID.
        tool_call_id: String,
        /// Tool name.
        tool_name: String,
        /// Parsed input arguments.
        input: serde_json::Value,
        /// Provider-specific metadata.
        #[serde(skip_serializing_if = "Option::is_none")]
        provider_metadata: Option<serde_json::Value>,
    },

    /// Tool result (after execution).
    ToolResult {
        /// Tool call ID this result corresponds to.
        tool_call_id: String,
        /// Tool name.
        tool_name: String,
        /// Result content.
        result: crate::message::ToolResultContent,
        /// Whether this is an error result.
        #[serde(default)]
        is_error: bool,
    },

    /// Step finished (for multi-turn conversations).
    FinishStep {
        /// Reason for finishing this step.
        finish_reason: FinishReason,
        /// Token usage for this step.
        usage: Usage,
        /// Provider-specific metadata.
        #[serde(skip_serializing_if = "Option::is_none")]
        provider_metadata: Option<serde_json::Value>,
    },

    /// Stream finished.
    Finish {
        /// Reason for finishing.
        finish_reason: FinishReason,
        /// Total token usage.
        usage: Usage,
        /// Provider-specific metadata.
        #[serde(skip_serializing_if = "Option::is_none")]
        provider_metadata: Option<serde_json::Value>,
    },

    /// Error during streaming.
    Error {
        /// Error details.
        error: StreamError,
    },
}

/// Reason for finishing a completion.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum FinishReason {
    /// Natural stop (end of response).
    #[default]
    Stop,
    /// Hit token limit.
    Length,
    /// Model wants to call tools.
    ToolCalls,
    /// Content was filtered.
    ContentFilter,
    /// Error occurred.
    Error,
    /// Unknown reason.
    Unknown,
}

/// Token usage statistics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Usage {
    /// Input tokens consumed.
    pub input_tokens: u64,
    /// Output tokens generated.
    pub output_tokens: u64,
    /// Reasoning/thinking tokens (if supported).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_tokens: Option<u64>,
    /// Tokens read from cache.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_read_tokens: Option<u64>,
    /// Tokens written to cache.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_write_tokens: Option<u64>,
}

impl Usage {
    /// Create empty usage.
    pub fn new() -> Self {
        Self::default()
    }

    /// Total tokens (input + output).
    pub fn total_tokens(&self) -> u64 {
        self.input_tokens + self.output_tokens + self.reasoning_tokens.unwrap_or(0)
    }

    /// Calculate cost in USD for this usage.
    pub fn calculate_cost(&self, pricing: &crate::model::ModelPricing) -> f64 {
        let input_cost = (self.input_tokens as f64 / 1_000_000.0) * pricing.input_per_mtok;
        let output_cost = (self.output_tokens as f64 / 1_000_000.0) * pricing.output_per_mtok;

        let cache_read_cost = self
            .cache_read_tokens
            .map(|t| (t as f64 / 1_000_000.0) * pricing.cache_read_per_mtok)
            .unwrap_or(0.0);

        let cache_write_cost = self
            .cache_write_tokens
            .map(|t| (t as f64 / 1_000_000.0) * pricing.cache_write_per_mtok)
            .unwrap_or(0.0);

        input_cost + output_cost + cache_read_cost + cache_write_cost
    }
}

/// Error that occurred during streaming.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamError {
    /// Error code.
    pub code: String,
    /// Error message.
    pub message: String,
    /// Additional details.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

impl std::fmt::Display for StreamError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}
