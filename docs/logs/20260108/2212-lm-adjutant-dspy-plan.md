# Multi-Provider LM Config for Adjutant DSPy

Extend Adjutant's `lm_config.rs` to support multiple LM providers with smart priority/fallback.

## Goal

Replace Cerebras-only DSPy config with multi-provider support:
1. **Claude Code headless** (preferred) - Best quality via `claude -p` subprocess
2. **Pylon swarm** - Distributed inference via NIP-90
3. **Cerebras** - Fast, cheap tiered execution
4. **Pylon local** (fallback) - Ollama when nothing else available

## Current State

```rust
// lm_config.rs - Cerebras only
pub async fn create_lm_from_env(model: &str) -> Result<LM> {
    if std::env::var("CEREBRAS_API_KEY").is_ok() { ... }
    if std::env::var("OPENAI_API_KEY").is_ok() { ... }
    Err("No API key found")
}
```

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Provider Priority                          │
├─────────────────────────────────────────────────────────────┤
│  1. `claude` CLI available? ──YES──►  Claude Code headless   │
│           │                           (Pro/Max subscription)  │
│           NO                                                  │
│           ▼                                                   │
│  2. PYLON_MNEMONIC set?     ──YES──►  pylon:swarm            │
│           │                           (Distributed)           │
│           NO                                                  │
│           ▼                                                   │
│  3. CEREBRAS_API_KEY set?   ──YES──►  cerebras:glm-4.7       │
│           │                           (Fast/cheap)            │
│           NO                                                  │
│           ▼                                                   │
│  4. Ollama running?         ──YES──►  pylon:local            │
│           │                           (Fallback)              │
│           NO                                                  │
│           ▼                                                   │
│  5. Error: No LM provider available                          │
└─────────────────────────────────────────────────────────────┘
```

## Files to Modify

| File | Action | Purpose |
|------|--------|---------|
| `crates/dsrs/src/core/lm/claude_sdk.rs` | NEW | Claude via claude-agent-sdk |
| `crates/dsrs/src/core/lm/client_registry.rs` | MODIFY | Add ClaudeSdk variant |
| `crates/dsrs/src/core/lm/mod.rs` | MODIFY | Export claude_sdk |
| `crates/dsrs/Cargo.toml` | MODIFY | Add claude-agent-sdk dep |
| `crates/adjutant/src/dspy/lm_config.rs` | REWRITE | Multi-provider with priority chain |

## Implementation

### Step 1: Add ClaudeSdk provider to dsrs (uses claude-agent-sdk)

**File:** `crates/dsrs/src/core/lm/claude_sdk.rs` (NEW)

```rust
//! Claude provider via claude-agent-sdk.
//!
//! Uses the existing SDK which wraps Claude CLI headless mode.

use anyhow::Result;
use claude_agent_sdk::{query, QueryOptions, SdkMessage, ToolsConfig};
use futures::StreamExt;
use rig::completion::{CompletionError, CompletionRequest, CompletionResponse, Usage};
use rig::message::{AssistantContent, Text};
use rig::OneOrMany;

/// Check if Claude CLI is available (re-export from adjutant::auth pattern)
pub fn has_claude_cli() -> bool {
    which::which("claude").is_ok()
        || dirs::home_dir()
            .map(|h| h.join(".claude/local/claude").exists())
            .unwrap_or(false)
}

/// Claude completion model via claude-agent-sdk
#[derive(Clone)]
pub struct ClaudeSdkModel {
    pub max_turns: Option<u32>,
}

impl Default for ClaudeSdkModel {
    fn default() -> Self {
        Self { max_turns: Some(1) } // Single turn for pure completion
    }
}

impl ClaudeSdkModel {
    pub fn new() -> Self {
        Self::default()
    }

    /// Execute completion via claude-agent-sdk
    pub async fn complete(&self, prompt: &str) -> Result<String, CompletionError> {
        let options = QueryOptions::new()
            .max_turns(self.max_turns.unwrap_or(1))
            .tools(ToolsConfig::none()); // No tools for pure LM completion

        let mut stream = query(prompt, options)
            .await
            .map_err(|e| CompletionError::ProviderError(format!("Failed to start query: {}", e)))?;

        let mut result_text = String::new();

        while let Some(msg) = stream.next().await {
            match msg {
                Ok(SdkMessage::Result(result)) => {
                    // Final result - extract the text
                    if let Some(text) = result.result {
                        result_text = text;
                    }
                    break;
                }
                Ok(SdkMessage::Assistant(assistant)) => {
                    // Accumulate assistant messages
                    if let Some(content) = &assistant.message.content {
                        result_text.push_str(content);
                    }
                }
                Ok(_) => {} // Ignore other message types
                Err(e) => {
                    return Err(CompletionError::ProviderError(format!(
                        "Stream error: {}",
                        e
                    )));
                }
            }
        }

        if result_text.is_empty() {
            return Err(CompletionError::ProviderError(
                "No response from Claude".into(),
            ));
        }

        Ok(result_text)
    }
}

