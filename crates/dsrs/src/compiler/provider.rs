//! LM Provider abstraction for SwarmCompiler
//!
//! Provides a unified interface for different LM backends with cost tracking.

use crate::core::lm::pylon::{PylonCompletionModel, PylonConfig, PylonVenue};
use anyhow::Result;
use async_trait::async_trait;
use rig::OneOrMany;
use rig::completion::CompletionRequest;
use rig::message::{AssistantContent, Message, Text, UserContent};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::RwLock;

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
/// premium providers (Codex/GPT-4) during optimization.
#[async_trait]
pub trait LMProvider: Send + Sync {
    /// Name of this provider (for logging/debugging).
    fn name(&self) -> &str;

    /// Estimated cost per 1000 tokens in millisatoshis.
    fn cost_per_1k_tokens(&self) -> u64;

    /// Execute a completion request.
    async fn complete(&self, messages: Vec<LMMessage>, config: &LMConfig) -> Result<LMCompletion>;

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

    /// Create an expensive mock (simulating Codex).
    pub fn expensive() -> Self {
        Self::new("mock-codex", 1000)
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

    async fn complete(&self, _messages: Vec<LMMessage>, config: &LMConfig) -> Result<LMCompletion> {
        let idx = self.response_idx.fetch_add(1, Ordering::SeqCst) as usize;
        let response = &self.responses[idx % self.responses.len()];

        // Estimate tokens from response length
        let tokens = config
            .max_tokens
            .unwrap_or(100)
            .min(response.len() as u64 / 4 + 10);
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
/// Uses the existing PylonConfig infrastructure and PylonCompletionModel
/// for actual inference (local Ollama or swarm via NIP-90).
pub struct PylonLM {
    name: String,
    venue_cost: u64, // Cost per 1k tokens based on venue
    model: Option<Arc<RwLock<PylonCompletionModel>>>,
    spent: AtomicU64,
}

impl PylonLM {
    /// Create a new Pylon LM provider (mock mode for testing).
    ///
    /// # Arguments
    /// * `name` - Identifier for this provider
    /// * `is_swarm` - If true, uses swarm pricing (~10 msats/1k), else local (0)
    pub fn new(name: impl Into<String>, is_swarm: bool) -> Self {
        Self {
            name: name.into(),
            venue_cost: if is_swarm { 10 } else { 0 },
            model: None,
            spent: AtomicU64::new(0),
        }
    }

    /// Create a local Pylon provider (mock mode).
    pub fn local() -> Self {
        Self::new("pylon-local", false)
    }

    /// Create a swarm Pylon provider (mock mode, ~10 msats/1k tokens).
    pub fn swarm() -> Self {
        Self::new("pylon-swarm", true)
    }

    /// Create with an actual PylonCompletionModel (online mode).
    pub fn with_model(
        name: impl Into<String>,
        model: PylonCompletionModel,
        is_swarm: bool,
    ) -> Self {
        Self {
            name: name.into(),
            venue_cost: if is_swarm { 10 } else { 0 },
            model: Some(Arc::new(RwLock::new(model))),
            spent: AtomicU64::new(0),
        }
    }

    /// Create a local Pylon provider with actual Ollama backend.
    pub async fn local_connected(model_name: &str) -> Result<Self> {
        let config = PylonConfig::local_with(model_name);
        let model = PylonCompletionModel::local(config).await?;
        Ok(Self::with_model("pylon-local", model, false))
    }

    /// Create a swarm Pylon provider with actual NIP-90 backend.
    pub async fn swarm_connected(private_key: [u8; 32]) -> Result<Self> {
        let config = PylonConfig::swarm();
        let model = PylonCompletionModel::swarm(private_key, config).await?;
        Ok(Self::with_model("pylon-swarm", model, true))
    }

    /// Create a hybrid Pylon provider (local + swarm fallback).
    pub async fn hybrid_connected(private_key: [u8; 32], model_name: &str) -> Result<Self> {
        let config = PylonConfig {
            venue: PylonVenue::Hybrid {
                base_url: None,
                model: model_name.to_string(),
                relays: vec!["wss://nexus.openagents.com".to_string()],
                bid_msats: 1000,
                auto_pay: true,
            },
            ..Default::default()
        };
        let model = PylonCompletionModel::hybrid(private_key, config).await?;
        Ok(Self::with_model("pylon-hybrid", model, true))
    }

    /// Create from BIP-39 mnemonic.
    pub async fn from_mnemonic(mnemonic: &str, venue: PylonVenue) -> Result<Self> {
        let config = PylonConfig {
            venue: venue.clone(),
            ..Default::default()
        };
        let model = PylonCompletionModel::from_mnemonic(mnemonic, config).await?;
        let is_swarm = !matches!(venue, PylonVenue::Local { .. });
        Ok(Self::with_model("pylon-from-mnemonic", model, is_swarm))
    }

    /// Check if this provider has an actual model connected.
    pub fn is_connected(&self) -> bool {
        self.model.is_some()
    }

    /// Convert LMMessage to rig Message format.
    fn to_rig_messages(messages: &[LMMessage]) -> (Option<String>, OneOrMany<Message>) {
        let mut preamble = None;
        let mut chat_history = Vec::new();

        for msg in messages {
            match msg.role {
                MessageRole::System => {
                    preamble = Some(msg.content.clone());
                }
                MessageRole::User => {
                    chat_history.push(Message::User {
                        content: OneOrMany::one(UserContent::Text(Text {
                            text: msg.content.clone(),
                        })),
                    });
                }
                MessageRole::Assistant => {
                    chat_history.push(Message::Assistant {
                        content: OneOrMany::one(AssistantContent::Text(Text {
                            text: msg.content.clone(),
                        })),
                        id: None,
                    });
                }
            }
        }

        // Convert Vec to OneOrMany
        let history = if chat_history.is_empty() {
            // If empty, create a dummy user message
            OneOrMany::one(Message::User {
                content: OneOrMany::one(UserContent::Text(Text {
                    text: "".to_string(),
                })),
            })
        } else if chat_history.len() == 1 {
            OneOrMany::one(chat_history.remove(0))
        } else {
            OneOrMany::many(chat_history).expect("non-empty chat history")
        };

        (preamble, history)
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

    async fn complete(&self, messages: Vec<LMMessage>, config: &LMConfig) -> Result<LMCompletion> {
        // If we have a connected model, use it
        if let Some(model) = &self.model {
            let model_guard = model.read().await;

            // Convert messages to rig format
            let (preamble, chat_history) = Self::to_rig_messages(&messages);

            // Build completion request
            let request = CompletionRequest {
                preamble,
                chat_history,
                tools: vec![],
                tool_choice: None,
                max_tokens: config.max_tokens,
                temperature: config.temperature,
                additional_params: None,
                documents: vec![],
            };

            // Execute completion via PylonCompletionModel
            use crate::core::lm::CompletionProvider;
            let response = model_guard.completion(request).await?;

            // Extract text from response
            let content = response
                .choice
                .iter()
                .filter_map(|c| {
                    if let rig::message::AssistantContent::Text(t) = c {
                        Some(t.text.clone())
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
                .join("");

            let tokens = response.usage.total_tokens as u64;
            let cost = (tokens * self.venue_cost) / 1000;

            self.spent.fetch_add(cost, Ordering::SeqCst);

            return Ok(LMCompletion {
                content,
                tokens_used: tokens,
                cost_msats: cost,
                model_id: self.name.clone(),
            });
        }

        // Mock mode - return placeholder
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
                "Pylon LM (mock mode). Prompt preview: {}...",
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
        self.providers
            .first()
            .map(|p| p.cost_per_1k_tokens())
            .unwrap_or(0)
    }

    async fn complete(&self, messages: Vec<LMMessage>, config: &LMConfig) -> Result<LMCompletion> {
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
