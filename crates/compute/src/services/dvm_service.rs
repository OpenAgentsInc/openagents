//! NIP-90 Data Vending Machine service
//!
//! Handles job requests from Nostr relays and processes them using available backends.
//! Supports optional Spark wallet integration for paid jobs.

use crate::backends::{AgentRegistry, BackendRegistry, CompletionRequest};
use crate::domain::{
    CodeReviewRequest, DomainEvent, Job, PatchGenRequest, SandboxRunRequest, UnifiedIdentity,
};
use crate::services::RelayService;
use chrono::Utc;
use nostr::{
    EventTemplate, HandlerInfo, HandlerMetadata, HandlerType, JobInput, JobResult,
    KIND_HANDLER_INFO, KIND_JOB_TEXT_GENERATION, PricingInfo, finalize_event,
};
use nostr::nip90::{
    JobFeedback, JobStatus, KIND_JOB_CODE_REVIEW, KIND_JOB_PATCH_GEN, KIND_JOB_REPO_INDEX,
    KIND_JOB_RLM_SUBQUERY, KIND_JOB_SANDBOX_RUN, create_job_feedback_event,
};
use spark::{Payment, PaymentDetails, PaymentStatus, PaymentType, SparkWallet};
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::{RwLock, broadcast};

/// Supported NIP-90 job kinds - inference
pub const INFERENCE_KINDS: &[u16] = &[
    KIND_JOB_TEXT_GENERATION, // 5050
    KIND_JOB_RLM_SUBQUERY,    // 5940 - RLM sub-query
];

/// Supported NIP-90 job kinds - agent (Bazaar)
pub const AGENT_KINDS: &[u16] = &[
    KIND_JOB_SANDBOX_RUN,  // 5930
    KIND_JOB_REPO_INDEX,   // 5931
    KIND_JOB_PATCH_GEN,    // 5932
    KIND_JOB_CODE_REVIEW,  // 5933
];

/// All supported NIP-90 job kinds
pub const SUPPORTED_KINDS: &[u16] = &[
    KIND_JOB_TEXT_GENERATION, // 5050
    KIND_JOB_SANDBOX_RUN,     // 5930
    KIND_JOB_REPO_INDEX,      // 5931
    KIND_JOB_PATCH_GEN,       // 5932
    KIND_JOB_CODE_REVIEW,     // 5933
    KIND_JOB_RLM_SUBQUERY,    // 5940 - RLM sub-query
];

fn job_targets_pubkey(event: &nostr::Event, pubkey: &str) -> bool {
    let tagged_pubkeys: Vec<&str> = event
        .tags
        .iter()
        .filter(|tag| tag.len() >= 2 && tag[0] == "p")
        .map(|tag| tag[1].as_str())
        .collect();

    if tagged_pubkeys.is_empty() {
        return true;
    }

    tagged_pubkeys.iter().any(|tagged| *tagged == pubkey)
}

fn payment_matches_invoice(payment: &Payment, invoice: &str) -> bool {
    match &payment.details {
        Some(PaymentDetails::Lightning { invoice: inv, .. }) => inv == invoice,
        Some(PaymentDetails::Spark { invoice_details, .. }) => invoice_details
            .as_ref()
            .map(|details| details.invoice == invoice)
            .unwrap_or(false),
        Some(PaymentDetails::Token { invoice_details, .. }) => invoice_details
            .as_ref()
            .map(|details| details.invoice == invoice)
            .unwrap_or(false),
        _ => false,
    }
}

/// Errors from the DVM service
#[derive(Debug, Error)]
pub enum DvmError {
    #[error("not initialized")]
    NotInitialized,

    #[error("unsupported job kind: {0}")]
    UnsupportedKind(u16),

    #[error("inference failed: {0}")]
    InferenceFailed(String),

    #[error("agent job failed: {0}")]
    AgentFailed(String),

    #[error("no agent backend available for kind: {0}")]
    NoAgentBackend(u16),

    #[error("relay error: {0}")]
    RelayError(String),

    #[error("signing failed: {0}")]
    SigningFailed(String),

    #[error("NIP-89 handler publishing failed: {0}")]
    HandlerPublishFailed(String),

    #[error("payment required but no wallet configured")]
    NoWalletConfigured,

    #[error("payment error: {0}")]
    PaymentError(String),

    #[error("job not found: {0}")]
    JobNotFound(String),

    #[error("payment not received for job: {0}")]
    PaymentNotReceived(String),
}

/// Configuration for the DVM service
#[derive(Clone)]
pub struct DvmConfig {
    /// Minimum price in millisats per job
    pub min_price_msats: u64,
    /// Default model to use for inference
    pub default_model: String,
    /// Whether to require payment before processing
    pub require_payment: bool,
    /// Network for Lightning payments (mainnet, testnet, signet, regtest)
    pub network: String,
}

impl Default for DvmConfig {
    fn default() -> Self {
        Self {
            min_price_msats: 1000, // 1 sat minimum
            default_model: "llama3.2".to_string(),
            require_payment: false, // For testing, don't require payment
            network: "regtest".to_string(),
        }
    }
}

/// NIP-90 Data Vending Machine service
pub struct DvmService {
    /// User identity for signing events
    identity: Arc<RwLock<Option<Arc<UnifiedIdentity>>>>,
    /// Relay service for Nostr communication
    relay_service: Arc<RelayService>,
    /// Backend registry for inference (Ollama, Apple FM, Llama.cpp)
    backend_registry: Arc<RwLock<BackendRegistry>>,
    /// Agent registry for Bazaar jobs (Codex, etc.)
    agent_registry: Arc<RwLock<AgentRegistry>>,
    /// Optional Spark wallet for payments
    wallet: Arc<RwLock<Option<Arc<SparkWallet>>>>,
    /// Service configuration
    config: DvmConfig,
    /// Event broadcaster for domain events
    event_tx: broadcast::Sender<DomainEvent>,
    /// Active jobs being processed
    active_jobs: Arc<RwLock<HashMap<String, Job>>>,
    /// Pending invoices: job_id -> (bolt11, amount_msats)
    pending_invoices: Arc<RwLock<HashMap<String, (String, u64)>>>,
    /// Whether the service is running
    running: Arc<RwLock<bool>>,
}

