//! Unified backend router for MechaCoder.
//!
//! Automatically detects and routes to available AI backends:
//! - Claude Code (Claude Agent SDK)
//! - OpenAI (via API key)
//! - Ollama (local)
//! - Pi (built-in agent)
//! - OpenAgents Cloud (future)

use std::collections::HashSet;

/// Available AI backends.
#[derive(Clone, Copy, Debug, Hash, Eq, PartialEq)]
pub enum Backend {
    /// Claude Agent SDK (requires claude CLI installed)
    ClaudeCode,
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
            Backend::ClaudeCode | Backend::Ollama | Backend::Pi | Backend::OpenAgentsCloud
        )
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
        // 2. Claude Code (most capable)
        // 3. Ollama (local, no API key needed)
        // 4. Pi (always available)

        // Check preferred first
        if let Some(preferred) = self.config.preferred {
            if self.detected.contains(&preferred) && self.config.is_enabled(preferred) {
                return Some(preferred);
            }
        }

        // Fall through priority order
        let priority = [
            Backend::ClaudeCode,
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
    async fn detect_backends() -> Vec<Backend> {
        let mut backends = Vec::new();

        // Check Claude Code (check if claude CLI exists)
        if Self::check_claude_code().await {
            backends.push(Backend::ClaudeCode);
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

    /// Check if Claude Code CLI is available.
    async fn check_claude_code() -> bool {
        // Check if 'claude' command exists
        tokio::process::Command::new("which")
            .arg("claude")
            .output()
            .await
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    /// Synchronous Claude Code check.
    fn check_claude_code_sync() -> bool {
        std::process::Command::new("which")
            .arg("claude")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    /// Check if Ollama is running.
    async fn check_ollama() -> bool {
        // Try to connect to Ollama's default port
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
        assert_eq!(Backend::Pi.display_name(), "Pi Agent");
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
}
