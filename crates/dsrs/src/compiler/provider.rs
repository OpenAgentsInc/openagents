//! LM Provider abstraction for SwarmCompiler
//!
//! Provides a unified interface for different LM backends with cost tracking.

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Message role in a conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageRole {
    System,
    User,
    Assistant,
}

/// A message in a conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LMMessage {
    pub role: MessageRole,
    pub content: String,
}

impl LMMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::System,
            content: content.into(),
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::User,
            content: content.into(),
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::Assistant,
            content: content.into(),
        }
    }
}

/// Configuration for LM completion requests.
#[derive(Debug, Clone, Default)]
pub struct LMConfig {
    /// Maximum tokens to generate.
    pub max_tokens: Option<u64>,
    /// Sampling temperature (0.0 - 2.0).
    pub temperature: Option<f64>,
    /// Stop sequences.
    pub stop_sequences: Vec<String>,
}

impl LMConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_max_tokens(mut self, tokens: u64) -> Self {
        self.max_tokens = Some(tokens);
        self
    }

    pub fn with_temperature(mut self, temp: f64) -> Self {
        self.temperature = Some(temp);
        self
    }

    pub fn with_stop(mut self, stop: Vec<String>) -> Self {
        self.stop_sequences = stop;
        self
    }
}

/// Completion result from an LM provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LMCompletion {
    /// Generated content.
    pub content: String,
    /// Tokens used in this request.
    pub tokens_used: u64,
    /// Cost in millisatoshis.
    pub cost_msats: u64,
    /// Model identifier.
    pub model_id: String,
}

/// Trait for LM providers that can be used in SwarmCompiler.
///
/// This abstraction allows mixing cheap providers (Pylon swarm) with
/// premium providers (Claude/GPT-4) during optimization.
#[async_trait]
pub trait LMProvider: Send + Sync {
    /// Name of this provider (for logging/debugging).
    fn name(&self) -> &str;

    /// Estimated cost per 1000 tokens in millisatoshis.
    fn cost_per_1k_tokens(&self) -> u64;

    /// Execute a completion request.
    async fn complete(
        &self,
        messages: Vec<LMMessage>,
        config: &LMConfig,
    ) -> Result<LMCompletion>;

    /// Total spent so far (for budget tracking).
    fn total_spent_msats(&self) -> u64;
}

/// Mock LM provider for testing.
///
/// Returns deterministic responses based on configuration.
pub struct MockLM {
    name: String,
    cost_per_1k: u64,
    responses: Vec<String>,
    response_idx: AtomicU64,
    spent: AtomicU64,
}

impl MockLM {
    /// Create a new mock LM with a name and cost.
    pub fn new(name: impl Into<String>, cost_per_1k_tokens: u64) -> Self {
        Self {
            name: name.into(),
            cost_per_1k: cost_per_1k_tokens,
            responses: vec!["Mock response".to_string()],
            response_idx: AtomicU64::new(0),
            spent: AtomicU64::new(0),
        }
    }

    /// Create a cheap mock (simulating Pylon swarm).
    pub fn cheap() -> Self {
        Self::new("mock-pylon", 10)
    }

    /// Create an expensive mock (simulating Claude).
    pub fn expensive() -> Self {
        Self::new("mock-claude", 1000)
    }

    /// Set custom responses (cycles through them).
    pub fn with_responses(mut self, responses: Vec<String>) -> Self {
        self.responses = responses;
        self
    }
}

#[async_trait]
impl LMProvider for MockLM {
    fn name(&self) -> &str {
        &self.name
    }

    fn cost_per_1k_tokens(&self) -> u64 {
        self.cost_per_1k
    }

    async fn complete(
        &self,
        _messages: Vec<LMMessage>,
        config: &LMConfig,
    ) -> Result<LMCompletion> {
        let idx = self.response_idx.fetch_add(1, Ordering::SeqCst) as usize;
        let response = &self.responses[idx % self.responses.len()];

        // Estimate tokens from response length
        let tokens = config.max_tokens.unwrap_or(100).min(response.len() as u64 / 4 + 10);
        let cost = (tokens * self.cost_per_1k) / 1000;

        self.spent.fetch_add(cost, Ordering::SeqCst);

        Ok(LMCompletion {
            content: response.clone(),
            tokens_used: tokens,
            cost_msats: cost,
            model_id: self.name.clone(),
        })
    }

    fn total_spent_msats(&self) -> u64 {
        self.spent.load(Ordering::SeqCst)
    }
}

/// Wrapper around Pylon LM to implement LMProvider.
///
/// Uses the existing PylonConfig infrastructure.
pub struct PylonLM {
    name: String,
    venue_cost: u64, // Cost per 1k tokens based on venue
    spent: AtomicU64,
}

impl PylonLM {
    /// Create a new Pylon LM provider.
    ///
    /// # Arguments
    /// * `name` - Identifier for this provider
    /// * `is_swarm` - If true, uses swarm pricing (~10 msats/1k), else local (0)
    pub fn new(name: impl Into<String>, is_swarm: bool) -> Self {
        Self {
            name: name.into(),
            venue_cost: if is_swarm { 10 } else { 0 },
            spent: AtomicU64::new(0),
        }
    }

