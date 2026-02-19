//! Pylon LM Provider for dsrs
//!
//! Enables both local and swarm-backed inference for DSPy optimization.
//!
//! ## Venues
//! - **Local**: Use Ollama backend directly via rig's provider (no network cost)
//! - **Swarm**: Distribute via NIP-90 to Pylon providers (paid, parallel execution)
//! - **Hybrid**: Try local first, fall back to swarm

use anyhow::Result;
use nostr::{JobInput, JobRequest, JobStatus, KIND_JOB_TEXT_GENERATION};
use nostr_client::dvm::DvmClient;
use rig::OneOrMany;
use rig::client::Nothing;
use rig::completion::{CompletionError, CompletionRequest, CompletionResponse, Usage};
use rig::message::{AssistantContent, Text};
use rig::providers::ollama;
use serde::Deserialize;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

/// Execution venue for Pylon inference
#[derive(Clone, Debug)]
pub enum PylonVenue {
    /// Use local Ollama backend via rig's provider
    /// No network cost, fastest iteration
    Local {
        /// Ollama base URL (default: http://localhost:11434)
        base_url: Option<String>,
        /// Model to use (e.g., "llama3.2", "mistral")
        model: String,
    },
    /// Use swarm network via NIP-90
    /// Distributed, paid, parallel execution
    Swarm {
        relays: Vec<String>,
        bid_msats: u64,
        auto_pay: bool,
    },
    /// Try local first, fall back to swarm
    Hybrid {
        base_url: Option<String>,
        model: String,
        relays: Vec<String>,
        bid_msats: u64,
        auto_pay: bool,
    },
}

impl Default for PylonVenue {
    fn default() -> Self {
        Self::Local {
            base_url: None,
            model: "llama3.2".to_string(),
        }
    }
}

/// Configuration for Pylon LM provider
#[derive(Clone)]
pub struct PylonConfig {
    pub venue: PylonVenue,
    pub timeout: Duration,
    pub budget_sats: u64, // 0 = unlimited
    pub max_tokens: Option<u64>,
    pub temperature: Option<f64>,
}

impl Default for PylonConfig {
    fn default() -> Self {
        Self {
            venue: PylonVenue::Local {
                base_url: None,
                model: "llama3.2".to_string(),
            },
            timeout: Duration::from_secs(60),
            budget_sats: 0,
            max_tokens: Some(512),
            temperature: Some(0.7),
        }
    }
}

impl PylonConfig {
    /// Local-only mode with default model (llama3.2)
    pub fn local() -> Self {
        Self {
            venue: PylonVenue::Local {
                base_url: None,
                model: "llama3.2".to_string(),
            },
            ..Default::default()
        }
    }

    /// Local with specific model
    pub fn local_with(model: &str) -> Self {
        Self {
            venue: PylonVenue::Local {
                base_url: None,
                model: model.to_string(),
            },
            ..Default::default()
        }
    }

    /// Swarm mode (distributed)
    pub fn swarm() -> Self {
        Self {
            venue: PylonVenue::Swarm {
                relays: vec!["wss://nexus.openagents.com".to_string()],
                bid_msats: 1000,
                auto_pay: true,
            },
            ..Default::default()
        }
    }

    /// Swarm with custom relays
    pub fn swarm_with(relays: Vec<String>, bid_msats: u64) -> Self {
        Self {
            venue: PylonVenue::Swarm {
                relays,
                bid_msats,
                auto_pay: true,
            },
            ..Default::default()
        }
    }

    /// Hybrid mode (local first, swarm fallback)
    pub fn hybrid() -> Self {
        Self {
            venue: PylonVenue::Hybrid {
                base_url: None,
                model: "llama3.2".to_string(),
                relays: vec!["wss://nexus.openagents.com".to_string()],
                bid_msats: 1000,
                auto_pay: true,
            },
            ..Default::default()
        }
    }
}

/// Pylon completion model supporting local and swarm inference
pub struct PylonCompletionModel {
    config: PylonConfig,
    // For swarm mode
    dvm_client: Option<Arc<DvmClient>>,
    // For local mode - Ollama client
    ollama_model: Option<ollama::CompletionModel<reqwest::Client>>,
    // Cost tracking
    spent_msats: Arc<RwLock<u64>>,
}

impl Clone for PylonCompletionModel {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            dvm_client: self.dvm_client.clone(),
            ollama_model: self.ollama_model.clone(),
            spent_msats: Arc::clone(&self.spent_msats),
        }
    }
}

