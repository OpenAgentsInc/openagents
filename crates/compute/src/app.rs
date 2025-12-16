//! Main application struct for the compute provider

use crate::domain::DomainEvent;
use crate::services::{DvmService, OllamaService, RelayService};
use crate::state::AppState;
use crate::storage::SecureStore;
use crate::ui::RootView;
use std::sync::Arc;
use tokio::sync::broadcast;
use wgpui::{Bounds, Scene, TextSystem};

/// Main compute provider application
pub struct ComputeApp {
    /// Application state
    state: Arc<AppState>,
    /// Root UI view
    root_view: RootView,
    /// Secure storage for identity
    storage: SecureStore,
    /// Ollama service
    ollama_service: Arc<OllamaService>,
    /// Relay service
    relay_service: Arc<RelayService>,
    /// DVM service
    dvm_service: Arc<DvmService>,
    /// Event receiver for domain events
    event_rx: broadcast::Receiver<DomainEvent>,
    /// Event sender (for creating new subscriptions)
    event_tx: broadcast::Sender<DomainEvent>,
}

impl ComputeApp {
    /// Create a new compute application
    pub fn new() -> Self {
        let state = Arc::new(AppState::new());
        let (event_tx, event_rx) = broadcast::channel(100);

        let ollama_service = Arc::new(OllamaService::new());
        let relay_service = Arc::new(RelayService::new());
        let dvm_service = Arc::new(DvmService::new(
            relay_service.clone(),
            ollama_service.clone(),
            event_tx.clone(),
        ));

        Self {
            root_view: RootView::new(state.clone()),
            state,
            storage: SecureStore::default(),
            ollama_service,
            relay_service,
            dvm_service,
            event_rx,
            event_tx,
        }
    }

    /// Initialize the application (async)
    pub async fn init(&mut self) {
        // Try to load existing identity or generate new one
        if let Err(e) = self.load_or_generate_identity().await {
            log::error!("Failed to initialize identity: {}", e);
        }

        // Check if Ollama is available
        let ollama_available = self.ollama_service.is_available().await;
        self.state.ollama_available.set(ollama_available);

        if ollama_available {
            // Load available models
            match self.ollama_service.list_models().await {
                Ok(models) => {
                    log::info!("Found {} Ollama models", models.len());
                    self.state.set_models(models);
                }
                Err(e) => {
                    log::warn!("Failed to list Ollama models: {}", e);
                }
            }
        } else {
            log::warn!("Ollama is not available");
        }
    }

    /// Load existing identity or generate a new one
    async fn load_or_generate_identity(&mut self) -> Result<(), String> {
        use crate::domain::UnifiedIdentity;

        // Try to load from plaintext storage first (auto-generated, not yet backed up)
        if self.storage.plaintext_exists().await {
            match self.storage.load_plaintext().await {
                Ok(mnemonic) => {
                    match UnifiedIdentity::from_mnemonic(&mnemonic, "") {
                        Ok(identity) => {
                            let npub = identity.npub().unwrap_or_else(|_| "unknown".to_string());
                            log::info!("Loaded existing identity: {}", npub);
                            self.state.set_identity(identity);
                            self.state.is_backed_up.set(false); // Not backed up yet
                            return Ok(());
                        }
                        Err(e) => {
                            log::warn!("Failed to restore identity from stored mnemonic: {}", e);
                        }
                    }
                }
                Err(e) => {
                    log::warn!("Failed to load plaintext mnemonic: {}", e);
                }
            }
        }

        // Try to load from encrypted storage (backed up)
        // TODO: Implement password prompt for encrypted storage

        // No identity found, generate a new one
        log::info!("No identity found, generating new one");
        match UnifiedIdentity::generate() {
            Ok(identity) => {
                // Save to plaintext storage
                if let Err(e) = self.storage.store_plaintext(identity.mnemonic()).await {
                    log::error!("Failed to save identity: {}", e);
                    return Err(format!("Failed to save identity: {}", e));
                }
                let npub = identity.npub().unwrap_or_else(|_| "unknown".to_string());
                log::info!("Generated new identity: {}", npub);
                self.state.set_identity(identity);
                self.state.is_backed_up.set(false);
                Ok(())
            }
            Err(e) => {
                log::error!("Failed to generate identity: {}", e);
                Err(format!("Failed to generate identity: {}", e))
            }
        }
    }

