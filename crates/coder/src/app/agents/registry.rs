//! Agent registry
//!
//! Manages available agent backends and provides discovery/selection.

use std::collections::HashMap;
use std::sync::Arc;

use super::backend::{AgentAvailability, AgentKind, BoxedAgentBackend, ModelInfo};
use super::claude_backend::ClaudeBackend;
use super::codex_backend::CodexBackend;

/// Agent registry that manages available backends
pub struct AgentRegistry {
    /// Registered backends
    backends: HashMap<AgentKind, BoxedAgentBackend>,
    /// Cached availability status
    availability: HashMap<AgentKind, AgentAvailability>,
}

impl AgentRegistry {
    /// Create a new registry with default backends
    pub fn new() -> Self {
        let mut registry = Self {
            backends: HashMap::new(),
            availability: HashMap::new(),
        };

        // Register default backends
        registry.register(Arc::new(ClaudeBackend::new()));
        registry.register(Arc::new(CodexBackend::new()));

        // Check availability at startup
        registry.refresh_availability();

        registry
    }

    /// Register a backend
    pub fn register(&mut self, backend: BoxedAgentBackend) {
        let kind = backend.kind();
        self.backends.insert(kind, backend);
    }

    /// Refresh availability status for all backends
    pub fn refresh_availability(&mut self) {
        for (kind, backend) in &self.backends {
            let availability = backend.check_availability();
            self.availability.insert(*kind, availability);
        }
    }

    /// Get backend by kind
    pub fn get(&self, kind: AgentKind) -> Option<&BoxedAgentBackend> {
        self.backends.get(&kind)
    }

    /// Get availability for a kind
    pub fn get_availability(&self, kind: AgentKind) -> Option<&AgentAvailability> {
        self.availability.get(&kind)
    }

    /// Check if an agent kind is available
    pub fn is_available(&self, kind: AgentKind) -> bool {
        self.availability
            .get(&kind)
            .map(|a| a.available)
            .unwrap_or(false)
    }

    /// Get all available agent kinds
    pub fn available_kinds(&self) -> Vec<AgentKind> {
        self.availability
            .iter()
            .filter(|(_, a)| a.available)
            .map(|(k, _)| *k)
            .collect()
    }

    /// Get all registered agent kinds
    pub fn all_kinds(&self) -> Vec<AgentKind> {
        self.backends.keys().copied().collect()
    }

    /// Get the default agent kind (first available, preferring Claude)
    pub fn default_kind(&self) -> Option<AgentKind> {
        // Prefer Claude, fall back to Codex
        if self.is_available(AgentKind::Claude) {
            Some(AgentKind::Claude)
        } else if self.is_available(AgentKind::Codex) {
            Some(AgentKind::Codex)
        } else {
            None
        }
    }

    /// Get models for a specific agent kind
    pub async fn models_for(&self, kind: AgentKind) -> Vec<ModelInfo> {
        if let Some(backend) = self.backends.get(&kind) {
            backend.available_models().await
        } else {
            Vec::new()
        }
    }

    /// Get status summary for display
    pub fn status_summary(&self) -> Vec<AgentStatus> {
        self.backends
            .iter()
            .map(|(kind, backend)| {
                let availability = self.availability.get(kind).cloned().unwrap_or(AgentAvailability {
                    available: false,
                    executable_path: None,
                    version: None,
                    error: Some("Not checked".to_string()),
                });
                AgentStatus {
                    kind: *kind,
                    name: backend.display_name().to_string(),
                    icon: backend.icon().to_string(),
                    available: availability.available,
                    error: availability.error,
                }
            })
            .collect()
    }
}

impl Default for AgentRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Status of an agent for display
#[derive(Debug, Clone)]
pub struct AgentStatus {
    pub kind: AgentKind,
    pub name: String,
    pub icon: String,
    pub available: bool,
    pub error: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_creation() {
        let registry = AgentRegistry::new();
        assert!(registry.get(AgentKind::Claude).is_some());
        assert!(registry.get(AgentKind::Codex).is_some());
    }

    #[test]
    fn test_all_kinds() {
        let registry = AgentRegistry::new();
        let kinds = registry.all_kinds();
        assert!(kinds.contains(&AgentKind::Claude));
        assert!(kinds.contains(&AgentKind::Codex));
    }
}
