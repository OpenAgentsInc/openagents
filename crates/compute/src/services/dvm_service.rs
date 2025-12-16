//! NIP-90 Data Vending Machine service
//!
//! Handles job requests from Nostr relays and processes them using Ollama.

use crate::domain::{DomainEvent, Job, UnifiedIdentity};
use crate::services::{OllamaService, RelayService};
use chrono::Utc;
use nostr::{JobInput, KIND_JOB_TEXT_GENERATION};
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::{broadcast, RwLock};

/// Supported NIP-90 job kinds
pub const SUPPORTED_KINDS: &[u16] = &[
    KIND_JOB_TEXT_GENERATION, // 5050
];

/// Errors from the DVM service
#[derive(Debug, Error)]
pub enum DvmError {
    #[error("not initialized")]
    NotInitialized,

    #[error("unsupported job kind: {0}")]
    UnsupportedKind(u16),

    #[error("inference failed: {0}")]
    InferenceFailed(String),

    #[error("relay error: {0}")]
    RelayError(String),

    #[error("signing failed: {0}")]
    SigningFailed(String),
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
}

impl Default for DvmConfig {
    fn default() -> Self {
        Self {
            min_price_msats: 1000, // 1 sat minimum
            default_model: "llama3.2".to_string(),
            require_payment: false, // For testing, don't require payment
        }
    }
}

/// NIP-90 Data Vending Machine service
pub struct DvmService {
    /// User identity for signing events
    identity: Arc<RwLock<Option<Arc<UnifiedIdentity>>>>,
    /// Relay service for Nostr communication
    relay_service: Arc<RelayService>,
    /// Ollama service for inference
    ollama_service: Arc<OllamaService>,
    /// Service configuration
    config: DvmConfig,
    /// Event broadcaster for domain events
    event_tx: broadcast::Sender<DomainEvent>,
    /// Active jobs being processed
    active_jobs: Arc<RwLock<HashMap<String, Job>>>,
    /// Whether the service is running
    running: Arc<RwLock<bool>>,
}

impl DvmService {
    /// Create a new DVM service
    pub fn new(
        relay_service: Arc<RelayService>,
        ollama_service: Arc<OllamaService>,
        event_tx: broadcast::Sender<DomainEvent>,
    ) -> Self {
        Self {
            identity: Arc::new(RwLock::new(None)),
            relay_service,
            ollama_service,
            config: DvmConfig::default(),
            event_tx,
            active_jobs: Arc::new(RwLock::new(HashMap::new())),
            running: Arc::new(RwLock::new(false)),
        }
    }

    /// Set the identity for signing events
    pub async fn set_identity(&self, identity: Arc<UnifiedIdentity>) {
        *self.identity.write().await = Some(identity);
    }

    /// Set configuration
    pub fn set_config(&mut self, config: DvmConfig) {
        self.config = config;
    }

    /// Start the DVM service
    pub async fn start(&self) -> Result<(), DvmError> {
        let identity = self
            .identity
            .read()
            .await
            .clone()
            .ok_or(DvmError::NotInitialized)?;

        // Connect to relays
        self.relay_service
            .connect()
            .await
            .map_err(|e| DvmError::RelayError(e.to_string()))?;

        // Subscribe to job requests
        let pubkey = identity.public_key_hex();
        self.relay_service
            .subscribe_job_requests(&pubkey)
            .await
            .map_err(|e| DvmError::RelayError(e.to_string()))?;

        *self.running.write().await = true;

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
        let job = Job::new(
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

        // Store job
        self.active_jobs.write().await.insert(job_id.clone(), job);

        // Process the job
        self.process_job(&job_id).await
    }

    /// Process a job
    async fn process_job(&self, job_id: &str) -> Result<(), DvmError> {
        let job = {
            let jobs = self.active_jobs.read().await;
            jobs.get(job_id).cloned()
        };

        let mut job = match job {
            Some(j) => j,
            None => return Ok(()),
        };

        // Get the model to use
        let model = job
            .requested_model()
            .unwrap_or(&self.config.default_model)
            .to_string();

        // Update job status
        job.set_processing();
        job.model = Some(model.clone());
        self.active_jobs.write().await.insert(job_id.to_string(), job.clone());

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
                self.active_jobs.write().await.insert(job_id.to_string(), job.clone());

                let _ = self.event_tx.send(DomainEvent::JobFailed {
                    job_id: job_id.to_string(),
                    error: "No text input".to_string(),
                    timestamp: Utc::now(),
                });

                return Ok(());
            }
        };

        // Run inference
        let start_time = std::time::Instant::now();
        let result = self.ollama_service.generate(&model, &prompt).await;

        match result {
            Ok(output) => {
                let duration_ms = start_time.elapsed().as_millis() as u64;

                job.set_completed(output);
                self.active_jobs.write().await.insert(job_id.to_string(), job.clone());

                let _ = self.event_tx.send(DomainEvent::JobCompleted {
                    job_id: job_id.to_string(),
                    amount_msats: job.amount_msats,
                    duration_ms,
                    timestamp: Utc::now(),
                });

                // TODO: Publish result to Nostr
                // self.publish_result(&job).await?;

                log::info!("Job {} completed in {}ms", job_id, duration_ms);
            }
            Err(e) => {
                let error = e.to_string();
                job.set_failed(error.clone());
                self.active_jobs.write().await.insert(job_id.to_string(), job.clone());

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
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
