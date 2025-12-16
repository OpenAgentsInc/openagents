//! Unified backend router for MechaCoder.
//!
//! Automatically detects and routes to available AI backends:
//! - Claude Code (Claude Agent SDK)
//! - OpenAI (via API key)
//! - Ollama (local)
//! - Pi (built-in agent)
//! - OpenAgents Cloud (future)

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Available AI backends.
#[derive(Clone, Copy, Debug, Hash, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Backend {
    /// Claude Agent SDK (requires claude CLI installed)
    ClaudeCode,
    /// Anthropic API direct (requires ANTHROPIC_API_KEY env var)
    Anthropic,
    /// OpenRouter API (requires OPENROUTER_API_KEY env var)
    OpenRouter,
    /// OpenAI API (requires OPENAI_API_KEY env var)
    OpenAI,
    /// Ollama running locally (localhost:11434)
    Ollama,
    /// Pi agent (always available, built-in)
    Pi,
    /// OpenAgents Cloud (future, requires credits)
    OpenAgentsCloud,
}

impl Backend {
    /// Display name for the backend.
    pub fn display_name(&self) -> &'static str {
        match self {
            Backend::ClaudeCode => "Claude Code",
            Backend::Anthropic => "Anthropic",
            Backend::OpenRouter => "OpenRouter",
            Backend::OpenAI => "OpenAI",
            Backend::Ollama => "Ollama",
            Backend::Pi => "Pi Agent",
            Backend::OpenAgentsCloud => "OpenAgents Cloud",
        }
    }

    /// Whether this backend is a primary chat backend.
    pub fn is_chat_backend(&self) -> bool {
        matches!(
            self,
            Backend::ClaudeCode
                | Backend::Anthropic
                | Backend::OpenRouter
                | Backend::OpenAI
                | Backend::Ollama
                | Backend::Pi
                | Backend::OpenAgentsCloud
        )
    }

    /// Get the default model for this backend.
    pub fn default_model(&self) -> Option<&'static str> {
        match self {
            Backend::ClaudeCode => None, // Uses Claude CLI's default
            Backend::Anthropic => Some("claude-sonnet-4-20250514"),
            Backend::OpenRouter => Some("anthropic/claude-3.5-sonnet"),
            Backend::OpenAI => Some("gpt-4o"),
            Backend::Ollama => Some("llama3.2"),
            Backend::Pi => None,
            Backend::OpenAgentsCloud => None,
        }
    }

    /// Get the provider ID for llm crate (if applicable).
    pub fn provider_id(&self) -> Option<&'static str> {
        match self {
            Backend::Anthropic => Some("anthropic"),
            Backend::OpenRouter => Some("openrouter"),
            Backend::OpenAI => Some("openai"),
            Backend::Ollama => Some("ollama"),
            _ => None,
        }
    }
}

/// Router configuration (user preferences).
#[derive(Clone, Debug, Default)]
pub struct RouterConfig {
    /// Backends explicitly disabled by user.
    pub disabled_backends: HashSet<Backend>,
    /// Preferred backend (if available, use this first).
    pub preferred: Option<Backend>,
}

impl RouterConfig {
    /// Create default config with no disabled backends.
    pub fn new() -> Self {
        Self::default()
    }

    /// Disable a backend.
    pub fn disable(&mut self, backend: Backend) {
        self.disabled_backends.insert(backend);
    }

    /// Enable a backend.
    pub fn enable(&mut self, backend: Backend) {
        self.disabled_backends.remove(&backend);
    }

    /// Check if a backend is enabled.
    pub fn is_enabled(&self, backend: Backend) -> bool {
        !self.disabled_backends.contains(&backend)
    }

    /// Set preferred backend.
    pub fn set_preferred(&mut self, backend: Option<Backend>) {
        self.preferred = backend;
    }
}

/// Router status for UI display.
#[derive(Clone, Debug)]
pub struct RouterStatus {
    /// Currently active backend (if any).
    pub active: Option<Backend>,
    /// All detected backends.
    pub detected: Vec<Backend>,
    /// Whether any chat backend is available.
    pub has_chat_backend: bool,
}

/// Unified backend router.
#[derive(Clone, Debug)]
pub struct Router {
    /// User configuration.
    config: RouterConfig,
    /// Detected available backends.
    detected: Vec<Backend>,
}

impl Router {
    /// Create a new router with detection.
    pub fn new(config: RouterConfig) -> Self {
        Self {
            config,
            detected: Vec::new(),
        }
    }

    /// Detect available backends (call on startup).
    #[cfg(feature = "server")]
    pub async fn detect(&mut self) {
        self.detected = Self::detect_backends().await;
    }

    /// Synchronous detection (for immediate use).
    pub fn detect_sync(&mut self) {
        self.detected = Self::detect_backends_sync();
    }