#[derive(Debug, Deserialize)]
struct OllamaTags {
    models: Vec<OllamaTag>,
}

#[derive(Debug, Deserialize)]
struct OllamaTag {
    name: String,
}

impl PylonCompletionModel {
    /// Create with local-only mode using Ollama
    pub async fn local(config: PylonConfig) -> Result<Self> {
        let (base_url, model) = match &config.venue {
            PylonVenue::Local { base_url, model } => (base_url.clone(), model.clone()),
            PylonVenue::Hybrid {
                base_url, model, ..
            } => (base_url.clone(), model.clone()),
            _ => (None, "llama3.2".to_string()),
        };

        let model = resolve_ollama_model(&config, base_url.clone(), model).await;

        // Build Ollama client
        let mut builder = ollama::Client::builder().api_key(Nothing);
        if let Some(url) = base_url {
            builder = builder.base_url(&url);
        }
        let client = builder.build()?;
        let ollama_model = ollama::CompletionModel::new(client, &model);

        Ok(Self {
            config,
            dvm_client: None,
            ollama_model: Some(ollama_model),
            spent_msats: Arc::new(RwLock::new(0)),
        })
    }

    /// Create with swarm mode (requires private key for signing)
    pub async fn swarm(private_key: [u8; 32], config: PylonConfig) -> Result<Self> {
        let dvm = DvmClient::new(private_key)
            .map_err(|e| anyhow::anyhow!("Failed to create DVM client: {}", e))?;
        Ok(Self {
            config,
            dvm_client: Some(Arc::new(dvm)),
            ollama_model: None,
            spent_msats: Arc::new(RwLock::new(0)),
        })
    }

    /// Create with hybrid mode (local + swarm fallback)
    pub async fn hybrid(private_key: [u8; 32], config: PylonConfig) -> Result<Self> {
        let dvm = DvmClient::new(private_key)
            .map_err(|e| anyhow::anyhow!("Failed to create DVM client: {}", e))?;

        let (base_url, model) = match &config.venue {
            PylonVenue::Hybrid {
                base_url, model, ..
            } => (base_url.clone(), model.clone()),
            _ => (None, "llama3.2".to_string()),
        };

        let model = resolve_ollama_model(&config, base_url.clone(), model).await;

        // Build Ollama client
        let mut builder = ollama::Client::builder().api_key(Nothing);
        if let Some(url) = base_url {
            builder = builder.base_url(&url);
        }
        let client = builder.build()?;
        let ollama_model = ollama::CompletionModel::new(client, &model);

        Ok(Self {
            config,
            dvm_client: Some(Arc::new(dvm)),
            ollama_model: Some(ollama_model),
            spent_msats: Arc::new(RwLock::new(0)),
        })
    }

    /// Create from mnemonic (for swarm or hybrid mode)
    pub async fn from_mnemonic(mnemonic: &str, config: PylonConfig) -> Result<Self> {
        match &config.venue {
            PylonVenue::Local { .. } => Self::local(config).await,
            PylonVenue::Swarm { .. } | PylonVenue::Hybrid { .. } => {
                let private_key = derive_private_key_from_mnemonic(mnemonic)?;

                match &config.venue {
                    PylonVenue::Swarm { .. } => Self::swarm(private_key, config).await,
                    PylonVenue::Hybrid { .. } => Self::hybrid(private_key, config).await,
                    _ => unreachable!(),
                }
            }
        }
    }

    /// Get total spent in millisatoshis
    pub async fn total_spent_msats(&self) -> u64 {
        *self.spent_msats.read().await
    }

    /// Get remaining budget in satoshis (None if unlimited)
    pub async fn remaining_budget_sats(&self) -> Option<u64> {
        if self.config.budget_sats == 0 {
            None
        } else {
            let spent = *self.spent_msats.read().await;
            let spent_sats = spent.div_ceil(1000);
            Some(self.config.budget_sats.saturating_sub(spent_sats))
        }
    }

    /// Complete using local Ollama backend
    async fn complete_local(
        &self,
        request: &CompletionRequest,
    ) -> Result<CompletionResponse<()>, CompletionError> {
        let ollama_model = self
            .ollama_model
            .as_ref()
            .ok_or_else(|| CompletionError::ProviderError("No Ollama model configured".into()))?;

        // Use rig's CompletionModel trait directly (disambiguate from our CompletionProvider)
        let response =
            rig::completion::CompletionModel::completion(ollama_model, request.clone()).await?;

        Ok(CompletionResponse {
            choice: response.choice,
            usage: response.usage,
            raw_response: (),
        })
    }

