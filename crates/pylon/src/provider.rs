//! Pylon provider - wraps compute primitives

use crate::config::PylonConfig;
use compute::backends::BackendRegistry;
use compute::domain::{DomainEvent, UnifiedIdentity};
use compute::services::{DvmService, RelayService};
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::{broadcast, RwLock};

/// Errors from the Pylon provider
#[derive(Debug, Error)]
pub enum ProviderError {
    #[error("not initialized")]
    NotInitialized,

    #[error("no backends available")]
    NoBackends,

    #[error("already running")]
    AlreadyRunning,

    #[error("not running")]
    NotRunning,

    #[error("identity error: {0}")]
    IdentityError(String),

    #[error("service error: {0}")]
    ServiceError(String),

    #[error("config error: {0}")]
    ConfigError(String),
}

/// Provider status
#[derive(Debug, Clone)]
pub struct ProviderStatus {
    /// Whether the provider is running
    pub running: bool,
    /// Connected relays
    pub relays: Vec<String>,
    /// Available backends
    pub backends: Vec<String>,
    /// Default backend
    pub default_backend: Option<String>,
    /// Total jobs processed
    pub jobs_processed: u64,
    /// Total earnings in millisats
    pub total_earnings_msats: u64,
}

impl Default for ProviderStatus {
    fn default() -> Self {
        Self {
            running: false,
            relays: Vec::new(),
            backends: Vec::new(),
            default_backend: None,
            jobs_processed: 0,
            total_earnings_msats: 0,
        }
    }
}

/// Diagnostic result from doctor command
#[derive(Debug, Clone)]
pub struct DiagnosticResult {
    /// Identity status
    pub identity_ok: bool,
    /// Identity npub if available
    pub identity_npub: Option<String>,
    /// Backend availability
    pub backends: Vec<(String, bool)>,
    /// Relay connectivity
    pub relays: Vec<(String, bool)>,
    /// Any warnings or issues
    pub warnings: Vec<String>,
}

/// Pylon provider - the main application struct
pub struct PylonProvider {
    /// Configuration
    config: PylonConfig,
    /// User identity
    identity: Option<Arc<UnifiedIdentity>>,
    /// Backend registry
    backend_registry: Arc<RwLock<BackendRegistry>>,
    /// Relay service
    relay_service: Arc<RelayService>,
    /// DVM service
    dvm_service: Option<DvmService>,
    /// Event broadcaster
    event_tx: broadcast::Sender<DomainEvent>,
    /// Whether the provider is running
    running: bool,
    /// Job counter
    jobs_processed: u64,
    /// Total earnings
    total_earnings_msats: u64,
}

impl PylonProvider {
    /// Create a new provider with the given config
    pub async fn new(config: PylonConfig) -> Result<Self, ProviderError> {
        // Auto-detect backends
        let registry = BackendRegistry::detect().await;

        if !registry.has_backends() {
            tracing::warn!("No inference backends detected");
        } else {
            let backends = registry.available_backends();
            tracing::info!("Detected backends: {}", backends.join(", "));
        }

        // Create relay service
        let relay_service = Arc::new(RelayService::new());

        // Create event channel
        let (event_tx, _) = broadcast::channel(100);

        Ok(Self {
            config,
            identity: None,
            backend_registry: Arc::new(RwLock::new(registry)),
            relay_service,
            dvm_service: None,
            event_tx,
            running: false,
            jobs_processed: 0,
            total_earnings_msats: 0,
        })
    }

    /// Set the provider identity
    pub fn set_identity(&mut self, identity: UnifiedIdentity) {
        self.identity = Some(Arc::new(identity));
    }

    /// Initialize the provider with an existing identity
    pub async fn init_with_identity(&mut self, identity: UnifiedIdentity) -> Result<(), ProviderError> {
        self.set_identity(identity);
        self.init_services().await
    }

    /// Initialize services
    async fn init_services(&mut self) -> Result<(), ProviderError> {
        let identity = self
            .identity
            .clone()
            .ok_or(ProviderError::NotInitialized)?;

        // Create DVM service
        let mut dvm_service = DvmService::new(
            self.relay_service.clone(),
            self.backend_registry.clone(),
            self.event_tx.clone(),
        );

        // Configure network for NIP-89 discovery
        dvm_service.set_network(&self.config.network);

        // Set identity on DVM service
        dvm_service.set_identity(identity).await;

        self.dvm_service = Some(dvm_service);

        Ok(())
    }