    /// Get detected backends.
    pub fn detected_backends(&self) -> &[Backend] {
        &self.detected
    }

    /// Check if any backend is available for chat.
    pub fn has_chat_backend(&self) -> bool {
        self.detected
            .iter()
            .any(|b| b.is_chat_backend() && self.config.is_enabled(*b))
    }

    /// Get the best available backend for a message.
    pub fn route(&self) -> Option<Backend> {
        // Priority order:
        // 1. User's preferred backend (if available and enabled)
        // 2. Claude Code (most capable, uses claude CLI)
        // 3. Anthropic API direct (requires API key)
        // 4. OpenRouter (requires API key, access to many models)
        // 5. OpenAI (requires API key)
        // 6. Ollama (local, no API key needed)
        // 7. Pi (always available)

        // Check preferred first
        if let Some(preferred) = self.config.preferred {
            if self.detected.contains(&preferred) && self.config.is_enabled(preferred) {
                return Some(preferred);
            }
        }

        // Fall through priority order
        let priority = [
            Backend::ClaudeCode,
            Backend::Anthropic,
            Backend::OpenRouter,
            Backend::OpenAI,
            Backend::Ollama,
            Backend::Pi,
            Backend::OpenAgentsCloud,
        ];

        for backend in priority {
            if self.detected.contains(&backend) && self.config.is_enabled(backend) {
                return Some(backend);
            }
        }

        None
    }

    /// Get router status for UI.
    pub fn status(&self) -> RouterStatus {
        RouterStatus {
            active: self.route(),
            detected: self.detected.clone(),
            has_chat_backend: self.has_chat_backend(),
        }
    }

    /// Get the config for modification.
    pub fn config(&self) -> &RouterConfig {
        &self.config
    }

    /// Get mutable config.
    pub fn config_mut(&mut self) -> &mut RouterConfig {
        &mut self.config
    }

    /// Async backend detection.
    #[cfg(feature = "server")]
    async fn detect_backends() -> Vec<Backend> {
        let mut backends = Vec::new();

        // Check Claude Code (check if claude CLI exists)
        if Self::check_claude_code().await {
            backends.push(Backend::ClaudeCode);
        }

        // Check Anthropic API key
        if std::env::var("ANTHROPIC_API_KEY").is_ok() {
            backends.push(Backend::Anthropic);
        }

        // Check OpenRouter API key (also check .env.local)
        if Self::check_openrouter_key() {
            backends.push(Backend::OpenRouter);
        }

        // Check OpenAI API key
        if std::env::var("OPENAI_API_KEY").is_ok() {
            backends.push(Backend::OpenAI);
        }

        // Check Ollama (try to connect to localhost:11434)
        if Self::check_ollama().await {
            backends.push(Backend::Ollama);
        }

        // Pi is always available (built-in)
        backends.push(Backend::Pi);

        backends
    }

    /// Synchronous backend detection.
    fn detect_backends_sync() -> Vec<Backend> {
        let mut backends = Vec::new();

        // Check Claude Code (check if claude CLI exists)
        if Self::check_claude_code_sync() {
            backends.push(Backend::ClaudeCode);
        }

        // Check Anthropic API key
        if std::env::var("ANTHROPIC_API_KEY").is_ok() {
            backends.push(Backend::Anthropic);
        }

        // Check OpenRouter API key (also check .env.local)
        if Self::check_openrouter_key() {
            backends.push(Backend::OpenRouter);
        }

        // Check OpenAI API key
        if std::env::var("OPENAI_API_KEY").is_ok() {
            backends.push(Backend::OpenAI);
        }

        // Check Ollama (synchronous - just check if port is open)
        if Self::check_ollama_sync() {
            backends.push(Backend::Ollama);
        }

        // Pi is always available (built-in)
        backends.push(Backend::Pi);

        backends
    }

    /// Check for OpenRouter API key.
    ///
    /// Checks environment variable first, then .env.local file.
    fn check_openrouter_key() -> bool {
        // Check environment variable first
        if std::env::var("OPENROUTER_API_KEY").is_ok() {
            return true;
        }

        // Check .env.local file in current directory
        if let Ok(content) = std::fs::read_to_string(".env.local") {
            for line in content.lines() {
                let line = line.trim();
                if line.starts_with("OPENROUTER_API_KEY=") {
                    let value = line.trim_start_matches("OPENROUTER_API_KEY=").trim();
                    // Remove quotes if present
                    let value = value.trim_matches('"').trim_matches('\'');
                    if !value.is_empty() {
                        // Set the env var for later use
                        // SAFETY: This is called during initialization before multi-threading,
                        // and we're only setting a single env var that will be read later.
                        unsafe {
                            std::env::set_var("OPENROUTER_API_KEY", value);
                        }
                        return true;
                    }
                }
            }
        }

        false
    }

