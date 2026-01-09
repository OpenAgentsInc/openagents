//! Multi-provider LM configuration for dsrs.
//!
//! Supports multiple LM providers with smart priority/fallback:
//! 1. Claude Code headless (via claude-agent-sdk) - Best quality
//! 2. Pylon swarm - Distributed inference via NIP-90
//! 3. Cerebras - Fast, cheap tiered execution
//! 4. Pylon local (Ollama) - Fallback when nothing else available

use anyhow::Result;
use dsrs::{configure, has_claude_cli, ChatAdapter, LMClient, LM};
use std::sync::Arc;

/// Provider priority for LM selection.
#[derive(Clone, Debug, PartialEq)]
pub enum LmProvider {
    /// llama.cpp/GPT-OSS: Local OSS models via OpenAI-compatible API (top priority)
    LlamaCpp,
    /// Claude Code headless via claude-agent-sdk (Pro/Max subscription)
    ClaudeSdk,
    /// Pylon swarm: distributed inference via NIP-90
    PylonSwarm,
    /// Cerebras: OpenAI-compatible fast inference
    Cerebras,
    /// Pylon local: Ollama fallback
    PylonLocal,
}

impl std::fmt::Display for LmProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LmProvider::LlamaCpp => write!(f, "llama.cpp/GPT-OSS (local)"),
            LmProvider::ClaudeSdk => write!(f, "Claude SDK (headless)"),
            LmProvider::PylonSwarm => write!(f, "Pylon Swarm (NIP-90)"),
            LmProvider::Cerebras => write!(f, "Cerebras"),
            LmProvider::PylonLocal => write!(f, "Pylon Local (Ollama)"),
        }
    }
}

impl LmProvider {
    /// Short name for status bar display.
    pub fn short_name(&self) -> &'static str {
        match self {
            LmProvider::LlamaCpp => "gptoss",
            LmProvider::ClaudeSdk => "claude-sdk",
            LmProvider::PylonSwarm => "swarm",
            LmProvider::Cerebras => "cerebras",
            LmProvider::PylonLocal => "ollama",
        }
    }
}

/// Detect best available provider based on environment.
///
/// Priority order:
/// 1. llama.cpp/GPT-OSS running on localhost:8080 → LlamaCpp (top priority for Autopilot)
/// 2. Claude CLI available → ClaudeSdk
/// 3. PYLON_MNEMONIC set → PylonSwarm
/// 4. CEREBRAS_API_KEY set → Cerebras
/// 5. Ollama running on localhost:11434 → PylonLocal
pub fn detect_provider() -> Option<LmProvider> {
    // Priority 1: llama.cpp/GPT-OSS (local inference - top priority for Autopilot)
    if check_llamacpp_available() {
        return Some(LmProvider::LlamaCpp);
    }

    // Priority 2: Claude via SDK (uses subscription)
    if has_claude_cli() {
        return Some(LmProvider::ClaudeSdk);
    }

    // Priority 3: Pylon swarm (requires mnemonic)
    if std::env::var("PYLON_MNEMONIC").is_ok() {
        return Some(LmProvider::PylonSwarm);
    }

    // Priority 4: Cerebras
    if std::env::var("CEREBRAS_API_KEY").is_ok() {
        return Some(LmProvider::Cerebras);
    }

    // Priority 5: Check for local Ollama
    if check_ollama_available() {
        return Some(LmProvider::PylonLocal);
    }

    None
}

/// Detect all available providers (not just highest priority).
///
/// Returns a Vec of all providers that are currently available.
pub fn detect_all_providers() -> Vec<LmProvider> {
    let mut providers = Vec::new();

    if check_llamacpp_available() {
        providers.push(LmProvider::LlamaCpp);
    }
    if has_claude_cli() {
        providers.push(LmProvider::ClaudeSdk);
    }
    if std::env::var("PYLON_MNEMONIC").is_ok() {
        providers.push(LmProvider::PylonSwarm);
    }
    if std::env::var("CEREBRAS_API_KEY").is_ok() {
        providers.push(LmProvider::Cerebras);
    }
    if check_ollama_available() {
        providers.push(LmProvider::PylonLocal);
    }

    providers
}

/// Check if llama.cpp/GPT-OSS server is running locally.
pub fn check_llamacpp_available() -> bool {
    // Check custom endpoint first via environment variable
    if let Ok(endpoint) = std::env::var("LLAMACPP_URL") {
        // Try to extract host:port from URL
        if let Some(host_port) = endpoint
            .strip_prefix("http://")
            .or_else(|| endpoint.strip_prefix("https://"))
            .and_then(|s| s.split('/').next())
        {
            if let Ok(addr) = host_port.parse::<std::net::SocketAddr>() {
                return std::net::TcpStream::connect_timeout(
                    &addr,
                    std::time::Duration::from_millis(100),
                )
                .is_ok();
            }
            // Try with default port if no port specified
            let addr_with_port = if host_port.contains(':') {
                host_port.to_string()
            } else {
                format!("{}:8080", host_port)
            };
            if let Ok(addr) = addr_with_port.parse::<std::net::SocketAddr>() {
                return std::net::TcpStream::connect_timeout(
                    &addr,
                    std::time::Duration::from_millis(100),
                )
                .is_ok();
            }
        }
    }

    // Check default ports: 8080 (llama.cpp default) and 8000 (GPT-OSS default)
    for port in [8080, 8000] {
        if std::net::TcpStream::connect_timeout(
            &format!("127.0.0.1:{}", port).parse().unwrap(),
            std::time::Duration::from_millis(100),
        )
        .is_ok()
        {
            return true;
        }
    }
    false
}