// Implement CompletionProvider
use super::CompletionProvider;

impl CompletionProvider for ClaudeSdkModel {
    async fn completion(
        &self,
        request: CompletionRequest,
    ) -> Result<CompletionResponse<()>, CompletionError> {
        let prompt = build_prompt_from_request(&request);
        let result = self.complete(&prompt).await?;

        Ok(CompletionResponse {
            choice: OneOrMany::one(AssistantContent::Text(Text { text: result })),
            usage: Usage {
                input_tokens: 0,
                output_tokens: 0,
                total_tokens: 0,
            },
            raw_response: (),
        })
    }
}

fn build_prompt_from_request(request: &CompletionRequest) -> String {
    let mut parts = Vec::new();
    if let Some(preamble) = &request.preamble {
        parts.push(format!("System: {}", preamble));
    }
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
```

### Step 2: Add to LMClient enum

**File:** `crates/dsrs/src/core/lm/client_registry.rs`

```rust
// Add import
use super::claude_sdk::ClaudeSdkModel;

// Add variant to enum
pub enum LMClient {
    // ... existing variants ...
    ClaudeSdk(ClaudeSdkModel),
}

// Add factory method
impl LMClient {
    pub fn claude_sdk() -> Result<Self> {
        if !claude_sdk::has_claude_cli() {
            anyhow::bail!("Claude CLI not found");
        }
        Ok(LMClient::ClaudeSdk(ClaudeSdkModel::new()))
    }
}

// Update from_model_string
match provider {
    "claude-sdk" | "claude" => Self::claude_sdk(),
    // ... existing ...
}
```

### Step 3: Update Adjutant lm_config.rs

**File:** `crates/adjutant/src/dspy/lm_config.rs`

```rust
//! Multi-provider LM configuration for dsrs.

use anyhow::Result;
use dsrs::{ChatAdapter, LM, LMClient, configure};
use dsrs::claude_sdk::has_claude_cli;
use std::sync::Arc;

/// Provider priority for LM selection
#[derive(Clone, Debug, PartialEq)]
pub enum LmProvider {
    ClaudeSdk,       // claude-agent-sdk (Pro/Max subscription)
    PylonSwarm,      // pylon:swarm (distributed NIP-90)
    Cerebras,        // OpenAI-compatible Cerebras API
    PylonLocal,      // pylon:local (Ollama fallback)
}

/// Detect best available provider based on environment
pub fn detect_provider() -> Option<LmProvider> {
    // Priority 1: Claude via SDK (uses subscription)
    if has_claude_cli() {
        return Some(LmProvider::ClaudeSdk);
    }

    // Priority 2: Pylon swarm (requires mnemonic)
    if std::env::var("PYLON_MNEMONIC").is_ok() {
        return Some(LmProvider::PylonSwarm);
    }

    // Priority 3: Cerebras
    if std::env::var("CEREBRAS_API_KEY").is_ok() {
        return Some(LmProvider::Cerebras);
    }

    // Priority 4: Check for local Ollama
    if check_ollama_available() {
        return Some(LmProvider::PylonLocal);
    }

    None
}

fn check_ollama_available() -> bool {
    std::net::TcpStream::connect_timeout(
        &"127.0.0.1:11434".parse().unwrap(),
        std::time::Duration::from_millis(100),
    ).is_ok()
}

/// Create LM for detected or specified provider
pub async fn create_lm(provider: &LmProvider) -> Result<LM> {
    match provider {
        LmProvider::ClaudeSdk => {
            let client = LMClient::claude_sdk()?;
            LM::builder()
                .model("claude-sdk:default".to_string())
                .temperature(0.7)
                .max_tokens(4000)
                .build()
                .await?
                .with_client(client)
                .await
        }
        LmProvider::PylonSwarm => {
            let mnemonic = std::env::var("PYLON_MNEMONIC")?;
            LM::builder()
                .model("pylon:swarm".to_string())
                .api_key(mnemonic)
                .temperature(0.7)
                .max_tokens(4000)
                .build()
                .await
        }
        LmProvider::Cerebras => {
            let api_key = std::env::var("CEREBRAS_API_KEY")?;
            LM::builder()
                .base_url("https://api.cerebras.ai/v1".to_string())
                .api_key(api_key)
                .model("zai-glm-4.7".to_string())
                .temperature(0.7)
                .max_tokens(4000)
                .build()
                .await
        }
        LmProvider::PylonLocal => {
            LM::builder()
                .model("pylon:local".to_string())
                .temperature(0.7)
                .max_tokens(4000)
                .build()
                .await
        }
    }
}

/// Create planning/execution LM with auto-detected provider
pub async fn create_planning_lm() -> Result<LM> {
    let provider = detect_provider()
        .ok_or_else(|| anyhow::anyhow!(
            "No LM provider available. Install Claude CLI, set PYLON_MNEMONIC, \
             CEREBRAS_API_KEY, or run Ollama locally."
        ))?;
    tracing::info!("Using LM provider: {:?}", provider);
    create_lm(&provider).await
}

pub async fn create_execution_lm() -> Result<LM> {
    let provider = detect_provider()
        .ok_or_else(|| anyhow::anyhow!("No LM provider available"))?;

    // For Cerebras, use cheaper model for execution
    if provider == LmProvider::Cerebras {
        let api_key = std::env::var("CEREBRAS_API_KEY")?;
        return LM::builder()
            .base_url("https://api.cerebras.ai/v1".to_string())
            .api_key(api_key)
            .model("qwen-3-32b".to_string())
            .temperature(0.7)
            .max_tokens(4000)
            .build()
            .await;
    }

    create_lm(&provider).await
}

/// Get active provider for logging
pub fn get_active_provider() -> Option<LmProvider> {
    detect_provider()
}

/// Arc-wrapped LMs
pub async fn get_planning_lm() -> Result<Arc<LM>> {
    Ok(Arc::new(create_planning_lm().await?))
}

pub async fn get_execution_lm() -> Result<Arc<LM>> {
    Ok(Arc::new(create_execution_lm().await?))
}

/// Configure global dsrs
pub async fn configure_dsrs() -> Result<()> {
    let lm = create_planning_lm().await?;
    configure(lm, ChatAdapter);
    Ok(())
}

// Legacy compatibility
pub const PLANNING_MODEL: &str = "zai-glm-4.7";
pub const EXECUTION_MODEL: &str = "qwen-3-32b";
const CEREBRAS_BASE_URL: &str = "https://api.cerebras.ai/v1";

pub async fn create_cerebras_lm(model: &str) -> Result<LM> {
    let api_key = std::env::var("CEREBRAS_API_KEY")?;
    LM::builder()
        .base_url(CEREBRAS_BASE_URL.to_string())
        .api_key(api_key)
        .model(model.to_string())
        .temperature(0.7)
        .max_tokens(4000)
        .build()
        .await
}
```

## Environment Variables

| Variable | Provider | Description |
|----------|----------|-------------|
| (none - CLI auth) | Claude Headless | Uses existing `claude` CLI authentication |
| `PYLON_MNEMONIC` | Pylon Swarm | BIP-39 mnemonic for NIP-90 signing |
| `CEREBRAS_API_KEY` | Cerebras | Cerebras API key |
| (none needed) | Pylon Local | Auto-detects running Ollama |

## Usage

```rust
// Auto-detect best provider
let lm = create_planning_lm().await?;

// Check what's being used
if let Some(provider) = get_active_provider() {
    println!("Using {:?}", provider);
}

// Force specific provider
let lm = create_lm(&LmProvider::ClaudeHeadless).await?;
```

## Dependencies to Add

**dsrs Cargo.toml:**
```toml
which = "8.0"
dirs = "5.0"
```

## Success Criteria

1. `claude` CLI installed → Uses Claude headless
2. `PYLON_MNEMONIC` set (no Claude) → Uses Pylon swarm
3. `CEREBRAS_API_KEY` set (no above) → Uses Cerebras
4. Ollama running (no above) → Uses Pylon local
5. Nothing available → Clear error message
6. Existing code keeps working (legacy compat)
7. `cargo build -p dsrs -p adjutant` compiles
