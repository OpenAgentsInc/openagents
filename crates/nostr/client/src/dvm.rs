//! DVM (Data Vending Machine) Client for NIP-90 Job Submission
//!
//! This module provides a client for submitting jobs to NIP-90 Data Vending Machines.
//! DVMs enable on-demand computation over Nostr: money in, data out.
//!
//! # Protocol Flow
//!
//! ```text
//! Customer                Service Provider
//!    |                           |
//!    |---- Job Request --------->|
//!    |      (kind 5000-5999)     |
//!    |                           |
//!    |<--- Job Feedback ---------|  (optional)
//!    |      (kind 7000)          |
//!    |                           |
//!    |<--- Job Result -----------|
//!    |      (kind 6000-6999)     |
//!    |                           |
//!    |---- Payment ----------->  |
//!    |   (bolt11 or zap)         |
//! ```
//!
//! # Example
//!
//! ```ignore
//! use nostr_client::dvm::{DvmClient, JobSubmission};
//! use nostr::{JobRequest, JobInput, KIND_JOB_TEXT_GENERATION};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let private_key = [0u8; 32];
//!     let client = DvmClient::new(private_key)?;
//!
//!     let request = JobRequest::new(KIND_JOB_TEXT_GENERATION)?
//!         .add_input(JobInput::text("What is the capital of France?"))
//!         .add_param("model", "llama3")
//!         .with_bid(1000);
//!
//!     let submission = client.submit_job(
//!         request,
//!         &["wss://relay.damus.io", "wss://nos.lol"],
//!     ).await?;
//!
//!     println!("Job submitted: {}", submission.event_id);
//!
//!     let result = client.await_result(
//!         &submission.event_id,
//!         std::time::Duration::from_secs(60),
//!     ).await?;
//!
//!     println!("Result: {}", result.content);
//!     Ok(())
//! }
//! ```

use crate::error::{ClientError, Result};
use crate::pool::{PoolConfig, RelayPool};
use nostr::{
    Event, JobFeedback, JobRequest, JobResult, JobStatus, KIND_JOB_FEEDBACK,
    create_job_request_event, finalize_event, get_result_kind, is_job_feedback_kind,
    is_job_result_kind,
};
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::{Mutex, RwLock, mpsc};
use tracing::{debug, info, warn};

/// Represents a submitted job with its metadata
#[derive(Debug, Clone)]
pub struct JobSubmission {
    /// Event ID of the published job request
    pub event_id: String,
    /// The original job request
    pub request: JobRequest,
    /// Timestamp when the job was submitted
    pub submitted_at: u64,
    /// Relays the job was published to
    pub relays: Vec<String>,
}

impl JobSubmission {
    /// Create a new job submission
    pub fn new(event_id: String, request: JobRequest, relays: Vec<String>) -> Self {
        let submitted_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        Self {
            event_id,
            request,
            submitted_at,
            relays,
        }
    }

    /// Get the expected result kind for this job
    pub fn result_kind(&self) -> u16 {
        self.request.result_kind()
    }

    /// Check if the job was recently submitted (within given duration)
    pub fn is_recent(&self, max_age: Duration) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        now.saturating_sub(self.submitted_at) < max_age.as_secs()
    }
}

/// Information about a DVM service provider
#[derive(Debug, Clone)]
pub struct DvmProvider {
    /// Provider's public key
    pub pubkey: String,
    /// Provider's display name (from NIP-89 handler info)
    pub name: Option<String>,
    /// Description of the provider's service
    pub about: Option<String>,
    /// Job kinds this provider handles
    pub supported_kinds: Vec<u16>,
    /// Relay URLs where the provider publishes
    pub relays: Vec<String>,
}

/// Feedback received during job processing
#[derive(Debug, Clone)]
pub struct JobFeedbackEvent {
    /// The feedback event
    pub feedback: JobFeedback,
    /// Event timestamp
    pub created_at: u64,
    /// Event ID
    pub event_id: String,
}