/// Check if Ollama is running locally.
pub fn check_ollama_available() -> bool {
    std::net::TcpStream::connect_timeout(
        &"127.0.0.1:11434".parse().unwrap(),
        std::time::Duration::from_millis(100),
    )
    .is_ok()
}

/// Create LM for detected or specified provider.
pub async fn create_lm(provider: &LmProvider) -> Result<LM> {
    match provider {
        LmProvider::LlamaCpp => {
            let base_url = std::env::var("LLAMACPP_URL").unwrap_or_else(|_| {
                // Auto-detect which port is available
                for port in [8080, 8000] {
                    if std::net::TcpStream::connect_timeout(
                        &format!("127.0.0.1:{}", port).parse().unwrap(),
                        std::time::Duration::from_millis(100),
                    )
                    .is_ok()
                    {
                        return format!("http://127.0.0.1:{}/v1", port);
                    }
                }
                "http://127.0.0.1:8080/v1".to_string()
            });
            tracing::info!("LlamaCpp: using endpoint {}", base_url);

            LM::builder()
                .base_url(base_url)
                .api_key("not-needed".to_string()) // llama.cpp doesn't require API key
                .model("local".to_string())
                .temperature(0.7)
                .max_tokens(4000)
                .build()
                .await
        }
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
                .base_url(CEREBRAS_BASE_URL.to_string())
                .api_key(api_key)
                .model(PLANNING_MODEL.to_string())
                .temperature(0.7)
                .max_tokens(4000)
                .build()
                .await
        }
        LmProvider::PylonLocal => LM::builder()
            .model("pylon:local".to_string())
            .temperature(0.7)
            .max_tokens(4000)
            .build()
            .await,
    }
}

/// Create planning/execution LM with auto-detected provider.
pub async fn create_planning_lm() -> Result<LM> {
    let provider = detect_provider().ok_or_else(|| {
        anyhow::anyhow!(
            "No LM provider available. Options:\n\
             - Run llama.cpp server (./llama-server -m model.gguf --port 8080)\n\
             - Install Claude CLI (https://claude.ai/download)\n\
             - Set PYLON_MNEMONIC for swarm inference\n\
             - Set CEREBRAS_API_KEY for Cerebras\n\
             - Run Ollama locally (ollama serve)"
        )
    })?;
    tracing::info!("Using LM provider: {}", provider);
    create_lm(&provider).await
}

/// Create execution LM with auto-detected provider.
///
/// For Cerebras, uses cheaper model for execution tasks.
pub async fn create_execution_lm() -> Result<LM> {
    let provider = detect_provider().ok_or_else(|| anyhow::anyhow!("No LM provider available"))?;

    // For Cerebras, use cheaper model for execution
    if provider == LmProvider::Cerebras {
        let api_key = std::env::var("CEREBRAS_API_KEY")?;
        return LM::builder()
            .base_url(CEREBRAS_BASE_URL.to_string())
            .api_key(api_key)
            .model(EXECUTION_MODEL.to_string())
            .temperature(0.7)
            .max_tokens(4000)
            .build()
            .await;
    }

    create_lm(&provider).await
}

/// Get active provider for logging/diagnostics.
pub fn get_active_provider() -> Option<LmProvider> {
    detect_provider()
}

/// Arc-wrapped planning LM.
pub async fn get_planning_lm() -> Result<Arc<LM>> {
    Ok(Arc::new(create_planning_lm().await?))
}

/// Arc-wrapped execution LM.
pub async fn get_execution_lm() -> Result<Arc<LM>> {
    Ok(Arc::new(create_execution_lm().await?))
}

/// Configure global dsrs with auto-detected provider.
pub async fn configure_dsrs() -> Result<()> {
    let lm = create_planning_lm().await?;
    configure(lm, ChatAdapter);
    Ok(())
}

// ============ Legacy compatibility ============

/// Planning model for Cerebras (GLM 4.7 - smart model for strategic decisions)
pub const PLANNING_MODEL: &str = "zai-glm-4.7";

/// Execution model for Cerebras (Qwen-3-32B - cost-effective for tactical work)
pub const EXECUTION_MODEL: &str = "qwen-3-32b";

/// Cerebras API endpoint
const CEREBRAS_BASE_URL: &str = "https://api.cerebras.ai/v1";

/// Create an LM configured for Cerebras (legacy).
pub async fn create_cerebras_lm(model: &str) -> Result<LM> {
    let api_key = std::env::var("CEREBRAS_API_KEY")
        .map_err(|_| anyhow::anyhow!("CEREBRAS_API_KEY not set"))?;

    LM::builder()
        .base_url(CEREBRAS_BASE_URL.to_string())
        .api_key(api_key)
        .model(model.to_string())
        .temperature(0.7)
        .max_tokens(4000)
        .build()
        .await
}

/// Configure global dsrs settings for Cerebras (legacy).
pub async fn configure_cerebras_dsrs() -> Result<()> {
    let lm = create_cerebras_lm(PLANNING_MODEL).await?;
    configure(lm, ChatAdapter);
    Ok(())
}

/// Create an LM from environment (legacy).
///
/// Checks CEREBRAS_API_KEY first, then falls back to OPENAI_API_KEY.
/// Prefer using `create_planning_lm()` which supports all providers.
pub async fn create_lm_from_env(model: &str) -> Result<LM> {
    // Try Cerebras first
    if std::env::var("CEREBRAS_API_KEY").is_ok() {
        return create_cerebras_lm(model).await;
    }

    // Fall back to OpenAI-compatible (model should be in provider:model format)
    if std::env::var("OPENAI_API_KEY").is_ok() {
        return LM::builder()
            .model(format!("openai:{}", model))
            .build()
            .await;
    }

    Err(anyhow::anyhow!(
        "No API key found. Set CEREBRAS_API_KEY or OPENAI_API_KEY"
    ))
}
