//! Multi-provider LM configuration for dsrs.
//!
//! Supports multiple LM providers with smart priority/fallback:
//! 1. llama.cpp/GPT-OSS - Local inference fallback
//! 2. Pylon swarm - Distributed inference via NIP-90
//! 3. Cerebras - Fast, cheap tiered execution
//! 4. Pylon local (Ollama) - Fallback when nothing else available

use anyhow::Result;
use dsrs::{ChatAdapter, LM, configure, check_codex_available};
use std::sync::Arc;

/// Provider priority for LM selection.
#[derive(Clone, Debug, PartialEq)]
pub enum LmProvider {
    /// OpenAI Responses API (preferred for DSPy signatures)
    OpenAiResponses,
    /// Codex: Uses Codex app-server (highest priority when available)
    Codex,
    /// llama.cpp/GPT-OSS: Local OSS models via OpenAI-compatible API
    LlamaCpp,
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
            LmProvider::Codex => write!(f, "Codex"),
            LmProvider::OpenAiResponses => write!(f, "OpenAI Responses"),
            LmProvider::LlamaCpp => write!(f, "llama.cpp/GPT-OSS (local)"),
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
            LmProvider::Codex => "codex",
            LmProvider::OpenAiResponses => "openai",
            LmProvider::LlamaCpp => "gptoss",
            LmProvider::PylonSwarm => "swarm",
            LmProvider::Cerebras => "cerebras",
            LmProvider::PylonLocal => "ollama",
        }
    }
}

/// Detect best available provider based on environment.
///
/// Priority order:
/// 0. Codex app-server available → Codex
/// 1. llama.cpp/GPT-OSS running on localhost:8080 → LlamaCpp
/// 2. PYLON_MNEMONIC set → PylonSwarm
/// 3. CEREBRAS_API_KEY set → Cerebras
/// 4. Ollama running on localhost:11434 → PylonLocal
pub fn detect_provider() -> Option<LmProvider> {
    // Priority 0: OpenAI Responses (preferred for DSPy signatures)
    if std::env::var("OPENAI_API_KEY").is_ok() {
        return Some(LmProvider::OpenAiResponses);
    }

    // Priority 1: Codex (when available)
    if check_codex_available() {
        return Some(LmProvider::Codex);
    }

    // Priority 2: llama.cpp/GPT-OSS (local inference fallback)
    if check_llamacpp_available() {
        return Some(LmProvider::LlamaCpp);
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

    if std::env::var("OPENAI_API_KEY").is_ok() {
        providers.push(LmProvider::OpenAiResponses);
    }
    if check_codex_available() {
        providers.push(LmProvider::Codex);
    }
    if check_llamacpp_available() {
        providers.push(LmProvider::LlamaCpp);
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
        LmProvider::OpenAiResponses => {
            let api_key = std::env::var("OPENAI_API_KEY")?;
            let model = std::env::var("OPENAI_RESPONSES_MODEL")
                .or_else(|_| std::env::var("OPENAI_MODEL"))
                .unwrap_or_else(|_| "gpt-5-nano".to_string());
            let max_tokens = std::env::var("OPENAI_MAX_TOKENS")
                .ok()
                .and_then(|value| value.parse::<u32>().ok())
                .unwrap_or(4000);
            let temperature = std::env::var("OPENAI_TEMPERATURE")
                .ok()
                .and_then(|value| value.parse::<f32>().ok())
                .unwrap_or(0.7);

            let mut builder = LM::builder()
                .model(format!("openai-responses:{}", model))
                .api_key(api_key)
                .max_tokens(max_tokens)
                .temperature(temperature);

            if let Ok(base_url) = std::env::var("OPENAI_BASE_URL") {
                builder = builder.base_url(base_url);
            }

            builder.build().await
        }
        LmProvider::Codex => {
            tracing::info!("Codex: using codex app-server for LM");
            LM::builder()
                .model("codex:auto".to_string())
                .temperature(0.7)
                .max_tokens(4000)
                .build()
                .await
        }
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

/// Create planning/execution LM with auto-detected provider.
pub async fn create_planning_lm() -> Result<LM> {
    let provider = detect_provider().ok_or_else(|| {
        anyhow::anyhow!(
            "No LM provider available. Options:\n\
             - Set OPENAI_API_KEY for OpenAI Responses\n\
             - Install Codex (npm install -g @anthropic-ai/claude-code)\n\
             - Run llama.cpp server (./llama-server -m model.gguf --port 8080)\n\
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
    configure_dsrs_with_preference(true).await
}

/// Configure global dsrs with optional provider preferences.
///
/// `use_codex` controls whether Codex is considered as a provider.
pub async fn configure_dsrs_with_preference(use_codex: bool) -> Result<()> {
    let provider = if use_codex {
        detect_provider()
    } else {
        detect_provider_skip_codex()
    };

    let provider = provider.ok_or_else(|| {
        anyhow::anyhow!(
            "No LM provider available. Options:\n\
             - Set OPENAI_API_KEY for OpenAI Responses\n\
             - Install Codex (npm install -g @anthropic-ai/claude-code)\n\
             - Run llama.cpp server (./llama-server -m model.gguf --port 8080)\n\
             - Set PYLON_MNEMONIC for swarm inference\n\
             - Set CEREBRAS_API_KEY for Cerebras\n\
             - Run Ollama locally (ollama serve)"
        )
    })?;

    tracing::info!("Autopilot: configuring dsrs with provider: {}", provider);
    let lm = create_lm(&provider).await?;
    configure(lm, ChatAdapter);
    Ok(())
}

/// Detect provider without Codex - skips Codex even if available.
pub fn detect_provider_skip_codex() -> Option<LmProvider> {
    // Skip Codex, check other providers in priority order
    if std::env::var("OPENAI_API_KEY").is_ok() {
        return Some(LmProvider::OpenAiResponses);
    }
    if check_llamacpp_available() {
        return Some(LmProvider::LlamaCpp);
    }
    if std::env::var("PYLON_MNEMONIC").is_ok() {
        return Some(LmProvider::PylonSwarm);
    }
    if std::env::var("CEREBRAS_API_KEY").is_ok() {
        return Some(LmProvider::Cerebras);
    }
    if check_ollama_available() {
        return Some(LmProvider::PylonLocal);
    }
    None
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
