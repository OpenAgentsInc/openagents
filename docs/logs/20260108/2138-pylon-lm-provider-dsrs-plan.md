# Pylon LM Provider for dsrs

Add Pylon as an LM provider in dsrs, enabling both local and swarm-backed inference for DSPy optimization.

## Overview

**Goal:** Allow dsrs to route inference requests through Pylon - either to local backends (Ollama, llama.cpp, Apple FM) or out to the swarm network via NIP-90.

**Key Insight:**
- **Local mode**: Use local backends for privacy, no cost, faster iteration
- **Swarm mode**: Distributed inference at ~10 msats/call for parallel optimization runs

## Architecture

```
dsrs LM abstraction                     Pylon
┌─────────────────┐                   ┌─────────────────────────┐
│   ChatAdapter   │                   │    PylonCompletionModel │
│       ↓         │                   │           │             │
│  LM::completion │                   │     ┌─────┴─────┐       │
│       ↓         │                   │     │           │       │
│   LMClient      │                   │  local?      swarm?     │
│  ┌──────────┐   │                   │     │           │       │
│  │  Pylon   │───┼──────────────────►│     ▼           ▼       │
│  │  variant │   │                   │ ┌───────┐  ┌─────────┐  │
│  └──────────┘   │                   │ │Ollama │  │ NIP-90  │  │
└─────────────────┘                   │ │llama  │  │ Swarm   │  │
                                      │ │Apple  │  │         │  │
                                      │ └───────┘  └─────────┘  │
                                      └─────────────────────────┘
```

## Files to Create/Modify

| File | Action | Purpose |
|------|--------|---------|
| `crates/dsrs/src/core/lm/pylon.rs` | NEW | PylonCompletionModel + PylonConfig |
| `crates/dsrs/src/core/lm/client_registry.rs` | MODIFY | Add Pylon variant to LMClient enum |
| `crates/dsrs/src/core/lm/usage.rs` | MODIFY | Add cost_msats field for sats tracking |
| `crates/dsrs/src/core/lm/mod.rs` | MODIFY | Export pylon module |
| `crates/dsrs/Cargo.toml` | MODIFY | Add nostr/compute dependencies |

## Implementation Steps

### Step 1: Extend LmUsage with cost tracking

**File:** `crates/dsrs/src/core/lm/usage.rs`

```rust
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct LmUsage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
    #[serde(default)]
    pub cost_msats: u64,  // millisatoshi cost (0 for local)
}

impl LmUsage {
    pub fn cost_sats(&self) -> u64 {
        (self.cost_msats + 999) / 1000
    }
}
```

### Step 2: Create PylonConfig with local/swarm modes

**File:** `crates/dsrs/src/core/lm/pylon.rs` (NEW)