impl DvmService {
    /// Create a new DVM service
    pub fn new(
        relay_service: Arc<RelayService>,
        backend_registry: Arc<RwLock<BackendRegistry>>,
        event_tx: broadcast::Sender<DomainEvent>,
    ) -> Self {
        Self {
            identity: Arc::new(RwLock::new(None)),
            relay_service,
            backend_registry,
            agent_registry: Arc::new(RwLock::new(AgentRegistry::new())),
            wallet: Arc::new(RwLock::new(None)),
            config: DvmConfig::default(),
            event_tx,
            active_jobs: Arc::new(RwLock::new(HashMap::new())),
            pending_invoices: Arc::new(RwLock::new(HashMap::new())),
            running: Arc::new(RwLock::new(false)),
        }
    }

    /// Create a new DVM service with agent registry
    pub fn with_agent_registry(
        relay_service: Arc<RelayService>,
        backend_registry: Arc<RwLock<BackendRegistry>>,
        agent_registry: Arc<RwLock<AgentRegistry>>,
        event_tx: broadcast::Sender<DomainEvent>,
    ) -> Self {
        Self {
            identity: Arc::new(RwLock::new(None)),
            relay_service,
            backend_registry,
            agent_registry,
            wallet: Arc::new(RwLock::new(None)),
            config: DvmConfig::default(),
            event_tx,
            active_jobs: Arc::new(RwLock::new(HashMap::new())),
            pending_invoices: Arc::new(RwLock::new(HashMap::new())),
            running: Arc::new(RwLock::new(false)),
        }
    }

    /// Set the agent registry
    pub async fn set_agent_registry(&self, registry: AgentRegistry) {
        *self.agent_registry.write().await = registry;
    }

    /// Get the agent registry
    pub fn agent_registry(&self) -> Arc<RwLock<AgentRegistry>> {
        self.agent_registry.clone()
    }

    /// Check if an agent backend is available for the given kind
    pub async fn has_agent_for_kind(&self, kind: u16) -> bool {
        self.agent_registry.read().await.find_for_kind(kind).await.is_some()
    }

    /// Set the DVM configuration
    pub fn set_config(&mut self, config: DvmConfig) {
        self.config = config;
    }

    /// Set the network for Lightning payments
    pub fn set_network(&mut self, network: impl Into<String>) {
        self.config.network = network.into();
    }

    /// Set the Spark wallet for payment processing
    pub async fn set_wallet(&self, wallet: Arc<SparkWallet>) {
        *self.wallet.write().await = Some(wallet);
    }

    /// Check if a wallet is configured
    pub async fn has_wallet(&self) -> bool {
        self.wallet.read().await.is_some()
    }

    /// Create a DVM service with auto-detected backends
    pub async fn with_auto_detect(
        relay_service: Arc<RelayService>,
        event_tx: broadcast::Sender<DomainEvent>,
    ) -> Self {
        let registry = BackendRegistry::detect().await;
        Self::new(relay_service, Arc::new(RwLock::new(registry)), event_tx)
    }

    /// Get the backend registry
    pub fn backend_registry(&self) -> Arc<RwLock<BackendRegistry>> {
        self.backend_registry.clone()
    }

    /// Get available backend IDs
    pub async fn available_backends(&self) -> Vec<String> {
        self.backend_registry
            .read()
            .await
            .available_backends()
            .into_iter()
            .map(String::from)
            .collect()
    }

    /// Set the identity for signing events
    pub async fn set_identity(&self, identity: Arc<UnifiedIdentity>) {
        *self.identity.write().await = Some(identity);
    }

