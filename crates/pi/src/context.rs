//! Context management and overflow handling
//!
//! Manages conversation context to stay within token limits,
//! using strategies like truncation or summarization.

use llm::Message;
use tracing::{debug, warn};

use crate::config::OverflowStrategy;

/// Context manager for handling token limits
pub struct ContextManager {
    /// Maximum context tokens
    max_tokens: u32,

    /// Overflow strategy
    strategy: OverflowStrategy,

    /// Reserved tokens for output
    reserved_output_tokens: u32,
}

impl Default for ContextManager {
    fn default() -> Self {
        Self::new(128_000)
    }
}

impl ContextManager {
    /// Create a new context manager
    pub fn new(max_tokens: u32) -> Self {
        Self {
            max_tokens,
            strategy: OverflowStrategy::Truncate { keep_last_n: 10 },
            reserved_output_tokens: 8_000,
        }
    }

    /// Set the overflow strategy
    pub fn with_strategy(mut self, strategy: OverflowStrategy) -> Self {
        self.strategy = strategy;
        self
    }

    /// Set reserved output tokens
    pub fn with_reserved_output(mut self, tokens: u32) -> Self {
        self.reserved_output_tokens = tokens;
        self
    }

    /// Get available context tokens (max - reserved for output)
    pub fn available_tokens(&self) -> u32 {
        self.max_tokens.saturating_sub(self.reserved_output_tokens)
    }

    /// Estimate tokens for a message (rough estimate: ~4 chars per token)
    pub fn estimate_tokens(text: &str) -> u32 {
        (text.len() as f64 / 4.0).ceil() as u32
    }

    /// Estimate total tokens for messages
    pub fn estimate_messages_tokens(messages: &[Message]) -> u32 {
        messages
            .iter()
            .map(|m| Self::estimate_message_tokens(m))
            .sum()
    }

    /// Estimate tokens for a single message
    fn estimate_message_tokens(message: &Message) -> u32 {
        let content_size = match &message.content {
            llm::Content::Text(text) => text.len(),
            llm::Content::Parts(parts) => parts
                .iter()
                .map(|p| match p {
                    llm::ContentPart::Text { text, .. } => text.len(),
                    llm::ContentPart::ToolUse { input, .. } => {
                        serde_json::to_string(input).map(|s| s.len()).unwrap_or(100)
                    }
                    llm::ContentPart::ToolResult { content, .. } => {
                        serde_json::to_string(content)
                            .map(|s| s.len())
                            .unwrap_or(100)
                    }
                    _ => 100, // Default estimate for other content types
                })
                .sum(),
        };

        // Add overhead for role, etc.
        ((content_size as f64 / 4.0) + 10.0).ceil() as u32
    }

    /// Check if context would overflow with these messages
    pub fn would_overflow(&self, messages: &[Message]) -> bool {
        let estimated = Self::estimate_messages_tokens(messages);
        estimated > self.available_tokens()
    }

    /// Get messages that fit within context limits
    ///
    /// Returns (processed_messages, was_truncated)
    pub fn fit_to_context(&self, messages: Vec<Message>) -> (Vec<Message>, bool) {
        let estimated = Self::estimate_messages_tokens(&messages);

        if estimated <= self.available_tokens() {
            return (messages, false);
        }

        debug!(
            estimated = estimated,
            available = self.available_tokens(),
            "Context overflow detected, applying strategy"
        );

        match &self.strategy {
            OverflowStrategy::Truncate { keep_last_n } => {
                let truncated = self.truncate_messages(messages, *keep_last_n);
                (truncated, true)
            }
            OverflowStrategy::Summarize { .. } => {
                // For now, fall back to truncation
                // Full summarization would require an LLM call
                warn!("Summarization not yet implemented, falling back to truncation");
                let truncated = self.truncate_messages(messages, 10);
                (truncated, true)
            }
            OverflowStrategy::Error => {
                // Return original messages, caller should handle the error
                (messages, false)
            }
        }
    }