```rust
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use anyhow::Result;

/// Execution venue for Pylon inference
#[derive(Clone, Debug, Default)]
pub enum PylonVenue {
    /// Use local backends only (Ollama, llama.cpp, Apple FM)
    /// No network, no cost, fastest iteration
    Local {
        /// Preferred backend: "ollama", "llama", "apple-fm", or "auto"
        backend: String,
    },
    /// Use swarm network via NIP-90
    /// Distributed, paid, parallel execution
    #[default]
    Swarm {
        relays: Vec<String>,
        bid_msats: u64,
        auto_pay: bool,
    },
    /// Try local first, fall back to swarm
    Hybrid {
        backend: String,
        relays: Vec<String>,
        bid_msats: u64,
        auto_pay: bool,
    },
}

/// Configuration for Pylon LM provider
#[derive(Clone)]
pub struct PylonConfig {
    pub venue: PylonVenue,
    pub timeout: Duration,
    pub budget_sats: u64,  // 0 = unlimited
    pub model: Option<String>,  // Model hint for local backends
}

impl Default for PylonConfig {
    fn default() -> Self {
        Self {
            venue: PylonVenue::Local {
                backend: "auto".to_string()
            },
            timeout: Duration::from_secs(60),
            budget_sats: 0,
            model: None,
        }
    }
}

impl PylonConfig {
    /// Local-only mode (no network)
    pub fn local() -> Self {
        Self {
            venue: PylonVenue::Local {
                backend: "auto".to_string()
            },
            ..Default::default()
        }
    }

    /// Local with specific backend
    pub fn local_with(backend: &str) -> Self {
        Self {
            venue: PylonVenue::Local {
                backend: backend.to_string()
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
                backend: "auto".to_string(),
                relays: vec!["wss://nexus.openagents.com".to_string()],
                bid_msats: 1000,
                auto_pay: true,
            },
            ..Default::default()
        }
    }
}

/// Pylon completion model supporting local and swarm inference
#[derive(Clone)]
pub struct PylonCompletionModel {
    config: PylonConfig,
    // For swarm mode
    dvm_client: Option<Arc<nostr_client::dvm::DvmClient>>,
    // For local mode
    local_executor: Option<Arc<LocalBackendExecutor>>,
    // Cost tracking
    spent_msats: Arc<RwLock<u64>>,
}

impl PylonCompletionModel {
    /// Create with local-only mode (no private key needed)
    pub async fn local(config: PylonConfig) -> Result<Self> {
        let executor = LocalBackendExecutor::detect().await?;
        Ok(Self {
            config,
            dvm_client: None,
            local_executor: Some(Arc::new(executor)),
            spent_msats: Arc::new(RwLock::new(0)),
        })
    }

    /// Create with swarm mode (requires private key for signing)
    pub async fn swarm(private_key: [u8; 32], config: PylonConfig) -> Result<Self> {
        let dvm = DvmClient::new(private_key)?;
        Ok(Self {
            config,
            dvm_client: Some(Arc::new(dvm)),
            local_executor: None,
            spent_msats: Arc::new(RwLock::new(0)),
        })
    }

    /// Create with hybrid mode (local + swarm fallback)
    pub async fn hybrid(private_key: [u8; 32], config: PylonConfig) -> Result<Self> {
        let dvm = DvmClient::new(private_key)?;
        let executor = LocalBackendExecutor::detect().await?;
        Ok(Self {
            config,
            dvm_client: Some(Arc::new(dvm)),
            local_executor: Some(Arc::new(executor)),
            spent_msats: Arc::new(RwLock::new(0)),
        })
    }

    /// Create from mnemonic (swarm or hybrid mode)
    pub async fn from_mnemonic(mnemonic: &str, config: PylonConfig) -> Result<Self> {
        let identity = UnifiedIdentity::from_mnemonic(mnemonic, "")?;
        let private_key = *identity.private_key_bytes();

        match &config.venue {
            PylonVenue::Local { .. } => Self::local(config).await,
            PylonVenue::Swarm { .. } => Self::swarm(private_key, config).await,
            PylonVenue::Hybrid { .. } => Self::hybrid(private_key, config).await,
        }
    }

    pub async fn total_spent_msats(&self) -> u64 {
        *self.spent_msats.read().await
    }
}
```

### Step 3: Implement CompletionProvider with venue routing

**File:** `crates/dsrs/src/core/lm/pylon.rs` (continued)