/// DVM Client for submitting NIP-90 jobs
///
/// Provides methods to:
/// - Submit job requests to DVMs
/// - Wait for job results with timeout
/// - Subscribe to job feedback updates
/// - Discover available DVM providers
pub struct DvmClient {
    /// Private key for signing events (32 bytes)
    private_key: [u8; 32],
    /// Public key (hex-encoded)
    pubkey: String,
    /// Relay pool for multi-relay operations
    pool: Arc<RelayPool>,
    /// Active job subscriptions (event_id -> subscription_id)
    active_jobs: Arc<RwLock<HashMap<String, String>>>,
    /// Pending results (event_id -> sender)
    pending_results: Arc<Mutex<HashMap<String, mpsc::Sender<JobResult>>>>,
    /// Pending feedback (event_id -> sender)
    pending_feedback: Arc<Mutex<HashMap<String, mpsc::Sender<JobFeedbackEvent>>>>,
}

impl DvmClient {
    /// Create a new DVM client with the given private key
    ///
    /// # Arguments
    /// * `private_key` - 32-byte private key for signing job requests
    ///
    /// # Errors
    /// Returns error if the private key is invalid
    pub fn new(private_key: [u8; 32]) -> Result<Self> {
        let pubkey = derive_pubkey(&private_key)?;
        let pool = Arc::new(RelayPool::new(PoolConfig::default()));

        Ok(Self {
            private_key,
            pubkey,
            pool,
            active_jobs: Arc::new(RwLock::new(HashMap::new())),
            pending_results: Arc::new(Mutex::new(HashMap::new())),
            pending_feedback: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    /// Create a DVM client with a custom relay pool
    pub fn with_pool(private_key: [u8; 32], pool: RelayPool) -> Result<Self> {
        let pubkey = derive_pubkey(&private_key)?;

        Ok(Self {
            private_key,
            pubkey,
            pool: Arc::new(pool),
            active_jobs: Arc::new(RwLock::new(HashMap::new())),
            pending_results: Arc::new(Mutex::new(HashMap::new())),
            pending_feedback: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    /// Get the client's public key (hex-encoded)
    pub fn pubkey(&self) -> &str {
        &self.pubkey
    }

    /// Submit a job request to DVM providers
    ///
    /// This publishes the job request event to the specified relays and returns
    /// immediately. Use `await_result` to wait for the result.
    ///
    /// # Arguments
    /// * `request` - The NIP-90 job request to submit
    /// * `relays` - Relay URLs to publish the job to
    ///
    /// # Returns
    /// A `JobSubmission` containing the event ID and metadata
    pub async fn submit_job(&self, request: JobRequest, relays: &[&str]) -> Result<JobSubmission> {
        if request.inputs.is_empty() {
            return Err(ClientError::InvalidRequest(
                "Job request must have at least one input".to_string(),
            ));
        }

        for relay_url in relays {
            if let Err(e) = self.pool.add_relay(relay_url).await {
                warn!("Failed to add relay {}: {}", relay_url, e);
            }
        }

        let template = create_job_request_event(&request);
        let event = finalize_event(&template, &self.private_key).map_err(|e| {
            ClientError::Internal(format!("Failed to sign job request event: {}", e))
        })?;

        let event_id = event.id.clone();
        info!("Submitting job {} (kind {})", event_id, request.kind);

        // Set auth key for NIP-42 authentication before connecting
        self.pool.set_auth_key(self.private_key).await;

        self.pool.connect_all().await?;

        // Subscribe to responses BEFORE publishing to avoid race condition
        // where feedback arrives before subscription is active
        self.subscribe_to_job_events(&event_id, request.kind)
            .await?;

        let confirmations = self.pool.publish(&event).await?;

        let accepted_count = confirmations.iter().filter(|c| c.accepted).count();
        let connected_relays = self.pool.connected_relays().await;

        if accepted_count == 0 {
            return Err(ClientError::PublishFailed(
                "Failed to publish job request to any relay".to_string(),
            ));
        }

        info!("Job {} published to {} relays", event_id, accepted_count);

        Ok(JobSubmission::new(event_id, request, connected_relays))
    }

    /// Wait for a job result with timeout
    ///
    /// Blocks until either:
    /// - A job result is received
    /// - The timeout expires
    /// - An error status is received in feedback
    ///
    /// # Arguments
    /// * `job_id` - Event ID of the submitted job request
    /// * `timeout` - Maximum time to wait for result
    ///
    /// # Returns
    /// The `JobResult` containing the computation output
    pub async fn await_result(&self, job_id: &str, timeout: Duration) -> Result<JobResult> {
        let (tx, mut rx) = mpsc::channel(1);

        {
            let mut pending = self.pending_results.lock().await;
            pending.insert(job_id.to_string(), tx);
        }

        let job_id_owned = job_id.to_string();
        let pending_results = Arc::clone(&self.pending_results);

        let result = tokio::time::timeout(timeout, rx.recv()).await;

        {
            let mut pending = pending_results.lock().await;
            pending.remove(&job_id_owned);
        }

        match result {
            Ok(Some(job_result)) => Ok(job_result),
            Ok(None) => Err(ClientError::Internal("Result channel closed".to_string())),
            Err(_) => Err(ClientError::Timeout(format!(
                "Job result timeout after {:?}",
                timeout
            ))),
        }
    }

    /// Subscribe to job feedback events
    ///
    /// Returns a channel that will receive feedback events (processing status,
    /// payment requests, etc.) for the specified job.
    ///
    /// # Arguments
    /// * `job_id` - Event ID of the submitted job request
    ///
    /// # Returns
    /// A receiver channel for `JobFeedbackEvent`s
    pub async fn subscribe_to_feedback(
        &self,
        job_id: &str,
    ) -> Result<mpsc::Receiver<JobFeedbackEvent>> {
        let (tx, rx) = mpsc::channel(16);

        {
            let mut pending = self.pending_feedback.lock().await;
            pending.insert(job_id.to_string(), tx);
        }

        Ok(rx)
    }

    /// Cancel a job subscription
    ///
    /// Stops listening for results and feedback for the specified job.
    pub async fn cancel_job(&self, job_id: &str) -> Result<()> {
        let subscription_id = {
            let mut active = self.active_jobs.write().await;
            active.remove(job_id)
        };

        if let Some(sub_id) = subscription_id {
            self.pool.unsubscribe(&sub_id).await?;
        }

        {
            let mut pending = self.pending_results.lock().await;
            pending.remove(job_id);
        }
        {
            let mut pending = self.pending_feedback.lock().await;
            pending.remove(job_id);
        }

        info!("Cancelled job subscription: {}", job_id);
        Ok(())
    }

    /// Discover DVM providers for a specific job kind
    ///
    /// Queries relays for NIP-89 handler info events that advertise support
    /// for the specified job kind.
    ///
    /// # Arguments
    /// * `job_kind` - The NIP-90 job kind to find providers for (e.g., 5050 for text generation)
    /// * `relays` - Relay URLs to query
    ///
    /// # Returns
    /// A list of `DvmProvider`s that support the requested job kind
    pub async fn discover_providers(
        &self,
        job_kind: u16,
        relays: &[&str],
    ) -> Result<Vec<DvmProvider>> {
        for relay_url in relays {
            if let Err(e) = self.pool.add_relay(relay_url).await {
                warn!("Failed to add relay {}: {}", relay_url, e);
            }
        }

        // Set auth key for NIP-42 authentication before connecting
        self.pool.set_auth_key(self.private_key).await;

        self.pool.connect_all().await?;

        let filter = json!({
            "kinds": [31990],
            "#k": [job_kind.to_string()],
            "limit": 50
        });

        let subscription_id = format!("dvm-discover-{}", job_kind);
        let mut rx = self.pool.subscribe(&subscription_id, &[filter]).await?;

        let mut providers = Vec::new();
        let timeout = Duration::from_secs(5);
        let start = Instant::now();

        while start.elapsed() < timeout {
            match tokio::time::timeout(Duration::from_millis(100), rx.recv()).await {
                Ok(Some(event)) => {
                    if let Some(provider) = parse_handler_info(&event, job_kind) {
                        providers.push(provider);
                    }
                }
                Ok(None) => break,
                Err(_) => continue,
            }
        }

        self.pool.unsubscribe(&subscription_id).await?;

        info!(
            "Discovered {} providers for kind {}",
            providers.len(),
            job_kind
        );

        Ok(providers)
    }

    /// Get the number of active job subscriptions
    pub async fn active_job_count(&self) -> usize {
        self.active_jobs.read().await.len()
    }

    async fn subscribe_to_job_events(&self, job_id: &str, request_kind: u16) -> Result<()> {
        let result_kind = get_result_kind(request_kind).ok_or_else(|| {
            ClientError::InvalidRequest(format!("Invalid request kind: {}", request_kind))
        })?;

        let filters = vec![
            json!({
                "kinds": [result_kind],
                "#e": [job_id]
            }),
            json!({
                "kinds": [KIND_JOB_FEEDBACK],
                "#e": [job_id]
            }),
        ];

        // Use shorter subscription ID - many relays have 64 char limit
        let subscription_id = format!("dvm-{}", &job_id[..16]);

        {
            let mut active = self.active_jobs.write().await;
            active.insert(job_id.to_string(), subscription_id.clone());
        }

        let mut rx = self.pool.subscribe(&subscription_id, &filters).await?;
        let pending_results = Arc::clone(&self.pending_results);
        let pending_feedback = Arc::clone(&self.pending_feedback);
        let job_id_owned = job_id.to_string();

        tokio::spawn(async move {
            while let Some(event) = rx.recv().await {
                let job_id = job_id_owned.clone();

                if is_job_result_kind(event.kind) {
                    match JobResult::from_event(&event) {
                        Ok(result) => {
                            debug!("Received job result for {}", job_id);
                            let pending = pending_results.lock().await;
                            if let Some(tx) = pending.get(&job_id) {
                                let _ = tx.send(result).await;
                            }
                        }
                        Err(e) => {
                            warn!("Failed to parse job result: {}", e);
                        }
                    }
                } else if is_job_feedback_kind(event.kind) {
                    match parse_job_feedback(&event) {
                        Some(feedback_event) => {
                            debug!(
                                "Received job feedback for {}: {:?}",
                                job_id, feedback_event.feedback.status
                            );
                            let pending = pending_feedback.lock().await;
                            if let Some(tx) = pending.get(&job_id) {
                                let _ = tx.send(feedback_event).await;
                            }
                        }
                        None => {
                            warn!("Failed to parse job feedback");
                        }
                    }
                }
            }
        });

        Ok(())
    }
}

fn derive_pubkey(private_key: &[u8; 32]) -> Result<String> {
    nostr::get_public_key_hex(private_key)
        .map_err(|e| ClientError::Internal(format!("Failed to derive public key: {}", e)))
}

fn parse_handler_info(event: &Event, job_kind: u16) -> Option<DvmProvider> {
    let mut supported_kinds = Vec::new();
    let mut relays = Vec::new();

    for tag in &event.tags {
        if tag.len() >= 2 {
            match tag[0].as_str() {
                "k" => {
                    if let Ok(kind) = tag[1].parse::<u16>() {
                        supported_kinds.push(kind);
                    }
                }
                "relay" => {
                    relays.push(tag[1].clone());
                }
                _ => {}
            }
        }
    }

    if !supported_kinds.contains(&job_kind) {
        return None;
    }

    let (name, about) =
        if let Ok(content) = serde_json::from_str::<serde_json::Value>(&event.content) {
            (
                content
                    .get("name")
                    .and_then(|v| v.as_str())
                    .map(String::from),
                content
                    .get("about")
                    .and_then(|v| v.as_str())
                    .map(String::from),
            )
        } else {
            (None, None)
        };

    Some(DvmProvider {
        pubkey: event.pubkey.clone(),
        name,
        about,
        supported_kinds,
        relays,
    })
}

fn parse_job_feedback(event: &Event) -> Option<JobFeedbackEvent> {
    let mut status = None;
    let mut status_extra = None;
    let mut request_id = None;
    let mut customer_pubkey = None;
    let mut amount = None;
    let mut bolt11 = None;

    for tag in &event.tags {
        if tag.is_empty() {
            continue;
        }
        match tag[0].as_str() {
            "status" if tag.len() >= 2 => {
                status = JobStatus::from_str(&tag[1]).ok();
                if tag.len() >= 3 {
                    status_extra = Some(tag[2].clone());
                }
            }
            "e" if tag.len() >= 2 => {
                request_id = Some(tag[1].clone());
            }
            "p" if tag.len() >= 2 => {
                customer_pubkey = Some(tag[1].clone());
            }
            "amount" if tag.len() >= 2 => {
                amount = tag[1].parse().ok();
                if tag.len() >= 3 {
                    bolt11 = Some(tag[2].clone());
                }
            }
            _ => {}
        }
    }

    let status = status?;
    let request_id = request_id?;
    let customer_pubkey = customer_pubkey?;

    let mut feedback = JobFeedback::new(status, request_id, customer_pubkey);
    if let Some(extra) = status_extra {
        feedback = feedback.with_status_extra(extra);
    }
    if let Some(amt) = amount {
        feedback = feedback.with_amount(amt, bolt11);
    }
    feedback = feedback.with_content(&event.content);

    Some(JobFeedbackEvent {
        feedback,
        created_at: event.created_at,
        event_id: event.id.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use nostr::{JobInput, KIND_JOB_TEXT_GENERATION};

    fn test_private_key() -> [u8; 32] {
        let mut key = [0u8; 32];
        key[31] = 1;
        key
    }

    #[test]
    fn test_job_submission_creation() {
        let request = JobRequest::new(KIND_JOB_TEXT_GENERATION)
            .unwrap()
            .add_input(JobInput::text("test prompt"));

        let submission = JobSubmission::new(
            "abc123".to_string(),
            request,
            vec!["wss://relay.damus.io".to_string()],
        );

        assert_eq!(submission.event_id, "abc123");
        assert_eq!(submission.result_kind(), 6050);
        assert!(!submission.relays.is_empty());
    }

    #[test]
    fn test_job_submission_is_recent() {
        let request = JobRequest::new(KIND_JOB_TEXT_GENERATION)
            .unwrap()
            .add_input(JobInput::text("test"));

        let submission = JobSubmission::new(
            "abc123".to_string(),
            request,
            vec!["wss://relay.damus.io".to_string()],
        );

        assert!(submission.is_recent(Duration::from_secs(60)));
    }

    #[test]
    fn test_dvm_provider_creation() {
        let provider = DvmProvider {
            pubkey: "abc123".to_string(),
            name: Some("Test Provider".to_string()),
            about: Some("A test DVM".to_string()),
            supported_kinds: vec![5050, 5051],
            relays: vec!["wss://relay.example.com".to_string()],
        };

        assert_eq!(provider.pubkey, "abc123");
        assert_eq!(provider.name, Some("Test Provider".to_string()));
        assert!(provider.supported_kinds.contains(&5050));
    }

    #[tokio::test]
    async fn test_dvm_client_creation() {
        let key = test_private_key();
        let client = DvmClient::new(key);

        assert!(client.is_ok());
        let client = client.unwrap();
        assert!(!client.pubkey().is_empty());
    }

    #[tokio::test]
    async fn test_active_job_count() {
        let key = test_private_key();
        let client = DvmClient::new(key).unwrap();

        assert_eq!(client.active_job_count().await, 0);
    }

    #[test]
    fn test_parse_handler_info() {
        let event = Event {
            id: "test_id".to_string(),
            pubkey: "provider_pubkey".to_string(),
            created_at: 1234567890,
            kind: 31990,
            tags: vec![
                vec!["k".to_string(), "5050".to_string()],
                vec!["k".to_string(), "5051".to_string()],
                vec!["relay".to_string(), "wss://relay.example.com".to_string()],
            ],
            content: r#"{"name": "Test DVM", "about": "A test provider"}"#.to_string(),
            sig: "test_sig".to_string(),
        };

        let provider = parse_handler_info(&event, 5050);
        assert!(provider.is_some());

        let provider = provider.unwrap();
        assert_eq!(provider.pubkey, "provider_pubkey");
        assert_eq!(provider.name, Some("Test DVM".to_string()));
        assert_eq!(provider.about, Some("A test provider".to_string()));
        assert!(provider.supported_kinds.contains(&5050));
        assert!(provider.supported_kinds.contains(&5051));
        assert!(
            provider
                .relays
                .contains(&"wss://relay.example.com".to_string())
        );
    }

    #[test]
    fn test_parse_handler_info_wrong_kind() {
        let event = Event {
            id: "test_id".to_string(),
            pubkey: "provider_pubkey".to_string(),
            created_at: 1234567890,
            kind: 31990,
            tags: vec![vec!["k".to_string(), "5100".to_string()]],
            content: "{}".to_string(),
            sig: "test_sig".to_string(),
        };

        let provider = parse_handler_info(&event, 5050);
        assert!(provider.is_none());
    }

    #[test]
    fn test_parse_job_feedback() {
        let event = Event {
            id: "feedback_id".to_string(),
            pubkey: "provider_pubkey".to_string(),
            created_at: 1234567890,
            kind: 7000,
            tags: vec![
                vec![
                    "status".to_string(),
                    "processing".to_string(),
                    "Working on it".to_string(),
                ],
                vec!["e".to_string(), "job_request_id".to_string()],
                vec!["p".to_string(), "customer_pubkey".to_string()],
            ],
            content: "Partial result...".to_string(),
            sig: "test_sig".to_string(),
        };

        let feedback = parse_job_feedback(&event);
        assert!(feedback.is_some());

        let feedback = feedback.unwrap();
        assert_eq!(feedback.feedback.status, JobStatus::Processing);
        assert_eq!(
            feedback.feedback.status_extra,
            Some("Working on it".to_string())
        );
        assert_eq!(feedback.feedback.request_id, "job_request_id");
        assert_eq!(feedback.feedback.customer_pubkey, "customer_pubkey");
        assert_eq!(feedback.feedback.content, "Partial result...");
        assert_eq!(feedback.event_id, "feedback_id");
    }

    #[test]
    fn test_parse_job_feedback_with_payment() {
        let event = Event {
            id: "feedback_id".to_string(),
            pubkey: "provider_pubkey".to_string(),
            created_at: 1234567890,
            kind: 7000,
            tags: vec![
                vec!["status".to_string(), "payment-required".to_string()],
                vec!["e".to_string(), "job_request_id".to_string()],
                vec!["p".to_string(), "customer_pubkey".to_string()],
                vec![
                    "amount".to_string(),
                    "5000".to_string(),
                    "lnbc5000n1...".to_string(),
                ],
            ],
            content: "".to_string(),
            sig: "test_sig".to_string(),
        };

        let feedback = parse_job_feedback(&event);
        assert!(feedback.is_some());

        let feedback = feedback.unwrap();
        assert_eq!(feedback.feedback.status, JobStatus::PaymentRequired);
        assert_eq!(feedback.feedback.amount, Some(5000));
        assert_eq!(feedback.feedback.bolt11, Some("lnbc5000n1...".to_string()));
    }

    #[test]
    fn test_parse_job_feedback_missing_fields() {
        let event = Event {
            id: "feedback_id".to_string(),
            pubkey: "provider_pubkey".to_string(),
            created_at: 1234567890,
            kind: 7000,
            tags: vec![
                vec!["status".to_string(), "processing".to_string()],
                vec!["e".to_string(), "job_request_id".to_string()],
            ],
            content: "".to_string(),
            sig: "test_sig".to_string(),
        };

        let feedback = parse_job_feedback(&event);
        assert!(feedback.is_none());
    }
}