    /// Create a local Pylon provider (free).
    pub fn local() -> Self {
        Self::new("pylon-local", false)
    }

    /// Create a swarm Pylon provider (~10 msats/1k tokens).
    pub fn swarm() -> Self {
        Self::new("pylon-swarm", true)
    }
}

#[async_trait]
impl LMProvider for PylonLM {
    fn name(&self) -> &str {
        &self.name
    }

    fn cost_per_1k_tokens(&self) -> u64 {
        self.venue_cost
    }

    async fn complete(
        &self,
        messages: Vec<LMMessage>,
        config: &LMConfig,
    ) -> Result<LMCompletion> {
        // TODO: Connect to actual Pylon infrastructure
        // For now, return a placeholder that indicates integration needed

        let tokens = config.max_tokens.unwrap_or(100);
        let cost = (tokens * self.venue_cost) / 1000;

        self.spent.fetch_add(cost, Ordering::SeqCst);

        // Format messages for logging
        let prompt_preview: String = messages
            .iter()
            .filter(|m| matches!(m.role, MessageRole::User))
            .map(|m| m.content.chars().take(50).collect::<String>())
            .collect::<Vec<_>>()
            .join(" | ");

        Ok(LMCompletion {
            content: format!(
                "Pylon LM not yet integrated. Prompt preview: {}...",
                prompt_preview
            ),
            tokens_used: tokens,
            cost_msats: cost,
            model_id: self.name.clone(),
        })
    }

    fn total_spent_msats(&self) -> u64 {
        self.spent.load(Ordering::SeqCst)
    }
}

/// Multi-provider wrapper that tries providers in order.
///
/// Useful for hybrid strategies (try local first, fall back to swarm).
pub struct FallbackLM {
    name: String,
    providers: Vec<Arc<dyn LMProvider>>,
}

impl FallbackLM {
    pub fn new(name: impl Into<String>, providers: Vec<Arc<dyn LMProvider>>) -> Self {
        Self {
            name: name.into(),
            providers,
        }
    }
}

#[async_trait]
impl LMProvider for FallbackLM {
    fn name(&self) -> &str {
        &self.name
    }

    fn cost_per_1k_tokens(&self) -> u64 {
        // Return the cost of the first provider (primary)
        self.providers.first().map(|p| p.cost_per_1k_tokens()).unwrap_or(0)
    }

    async fn complete(
        &self,
        messages: Vec<LMMessage>,
        config: &LMConfig,
    ) -> Result<LMCompletion> {
        let mut last_error = None;

        for provider in &self.providers {
            match provider.complete(messages.clone(), config).await {
                Ok(completion) => return Ok(completion),
                Err(e) => {
                    last_error = Some(e);
                    continue;
                }
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow::anyhow!("No providers available")))
    }

    fn total_spent_msats(&self) -> u64 {
        self.providers.iter().map(|p| p.total_spent_msats()).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_lm_basic() {
        let lm = MockLM::cheap();
        assert_eq!(lm.name(), "mock-pylon");
        assert_eq!(lm.cost_per_1k_tokens(), 10);

        let messages = vec![LMMessage::user("Hello")];
        let config = LMConfig::new().with_max_tokens(100);

        let result = lm.complete(messages, &config).await.unwrap();
        assert_eq!(result.content, "Mock response");
        assert!(result.cost_msats <= 1); // 100 tokens * 10/1000
    }

    #[tokio::test]
    async fn test_mock_lm_custom_responses() {
        let lm = MockLM::new("test", 100)
            .with_responses(vec!["First".to_string(), "Second".to_string()]);

        let messages = vec![LMMessage::user("Test")];
        let config = LMConfig::default();

        let r1 = lm.complete(messages.clone(), &config).await.unwrap();
        let r2 = lm.complete(messages.clone(), &config).await.unwrap();
        let r3 = lm.complete(messages, &config).await.unwrap();

        assert_eq!(r1.content, "First");
        assert_eq!(r2.content, "Second");
        assert_eq!(r3.content, "First"); // Cycles back
    }

    #[tokio::test]
    async fn test_pylon_lm_cost() {
        let local = PylonLM::local();
        let swarm = PylonLM::swarm();

        assert_eq!(local.cost_per_1k_tokens(), 0);
        assert_eq!(swarm.cost_per_1k_tokens(), 10);
    }

    #[tokio::test]
    async fn test_lm_message_constructors() {
        let system = LMMessage::system("You are helpful");
        let user = LMMessage::user("Hello");
        let assistant = LMMessage::assistant("Hi there");

        assert!(matches!(system.role, MessageRole::System));
        assert!(matches!(user.role, MessageRole::User));
        assert!(matches!(assistant.role, MessageRole::Assistant));
    }

    #[tokio::test]
    async fn test_fallback_lm() {
        let primary = Arc::new(MockLM::cheap().with_responses(vec!["Primary".to_string()]));
        let fallback = Arc::new(MockLM::expensive().with_responses(vec!["Fallback".to_string()]));

        let lm = FallbackLM::new("hybrid", vec![primary, fallback]);

        let messages = vec![LMMessage::user("Test")];
        let config = LMConfig::default();

        let result = lm.complete(messages, &config).await.unwrap();
        assert_eq!(result.content, "Primary");
    }
}