    /// Check if Claude Code CLI is available.
    #[cfg(feature = "server")]
    async fn check_claude_code() -> bool {
        // Check known installation paths first
        let home = std::env::var("HOME").unwrap_or_default();
        let known_paths = [
            format!("{}/.claude/local/claude", home),
            format!("{}/.npm-global/bin/claude", home),
            format!("{}/.local/bin/claude", home),
            "/usr/local/bin/claude".to_string(),
            "/opt/homebrew/bin/claude".to_string(),
        ];

        for path in &known_paths {
            if std::path::Path::new(path).exists() {
                return true;
            }
        }

        // Fall back to PATH check via login shell
        tokio::process::Command::new("zsh")
            .args(["-lc", "which claude"])
            .output()
            .await
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    /// Synchronous Claude Code check.
    fn check_claude_code_sync() -> bool {
        // Check known installation paths first
        let home = std::env::var("HOME").unwrap_or_default();
        let known_paths = [
            format!("{}/.claude/local/claude", home),
            format!("{}/.npm-global/bin/claude", home),
            format!("{}/.local/bin/claude", home),
            "/usr/local/bin/claude".to_string(),
            "/opt/homebrew/bin/claude".to_string(),
        ];

        for path in &known_paths {
            if std::path::Path::new(path).exists() {
                return true;
            }
        }

        // Fall back to PATH check via login shell
        std::process::Command::new("zsh")
            .args(["-lc", "which claude"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    /// Check if Ollama is running.
    #[cfg(feature = "server")]
    async fn check_ollama() -> bool {
        tokio::net::TcpStream::connect("127.0.0.1:11434")
            .await
            .is_ok()
    }

    /// Synchronous Ollama check.
    fn check_ollama_sync() -> bool {
        std::net::TcpStream::connect("127.0.0.1:11434").is_ok()
    }
}

impl Default for Router {
    fn default() -> Self {
        let mut router = Self::new(RouterConfig::default());
        router.detect_sync();
        router
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_router_config() {
        let mut config = RouterConfig::new();
        assert!(config.is_enabled(Backend::ClaudeCode));

        config.disable(Backend::ClaudeCode);
        assert!(!config.is_enabled(Backend::ClaudeCode));

        config.enable(Backend::ClaudeCode);
        assert!(config.is_enabled(Backend::ClaudeCode));
    }

    #[test]
    fn test_backend_display_name() {
        assert_eq!(Backend::ClaudeCode.display_name(), "Claude Code");
        assert_eq!(Backend::Anthropic.display_name(), "Anthropic");
        assert_eq!(Backend::OpenRouter.display_name(), "OpenRouter");
        assert_eq!(Backend::Pi.display_name(), "Pi Agent");
    }

    #[test]
    fn test_backend_provider_id() {
        assert_eq!(Backend::Anthropic.provider_id(), Some("anthropic"));
        assert_eq!(Backend::OpenRouter.provider_id(), Some("openrouter"));
        assert_eq!(Backend::OpenAI.provider_id(), Some("openai"));
        assert_eq!(Backend::ClaudeCode.provider_id(), None);
    }

    #[test]
    fn test_backend_default_model() {
        assert_eq!(
            Backend::Anthropic.default_model(),
            Some("claude-sonnet-4-20250514")
        );
        assert_eq!(
            Backend::OpenRouter.default_model(),
            Some("anthropic/claude-3.5-sonnet")
        );
        assert_eq!(Backend::OpenAI.default_model(), Some("gpt-4o"));
        assert_eq!(Backend::Ollama.default_model(), Some("llama3.2"));
    }

    #[test]
    fn test_router_priority() {
        let mut router = Router::new(RouterConfig::default());
        // Manually set detected backends for testing
        router.detected = vec![Backend::Pi, Backend::Ollama];

        // Ollama should be preferred over Pi
        assert_eq!(router.route(), Some(Backend::Ollama));

        // With preferred set, that takes priority
        router.config_mut().set_preferred(Some(Backend::Pi));
        assert_eq!(router.route(), Some(Backend::Pi));
    }

    #[test]
    fn test_router_anthropic_priority() {
        let mut router = Router::new(RouterConfig::default());
        // Anthropic should be preferred over Ollama (when no ClaudeCode)
        router.detected = vec![Backend::Anthropic, Backend::Ollama, Backend::Pi];

        assert_eq!(router.route(), Some(Backend::Anthropic));
    }

    #[test]
    fn test_router_openrouter_priority() {
        let mut router = Router::new(RouterConfig::default());
        // OpenRouter should be preferred over OpenAI and Ollama (when no Anthropic)
        router.detected = vec![
            Backend::OpenRouter,
            Backend::OpenAI,
            Backend::Ollama,
            Backend::Pi,
        ];

        assert_eq!(router.route(), Some(Backend::OpenRouter));
    }
}
