//! Agent events emitted during execution
//!
//! These events mirror pi-mono's `AssistantMessageEvent` and `AgentEvent` types,
//! providing fine-grained streaming updates for UI integration.

use llm::Usage;
use serde::{Deserialize, Serialize};

/// Events emitted by the Pi agent during execution.
///
/// These events allow consumers to track progress, update UI, and handle
/// tool execution in real-time.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgentEvent {
    /// Agent has started processing
    Started {
        /// Unique session identifier
        session_id: String,
        /// Model being used
        model: String,
    },

    /// New turn started
    TurnStart {
        /// Turn number (1-indexed)
        turn: u32,
    },

    /// Streaming text content from assistant
    TextDelta {
        /// Incremental text content
        text: String,
    },

    /// Thinking/reasoning content (for models that support it)
    ThinkingDelta {
        /// Incremental thinking content
        text: String,
    },

    /// Tool use started
    ToolUseStart {
        /// Unique tool call ID
        id: String,
        /// Name of the tool being called
        name: String,
    },

    /// Tool input streaming (partial JSON)
    ToolInputDelta {
        /// Tool call ID
        id: String,
        /// Partial JSON input
        json: String,
    },

    /// Tool execution started
    ToolExecuting {
        /// Tool call ID
        id: String,
        /// Tool name
        name: String,
        /// Full input arguments
        input: serde_json::Value,
    },

    /// Tool execution completed
    ToolResult {
        /// Tool call ID
        id: String,
        /// Tool name
        name: String,
        /// Output from tool
        output: String,
        /// Whether the tool returned an error
        is_error: bool,
    },

    /// Assistant message completed
    MessageComplete {
        /// Full text content
        text: String,
    },

    /// Turn completed
    TurnComplete {
        /// Turn number
        turn: u32,
        /// Token usage for this turn
        usage: Usage,
        /// Cost in USD for this turn
        cost_usd: f64,
        /// Stop reason
        stop_reason: StopReason,
    },

    /// Agent completed task successfully
    Completed {
        /// Total turns executed
        total_turns: u32,
        /// Total cost in USD
        total_cost_usd: f64,
        /// Final outcome
        outcome: AgentOutcome,
    },

    /// Error occurred
    Error {
        /// Error message
        message: String,
        /// Whether this error is retryable
        retryable: bool,
    },

    /// Agent was cancelled
    Cancelled,
}

/// Reason for stopping a turn
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    /// Natural end of response
    EndTurn,
    /// Tool use requested
    ToolUse,
    /// Max tokens reached
    MaxTokens,
    /// Stop sequence matched
    StopSequence,
    /// Error occurred
    Error,
    /// Cancelled by user
    Cancelled,
}

impl From<llm::StopReason> for StopReason {
    fn from(reason: llm::StopReason) -> Self {
        match reason {
            llm::StopReason::EndTurn => StopReason::EndTurn,
            llm::StopReason::ToolUse => StopReason::ToolUse,
            llm::StopReason::MaxTokens => StopReason::MaxTokens,
            llm::StopReason::StopSequence => StopReason::StopSequence,
            llm::StopReason::ContentFilter | llm::StopReason::Unknown => StopReason::Error,
        }
    }
}

/// Final outcome of agent execution
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgentOutcome {
    /// Task completed successfully
    Success {
        /// Final response text
        response: String,
    },
    /// Task failed
    Failure {
        /// Error message
        error: String,
    },
    /// Task was cancelled
    Cancelled,
    /// Max turns exceeded
    MaxTurnsExceeded {
        /// Number of turns executed
        turns: u32,
    },
}