    /// Paint the application to the scene
    pub fn paint(&mut self, bounds: Bounds, scene: &mut Scene, scale: f32, text_system: &mut TextSystem) {
        self.root_view.paint(bounds, scene, scale, text_system);
    }

    /// Handle input events
    pub fn handle_event(&mut self, event: &wgpui::InputEvent, bounds: Bounds) -> bool {
        self.root_view.handle_event(event, bounds)
    }

    /// Process pending domain events
    pub fn process_events(&mut self) {
        while let Ok(event) = self.event_rx.try_recv() {
            self.handle_domain_event(event);
        }
    }

    /// Handle a domain event
    fn handle_domain_event(&mut self, event: DomainEvent) {
        log::debug!("Domain event: {}", event.description());

        // Log the event
        self.state.log_event(event.clone());

        // Update state based on event
        match event {
            DomainEvent::WentOnline { relays, .. } => {
                self.state.is_online.set(true);
                for relay in relays {
                    self.state.add_relay(relay);
                }
            }
            DomainEvent::WentOffline { .. } => {
                self.state.is_online.set(false);
                self.state.connected_relays.set(Vec::new());
            }
            DomainEvent::RelayConnected { url, .. } => {
                self.state.add_relay(url);
            }
            DomainEvent::RelayDisconnected { url, .. } => {
                self.state.remove_relay(&url);
            }
            DomainEvent::JobReceived { job_id, kind, customer_pubkey, timestamp } => {
                let job = crate::domain::Job::new(
                    job_id,
                    String::new(),
                    kind,
                    customer_pubkey,
                    vec![],
                    std::collections::HashMap::new(),
                );
                self.state.add_job(job);
            }
            DomainEvent::JobCompleted { job_id, amount_msats, .. } => {
                self.state.complete_job(&job_id);
                if let Some(amount) = amount_msats {
                    self.state.record_payment(amount);
                }
            }
            DomainEvent::JobFailed { job_id, error, .. } => {
                self.state.update_job(&job_id, |job| {
                    job.set_failed(error.clone());
                });
            }
            DomainEvent::PaymentReceived { amount_msats, .. } => {
                self.state.record_payment(amount_msats);
            }
            DomainEvent::ModelsRefreshed { models, .. } => {
                let ollama_models: Vec<_> = models
                    .into_iter()
                    .map(|name| crate::state::OllamaModel {
                        name,
                        size: String::new(),
                        quantization: None,
                        selected: false,
                    })
                    .collect();
                self.state.set_models(ollama_models);
            }
            DomainEvent::OllamaAvailable { .. } => {
                self.state.ollama_available.set(true);
            }
            DomainEvent::OllamaUnavailable { .. } => {
                self.state.ollama_available.set(false);
            }
            DomainEvent::BalanceUpdated { balance_sats, .. } => {
                self.state.balance_sats.set(balance_sats);
            }
            _ => {}
        }
    }

    /// Get a reference to the state
    pub fn state(&self) -> &Arc<AppState> {
        &self.state
    }

    /// Get the event sender for creating new subscriptions
    pub fn event_sender(&self) -> broadcast::Sender<DomainEvent> {
        self.event_tx.clone()
    }

    /// Toggle online status
    pub async fn toggle_online(&self) {
        if self.state.is_online.get() {
            self.dvm_service.stop().await;
        } else {
            if let Err(e) = self.dvm_service.start().await {
                log::error!("Failed to go online: {}", e);
            }
        }
    }
}

impl Default for ComputeApp {
    fn default() -> Self {
        Self::new()
    }
}
