//! Pylon provider - wraps compute primitives

use crate::bridge_manager::BridgeManager;
use crate::config::PylonConfig;
use compute::backends::{AgentRegistry, BackendRegistry};
use compute::domain::DomainEvent;
use compute::services::{DvmConfig, DvmService, RelayService};
use openagents_runtime::UnifiedIdentity;
use spark::{Network as SparkNetwork, SparkWallet, WalletConfig};
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::{RwLock, broadcast};

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
    /// Available inference backends
    pub backends: Vec<String>,
    /// Default inference backend
    pub default_backend: Option<String>,
    /// Available agent backends (for Bazaar jobs)
    pub agent_backends: Vec<String>,
    /// Supported Bazaar job kinds
    pub supported_bazaar_kinds: Vec<u16>,
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
            agent_backends: Vec::new(),
            supported_bazaar_kinds: Vec::new(),
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
    /// Backend registry (inference)
    backend_registry: Arc<RwLock<BackendRegistry>>,
    /// Agent registry (Bazaar jobs)
    agent_registry: Arc<RwLock<AgentRegistry>>,
    /// Relay service
    relay_service: Arc<RelayService>,
    /// DVM service
    dvm_service: Option<DvmService>,
    /// Spark wallet for payments
    wallet: Option<Arc<SparkWallet>>,
    /// FM Bridge manager (for Apple Foundation Models)
    #[allow(dead_code)]
    bridge_manager: Option<BridgeManager>,
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
        // Try to start FM Bridge for Apple Foundation Models (macOS only)
        let bridge_manager = Self::try_start_fm_bridge().await;

        // Auto-detect inference backends (will now find Apple FM if bridge started)
        let mut registry = BackendRegistry::detect().await;

        if !config.backend_preference.is_empty() {
            if config.backend_preference.len() == 1 {
                let preferred = config.backend_preference[0].as_str();
                if let Some(backend) = registry.get(preferred) {
                    let mut filtered = BackendRegistry::new();
                    filtered.register_with_id(preferred, backend);
                    registry = filtered;
                } else {
                    registry = BackendRegistry::new();
                }
            } else {
                for backend in &config.backend_preference {
                    if registry.set_default(backend) {
                        break;
                    }
                }
            }
        }

        if !registry.has_backends() {
            tracing::warn!("No inference backends detected");
        } else {
            let backends = registry.available_backends();
            tracing::info!("Detected inference backends: {}", backends.join(", "));
        }

        // Agent registry for Bazaar jobs
        let agent_registry = AgentRegistry::new();

        // Create relay service
        let relay_service = if config.relays.is_empty() {
            Arc::new(RelayService::new())
        } else {
            Arc::new(RelayService::with_relays(config.relays.clone()))
        };

        // Create event channel
        let (event_tx, _) = broadcast::channel(100);

        Ok(Self {
            config,
            identity: None,
            backend_registry: Arc::new(RwLock::new(registry)),
            agent_registry: Arc::new(RwLock::new(agent_registry)),
            relay_service,
            dvm_service: None,
            wallet: None,
            bridge_manager,
            event_tx,
            running: false,
            jobs_processed: 0,
            total_earnings_msats: 0,
        })
    }

    /// Try to start the FM Bridge for Apple Foundation Models
    async fn try_start_fm_bridge() -> Option<BridgeManager> {
        // Check if FM Bridge binary is available
        if !BridgeManager::is_available() {
            tracing::debug!("FM Bridge binary not found, Apple FM will not be available");
            return None;
        }

        tracing::info!("FM Bridge binary found, attempting to start...");

        // Run blocking bridge startup in a blocking task
        let result = tokio::task::spawn_blocking(|| {
            let mut bridge = BridgeManager::new();

            // Try to start the bridge
            if let Err(e) = bridge.start() {
                tracing::warn!("Failed to start FM Bridge: {}", e);
                return None;
            }

            // Wait for it to become ready (blocking)
            if let Err(e) = bridge.wait_ready() {
                tracing::warn!("FM Bridge failed to become ready: {}", e);
                return None;
            }

            let url = bridge.url();
            Some((bridge, url))
        })
        .await
        .ok()
        .flatten();

        if let Some((bridge, url)) = result {
            // Set environment variable so BackendRegistry::detect() finds it
            // SAFETY: We're setting an env var before spawning other threads, and this is
            // the only place we modify FM_BRIDGE_URL
            unsafe {
                std::env::set_var("FM_BRIDGE_URL", &url);
            }
            tracing::info!("FM Bridge started at {}", url);
            Some(bridge)
        } else {
            None
        }
    }

    /// Set the provider identity
    pub fn set_identity(&mut self, identity: UnifiedIdentity) {
        self.identity = Some(Arc::new(identity));
    }

    /// Initialize the provider with an existing identity
    pub async fn init_with_identity(
        &mut self,
        identity: UnifiedIdentity,
    ) -> Result<(), ProviderError> {
        self.set_identity(identity);
        self.init_services().await
    }

    /// Initialize services
    async fn init_services(&mut self) -> Result<(), ProviderError> {
        let identity = self.identity.clone().ok_or(ProviderError::NotInitialized)?;

        // Create DVM service with both inference and agent registries
        let mut dvm_service = DvmService::with_agent_registry(
            self.relay_service.clone(),
            self.backend_registry.clone(),
            self.agent_registry.clone(),
            self.event_tx.clone(),
        );

        // Configure DVM service with payment settings from Pylon config
        let dvm_config = DvmConfig {
            min_price_msats: self.config.min_price_msats,
            require_payment: self.config.require_payment,
            network: self.config.network.clone(),
            default_model: self.config.default_model.clone(),
        };
        dvm_service.set_config(dvm_config);

        // Set identity on DVM service
        dvm_service.set_identity(identity.clone()).await;

        // Initialize wallet if payments are enabled
        if self.config.enable_payments {
            match self.init_wallet(&identity).await {
                Ok(wallet) => {
                    let wallet = Arc::new(wallet);
                    self.wallet = Some(wallet.clone());
                    dvm_service.set_wallet(wallet).await;
                    tracing::info!("Spark wallet initialized for payments");
                }
                Err(e) => {
                    tracing::warn!(
                        "Failed to initialize wallet: {}. Continuing in free mode.",
                        e
                    );
                    // Continue without wallet - free mode
                }
            }
        } else {
            tracing::info!("Payments disabled, running in free mode");
        }

        self.dvm_service = Some(dvm_service);

        Ok(())
    }

    /// Initialize the Spark wallet from the identity
    async fn init_wallet(&self, identity: &UnifiedIdentity) -> Result<SparkWallet, ProviderError> {
        // Convert network string to SparkNetwork
        let network = match self.config.network.as_str() {
            "mainnet" => SparkNetwork::Mainnet,
            "testnet" => SparkNetwork::Testnet,
            "signet" => SparkNetwork::Signet,
            _ => SparkNetwork::Regtest, // Default to regtest
        };

        // Get data directory for wallet storage
        let storage_dir = self
            .config
            .data_path()
            .map_err(|e| ProviderError::ConfigError(e.to_string()))?
            .join("wallet");

        // Create wallet config
        let wallet_config = WalletConfig {
            network,
            api_key: None, // API key only needed for mainnet
            storage_dir,
        };

        // Clone the signer from identity
        let signer = identity.spark_signer().clone();

        // Create the wallet
        SparkWallet::new(signer, wallet_config).await.map_err(|e| {
            ProviderError::ServiceError(format!("Wallet initialization failed: {}", e))
        })
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

        // Note: dvm.start() handles relay connection with proper auth key setup
        // Do NOT call relay_service.connect() here - it would connect without auth

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
        // Get inference backends
        let registry = self.backend_registry.read().await;
        let backends: Vec<String> = registry
            .available_backends()
            .into_iter()
            .map(String::from)
            .collect();
        let default_backend = registry.default_id().map(String::from);
        drop(registry);

        // Get agent backends and capabilities
        let agent_registry = self.agent_registry.read().await;
        let agent_backends: Vec<String> = agent_registry
            .available_backends()
            .into_iter()
            .map(String::from)
            .collect();
        let caps = agent_registry.aggregated_capabilities().await;
        let supported_bazaar_kinds = caps.supported_kinds();
        drop(agent_registry);

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
            agent_backends,
            supported_bazaar_kinds,
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
            warnings.push(
                "No inference backends available. Install Ollama or start llama.cpp server."
                    .to_string(),
            );
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