```rust
impl CompletionProvider for PylonCompletionModel {
    async fn completion(
        &self,
        request: CompletionRequest,
    ) -> Result<CompletionResponse<()>, CompletionError> {
        let prompt = build_prompt_from_request(&request);

        match &self.config.venue {
            PylonVenue::Local { backend } => {
                self.complete_local(&prompt, backend).await
            }
            PylonVenue::Swarm { relays, bid_msats, auto_pay } => {
                self.complete_swarm(&prompt, relays, *bid_msats, *auto_pay).await
            }
            PylonVenue::Hybrid { backend, relays, bid_msats, auto_pay } => {
                // Try local first
                match self.complete_local(&prompt, backend).await {
                    Ok(response) => Ok(response),
                    Err(_) => {
                        // Fall back to swarm
                        self.complete_swarm(&prompt, relays, *bid_msats, *auto_pay).await
                    }
                }
            }
        }
    }
}

impl PylonCompletionModel {
    async fn complete_local(
        &self,
        prompt: &str,
        backend: &str,
    ) -> Result<CompletionResponse<()>, CompletionError> {
        let executor = self.local_executor.as_ref()
            .ok_or_else(|| CompletionError::ProviderError("No local executor".into()))?;

        let result = executor.complete(prompt, backend, self.config.model.as_deref()).await
            .map_err(|e| CompletionError::ProviderError(e.to_string()))?;

        Ok(CompletionResponse {
            choice: OneOrMany::one(AssistantContent::Text(Text { text: result })),
            usage: Usage { input_tokens: 0, output_tokens: 0, total_tokens: 0 },
            raw_response: (),
        })
    }

    async fn complete_swarm(
        &self,
        prompt: &str,
        relays: &[String],
        bid_msats: u64,
        auto_pay: bool,
    ) -> Result<CompletionResponse<()>, CompletionError> {
        let dvm = self.dvm_client.as_ref()
            .ok_or_else(|| CompletionError::ProviderError("No DVM client".into()))?;

        // Submit NIP-90 job
        let mut job = JobRequest::new(KIND_JOB_TEXT_GENERATION)
            .map_err(|e| CompletionError::ProviderError(e.to_string()))?;
        job = job.add_input(JobInput::text(prompt));
        job = job.with_bid(bid_msats);
        for relay in relays {
            job = job.add_relay(relay);
        }

        let relay_refs: Vec<&str> = relays.iter().map(|s| s.as_str()).collect();
        let submission = dvm.submit_job(job, &relay_refs).await
            .map_err(|e| CompletionError::ProviderError(e.to_string()))?;

        // Handle payment if required
        let paid = if auto_pay {
            self.handle_payment(&submission.event_id).await.unwrap_or(0)
        } else {
            0
        };

        // Await result
        let result = dvm.await_result(&submission.event_id, self.config.timeout).await
            .map_err(|e| CompletionError::ProviderError(e.to_string()))?;

        // Track cost
        let cost = paid.max(result.amount.unwrap_or(0));
        if cost > 0 {
            let mut spent = self.spent_msats.write().await;
            *spent += cost;
        }

        Ok(CompletionResponse {
            choice: OneOrMany::one(AssistantContent::Text(Text { text: result.content })),
            usage: Usage { input_tokens: 0, output_tokens: 0, total_tokens: 0 },
            raw_response: (),
        })
    }
}
```

### Step 4: Add Pylon to LMClient enum (no feature gating)

**File:** `crates/dsrs/src/core/lm/client_registry.rs`

