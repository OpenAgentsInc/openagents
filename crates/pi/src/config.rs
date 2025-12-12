//! Pi agent configuration
//!
//! Configuration for the Pi agent including model selection, tool settings,
//! and runtime behavior.

use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};

/// Configuration for the Pi agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PiConfig {
    /// Model to use (e.g., "claude-sonnet-4-20250514")
    pub model: String,

    /// Maximum number of turns before stopping
    pub max_turns: u32,

    /// Maximum tokens per response
    pub max_tokens: u32,

    /// Custom system prompt (prepended to default)
    pub system_prompt: Option<String>,

    /// Tools to enable (empty = all default tools)
    pub tools: Vec<String>,

    /// Working directory for tool execution
    pub working_directory: PathBuf,

    /// Session directory for persistence
    pub session_dir: Option<PathBuf>,

    /// Retry configuration
    pub retry: RetryConfig,

    /// Context overflow handling strategy
    pub overflow_strategy: OverflowStrategy,

    /// Tool execution timeout
    pub tool_timeout: Duration,

    /// Enable thinking/reasoning (for supported models)
    pub enable_thinking: bool,
}

impl Default for PiConfig {
    fn default() -> Self {
        Self {
            model: "claude-sonnet-4-20250514".to_string(),
            max_turns: 50,
            max_tokens: 16384,
            system_prompt: None,
            tools: Vec::new(), // Empty = all defaults
            working_directory: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            session_dir: None,
            retry: RetryConfig::default(),
            overflow_strategy: OverflowStrategy::default(),
            tool_timeout: Duration::from_secs(120),
            enable_thinking: false,
        }
    }
}

impl PiConfig {
    /// Create a new config with the given model
    pub fn new(model: impl Into<String>) -> Self {
        Self {
            model: model.into(),
            ..Default::default()
        }
    }

    /// Set the working directory
    pub fn working_directory(mut self, dir: impl Into<PathBuf>) -> Self {
        self.working_directory = dir.into();
        self
    }

    /// Set the session directory
    pub fn session_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.session_dir = Some(dir.into());
        self
    }

    /// Set max turns
    pub fn max_turns(mut self, turns: u32) -> Self {
        self.max_turns = turns;
        self
    }

    /// Set max tokens
    pub fn max_tokens(mut self, tokens: u32) -> Self {
        self.max_tokens = tokens;
        self
    }

    /// Set custom system prompt
    pub fn system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt = Some(prompt.into());
        self
    }

    /// Set tools to enable
    pub fn tools(mut self, tools: Vec<String>) -> Self {
        self.tools = tools;
        self
    }

    /// Set retry configuration
    pub fn retry(mut self, retry: RetryConfig) -> Self {
        self.retry = retry;
        self
    }

    /// Set overflow strategy
    pub fn overflow_strategy(mut self, strategy: OverflowStrategy) -> Self {
        self.overflow_strategy = strategy;
        self
    }

    /// Enable thinking mode
    pub fn with_thinking(mut self) -> Self {
        self.enable_thinking = true;
        self
    }

    /// Set tool timeout
    pub fn tool_timeout(mut self, timeout: Duration) -> Self {
        self.tool_timeout = timeout;
        self
    }
}

/// Configuration for retry behavior
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConfig {
    /// Whether retry is enabled
    pub enabled: bool,

    /// Maximum number of retries
    pub max_retries: u32,

    /// Base delay for exponential backoff
    pub base_delay: Duration,

    /// Maximum delay between retries
    pub max_delay: Duration,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_retries: 3,
            base_delay: Duration::from_secs(2),
            max_delay: Duration::from_secs(30),
        }
    }
}

impl RetryConfig {
    /// Calculate delay for a given retry attempt (exponential backoff)
    pub fn delay_for_attempt(&self, attempt: u32) -> Duration {
        let delay = self.base_delay.as_secs_f64() * 2.0_f64.powi(attempt as i32);
        let capped = delay.min(self.max_delay.as_secs_f64());
        Duration::from_secs_f64(capped)
    }
}

/// Strategy for handling context overflow
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(tag = "strategy", rename_all = "snake_case")]
pub enum OverflowStrategy {
    /// Truncate oldest messages, keeping system prompt and last N messages
    #[default]
    Truncate,

    /// Summarize old messages using an LLM
    Summarize {
        /// Model to use for summarization (defaults to same model)
        model: Option<String>,
    },

    /// Return error on overflow
    Error,
}

/// Default tools available in Pi agent
pub const DEFAULT_TOOLS: &[&str] = &["bash", "read", "write", "edit", "grep", "find"];
