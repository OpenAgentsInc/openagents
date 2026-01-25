//! Unified types for all agents
//!
//! These types normalize events and conversation items from different agents
//! into a single, consistent format.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Unified agent identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
pub enum AgentId {
    Codex,
    ClaudeCode,
    Cursor,
    Gemini,
    Adjutant,
}

impl AgentId {
    pub fn as_str(&self) -> &'static str {
        match self {
            AgentId::Codex => "codex",
            AgentId::ClaudeCode => "claude_code",
            AgentId::Cursor => "cursor",
            AgentId::Gemini => "gemini",
            AgentId::Adjutant => "adjutant",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        let normalized: String = s
            .trim()
            .to_lowercase()
            .chars()
            .filter(|c| c.is_ascii_alphanumeric())
            .collect();

        match normalized.as_str() {
            "codex" => Some(AgentId::Codex),
            "claudecode" => Some(AgentId::ClaudeCode),
            "cursor" => Some(AgentId::Cursor),
            "gemini" => Some(AgentId::Gemini),
            "adjutant" => Some(AgentId::Adjutant),
            _ => None,
        }
    }
}

/// Unified event types (normalized from ACP + extensions)
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(tag = "type")]
pub enum UnifiedEvent {
    /// Message streaming chunk
    MessageChunk {
        session_id: String,
        content: String,
        is_complete: bool,
    },
    /// Reasoning/thinking chunk
    ThoughtChunk {
        session_id: String,
        content: String,
        is_complete: bool,
    },
    /// Tool execution started
    ToolCall {
        session_id: String,
        tool_id: String,
        tool_name: String,
        #[ts(type = "JsonValue")]
        arguments: serde_json::Value,
    },
    /// Tool execution update
    ToolCallUpdate {
        session_id: String,
        tool_id: String,
        output: String,
        is_complete: bool,
    },
    /// Session lifecycle - started
    SessionStarted {
        session_id: String,
        agent_id: AgentId,
    },
    /// Session lifecycle - completed
    SessionCompleted {
        session_id: String,
        stop_reason: String,
    },
    /// Token usage (from extensions)
    TokenUsage {
        session_id: String,
        #[ts(type = "number")]
        input_tokens: u64,
        #[ts(type = "number")]
        output_tokens: u64,
        #[ts(type = "number")]
        total_tokens: u64,
    },
    /// Rate limits (from extensions)
    RateLimitUpdate {
        agent_id: AgentId,
        used_percent: f64,
        #[ts(type = "number | null")]
        resets_at: Option<u64>,
    },
}

/// Unified conversation item (matches frontend ConversationItem)
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(tag = "kind")]
pub enum UnifiedConversationItem {
    Message {
        id: String,
        role: String, // "user" | "assistant"
        text: String,
    },
    Reasoning {
        id: String,
        summary: String,
        content: String,
    },
    Tool {
        id: String,
        tool_type: String,
        title: String,
        detail: String,
        status: Option<String>,
        output: Option<String>,
        #[ts(type = "number | null")]
        duration_ms: Option<u64>,
    },
    Diff {
        id: String,
        title: String,
        diff: String,
        status: Option<String>,
    },
}