```rust
use super::pylon::PylonCompletionModel;

#[enum_dispatch(CompletionProvider)]
#[derive(Clone)]
pub enum LMClient {
    OpenAI(openai::completion::CompletionModel),
    Gemini(gemini::completion::CompletionModel),
    Anthropic(anthropic::completion::CompletionModel),
    Groq(groq::CompletionModel<reqwest::Client>),
    OpenRouter(openrouter::completion::CompletionModel),
    Ollama(ollama::CompletionModel<reqwest::Client>),
    Azure(azure::CompletionModel<reqwest::Client>),
    XAI(xai::completion::CompletionModel),
    Cohere(cohere::completion::CompletionModel),
    Mistral(mistral::completion::CompletionModel),
    Together(together::completion::CompletionModel),
    Deepseek(deepseek::CompletionModel<reqwest::Client>),
    Pylon(PylonCompletionModel),  // NEW
}

impl LMClient {
    /// Pylon local-only (no network, no private key needed)
    pub async fn pylon_local() -> Result<Self> {
        let model = PylonCompletionModel::local(PylonConfig::local()).await?;
        Ok(LMClient::Pylon(model))
    }

    /// Pylon local with specific backend
    pub async fn pylon_local_with(backend: &str) -> Result<Self> {
        let model = PylonCompletionModel::local(PylonConfig::local_with(backend)).await?;
        Ok(LMClient::Pylon(model))
    }

    /// Pylon swarm (distributed, requires mnemonic)
    pub async fn pylon_swarm(mnemonic: &str) -> Result<Self> {
        let model = PylonCompletionModel::from_mnemonic(mnemonic, PylonConfig::swarm()).await?;
        Ok(LMClient::Pylon(model))
    }

    /// Pylon with custom config
    pub async fn pylon(mnemonic: Option<&str>, config: PylonConfig) -> Result<Self> {
        let model = match mnemonic {
            Some(m) => PylonCompletionModel::from_mnemonic(m, config).await?,
            None => PylonCompletionModel::local(config).await?,
        };
        Ok(LMClient::Pylon(model))
    }

    // Update from_model_string:
    pub fn from_model_string(model_str: &str, api_key: Option<&str>) -> Result<Self> {
        // ... existing code ...

        match provider {
            // ... existing providers ...

            "pylon" => {
                // model_id: "local", "swarm", "hybrid", or "local:ollama", etc.
                let rt = tokio::runtime::Handle::current();
                rt.block_on(async {
                    if model_id == "swarm" {
                        let mnemonic = api_key.ok_or_else(|| {
                            anyhow::anyhow!("pylon:swarm requires mnemonic")
                        })?;
                        Self::pylon_swarm(mnemonic).await
                    } else if model_id.starts_with("local:") {
                        let backend = model_id.strip_prefix("local:").unwrap();
                        Self::pylon_local_with(backend).await
                    } else {
                        // Default to local
                        Self::pylon_local().await
                    }
                })
            }

            // ... rest ...
        }
    }
}
```

### Step 5: Update Cargo.toml (no feature gating)

**File:** `crates/dsrs/Cargo.toml`

```toml
[dependencies]
# ... existing deps ...

# Pylon integration (always enabled)
nostr = { path = "../nostr/core" }
nostr-client = { path = "../nostr/client" }
compute = { path = "../compute" }
```

## Usage Examples

### Local-only (no network, no cost)
```rust
// Simplest - auto-detect local backend
let client = LMClient::pylon_local().await?;

// Or specific backend
let client = LMClient::pylon_local_with("ollama").await?;

// Via model string
let client = LMClient::from_model_string("pylon:local", None)?;
let client = LMClient::from_model_string("pylon:local:ollama", None)?;
```

### Swarm (distributed, paid)
```rust
let mnemonic = std::fs::read_to_string("~/.openagents/pylon/identity.mnemonic")?;
let client = LMClient::pylon_swarm(&mnemonic).await?;

// Via model string
let client = LMClient::from_model_string("pylon:swarm", Some(&mnemonic))?;
```

### Hybrid (local first, swarm fallback)
```rust
let client = LMClient::pylon(Some(&mnemonic), PylonConfig::hybrid()).await?;
```

### Custom config
```rust
let config = PylonConfig {
    venue: PylonVenue::Swarm {
        relays: vec!["wss://nexus.openagents.com".to_string()],
        bid_msats: 2000,
        auto_pay: true,
    },
    timeout: Duration::from_secs(120),
    budget_sats: 50000,
    model: Some("llama2".to_string()),
};
let client = LMClient::pylon(Some(&mnemonic), config).await?;
```

## Success Criteria

1. `cargo build -p dsrs` compiles (no feature flags needed)
2. `LMClient::pylon_local()` works with auto-detected backend
3. `LMClient::pylon_swarm()` submits NIP-90 jobs
4. Hybrid mode falls back correctly
5. Cost tracking works for swarm calls
6. Budget limits enforced

## Key Dependencies

- `nostr-client::dvm::DvmClient` - Job submission
- `nostr::JobRequest`, `JobInput` - NIP-90 types
- `compute::domain::identity::UnifiedIdentity` - Key derivation
- Pylon's `LocalBackendExecutor` for local inference

## Notes

- No feature gating - Pylon always available
- Local mode is default (safest, no network)
- Token counts are 0 (neither local nor DVM report tokens)
- Cost is 0 for local, tracked in msats for swarm