    /// Truncate messages keeping the first (system context) and last N turns
    fn truncate_messages(&self, messages: Vec<Message>, keep_last_n: usize) -> Vec<Message> {
        if messages.len() <= keep_last_n + 1 {
            return messages;
        }

        let mut result = Vec::new();

        // Always keep first message (usually has important context)
        if let Some(first) = messages.first() {
            result.push(first.clone());
        }

        // Keep last N messages
        let start_idx = messages.len().saturating_sub(keep_last_n);
        for msg in messages.into_iter().skip(start_idx) {
            result.push(msg);
        }

        debug!(
            kept_messages = result.len(),
            "Truncated messages for context"
        );

        result
    }

    /// Create a truncation notice message
    pub fn truncation_notice(removed_count: usize) -> String {
        format!(
            "[Context truncated: {} earlier messages removed to fit context window]",
            removed_count
        )
    }
}

/// Token budget tracker for a conversation
#[derive(Debug, Default)]
pub struct TokenBudget {
    /// Tokens used by system prompt
    pub system_tokens: u32,

    /// Tokens used by messages
    pub message_tokens: u32,

    /// Tokens used by tool definitions
    pub tool_tokens: u32,

    /// Reserved for output
    pub reserved_output: u32,

    /// Maximum total tokens
    pub max_tokens: u32,
}

impl TokenBudget {
    /// Create a new token budget
    pub fn new(max_tokens: u32) -> Self {
        Self {
            max_tokens,
            reserved_output: 8_000,
            ..Default::default()
        }
    }

    /// Get total used tokens
    pub fn used(&self) -> u32 {
        self.system_tokens + self.message_tokens + self.tool_tokens
    }

    /// Get available tokens for messages
    pub fn available_for_messages(&self) -> u32 {
        self.max_tokens
            .saturating_sub(self.system_tokens)
            .saturating_sub(self.tool_tokens)
            .saturating_sub(self.reserved_output)
    }

    /// Check if budget is exceeded
    pub fn is_exceeded(&self) -> bool {
        self.used() + self.reserved_output > self.max_tokens
    }

    /// Get utilization percentage
    pub fn utilization(&self) -> f64 {
        if self.max_tokens == 0 {
            return 0.0;
        }
        (self.used() as f64 / self.max_tokens as f64) * 100.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_estimate_tokens() {
        // ~4 chars per token
        assert_eq!(ContextManager::estimate_tokens("hello"), 2); // 5/4 = 1.25 -> 2
        assert_eq!(ContextManager::estimate_tokens(""), 0);
        assert_eq!(ContextManager::estimate_tokens("a".repeat(100).as_str()), 25);
    }

    #[test]
    fn test_context_manager_no_overflow() {
        let manager = ContextManager::new(100_000);
        let messages = vec![Message::user("Hello"), Message::assistant("Hi there!")];

        assert!(!manager.would_overflow(&messages));

        let (result, truncated) = manager.fit_to_context(messages);
        assert!(!truncated);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_truncation() {
        let manager = ContextManager::new(100); // Very small limit
        let messages: Vec<Message> = (0..20)
            .map(|i| Message::user(format!("Message {}", i)))
            .collect();

        let (result, truncated) = manager.fit_to_context(messages);
        assert!(truncated);
        assert!(result.len() < 20);
    }

    #[test]
    fn test_token_budget() {
        let mut budget = TokenBudget::new(100_000);
        budget.system_tokens = 5_000;
        budget.tool_tokens = 3_000;
        budget.message_tokens = 20_000;

        assert_eq!(budget.used(), 28_000);
        assert!(!budget.is_exceeded());

        // Available for messages = 100k - 5k (system) - 3k (tools) - 8k (reserved) = 84k
        assert_eq!(budget.available_for_messages(), 84_000);
    }

    #[test]
    fn test_token_budget_exceeded() {
        let mut budget = TokenBudget::new(50_000);
        budget.system_tokens = 30_000;
        budget.message_tokens = 20_000;
        budget.reserved_output = 8_000;

        // Used: 50k, max with reserved: 50k, so exceeded
        assert!(budget.is_exceeded());
    }
}