    /// Complete using swarm via NIP-90
    async fn complete_swarm(
        &self,
        prompt: &str,
        relays: &[String],
        bid_msats: u64,
        auto_pay: bool,
    ) -> Result<CompletionResponse<()>, CompletionError> {
        let dvm = self
            .dvm_client
            .as_ref()
            .ok_or_else(|| CompletionError::ProviderError("No DVM client".into()))?;

        // Check budget before submitting
        if self.config.budget_sats > 0 {
            let spent = *self.spent_msats.read().await;
            let spent_sats = spent.div_ceil(1000);
            if spent_sats >= self.config.budget_sats {
                return Err(CompletionError::ProviderError(format!(
                    "Budget exhausted: spent {} sats, limit is {} sats",
                    spent_sats, self.config.budget_sats
                )));
            }
        }

        // Submit NIP-90 job
        let mut job = JobRequest::new(KIND_JOB_TEXT_GENERATION)
            .map_err(|e| CompletionError::ProviderError(e.to_string()))?;
        job = job.add_input(JobInput::text(prompt));
        job = job.with_bid(bid_msats);
        for relay in relays {
            if !relay.is_empty() {
                job = job.add_relay(relay);
            }
        }

        let relay_refs: Vec<&str> = relays.iter().map(|s| s.as_str()).collect();
        let submission = dvm
            .submit_job(job, &relay_refs)
            .await
            .map_err(|e| CompletionError::ProviderError(format!("Job submission failed: {}", e)))?;

        let job_id = submission.event_id.clone();

        // Handle payment if required
        let paid_msats = if auto_pay {
            self.handle_payment(&job_id).await.unwrap_or(0)
        } else {
            0
        };

        // Await result
        let result = dvm
            .await_result(&job_id, self.config.timeout)
            .await
            .map_err(|e| CompletionError::ProviderError(format!("Result timeout: {}", e)))?;

        // Track cost
        let cost = paid_msats.max(result.amount.unwrap_or(0));
        if cost > 0 {
            let mut spent = self.spent_msats.write().await;
            *spent += cost;
        }

        Ok(CompletionResponse {
            choice: OneOrMany::one(AssistantContent::Text(Text {
                text: result.content,
            })),
            usage: Usage {
                input_tokens: 0,
                output_tokens: 0,
                total_tokens: 0,
            },
            raw_response: (),
        })
    }

    /// Handle payment flow for NIP-90 job
    async fn handle_payment(&self, job_id: &str) -> Result<u64> {
        let dvm = self
            .dvm_client
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No DVM client"))?;

        let mut feedback_rx = dvm
            .subscribe_to_feedback(job_id)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to subscribe to feedback: {}", e))?;

        let feedback_timeout = Duration::from_secs(30);
        let start = std::time::Instant::now();

        while start.elapsed() < feedback_timeout {
            match tokio::time::timeout(Duration::from_millis(500), feedback_rx.recv()).await {
                Ok(Some(feedback_event)) => {
                    if feedback_event.feedback.status == JobStatus::PaymentRequired {
                        if let Some(_bolt11) = &feedback_event.feedback.bolt11 {
                            let amount_msats = feedback_event.feedback.amount.unwrap_or(0);

                            // Check budget
                            if self.config.budget_sats > 0 {
                                let current = *self.spent_msats.read().await;
                                let would_spend = current + amount_msats;
                                if would_spend.div_ceil(1000) > self.config.budget_sats {
                                    return Err(anyhow::anyhow!(
                                        "Budget exceeded: would spend {} sats, limit is {} sats",
                                        would_spend.div_ceil(1000),
                                        self.config.budget_sats
                                    ));
                                }
                            }

                            // Note: Auto-payment via Spark wallet would go here
                            // For now, just return the amount as if it was paid
                            return Ok(amount_msats);
                        }
                    } else if feedback_event.feedback.status == JobStatus::Error {
                        let msg = feedback_event
                            .feedback
                            .status_extra
                            .unwrap_or_else(|| "Unknown error".to_string());
                        return Err(anyhow::anyhow!("Job failed: {}", msg));
                    }
                }
                Ok(None) => break,
                Err(_) => continue, // Timeout on single recv, continue loop
            }
        }

        Ok(0)
    }
}

const DEFAULT_OLLAMA_URL: &str = "http://localhost:11434";