    /// Start the provider
    pub async fn start(&mut self) -> Result<(), ProviderError> {
        if self.running {
            return Err(ProviderError::AlreadyRunning);
        }

        // Check backends are available
        let registry = self.backend_registry.read().await;
        if !registry.has_backends() {
            return Err(ProviderError::NoBackends);
        }
        drop(registry);

        // Ensure services are initialized
        if self.dvm_service.is_none() {
            self.init_services().await?;
        }

        // Start DVM service
        if let Some(ref dvm) = self.dvm_service {
            dvm.start()
                .await
                .map_err(|e| ProviderError::ServiceError(e.to_string()))?;
        }

        self.running = true;
        tracing::info!("Pylon provider started");

        Ok(())
    }

    /// Stop the provider
    pub async fn stop(&mut self) -> Result<(), ProviderError> {
        if !self.running {
            return Err(ProviderError::NotRunning);
        }

        // Stop DVM service
        if let Some(ref dvm) = self.dvm_service {
            dvm.stop().await;
        }

        self.running = false;
        tracing::info!("Pylon provider stopped");

        Ok(())
    }

    /// Get provider status
    pub async fn status(&self) -> ProviderStatus {
        let registry = self.backend_registry.read().await;
        let backends: Vec<String> = registry
            .available_backends()
            .into_iter()
            .map(String::from)
            .collect();
        let default_backend = registry.default_id().map(String::from);
        drop(registry);

        let relays = if self.dvm_service.is_some() && self.running {
            self.relay_service.connected_relays().await
        } else {
            Vec::new()
        };

        ProviderStatus {
            running: self.running,
            relays,
            backends,
            default_backend,
            jobs_processed: self.jobs_processed,
            total_earnings_msats: self.total_earnings_msats,
        }
    }

    /// Run diagnostics
    pub async fn doctor(&self) -> DiagnosticResult {
        let mut warnings = Vec::new();

        // Check identity
        let (identity_ok, identity_npub) = if let Some(ref identity) = self.identity {
            let npub = identity.npub().ok();
            (true, npub)
        } else {
            warnings.push("No identity configured. Run 'pylon init' first.".to_string());
            (false, None)
        };

        // Check backends
        let mut backends = Vec::new();
        let registry = self.backend_registry.read().await;

        // Check each potential backend
        for backend_id in ["ollama", "apple_fm", "llamacpp"] {
            let available = registry.get(backend_id).is_some();
            backends.push((backend_id.to_string(), available));
        }
        drop(registry);

        if backends.iter().all(|(_, ok)| !*ok) {
            warnings.push("No inference backends available. Install Ollama or start llama.cpp server.".to_string());
        }

        // Check relays (just config for now, not actual connectivity)
        let relays: Vec<(String, bool)> = self
            .config
            .relays
            .iter()
            .map(|url| (url.clone(), true)) // TODO: actually test connectivity
            .collect();

        if relays.is_empty() {
            warnings.push("No relays configured.".to_string());
        }

        DiagnosticResult {
            identity_ok,
            identity_npub,
            backends,
            relays,
            warnings,
        }
    }

    /// Subscribe to domain events
    pub fn events(&self) -> broadcast::Receiver<DomainEvent> {
        self.event_tx.subscribe()
    }

    /// Get the config
    pub fn config(&self) -> &PylonConfig {
        &self.config
    }

    /// Check if running
    pub fn is_running(&self) -> bool {
        self.running
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_provider_creation() {
        let config = PylonConfig::default();
        let provider = PylonProvider::new(config).await;
        assert!(provider.is_ok());
    }

    #[tokio::test]
    async fn test_provider_status() {
        let config = PylonConfig::default();
        let provider = PylonProvider::new(config).await.unwrap();
        let status = provider.status().await;
        assert!(!status.running);
    }

    #[tokio::test]
    async fn test_provider_doctor() {
        let config = PylonConfig::default();
        let provider = PylonProvider::new(config).await.unwrap();
        let diag = provider.doctor().await;
        assert!(!diag.identity_ok);
        assert!(!diag.warnings.is_empty());
    }
}
