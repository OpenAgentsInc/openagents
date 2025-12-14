//! Agent state machine
//!
//! Tracks the current state of the Pi agent during execution.

use serde::{Deserialize, Serialize};

/// State of the Pi agent
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum AgentState {
    /// Agent is idle, waiting for input
    #[default]
    Idle,

    /// Agent is waiting for user input
    WaitingForInput,

    /// Agent is thinking (calling LLM)
    Thinking {
        /// Current turn number
        turn: u32,
    },

    /// Agent is streaming response
    Streaming {
        /// Current turn number
        turn: u32,
    },

    /// Agent is executing a tool
    ExecutingTool {
        /// Current turn number
        turn: u32,
        /// Tool call ID
        tool_id: String,
        /// Tool name
        tool_name: String,
    },

    /// Agent completed successfully
    Completed,

    /// Agent encountered an error
    Error {
        /// Error message
        message: String,
    },

    /// Agent was cancelled
    Cancelled,
}

impl AgentState {
    /// Check if the agent is in a terminal state
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            AgentState::Completed | AgentState::Error { .. } | AgentState::Cancelled
        )
    }

    /// Check if the agent is currently processing
    pub fn is_processing(&self) -> bool {
        matches!(
            self,
            AgentState::Thinking { .. }
                | AgentState::Streaming { .. }
                | AgentState::ExecutingTool { .. }
        )
    }

    /// Check if the agent is idle
    pub fn is_idle(&self) -> bool {
        matches!(self, AgentState::Idle | AgentState::WaitingForInput)
    }

    /// Get the current turn number if processing
    pub fn current_turn(&self) -> Option<u32> {
        match self {
            AgentState::Thinking { turn }
            | AgentState::Streaming { turn }
            | AgentState::ExecutingTool { turn, .. } => Some(*turn),
            _ => None,
        }
    }
}