fn model_env_override() -> Option<String> {
    for key in ["PYLON_LOCAL_MODEL", "OLLAMA_MODEL"] {
        if let Ok(value) = std::env::var(key) {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    None
}

async fn resolve_ollama_model(
    config: &PylonConfig,
    base_url: Option<String>,
    default_model: String,
) -> String {
    if let Some(model) = model_env_override() {
        return model;
    }

    let base_url = base_url.unwrap_or_else(|| DEFAULT_OLLAMA_URL.to_string());
    let models = match fetch_ollama_models(&base_url, config.timeout).await {
        Ok(models) => models,
        Err(_) => return default_model,
    };

    if models.is_empty() {
        return default_model;
    }

    if models
        .iter()
        .any(|model| matches_model_name(&default_model, model))
    {
        return default_model;
    }

    if let Some(preferred) = select_preferred_model(&models) {
        return preferred;
    }

    models[0].clone()
}

async fn fetch_ollama_models(base_url: &str, timeout: Duration) -> Result<Vec<String>> {
    let client = reqwest::Client::builder().timeout(timeout).build()?;
    let url = format!("{}/api/tags", base_url.trim_end_matches('/'));
    let response = client.get(&url).send().await?.error_for_status()?;
    let tags: OllamaTags = response.json().await?;
    Ok(tags.models.into_iter().map(|model| model.name).collect())
}

fn matches_model_name(expected: &str, candidate: &str) -> bool {
    if expected == candidate {
        return true;
    }
    let expected_base = expected.split(':').next().unwrap_or(expected);
    let candidate_base = candidate.split(':').next().unwrap_or(candidate);
    expected_base == candidate_base
}

fn select_preferred_model(models: &[String]) -> Option<String> {
    let preferences = [
        "llama3.2",
        "llama3.1",
        "llama3",
        "mistral",
        "qwen",
        "gpt-oss",
        "codellama",
        "gemma",
        "phi",
    ];

    for preference in preferences {
        if let Some(model) = models
            .iter()
            .find(|name| name.to_lowercase().contains(preference))
        {
            return Some(model.clone());
        }
    }

    None
}

/// Derive private key from BIP-39 mnemonic
fn derive_private_key_from_mnemonic(mnemonic: &str) -> Result<[u8; 32]> {
    use bip39::Mnemonic;

    let mnemonic =
        Mnemonic::parse(mnemonic).map_err(|e| anyhow::anyhow!("Invalid mnemonic: {}", e))?;

    let seed = mnemonic.to_seed("");
    // Use first 32 bytes of seed as private key (simplified derivation)
    let mut key = [0u8; 32];
    key.copy_from_slice(&seed[0..32]);
    Ok(key)
}

/// Build prompt string from rig CompletionRequest
fn build_prompt_from_request(request: &CompletionRequest) -> String {
    let mut parts = Vec::new();

    // Add preamble/system prompt
    if let Some(preamble) = &request.preamble {
        parts.push(format!("System: {}", preamble));
    }

    // Add chat history
    for msg in request.chat_history.iter() {
        match msg {
            rig::message::Message::User { content } => {
                for c in content.iter() {
                    if let rig::message::UserContent::Text(text) = c {
                        parts.push(format!("User: {}", text.text));
                    }
                }
            }
            rig::message::Message::Assistant { content, .. } => {
                for c in content.iter() {
                    if let rig::message::AssistantContent::Text(text) = c {
                        parts.push(format!("Assistant: {}", text.text));
                    }
                }
            }
        }
    }

    parts.join("\n\n")
}

// Implement CompletionProvider for PylonCompletionModel
use super::CompletionProvider;

impl CompletionProvider for PylonCompletionModel {
    async fn completion(
        &self,
        request: CompletionRequest,
    ) -> Result<CompletionResponse<()>, CompletionError> {
        match &self.config.venue {
            PylonVenue::Local { .. } => self.complete_local(&request).await,
            PylonVenue::Swarm {
                relays,
                bid_msats,
                auto_pay,
            } => {
                let prompt = build_prompt_from_request(&request);
                self.complete_swarm(&prompt, relays, *bid_msats, *auto_pay)
                    .await
            }
            PylonVenue::Hybrid {
                relays,
                bid_msats,
                auto_pay,
                ..
            } => {
                // Try local first
                match self.complete_local(&request).await {
                    Ok(response) => Ok(response),
                    Err(_) => {
                        // Fall back to swarm
                        let prompt = build_prompt_from_request(&request);
                        self.complete_swarm(&prompt, relays, *bid_msats, *auto_pay)
                            .await
                    }
                }
            }
        }
    }
}