    /// Start the DVM service
    pub async fn start(&self) -> Result<(), DvmError> {
        let identity = self
            .identity
            .read()
            .await
            .clone()
            .ok_or(DvmError::NotInitialized)?;

        // Set auth key on relay service for NIP-42 authentication
        self.relay_service
            .set_auth_key(*identity.private_key_bytes())
            .await;

        // Connect to relays
        self.relay_service
            .connect()
            .await
            .map_err(|e| DvmError::RelayError(e.to_string()))?;

        // Subscribe to job requests
        let pubkey = identity.public_key_hex();
        let (_sub_id, mut event_rx) = self
            .relay_service
            .subscribe_job_requests(&pubkey)
            .await
            .map_err(|e| DvmError::RelayError(e.to_string()))?;

        *self.running.write().await = true;

        // Spawn task to process incoming job events
        let running = self.running.clone();
        let provider_pubkey = pubkey.clone();
        let identity_for_task = self.identity.clone();
        let relay_service = self.relay_service.clone();
        let backend_registry = self.backend_registry.clone();
        let event_tx = self.event_tx.clone();
        let config = self.config.clone();
        let active_jobs = self.active_jobs.clone();
        let wallet = self.wallet.clone();
        let pending_invoices = self.pending_invoices.clone();

        tokio::spawn(async move {
            log::info!("Job event processing task started");
            while let Some(event) = event_rx.recv().await {
                if !*running.read().await {
                    log::info!("Job event processing task stopping (service stopped)");
                    break;
                }

                log::info!(
                    "Received job request event: {} (kind: {})",
                    event.id,
                    event.kind
                );

                if !job_targets_pubkey(&event, &provider_pubkey) {
                    log::debug!(
                        "Skipping job {} not targeted to this provider",
                        event.id
                    );
                    continue;
                }

                // Parse the job request from the event
                match nostr::JobRequest::from_event(&event) {
                    Ok(job_request) => {
                        log::info!(
                            "Parsed job request from {}: {} inputs, {} params",
                            &event.pubkey[..16],
                            job_request.inputs.len(),
                            job_request.params.len()
                        );

                        // Convert inputs and params to the format expected by handle_job_request
                        let inputs: Vec<JobInput> = job_request.inputs;
                        let params: HashMap<String, String> = job_request
                            .params
                            .into_iter()
                            .map(|p| (p.key, p.value))
                            .collect();

                        // Process the job
                        let job_id = format!("job_{}", &event.id[..16.min(event.id.len())]);
                        let mut job = Job::new(
                            job_id.clone(),
                            event.id.clone(),
                            event.kind,
                            event.pubkey.clone(),
                            inputs.clone(),
                            params.clone(),
                        );

                        // Emit job received event
                        let _ = event_tx.send(DomainEvent::JobReceived {
                            job_id: job_id.clone(),
                            kind: event.kind,
                            customer_pubkey: event.pubkey.clone(),
                            timestamp: Utc::now(),
                        });

                        // Check if payment is required
                        let require_payment = config.require_payment;
                        if require_payment {
                            let wallet_guard = wallet.read().await;
                            if let Some(ref w) = *wallet_guard {
                                let amount_sats = config.min_price_msats / 1000;
                                let description = format!("NIP-90 job {}", job_id);

                                match w
                                    .create_invoice(amount_sats, Some(description), Some(3600))
                                    .await
                                {
                                    Ok(invoice_response) => {
                                        let bolt11 = invoice_response.payment_request.clone();
                                        let amount_msats = config.min_price_msats;

                                        job.require_payment(amount_msats, bolt11.clone());
                                        active_jobs.write().await.insert(job_id.clone(), job);
                                        pending_invoices
                                            .write()
                                            .await
                                            .insert(job_id.clone(), (bolt11.clone(), amount_msats));

                                        let _ = event_tx.send(DomainEvent::InvoiceCreated {
                                            job_id: job_id.clone(),
                                            bolt11: bolt11.clone(),
                                            amount_msats,
                                            timestamp: Utc::now(),
                                        });

                                        // Publish kind:7000 payment-required feedback to relay
                                        let identity_guard = identity_for_task.read().await;
                                        if let Some(ref identity) = *identity_guard {
                                            let feedback = JobFeedback::new(
                                                JobStatus::PaymentRequired,
                                                event.id.clone(),
                                                event.pubkey.clone(),
                                            )
                                            .with_amount(amount_msats, Some(bolt11));

                                            let template = create_job_feedback_event(&feedback);
                                            match finalize_event(&template, identity.private_key_bytes()) {
                                                Ok(feedback_event) => {
                                                    if let Err(e) = relay_service.publish(feedback_event).await {
                                                        log::error!("Failed to publish payment-required feedback: {}", e);
                                                    } else {
                                                        log::info!("Published payment-required feedback for job {}", job_id);
                                                    }
                                                }
                                                Err(e) => {
                                                    log::error!("Failed to sign feedback event: {}", e);
                                                }
                                            }
                                        }
                                        drop(identity_guard);
                                        continue;
                                    }
                                    Err(e) => {
                                        log::error!("Failed to create invoice: {}", e);
                                        continue;
                                    }
                                }
                            } else {
                                log::error!("Payment required but no wallet configured");
                                continue;
                            }
                        }

                        // Process job immediately (no payment required)
                        let start_time = std::time::Instant::now();
                        job.set_processing();
                        active_jobs
                            .write()
                            .await
                            .insert(job_id.clone(), job.clone());

                        // Get the prompt from inputs
                        let prompt = inputs
                            .iter()
                            .find(|i| i.input_type == nostr::InputType::Text)
                            .map(|i| i.data.clone())
                            .or_else(|| Some(job_request.content.clone()))
                            .unwrap_or_default();

                        log::info!(
                            "Processing job {} with prompt: {}",
                            job_id,
                            if prompt.len() > 50 {
                                format!("{}...", &prompt[..50])
                            } else {
                                prompt.clone()
                            }
                        );

                        // Get the backend
                        let registry = backend_registry.read().await;
                        if let Some(backend_id) = registry.default_id() {
                            if let Some(backend) = registry.get(backend_id) {
                                let request = CompletionRequest::new(
                                    config.default_model.clone(),
                                    prompt.clone(),
                                )
                                .with_max_tokens(
                                    params
                                        .get("max_tokens")
                                        .and_then(|s| s.parse().ok())
                                        .unwrap_or(512),
                                );

                                let backend_guard = backend.read().await;
                                drop(registry);

                                match backend_guard.complete(request).await {
                                    Ok(response) => {
                                        let tokens = response
                                            .usage
                                            .as_ref()
                                            .map(|u| u.total_tokens)
                                            .unwrap_or(0);
                                        log::info!("Job {} completed, {} tokens", job_id, tokens);

                                        // Publish result
                                        let identity_guard = identity_for_task.read().await;
                                        if let Some(ref identity) = *identity_guard {
                                            let result = nostr::JobResult::new(
                                                event.kind,
                                                &event.id,
                                                &event.pubkey,
                                                &response.text,
                                            );

                                            match result {
                                                Ok(result) => {
                                                    let template =
                                                        nostr::create_job_result_event(&result);
                                                    match nostr::finalize_event(
                                                        &template,
                                                        identity.private_key_bytes(),
                                                    ) {
                                                        Ok(result_event) => {
                                                            match relay_service
                                                                .publish(result_event)
                                                                .await
                                                            {
                                                                Ok(count) => {
                                                                    log::info!(
                                                                        "Published job result to {} relays",
                                                                        count
                                                                    );

                                                                    // Update job status
                                                                    if let Some(j) = active_jobs
                                                                        .write()
                                                                        .await
                                                                        .get_mut(&job_id)
                                                                    {
                                                                        j.set_completed(
                                                                            response.text.clone(),
                                                                        );
                                                                    }

                                                                    let duration_ms = start_time
                                                                        .elapsed()
                                                                        .as_millis()
                                                                        as u64;
                                                                    let _ = event_tx.send(
                                                                        DomainEvent::JobCompleted {
                                                                            job_id: job_id.clone(),
                                                                            amount_msats: None,
                                                                            duration_ms,
                                                                            timestamp: Utc::now(),
                                                                        },
                                                                    );
                                                                }
                                                                Err(e) => {
                                                                    log::error!(
                                                                        "Failed to publish job result: {}",
                                                                        e
                                                                    );
                                                                }
                                                            }
                                                        }
                                                        Err(e) => {
                                                            log::error!(
                                                                "Failed to sign job result: {}",
                                                                e
                                                            );
                                                        }
                                                    }
                                                }
                                                Err(e) => {
                                                    log::error!(
                                                        "Failed to create job result: {}",
                                                        e
                                                    );
                                                }
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        log::error!("Inference failed for job {}: {}", job_id, e);
                                        if let Some(j) = active_jobs.write().await.get_mut(&job_id)
                                        {
                                            j.set_failed(e.to_string());
                                        }
                                    }
                                }
                            } else {
                                log::error!("No backend found for job {}", job_id);
                            }
                        } else {
                            log::error!("No default backend available for job {}", job_id);
                        }
                    }
                    Err(e) => {
                        log::warn!("Failed to parse job request from event {}: {}", event.id, e);
                    }
                }
            }
            log::info!("Job event processing task exited");
        });

        // Spawn payment monitoring task if wallet is configured
        if self.wallet.read().await.is_some() {
            let running = self.running.clone();
            let wallet = self.wallet.clone();
            let pending_invoices = self.pending_invoices.clone();
            let active_jobs = self.active_jobs.clone();
            let event_tx = self.event_tx.clone();
            let backend_registry = self.backend_registry.clone();
            let relay_service = self.relay_service.clone();
            let identity = self.identity.clone();
            let config = self.config.clone();

            tokio::spawn(async move {
                log::info!("Payment monitoring task started");
                let mut interval = tokio::time::interval(std::time::Duration::from_secs(5));

                while *running.read().await {
                    interval.tick().await;

                    // Get list of pending invoices to check (job_id, bolt11, amount_msats)
                    let invoices: Vec<(String, (String, u64))> = {
                        pending_invoices
                            .read()
                            .await
                            .iter()
                            .map(|(job_id, (bolt11, amount))| {
                                (job_id.clone(), (bolt11.clone(), *amount))
                            })
                            .collect()
                    };

                    if invoices.is_empty() {
                        continue;
                    }

                    log::debug!("Checking {} pending invoices", invoices.len());

                    // Check each pending invoice
                    for (job_id, (bolt11, amount_msats)) in invoices {
                        let wallet_guard = wallet.read().await;
                        if let Some(ref w) = *wallet_guard {
                            // List recent payments and match by invoice string (not just amount)
                            match w.list_payments(Some(50), None).await {
                                Ok(payments) => {
                                    for payment in payments {
                                        let invoice_matches =
                                            payment_matches_invoice(&payment, &bolt11);

                                        if payment.status == PaymentStatus::Completed
                                            && payment.payment_type == PaymentType::Receive
                                            && invoice_matches
                                        {
                                            let amount_sats = amount_msats / 1000;
                                            log::info!(
                                                "Payment received for job {}: {} sats (invoice matched)",
                                                job_id,
                                                amount_sats
                                            );

                                            // Remove from pending invoices
                                            pending_invoices.write().await.remove(&job_id);

                                            // Emit payment received event
                                            let _ = event_tx.send(DomainEvent::PaymentReceived {
                                                job_id: job_id.clone(),
                                                amount_msats,
                                                timestamp: Utc::now(),
                                            });

                                            // Update job status and process
                                            if let Some(job) =
                                                active_jobs.write().await.get_mut(&job_id)
                                            {
                                                job.amount_msats = Some(amount_msats);
                                                job.status = crate::domain::job::JobStatus::Pending;
                                            }

                                            // Process the job (simplified - just get and run inference)
                                            drop(wallet_guard);
                                            if let Some(job) =
                                                active_jobs.read().await.get(&job_id).cloned()
                                            {
                                                let model = job
                                                    .requested_model()
                                                    .unwrap_or(&config.default_model)
                                                    .to_string();

                                                let _ = event_tx.send(DomainEvent::JobStarted {
                                                    job_id: job_id.clone(),
                                                    model: model.clone(),
                                                    timestamp: Utc::now(),
                                                });

                                                if let Some(prompt) = job.text_input() {
                                                    let registry = backend_registry.read().await;
                                                    if let Some(backend) = registry.default() {
                                                        let request =
                                                            CompletionRequest::new(&model, prompt);
                                                        let start_time = std::time::Instant::now();
                                                        drop(registry);

                                                        match backend
                                                            .read()
                                                            .await
                                                            .complete(request)
                                                            .await
                                                        {
                                                            Ok(response) => {
                                                                let duration_ms = start_time
                                                                    .elapsed()
                                                                    .as_millis()
                                                                    as u64;
                                                                log::info!(
                                                                    "Job {} completed after payment",
                                                                    job_id
                                                                );

                                                                // Update job status
                                                                if let Some(j) = active_jobs
                                                                    .write()
                                                                    .await
                                                                    .get_mut(&job_id)
                                                                {
                                                                    j.set_completed(
                                                                        response.text.clone(),
                                                                    );
                                                                }

                                                                let _ = event_tx.send(
                                                                    DomainEvent::JobCompleted {
                                                                        job_id: job_id.clone(),
                                                                        amount_msats: Some(
                                                                            amount_msats,
                                                                        ),
                                                                        duration_ms,
                                                                        timestamp: Utc::now(),
                                                                    },
                                                                );

                                                                // Publish result
                                                                if let Some(ref identity) =
                                                                    *identity.read().await
                                                                {
                                                                    if let Some(job) = active_jobs
                                                                        .read()
                                                                        .await
                                                                        .get(&job_id)
                                                                    {
                                                                        let result = nostr::JobResult::new(
                                                                            job.kind,
                                                                            &job.request_event_id,
                                                                            &job.customer_pubkey,
                                                                            &response.text,
                                                                        );
                                                                        if let Ok(result) = result {
                                                                            let template = nostr::create_job_result_event(&result);
                                                                            if let Ok(event) = nostr::finalize_event(&template, identity.private_key_bytes()) {
                                                                                let _ = relay_service.publish(event).await;
                                                                            }
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                            Err(e) => {
                                                                log::error!(
                                                                    "Inference failed for paid job {}: {}",
                                                                    job_id,
                                                                    e
                                                                );
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                            break;
                                        }
                                    }
                                }
                                Err(e) => {
                                    log::warn!("Failed to list payments: {}", e);
                                }
                            }
                        }
                    }

                    // Clean up expired invoices (older than 1 hour)
                    let now = Utc::now().timestamp();
                    let expired: Vec<String> = {
                        let jobs = active_jobs.read().await;
                        pending_invoices
                            .read()
                            .await
                            .keys()
                            .filter(|job_id| {
                                if let Some(job) = jobs.get(*job_id) {
                                    // Check if job was created more than 1 hour ago
                                    now - job.created_at.timestamp() > 3600
                                } else {
                                    true // No job found, consider expired
                                }
                            })
                            .cloned()
                            .collect()
                    };

                    for job_id in expired {
                        log::info!("Expiring invoice for job {}", job_id);
                        pending_invoices.write().await.remove(&job_id);
                        if let Some(j) = active_jobs.write().await.get_mut(&job_id) {
                            j.set_failed("Invoice expired".to_string());
                        }
                        let _ = event_tx.send(DomainEvent::JobFailed {
                            job_id: job_id.clone(),
                            error: "Invoice expired".to_string(),
                            timestamp: Utc::now(),
                        });
                    }
                }

                log::info!("Payment monitoring task exited");
            });
        }

        // Publish NIP-89 handler info to advertise capabilities
        match self.publish_handler_info().await {
            Ok(event_id) => {
                log::info!("Published handler info with event id: {}", event_id);
            }
            Err(e) => {
                log::warn!("Failed to publish handler info: {}", e);
                // Don't fail startup if handler publishing fails
            }
        }

        // Emit event
        let _ = self.event_tx.send(DomainEvent::WentOnline {
            timestamp: Utc::now(),
            relays: self.relay_service.connected_relays().await,
        });

        log::info!("DVM service started for pubkey: {}", pubkey);
        Ok(())
    }

    /// Stop the DVM service
    pub async fn stop(&self) {
        *self.running.write().await = false;
        self.relay_service.disconnect().await;

        let _ = self.event_tx.send(DomainEvent::WentOffline {
            timestamp: Utc::now(),
        });

        log::info!("DVM service stopped");
    }

    /// Check if the service is running
    pub async fn is_running(&self) -> bool {
        *self.running.read().await
    }

    /// Process a job request
    ///
    /// If `require_payment` is enabled in config and a wallet is configured,
    /// creates an invoice and returns the bolt11 string. The job won't be processed
    /// until `confirm_payment` is called.
    pub async fn handle_job_request(
        &self,
        event_id: &str,
        kind: u16,
        customer_pubkey: &str,
        inputs: Vec<JobInput>,
        params: HashMap<String, String>,
    ) -> Result<(), DvmError> {
        // Check if kind is supported
        if !SUPPORTED_KINDS.contains(&kind) {
            return Err(DvmError::UnsupportedKind(kind));
        }

        // Create job
        let job_id = format!("job_{}", &event_id[..16.min(event_id.len())]);
        let mut job = Job::new(
            job_id.clone(),
            event_id.to_string(),
            kind,
            customer_pubkey.to_string(),
            inputs.clone(),
            params.clone(),
        );

        // Emit job received event
        let _ = self.event_tx.send(DomainEvent::JobReceived {
            job_id: job_id.clone(),
            kind,
            customer_pubkey: customer_pubkey.to_string(),
            timestamp: Utc::now(),
        });

        // Check if payment is required
        if self.config.require_payment {
            let wallet_guard = self.wallet.read().await;
            if let Some(wallet) = wallet_guard.as_ref() {
                // Create invoice for the job
                let amount_sats = self.config.min_price_msats / 1000;
                let description = format!("NIP-90 job {}", job_id);

                match wallet
                    .create_invoice(amount_sats, Some(description), Some(3600))
                    .await
                {
                    Ok(invoice_response) => {
                        let bolt11 = invoice_response.payment_request.clone();
                        let amount_msats = self.config.min_price_msats;

                        // Update job status to require payment
                        job.require_payment(amount_msats, bolt11.clone());

                        // Store job and pending invoice
                        self.active_jobs.write().await.insert(job_id.clone(), job);
                        self.pending_invoices
                            .write()
                            .await
                            .insert(job_id.clone(), (bolt11.clone(), amount_msats));

                        // Emit invoice created event
                        let _ = self.event_tx.send(DomainEvent::InvoiceCreated {
                            job_id: job_id.clone(),
                            bolt11,
                            amount_msats,
                            timestamp: Utc::now(),
                        });

                        return Ok(());
                    }
                    Err(e) => {
                        return Err(DvmError::PaymentError(format!(
                            "Failed to create invoice: {}",
                            e
                        )));
                    }
                }
            } else {
                return Err(DvmError::NoWalletConfigured);
            }
        }

        // Store job (no payment required)
        self.active_jobs.write().await.insert(job_id.clone(), job);

        // Process the job immediately
        self.process_job(&job_id).await
    }

    /// Get the invoice for a pending job
    pub async fn get_job_invoice(&self, job_id: &str) -> Option<(String, u64)> {
        self.pending_invoices.read().await.get(job_id).cloned()
    }

    /// Confirm payment received and process the job
    ///
    /// Call this after verifying payment was received for a job.
    pub async fn confirm_payment(&self, job_id: &str) -> Result<(), DvmError> {
        // Get job
        let job = {
            let jobs = self.active_jobs.read().await;
            jobs.get(job_id).cloned()
        };

        let mut job = job.ok_or_else(|| DvmError::JobNotFound(job_id.to_string()))?;

        // Verify job was waiting for payment
        let amount_msats = match &job.status {
            crate::domain::job::JobStatus::PaymentRequired { amount_msats, .. } => *amount_msats,
            _ => {
                return Err(DvmError::PaymentError(format!(
                    "Job {} not waiting for payment",
                    job_id
                )));
            }
        };

        // Remove from pending invoices
        self.pending_invoices.write().await.remove(job_id);

        // Emit payment received event
        let _ = self.event_tx.send(DomainEvent::PaymentReceived {
            job_id: job_id.to_string(),
            amount_msats,
            timestamp: Utc::now(),
        });

        // Update job with payment info
        job.amount_msats = Some(amount_msats);
        job.status = crate::domain::job::JobStatus::Pending;
        self.active_jobs
            .write()
            .await
            .insert(job_id.to_string(), job);

        // Now process the job
        self.process_job(job_id).await
    }

    /// Check if a job's invoice has been paid by polling the wallet
    ///
    /// Returns true if the payment was received and the job is now processing.
    /// Note: For production use, consider using SDK event listeners instead of polling.
    pub async fn check_payment_status(&self, job_id: &str) -> Result<bool, DvmError> {
        let wallet_guard = self.wallet.read().await;
        let wallet = wallet_guard.as_ref().ok_or(DvmError::NoWalletConfigured)?;

        // Get the invoice for this job
        let (bolt11, _amount_msats) = self
            .pending_invoices
            .read()
            .await
            .get(job_id)
            .cloned()
            .ok_or_else(|| DvmError::JobNotFound(job_id.to_string()))?;

        // Check payment history for recent incoming payments matching our amount
        // In production, you'd use event listeners for real-time notification
        let payments = wallet
            .list_payments(Some(50), None)
            .await
            .map_err(|e| DvmError::PaymentError(e.to_string()))?;

        for payment in payments {
            if payment.status == PaymentStatus::Completed
                && payment.payment_type == PaymentType::Receive
                && payment_matches_invoice(&payment, &bolt11)
            {
                // Payment received! Confirm and process
                drop(wallet_guard); // Release lock before calling confirm_payment
                self.confirm_payment(job_id).await?;
                return Ok(true);
            }
        }

        Ok(false)
    }

    /// Convert a Job to a JobRequest for parsing by domain types
    fn job_to_request(&self, job: &Job) -> Result<nostr::JobRequest, DvmError> {
        let mut request = nostr::JobRequest::new(job.kind)
            .map_err(|e| DvmError::InferenceFailed(e.to_string()))?;

        // Convert StoredJobInput back to JobInput
        request.inputs = job
            .inputs
            .iter()
            .filter_map(|stored| stored.to_job_input())
            .collect();

        request.params = job
            .params
            .iter()
            .map(|(k, v)| nostr::JobParam {
                key: k.clone(),
                value: v.clone(),
            })
            .collect();

        Ok(request)
    }

    /// Process an agent job (Bazaar kinds 5930-5933)
    async fn process_agent_job(&self, job_id: &str) -> Result<(), DvmError> {
        let job = self
            .active_jobs
            .read()
            .await
            .get(job_id)
            .cloned()
            .ok_or_else(|| DvmError::JobNotFound(job_id.to_string()))?;

        let start_time = std::time::Instant::now();

        // Update job status
        {
            let mut jobs = self.active_jobs.write().await;
            if let Some(j) = jobs.get_mut(job_id) {
                j.set_processing();
            }
        }

        let _ = self.event_tx.send(DomainEvent::JobStarted {
            job_id: job_id.to_string(),
            model: "codex".to_string(),
            timestamp: Utc::now(),
        });

        // Find agent backend for this kind
        let agent = self
            .agent_registry
            .read()
            .await
            .find_for_kind(job.kind)
            .await
            .ok_or_else(|| DvmError::NoAgentBackend(job.kind))?;

        // Build JobRequest from Job for parsing
        let job_request = self.job_to_request(&job)?;

        // Route by kind and execute
        let result: Result<String, DvmError> = match job.kind {
            KIND_JOB_PATCH_GEN => {
                let req = PatchGenRequest::from_job_request(&job_request)
                    .map_err(|e| DvmError::AgentFailed(e.to_string()))?;
                let result = agent
                    .read()
                    .await
                    .patch_gen(req, None)
                    .await
                    .map_err(|e| DvmError::AgentFailed(e.to_string()))?;
                Ok(result.to_nip90_content())
            }
            KIND_JOB_CODE_REVIEW => {
                let req = CodeReviewRequest::from_job_request(&job_request)
                    .map_err(|e| DvmError::AgentFailed(e.to_string()))?;
                let result = agent
                    .read()
                    .await
                    .code_review(req, None)
                    .await
                    .map_err(|e| DvmError::AgentFailed(e.to_string()))?;
                Ok(result.to_nip90_content())
            }
            KIND_JOB_SANDBOX_RUN => {
                let req = SandboxRunRequest::from_job_request(&job_request)
                    .map_err(|e| DvmError::AgentFailed(e.to_string()))?;
                let result = agent
                    .read()
                    .await
                    .sandbox_run(req, None)
                    .await
                    .map_err(|e| DvmError::AgentFailed(e.to_string()))?;
                serde_json::to_string(&result)
                    .map_err(|e| DvmError::AgentFailed(e.to_string()))
            }
            KIND_JOB_REPO_INDEX => {
                // RepoIndex not implemented yet
                Err(DvmError::AgentFailed(
                    "RepoIndex not yet implemented".to_string(),
                ))
            }
            _ => Err(DvmError::UnsupportedKind(job.kind)),
        };

        let duration_ms = start_time.elapsed().as_millis() as u64;

        match result {
            Ok(result_content) => {
                // Update job with result
                let updated_job = {
                    let mut jobs = self.active_jobs.write().await;
                    if let Some(j) = jobs.get_mut(job_id) {
                        j.set_completed(result_content);
                    }
                    jobs.get(job_id).cloned()
                };

                let _ = self.event_tx.send(DomainEvent::JobCompleted {
                    job_id: job_id.to_string(),
                    amount_msats: job.amount_msats,
                    duration_ms,
                    timestamp: Utc::now(),
                });

                // Publish result to Nostr
                if let Some(ref j) = updated_job {
                    match self.publish_result(j).await {
                        Ok(event_id) => {
                            log::info!("Published agent job result with event id: {}", event_id);
                        }
                        Err(e) => {
                            log::warn!("Failed to publish agent job result: {}", e);
                        }
                    }
                }

                log::info!("Agent job {} completed in {}ms", job_id, duration_ms);
                Ok(())
            }
            Err(e) => {
                let error = e.to_string();
                {
                    let mut jobs = self.active_jobs.write().await;
                    if let Some(j) = jobs.get_mut(job_id) {
                        j.set_failed(error.clone());
                    }
                }

                let _ = self.event_tx.send(DomainEvent::JobFailed {
                    job_id: job_id.to_string(),
                    error: error.clone(),
                    timestamp: Utc::now(),
                });

                Err(e)
            }
        }
    }

    /// Process a job
    async fn process_job(&self, job_id: &str) -> Result<(), DvmError> {
        let job = {
            let jobs = self.active_jobs.read().await;
            jobs.get(job_id).cloned()
        };

        let job = match job {
            Some(j) => j,
            None => return Ok(()),
        };

        // Route agent jobs to agent backends
        if AGENT_KINDS.contains(&job.kind) {
            return self.process_agent_job(job_id).await;
        }

        // For inference jobs, continue with inference logic
        let mut job = job;

        // Get the model to use
        let model = job
            .requested_model()
            .unwrap_or(&self.config.default_model)
            .to_string();

        // Update job status
        job.set_processing();
        job.model = Some(model.clone());
        self.active_jobs
            .write()
            .await
            .insert(job_id.to_string(), job.clone());

        let _ = self.event_tx.send(DomainEvent::JobStarted {
            job_id: job_id.to_string(),
            model: model.clone(),
            timestamp: Utc::now(),
        });

        // Get the input text
        let prompt = match job.text_input() {
            Some(text) => text.to_string(),
            None => {
                job.set_failed("No text input provided".to_string());
                self.active_jobs
                    .write()
                    .await
                    .insert(job_id.to_string(), job.clone());

                let _ = self.event_tx.send(DomainEvent::JobFailed {
                    job_id: job_id.to_string(),
                    error: "No text input".to_string(),
                    timestamp: Utc::now(),
                });

                return Ok(());
            }
        };

        // Get backend from registry (use default or job-specified backend)
        let backend_id = job.params.get("backend").cloned();
        let backend = {
            let registry = self.backend_registry.read().await;
            if let Some(ref id) = backend_id {
                registry.get(id)
            } else {
                registry.default()
            }
        };

        let backend = match backend {
            Some(b) => b,
            None => {
                let error = "No inference backend available".to_string();
                job.set_failed(error.clone());
                self.active_jobs
                    .write()
                    .await
                    .insert(job_id.to_string(), job.clone());

                let _ = self.event_tx.send(DomainEvent::JobFailed {
                    job_id: job_id.to_string(),
                    error,
                    timestamp: Utc::now(),
                });

                return Err(DvmError::InferenceFailed(
                    "No inference backend available".to_string(),
                ));
            }
        };

        // Run inference using the backend
        let start_time = std::time::Instant::now();
        let request = CompletionRequest::new(&model, &prompt);
        let result = backend.read().await.complete(request).await;

        match result {
            Ok(response) => {
                let duration_ms = start_time.elapsed().as_millis() as u64;

                job.set_completed(response.text);
                self.active_jobs
                    .write()
                    .await
                    .insert(job_id.to_string(), job.clone());

                let _ = self.event_tx.send(DomainEvent::JobCompleted {
                    job_id: job_id.to_string(),
                    amount_msats: job.amount_msats,
                    duration_ms,
                    timestamp: Utc::now(),
                });

                // Publish result to Nostr
                match self.publish_result(&job).await {
                    Ok(event_id) => {
                        log::info!("Published job result with event id: {}", event_id);
                    }
                    Err(e) => {
                        log::warn!("Failed to publish job result: {}", e);
                        // Don't fail the job if publishing fails
                    }
                }

                log::info!("Job {} completed in {}ms", job_id, duration_ms);
            }
            Err(e) => {
                let error = e.to_string();
                job.set_failed(error.clone());
                self.active_jobs
                    .write()
                    .await
                    .insert(job_id.to_string(), job.clone());

                let _ = self.event_tx.send(DomainEvent::JobFailed {
                    job_id: job_id.to_string(),
                    error,
                    timestamp: Utc::now(),
                });
            }
        }

        Ok(())
    }

    /// Get active jobs
    pub async fn active_jobs(&self) -> Vec<Job> {
        self.active_jobs.read().await.values().cloned().collect()
    }

    /// Get a specific job
    pub async fn get_job(&self, job_id: &str) -> Option<Job> {
        self.active_jobs.read().await.get(job_id).cloned()
    }

    /// Publish NIP-89 handler information to advertise compute provider capabilities
    pub async fn publish_handler_info(&self) -> Result<String, DvmError> {
        let identity = self
            .identity
            .read()
            .await
            .clone()
            .ok_or(DvmError::NotInitialized)?;

        // Get available inference backends
        let inference_backends = self.available_backends().await;

        // Get available agent backends
        let agent_backends: Vec<String> = self
            .agent_registry
            .read()
            .await
            .available_backends()
            .iter()
            .map(|s| s.to_string())
            .collect();

        // Build description
        let mut desc_parts = vec![];
        if !inference_backends.is_empty() {
            desc_parts.push(format!("Inference: {}", inference_backends.join(", ")));
        }
        if !agent_backends.is_empty() {
            desc_parts.push(format!("Agents: {}", agent_backends.join(", ")));
        }
        let backend_desc = if desc_parts.is_empty() {
            "No backends available".to_string()
        } else {
            desc_parts.join(". ")
        };

        // Create handler metadata
        let metadata = HandlerMetadata::new(
            "OpenAgents Compute Provider",
            &format!(
                "AI compute provider for NIP-90 jobs (inference + agentic). {}",
                backend_desc
            ),
        )
        .with_website("https://openagents.com");

        // Build handler info
        let mut handler_info = HandlerInfo::new(
            identity.public_key_hex(),
            HandlerType::ComputeProvider,
            metadata,
        )
        .add_capability("text-generation")
        .add_capability("nip90-kind-5050")
        .add_capability("nip90-kind-5940") // RLM sub-query
        .add_custom_tag("network", &self.config.network);

        // Add capabilities for agent backends (Bazaar kinds)
        let agent_caps = self.agent_registry.read().await.aggregated_capabilities().await;
        for kind in agent_caps.supported_kinds() {
            handler_info = handler_info.add_capability(&format!("nip90-kind-{}", kind));
        }

        // Add pricing if configured
        if self.config.min_price_msats > 0 {
            let pricing = PricingInfo::new(self.config.min_price_msats)
                .with_model("per-request")
                .with_currency("sats");
            handler_info = handler_info.with_pricing(pricing);
        }

        // Serialize metadata to JSON for event content
        let content = serde_json::json!({
            "name": "OpenAgents Compute Provider",
            "description": format!("AI compute provider for NIP-90 jobs. {}", backend_desc),
            "website": "https://openagents.com",
            "inference_backends": inference_backends,
            "agent_backends": agent_backends,
            "supported_kinds": agent_caps.supported_kinds()
        })
        .to_string();

        // Create event template
        let template = EventTemplate {
            created_at: Utc::now().timestamp() as u64,
            kind: KIND_HANDLER_INFO,
            tags: handler_info.to_tags(),
            content,
        };

        // Sign the event
        let event = finalize_event(&template, identity.private_key_bytes())
            .map_err(|e| DvmError::SigningFailed(e.to_string()))?;

        // Publish to relays
        self.relay_service
            .publish(event.clone())
            .await
            .map_err(|e| DvmError::HandlerPublishFailed(e.to_string()))?;

        log::info!(
            "Published NIP-89 handler info (event id: {}) to {} relays",
            event.id,
            self.relay_service.connected_relays().await.len()
        );

        Ok(event.id)
    }

    /// Publish a job result to Nostr relays
    async fn publish_result(&self, job: &Job) -> Result<String, DvmError> {
        let identity = self
            .identity
            .read()
            .await
            .clone()
            .ok_or(DvmError::NotInitialized)?;

        // Extract result content
        let result_content = match &job.status {
            crate::domain::job::JobStatus::Completed { result } => result,
            _ => return Err(DvmError::InferenceFailed("Job not completed".to_string())),
        };

        // Create job result
        let mut job_result = JobResult::new(
            job.kind,
            &job.request_event_id,
            &job.customer_pubkey,
            result_content,
        )
        .map_err(|e| DvmError::SigningFailed(format!("Failed to create job result: {}", e)))?;

        // Add payment info if configured
        if let Some(amount) = job.amount_msats
            && let Some(bolt11) = &job.bolt11
        {
            job_result = job_result.with_amount(amount, Some(bolt11.clone()));
        }

        // Create event template
        let template = EventTemplate {
            created_at: Utc::now().timestamp() as u64,
            kind: job_result.kind,
            tags: job_result.to_tags(),
            content: job_result.content.clone(),
        };

        // Sign the event
        let event = finalize_event(&template, identity.private_key_bytes())
            .map_err(|e| DvmError::SigningFailed(e.to_string()))?;

        // Publish to relays
        self.relay_service
            .publish(event.clone())
            .await
            .map_err(|e| DvmError::RelayError(e.to_string()))?;

        log::info!(
            "Published NIP-90 job result (event id: {}) for request {} to {} relays",
            event.id,
            job.request_event_id,
            self.relay_service.connected_relays().await.len()
        );

        Ok(event.id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::UnifiedIdentity;

    #[test]
    fn test_supported_kinds() {
        assert!(SUPPORTED_KINDS.contains(&5050));
    }

    #[test]
    fn test_default_config() {
        let config = DvmConfig::default();
        assert_eq!(config.min_price_msats, 1000);
        assert_eq!(config.default_model, "llama3.2");
    }

    #[tokio::test]
    #[ignore] // Requires relay network connectivity
    async fn test_publish_handler_info() {
        // Create test identity
        let identity = UnifiedIdentity::generate().expect("should generate identity");

        // Create services
        let relay_service = Arc::new(RelayService::new());
        let backend_registry = Arc::new(RwLock::new(BackendRegistry::new()));
        let (event_tx, _event_rx) = broadcast::channel(100);

        // Create DVM service
        let dvm = DvmService::new(relay_service.clone(), backend_registry, event_tx);
        dvm.set_identity(Arc::new(identity.clone())).await;

        // Connect to relays (this is mocked in tests)
        relay_service.connect().await.expect("should connect");

        // Publish handler info
        let result = dvm.publish_handler_info().await;
        assert!(result.is_ok(), "should publish handler info");

        let event_id = result.unwrap();
        assert_eq!(event_id.len(), 64, "event id should be 64 hex characters");
    }

    #[tokio::test]
    #[ignore] // Requires relay network connectivity
    async fn test_publish_handler_info_with_pricing() {
        // Create test identity
        let identity = UnifiedIdentity::generate().expect("should generate identity");

        // Create services
        let relay_service = Arc::new(RelayService::new());
        let backend_registry = Arc::new(RwLock::new(BackendRegistry::new()));
        let (event_tx, _event_rx) = broadcast::channel(100);

        // Create DVM service with custom config
        let mut dvm = DvmService::new(relay_service.clone(), backend_registry, event_tx);
        let config = DvmConfig {
            min_price_msats: 5000,
            ..Default::default()
        };
        dvm.set_config(config);
        dvm.set_identity(Arc::new(identity.clone())).await;

        // Connect to relays
        relay_service.connect().await.expect("should connect");

        // Publish handler info
        let result = dvm.publish_handler_info().await;
        assert!(result.is_ok(), "should publish handler info with pricing");
    }

    #[tokio::test]
    async fn test_publish_handler_info_requires_identity() {
        // Create services without setting identity
        let relay_service = Arc::new(RelayService::new());
        let backend_registry = Arc::new(RwLock::new(BackendRegistry::new()));
        let (event_tx, _event_rx) = broadcast::channel(100);

        let dvm = DvmService::new(relay_service, backend_registry, event_tx);

        // Should fail without identity
        let result = dvm.publish_handler_info().await;
        assert!(result.is_err(), "should fail without identity");
        assert!(matches!(result.unwrap_err(), DvmError::NotInitialized));
    }
}
