//! NIP-90: Data Vending Machines
//!
//! Data Vending Machines (DVMs) enable on-demand computation over Nostr. Customers submit job
//! requests to service providers who perform computation and return results. Payment flows via
//! Lightning (bolt11 invoices) or Nostr Zaps.
//!
//! **Money in, data out.**
//!
//! ## Protocol Overview
//!
//! DVMs follow a simple request-response pattern:
//!
//! ```text
//! Customer                Service Provider
//!    │                           │
//!    │──── Job Request ──────────>│
//!    │      (kind 5000-5999)      │
//!    │                            │
//!    │<─── Job Feedback ──────────│ (optional)
//!    │      (kind 7000)           │
//!    │                            │
//!    │<─── Job Result ────────────│
//!    │      (kind 6000-6999)      │
//!    │                            │
//!    │──── Payment ───────────────>│
//!    │   (bolt11 or zap)          │
//! ```
//!
//! ## Event Kinds
//!
//! - **5000-5999**: Job request kinds
//!   - 5000: Text extraction/OCR
//!   - 5001: Summarization
//!   - 5002: Translation
//!   - 5050: Text generation (LLM inference)
//!   - 5100: Image generation
//!   - 5250: Speech-to-text
//!
//! - **6000-6999**: Job result kinds (= request kind + 1000)
//!   - Result kind is automatically calculated from request kind
//!
//! - **7000**: Job feedback (status updates, payment requests)
//!
//! ## Creating a Job Request
//!
//! ```rust
//! use nostr::nip90::{JobRequest, JobInput, KIND_JOB_TEXT_GENERATION};
//!
//! let request = JobRequest::new(KIND_JOB_TEXT_GENERATION)?
//!     .add_input(JobInput::text("Write a haiku about Nostr"))
//!     .add_param("temperature", "0.7")
//!     .add_param("max_tokens", "100")
//!     .with_bid(1000)  // millisats
//!     .add_relay("wss://relay.damus.io");
//!
//! // Convert to event tags and publish
//! let tags = request.to_tags();
//! # Ok::<(), nostr::nip90::Nip90Error>(())
//! ```
//!
//! ## Creating a Job Result
//!
//! ```rust
//! use nostr::nip90::{JobResult, KIND_JOB_TEXT_GENERATION};
//!
//! let result = JobResult::new(
//!     KIND_JOB_TEXT_GENERATION,
//!     "request_event_id",
//!     "customer_pubkey",
//!     "Nostr flows free,\nDecentralized thoughts connect,\nSovereign and true.",
//! )?
//! .with_amount(1000, Some("lnbc1000n...".to_string()));
//!
//! // Convert to event tags and publish
//! let tags = result.to_tags();
//! # Ok::<(), nostr::nip90::Nip90Error>(())
//! ```
//!
//! ## Input Types
//!
//! DVMs support four input types via the `i` tag:
//!
//! - **text**: Direct text input
//! - **url**: URL to fetch data from
//! - **event**: Nostr event ID (with optional relay hint)
//! - **job**: Output from another job (chaining)
//!
//! ```rust
//! use nostr::nip90::JobInput;
//!
//! // Direct text
//! let input = JobInput::text("Translate this");
//!
//! // URL with marker
//! let input = JobInput::url("https://example.com/doc.txt")
//!     .with_marker("source");
//!
//! // Event reference
//! let input = JobInput::event("event_id", Some("wss://relay.com".to_string()));
//!
//! // Chain from another job
//! let input = JobInput::job("previous_job_id", None);
//! ```
//!
//! ## Parameters
//!
//! Model-specific parameters are passed via `param` tags:
//!
//! ```rust
//! use nostr::nip90::JobParam;
//!
//! let param = JobParam::new("temperature", "0.7");
//! let param = JobParam::new("max_tokens", "2048");
//! let param = JobParam::new("model", "llama3.2");
//! ```
//!
//! ## Payment Flow
//!
//! 1. Customer includes `bid` tag in job request (optional)
//! 2. Provider MAY send feedback (kind 7000) with `payment-required` status
//! 3. Provider publishes result (kind 6000-6999) with `amount` and `bolt11` tags
//! 4. Customer pays bolt11 invoice or zaps the result event
//!
//! ## Service Provider Discovery
//!
//! Providers advertise their capabilities via NIP-89 application handler events.
//! Customers discover providers by querying relays for handlers of specific job kinds.
//!
//! See the [NIP-90 specification](https://github.com/nostr-protocol/nips/blob/master/90.md)
//! for complete details.

use crate::nip01::Event;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Kind range for job requests
pub const JOB_REQUEST_KIND_MIN: u16 = 5000;
pub const JOB_REQUEST_KIND_MAX: u16 = 5999;

/// Kind range for job results (request kind + 1000)
pub const JOB_RESULT_KIND_MIN: u16 = 6000;
pub const JOB_RESULT_KIND_MAX: u16 = 6999;

/// Kind for job feedback
pub const KIND_JOB_FEEDBACK: u16 = 7000;

// Common job request kinds (from DVM spec)
/// Text extraction / OCR
pub const KIND_JOB_TEXT_EXTRACTION: u16 = 5000;
/// Summarization
pub const KIND_JOB_SUMMARIZATION: u16 = 5001;
/// Translation
pub const KIND_JOB_TRANSLATION: u16 = 5002;
/// Text generation / Chat
pub const KIND_JOB_TEXT_GENERATION: u16 = 5050;
/// Image generation
pub const KIND_JOB_IMAGE_GENERATION: u16 = 5100;
/// Speech to text
pub const KIND_JOB_SPEECH_TO_TEXT: u16 = 5250;

// OpenAgents compute job kinds (Bazaar)
/// Sandbox run - execute commands in isolated container
pub const KIND_JOB_SANDBOX_RUN: u16 = 5930;
/// Repository index - generate embeddings/symbols for a git repo
pub const KIND_JOB_REPO_INDEX: u16 = 5931;
/// Patch generation - generate code patches from issue descriptions
pub const KIND_JOB_PATCH_GEN: u16 = 5932;
/// Code review - review code changes and provide feedback
pub const KIND_JOB_CODE_REVIEW: u16 = 5933;

/// Errors that can occur during NIP-90 operations.
#[derive(Debug, Error)]
pub enum Nip90Error {
    #[error("invalid kind: {0} (expected {1})")]
    InvalidKind(u16, String),

    #[error("missing required tag: {0}")]
    MissingTag(String),

    #[error("invalid input type: {0}")]
    InvalidInputType(String),

    #[error("invalid status: {0}")]
    InvalidStatus(String),

    #[error("serialization error: {0}")]
    Serialization(String),
}

/// Input type for job request `i` tag.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InputType {
    /// A URL to fetch data from
    Url,
    /// A Nostr event ID
    Event,
    /// Output of a previous job
    Job,
    /// Direct text input
    Text,
}

impl InputType {
    pub fn as_str(&self) -> &'static str {
        match self {
            InputType::Url => "url",
            InputType::Event => "event",
            InputType::Job => "job",
            InputType::Text => "text",
        }
    }

    pub fn from_str(s: &str) -> Result<Self, Nip90Error> {
        match s.to_lowercase().as_str() {
            "url" => Ok(InputType::Url),
            "event" => Ok(InputType::Event),
            "job" => Ok(InputType::Job),
            "text" => Ok(InputType::Text),
            _ => Err(Nip90Error::InvalidInputType(s.to_string())),
        }
    }
}

/// Job feedback status.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum JobStatus {
    /// Service Provider requires payment before continuing
    PaymentRequired,
    /// Service Provider is processing the job
    Processing,
    /// Service Provider was unable to process the job
    Error,
    /// Service Provider successfully processed the job
    Success,
    /// Service Provider partially processed the job
    Partial,
}

impl JobStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            JobStatus::PaymentRequired => "payment-required",
            JobStatus::Processing => "processing",
            JobStatus::Error => "error",
            JobStatus::Success => "success",
            JobStatus::Partial => "partial",
        }
    }

    pub fn from_str(s: &str) -> Result<Self, Nip90Error> {
        match s {
            "payment-required" => Ok(JobStatus::PaymentRequired),
            "processing" => Ok(JobStatus::Processing),
            "error" => Ok(JobStatus::Error),
            "success" => Ok(JobStatus::Success),
            "partial" => Ok(JobStatus::Partial),
            _ => Err(Nip90Error::InvalidStatus(s.to_string())),
        }
    }
}

/// An input for a job request (`i` tag).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobInput {
    /// The input data/argument
    pub data: String,
    /// How to interpret the input
    pub input_type: InputType,
    /// Relay hint (for event/job types)
    pub relay: Option<String>,
    /// Optional marker for how input should be used
    pub marker: Option<String>,
}

impl JobInput {
    /// Create a new text input.
    ///
    /// # Example
    ///
    /// ```rust
    /// use nostr::nip90::JobInput;
    ///
    /// let input = JobInput::text("Translate this text to Spanish");
    /// assert_eq!(input.data, "Translate this text to Spanish");
    /// ```
    pub fn text(data: impl Into<String>) -> Self {
        Self {
            data: data.into(),
            input_type: InputType::Text,
            relay: None,
            marker: None,
        }
    }

    /// Create a new URL input.
    ///
    /// # Example
    ///
    /// ```rust
    /// use nostr::nip90::JobInput;
    ///
    /// let input = JobInput::url("https://example.com/document.pdf");
    /// assert_eq!(input.data, "https://example.com/document.pdf");
    /// ```
    pub fn url(url: impl Into<String>) -> Self {
        Self {
            data: url.into(),
            input_type: InputType::Url,
            relay: None,
            marker: None,
        }
    }

    /// Create a new event input.
    ///
    /// # Example
    ///
    /// ```rust
    /// use nostr::nip90::JobInput;
    ///
    /// let input = JobInput::event("event_id_123", Some("wss://relay.damus.io".to_string()));
    /// assert_eq!(input.data, "event_id_123");
    /// assert_eq!(input.relay, Some("wss://relay.damus.io".to_string()));
    /// ```
    pub fn event(event_id: impl Into<String>, relay: Option<String>) -> Self {
        Self {
            data: event_id.into(),
            input_type: InputType::Event,
            relay,
            marker: None,
        }
    }

    /// Create a new job input (chaining from previous job).
    ///
    /// # Example
    ///
    /// ```rust
    /// use nostr::nip90::JobInput;
    ///
    /// // Chain from a previous job's output
    /// let input = JobInput::job("previous_job_event_id", None);
    /// assert_eq!(input.data, "previous_job_event_id");
    /// ```
    pub fn job(job_id: impl Into<String>, relay: Option<String>) -> Self {
        Self {
            data: job_id.into(),
            input_type: InputType::Job,
            relay,
            marker: None,
        }
    }

    /// Set the marker for this input.
    pub fn with_marker(mut self, marker: impl Into<String>) -> Self {
        self.marker = Some(marker.into());
        self
    }

    /// Convert to tag array.
    pub fn to_tag(&self) -> Vec<String> {
        let mut tag = vec![
            "i".to_string(),
            self.data.clone(),
            self.input_type.as_str().to_string(),
        ];

        // Add relay (empty string if not present but marker is)
        if self.relay.is_some() || self.marker.is_some() {
            tag.push(self.relay.clone().unwrap_or_default());
        }

        // Add marker if present
        if let Some(marker) = &self.marker {
            tag.push(marker.clone());
        }

        tag
    }

    /// Parse from tag array.
    pub fn from_tag(tag: &[String]) -> Result<Self, Nip90Error> {
        if tag.len() < 3 || tag[0] != "i" {
            return Err(Nip90Error::MissingTag(
                "i tag requires at least 3 elements".to_string(),
            ));
        }

        Ok(Self {
            data: tag[1].clone(),
            input_type: InputType::from_str(&tag[2])?,
            relay: tag.get(3).filter(|s| !s.is_empty()).cloned(),
            marker: tag.get(4).cloned(),
        })
    }
}

/// A parameter for a job request (`param` tag).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobParam {
    /// Parameter key
    pub key: String,
    /// Parameter value
    pub value: String,
}

impl JobParam {
    /// Create a new job parameter.
    ///
    /// Parameters are model-specific settings passed via `param` tags.
    ///
    /// # Example
    ///
    /// ```rust
    /// use nostr::nip90::JobParam;
    ///
    /// let temp = JobParam::new("temperature", "0.7");
    /// let tokens = JobParam::new("max_tokens", "2048");
    /// let model = JobParam::new("model", "llama3.2");
    ///
    /// assert_eq!(temp.key, "temperature");
    /// assert_eq!(temp.value, "0.7");
    /// ```
    pub fn new(key: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            value: value.into(),
        }
    }

    /// Convert to tag array.
    pub fn to_tag(&self) -> Vec<String> {
        vec!["param".to_string(), self.key.clone(), self.value.clone()]
    }

    /// Parse from tag array.
    pub fn from_tag(tag: &[String]) -> Result<Self, Nip90Error> {
        if tag.len() < 3 || tag[0] != "param" {
            return Err(Nip90Error::MissingTag(
                "param tag requires 3 elements".to_string(),
            ));
        }

        Ok(Self {
            key: tag[1].clone(),
            value: tag[2].clone(),
        })
    }
}

/// A job request event data (kind 5000-5999).
///
/// # Examples
///
/// ```
/// use nostr::nip90::{JobRequest, JobInput, JobParam, KIND_JOB_TEXT_GENERATION};
///
/// # fn example() -> Result<(), nostr::nip90::Nip90Error> {
/// // Create a text generation request
/// let request = JobRequest::new(KIND_JOB_TEXT_GENERATION)?
///     .add_input(JobInput::text("Write a haiku about Nostr"))
///     .add_param("temperature", "0.7")
///     .add_param("max_tokens", "100")
///     .with_bid(1000)  // 1000 millisats
///     .add_relay("wss://relay.damus.io");
///
/// assert_eq!(request.kind, KIND_JOB_TEXT_GENERATION);
/// assert_eq!(request.inputs.len(), 1);
/// assert_eq!(request.params.len(), 2);
/// assert_eq!(request.bid, Some(1000));
///
/// // Convert to tags for publishing
/// let tags = request.to_tags();
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct JobRequest {
    /// The specific job kind (5000-5999)
    pub kind: u16,
    /// Input data for the job
    pub inputs: Vec<JobInput>,
    /// Expected output format (MIME type)
    pub output: Option<String>,
    /// Parameters for the job
    pub params: Vec<JobParam>,
    /// Maximum bid in millisats
    pub bid: Option<u64>,
    /// Relays where service providers should publish responses
    pub relays: Vec<String>,
    /// Preferred service provider pubkeys
    pub service_providers: Vec<String>,
    /// Whether params are encrypted
    pub encrypted: bool,
    /// Additional content (encrypted params if encrypted=true)
    pub content: String,
}

impl JobRequest {
    /// Create a new job request with the given kind.
    pub fn new(kind: u16) -> Result<Self, Nip90Error> {
        if !is_job_request_kind(kind) {
            return Err(Nip90Error::InvalidKind(kind, "5000-5999".to_string()));
        }

        Ok(Self {
            kind,
            inputs: Vec::new(),
            output: None,
            params: Vec::new(),
            bid: None,
            relays: Vec::new(),
            service_providers: Vec::new(),
            encrypted: false,
            content: String::new(),
        })
    }

    /// Add an input to the job request.
    pub fn add_input(mut self, input: JobInput) -> Self {
        self.inputs.push(input);
        self
    }

    /// Set the expected output format.
    pub fn with_output(mut self, mime_type: impl Into<String>) -> Self {
        self.output = Some(mime_type.into());
        self
    }

    /// Add a parameter to the job request.
    pub fn add_param(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.params.push(JobParam::new(key, value));
        self
    }

    /// Set the maximum bid in millisats.
    pub fn with_bid(mut self, millisats: u64) -> Self {
        self.bid = Some(millisats);
        self
    }

    /// Add a relay for responses.
    pub fn add_relay(mut self, relay: impl Into<String>) -> Self {
        self.relays.push(relay.into());
        self
    }

    /// Add a preferred service provider.
    pub fn add_service_provider(mut self, pubkey: impl Into<String>) -> Self {
        self.service_providers.push(pubkey.into());
        self
    }

    /// Convert to tags for event creation.
    pub fn to_tags(&self) -> Vec<Vec<String>> {
        let mut tags = Vec::new();

        // Add inputs
        for input in &self.inputs {
            tags.push(input.to_tag());
        }

        // Add output
        if let Some(output) = &self.output {
            tags.push(vec!["output".to_string(), output.clone()]);
        }

        // Add params
        for param in &self.params {
            tags.push(param.to_tag());
        }

        // Add bid
        if let Some(bid) = self.bid {
            tags.push(vec!["bid".to_string(), bid.to_string()]);
        }

        // Add relays
        if !self.relays.is_empty() {
            let mut relay_tag = vec!["relays".to_string()];
            relay_tag.extend(self.relays.clone());
            tags.push(relay_tag);
        }

        // Add service providers
        for sp in &self.service_providers {
            tags.push(vec!["p".to_string(), sp.clone()]);
        }

        // Add encrypted tag if needed
        if self.encrypted {
            tags.push(vec!["encrypted".to_string()]);
        }

        tags
    }

    /// Parse a JobRequest from a Nostr event.
    ///
    /// This extracts all NIP-90 tags from the event and constructs a JobRequest.
    pub fn from_event(event: &Event) -> Result<Self, Nip90Error> {
        if !is_job_request_kind(event.kind) {
            return Err(Nip90Error::InvalidKind(event.kind, "5000-5999".to_string()));
        }

        let mut request = Self::new(event.kind)?;
        request.content = event.content.clone();

        for tag in &event.tags {
            if tag.is_empty() {
                continue;
            }
            match tag[0].as_str() {
                "i" if tag.len() >= 3 => {
                    let input_type = InputType::from_str(&tag[2])?;
                    let relay = tag.get(3).cloned();
                    let marker = tag.get(4).cloned();
                    request.inputs.push(JobInput {
                        data: tag[1].clone(),
                        input_type,
                        relay,
                        marker,
                    });
                }
                "output" if tag.len() >= 2 => {
                    request.output = Some(tag[1].clone());
                }
                "param" if tag.len() >= 3 => {
                    request.params.push(JobParam {
                        key: tag[1].clone(),
                        value: tag[2].clone(),
                    });
                }
                "bid" if tag.len() >= 2 => {
                    request.bid = tag[1].parse().ok();
                }
                "relays" => {
                    request.relays.extend(tag[1..].iter().cloned());
                }
                "p" if tag.len() >= 2 => {
                    request.service_providers.push(tag[1].clone());
                }
                "encrypted" => {
                    request.encrypted = true;
                }
                _ => {}
            }
        }

        Ok(request)
    }

    /// Get the corresponding result kind for this request.
    pub fn result_kind(&self) -> u16 {
        self.kind + 1000
    }
}

/// A job result event data (kind 6000-6999).
///
/// # Examples
///
/// ```
/// use nostr::nip90::{JobResult, KIND_JOB_TEXT_GENERATION};
///
/// # fn example() -> Result<(), nostr::nip90::Nip90Error> {
/// // Create a result for a text generation job
/// let result = JobResult::new(
///     KIND_JOB_TEXT_GENERATION,
///     "request_event_id_abc123",
///     "customer_pubkey_xyz",
///     "Nostr flows free,\nDecentralized thoughts connect,\nSovereign and true.",
/// )?
/// .with_amount(1000, Some("lnbc1000n1...".to_string()));
///
/// assert_eq!(result.kind, KIND_JOB_TEXT_GENERATION + 1000);
/// assert_eq!(result.amount, Some(1000));
/// assert!(result.bolt11.is_some());
///
/// // Convert to tags for publishing
/// let tags = result.to_tags();
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct JobResult {
    /// The result kind (6000-6999, = request kind + 1000)
    pub kind: u16,
    /// The result payload
    pub content: String,
    /// The original job request (stringified JSON)
    pub request: Option<String>,
    /// The job request event ID
    pub request_id: String,
    /// Relay hint for the request
    pub request_relay: Option<String>,
    /// Original inputs from the request
    pub inputs: Vec<JobInput>,
    /// Customer's pubkey
    pub customer_pubkey: String,
    /// Amount requested in millisats
    pub amount: Option<u64>,
    /// Optional bolt11 invoice
    pub bolt11: Option<String>,
    /// Whether output is encrypted
    pub encrypted: bool,
}

impl JobResult {
    /// Create a new job result.
    pub fn new(
        request_kind: u16,
        request_id: impl Into<String>,
        customer_pubkey: impl Into<String>,
        content: impl Into<String>,
    ) -> Result<Self, Nip90Error> {
        if !is_job_request_kind(request_kind) {
            return Err(Nip90Error::InvalidKind(
                request_kind,
                "5000-5999".to_string(),
            ));
        }

        Ok(Self {
            kind: request_kind + 1000,
            content: content.into(),
            request: None,
            request_id: request_id.into(),
            request_relay: None,
            inputs: Vec::new(),
            customer_pubkey: customer_pubkey.into(),
            amount: None,
            bolt11: None,
            encrypted: false,
        })
    }

    /// Set the original request JSON.
    pub fn with_request(mut self, request_json: impl Into<String>) -> Self {
        self.request = Some(request_json.into());
        self
    }

    /// Set the request relay hint.
    pub fn with_request_relay(mut self, relay: impl Into<String>) -> Self {
        self.request_relay = Some(relay.into());
        self
    }

    /// Add an original input.
    pub fn add_input(mut self, input: JobInput) -> Self {
        self.inputs.push(input);
        self
    }

    /// Set the payment amount and optional bolt11.
    pub fn with_amount(mut self, millisats: u64, bolt11: Option<String>) -> Self {
        self.amount = Some(millisats);
        self.bolt11 = bolt11;
        self
    }

    /// Convert to tags for event creation.
    pub fn to_tags(&self) -> Vec<Vec<String>> {
        let mut tags = Vec::new();

        // Add request JSON
        if let Some(request) = &self.request {
            tags.push(vec!["request".to_string(), request.clone()]);
        }

        // Add event reference
        let mut e_tag = vec!["e".to_string(), self.request_id.clone()];
        if let Some(relay) = &self.request_relay {
            e_tag.push(relay.clone());
        }
        tags.push(e_tag);

        // Add inputs
        for input in &self.inputs {
            tags.push(input.to_tag());
        }

        // Add customer pubkey
        tags.push(vec!["p".to_string(), self.customer_pubkey.clone()]);

        // Add amount
        if let Some(amount) = self.amount {
            let mut amount_tag = vec!["amount".to_string(), amount.to_string()];
            if let Some(bolt11) = &self.bolt11 {
                amount_tag.push(bolt11.clone());
            }
            tags.push(amount_tag);
        }

        // Add status tag (default: success)
        tags.push(vec!["status".to_string(), "success".to_string()]);

        // Add encrypted tag if needed
        if self.encrypted {
            tags.push(vec!["encrypted".to_string()]);
        }

        tags
    }

    /// Parse a JobResult from an event.
    pub fn from_event(event: &Event) -> Result<Self, Nip90Error> {
        if !is_job_result_kind(event.kind) {
            return Err(Nip90Error::InvalidKind(event.kind, "6000-6999".to_string()));
        }

        let mut request_id = String::new();
        let mut customer_pubkey = String::new();
        let mut request_relay = None;
        let mut inputs = Vec::new();
        let mut amount = None;
        let mut bolt11 = None;
        let mut encrypted = false;
        let mut request = None;

        for tag in &event.tags {
            if tag.is_empty() {
                continue;
            }
            match tag[0].as_str() {
                "e" if tag.len() >= 2 => {
                    request_id = tag[1].clone();
                    if tag.len() >= 3 {
                        request_relay = Some(tag[2].clone());
                    }
                }
                "p" if tag.len() >= 2 => {
                    customer_pubkey = tag[1].clone();
                }
                "i" if tag.len() >= 3 => {
                    let input_type = InputType::from_str(&tag[2])?;
                    let relay = tag.get(3).cloned();
                    let marker = tag.get(4).cloned();
                    inputs.push(JobInput {
                        data: tag[1].clone(),
                        input_type,
                        relay,
                        marker,
                    });
                }
                "amount" if tag.len() >= 2 => {
                    amount = tag[1].parse().ok();
                    if tag.len() >= 3 {
                        bolt11 = Some(tag[2].clone());
                    }
                }
                "bolt11" if tag.len() >= 2 => {
                    bolt11 = Some(tag[1].clone());
                }
                "encrypted" => {
                    encrypted = true;
                }
                "request" if tag.len() >= 2 => {
                    request = Some(tag[1].clone());
                }
                _ => {}
            }
        }

        if request_id.is_empty() {
            return Err(Nip90Error::MissingTag("e (request event id)".to_string()));
        }
        if customer_pubkey.is_empty() {
            return Err(Nip90Error::MissingTag("p (customer pubkey)".to_string()));
        }

        Ok(Self {
            kind: event.kind,
            content: event.content.clone(),
            request,
            request_id,
            request_relay,
            inputs,
            customer_pubkey,
            amount,
            bolt11,
            encrypted,
        })
    }
}

/// A job feedback event data (kind 7000).
///
/// # Examples
///
/// ```
/// use nostr::nip90::{JobFeedback, JobStatus};
///
/// // Send processing status update
/// let feedback = JobFeedback::new(
///     JobStatus::Processing,
///     "request_event_id_abc123",
///     "customer_pubkey_xyz",
/// )
/// .with_status_extra("Model loaded, generating response...")
/// .with_content("Partial result: Nostr flows...");
///
/// assert_eq!(feedback.status, JobStatus::Processing);
/// assert!(feedback.status_extra.is_some());
///
/// // Request payment before proceeding
/// let payment_request = JobFeedback::new(
///     JobStatus::PaymentRequired,
///     "request_event_id_abc123",
///     "customer_pubkey_xyz",
/// )
/// .with_amount(5000, Some("lnbc5000n1...".to_string()));
///
/// assert_eq!(payment_request.status, JobStatus::PaymentRequired);
/// assert_eq!(payment_request.amount, Some(5000));
///
/// // Convert to tags for publishing
/// let tags = feedback.to_tags();
/// ```
#[derive(Debug, Clone)]
pub struct JobFeedback {
    /// The feedback status
    pub status: JobStatus,
    /// Extra info about the status
    pub status_extra: Option<String>,
    /// The job request event ID
    pub request_id: String,
    /// Relay hint for the request
    pub request_relay: Option<String>,
    /// Customer's pubkey
    pub customer_pubkey: String,
    /// Optional content (partial results, etc.)
    pub content: String,
    /// Amount requested in millisats
    pub amount: Option<u64>,
    /// Optional bolt11 invoice
    pub bolt11: Option<String>,
}

impl JobFeedback {
    /// Create a new job feedback.
    pub fn new(
        status: JobStatus,
        request_id: impl Into<String>,
        customer_pubkey: impl Into<String>,
    ) -> Self {
        Self {
            status,
            status_extra: None,
            request_id: request_id.into(),
            request_relay: None,
            customer_pubkey: customer_pubkey.into(),
            content: String::new(),
            amount: None,
            bolt11: None,
        }
    }

    /// Set extra status info.
    pub fn with_status_extra(mut self, extra: impl Into<String>) -> Self {
        self.status_extra = Some(extra.into());
        self
    }

    /// Set the request relay hint.
    pub fn with_request_relay(mut self, relay: impl Into<String>) -> Self {
        self.request_relay = Some(relay.into());
        self
    }

    /// Set content (e.g., partial results).
    pub fn with_content(mut self, content: impl Into<String>) -> Self {
        self.content = content.into();
        self
    }

    /// Set the payment amount and optional bolt11.
    pub fn with_amount(mut self, millisats: u64, bolt11: Option<String>) -> Self {
        self.amount = Some(millisats);
        self.bolt11 = bolt11;
        self
    }

    /// Convert to tags for event creation.
    pub fn to_tags(&self) -> Vec<Vec<String>> {
        let mut tags = Vec::new();

        // Add status
        let mut status_tag = vec!["status".to_string(), self.status.as_str().to_string()];
        if let Some(extra) = &self.status_extra {
            status_tag.push(extra.clone());
        }
        tags.push(status_tag);

        // Add event reference
        let mut e_tag = vec!["e".to_string(), self.request_id.clone()];
        if let Some(relay) = &self.request_relay {
            e_tag.push(relay.clone());
        }
        tags.push(e_tag);

        // Add customer pubkey
        tags.push(vec!["p".to_string(), self.customer_pubkey.clone()]);

        // Add amount
        if let Some(amount) = self.amount {
            let mut amount_tag = vec!["amount".to_string(), amount.to_string()];
            if let Some(bolt11) = &self.bolt11 {
                amount_tag.push(bolt11.clone());
            }
            tags.push(amount_tag);
        }

        tags
    }
}

/// Check if a kind is a job request kind (5000-5999).
pub fn is_job_request_kind(kind: u16) -> bool {
    (JOB_REQUEST_KIND_MIN..=JOB_REQUEST_KIND_MAX).contains(&kind)
}

/// Check if a kind is a job result kind (6000-6999).
pub fn is_job_result_kind(kind: u16) -> bool {
    (JOB_RESULT_KIND_MIN..=JOB_RESULT_KIND_MAX).contains(&kind)
}

/// Check if a kind is a job feedback kind (7000).
pub fn is_job_feedback_kind(kind: u16) -> bool {
    kind == KIND_JOB_FEEDBACK
}

/// Check if a kind is any DVM-related kind (5000-7000).
pub fn is_dvm_kind(kind: u16) -> bool {
    is_job_request_kind(kind) || is_job_result_kind(kind) || is_job_feedback_kind(kind)
}

/// Get the result kind for a given request kind.
pub fn get_result_kind(request_kind: u16) -> Option<u16> {
    if is_job_request_kind(request_kind) {
        Some(request_kind + 1000)
    } else {
        None
    }
}

/// Get the request kind for a given result kind.
pub fn get_request_kind(result_kind: u16) -> Option<u16> {
    if is_job_result_kind(result_kind) {
        Some(result_kind - 1000)
    } else {
        None
    }
}

/// Create an EventTemplate for a job request.
///
/// This is a convenience function that converts a JobRequest into an EventTemplate
/// ready to be signed and published. The content field comes from the request's
/// content (empty for non-encrypted requests).
///
/// # Example
///
/// ```
/// use nostr::nip90::{JobRequest, JobInput, create_job_request_event, KIND_JOB_TEXT_GENERATION};
///
/// # fn example() -> Result<(), nostr::nip90::Nip90Error> {
/// let request = JobRequest::new(KIND_JOB_TEXT_GENERATION)?
///     .add_input(JobInput::text("Write a haiku about Nostr"))
///     .with_bid(1000);
///
/// let event_template = create_job_request_event(&request);
/// assert_eq!(event_template.kind, KIND_JOB_TEXT_GENERATION);
/// # Ok(())
/// # }
/// ```
pub fn create_job_request_event(request: &JobRequest) -> crate::nip01::EventTemplate {
    crate::nip01::EventTemplate {
        kind: request.kind,
        tags: request.to_tags(),
        content: request.content.clone(),
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    }
}

/// Create an EventTemplate for a job result.
///
/// This converts a JobResult into an EventTemplate ready to be signed and published.
/// The result kind is automatically calculated from the request kind.
///
/// # Example
///
/// ```
/// use nostr::nip90::{JobResult, create_job_result_event, KIND_JOB_TEXT_GENERATION};
///
/// # fn example() -> Result<(), nostr::nip90::Nip90Error> {
/// let result = JobResult::new(
///     KIND_JOB_TEXT_GENERATION,
///     "request_event_id",
///     "customer_pubkey",
///     "Nostr flows free,\nDecentralized thoughts connect,\nSovereign and true.",
/// )?;
///
/// let event_template = create_job_result_event(&result);
/// assert_eq!(event_template.kind, KIND_JOB_TEXT_GENERATION + 1000);
/// # Ok(())
/// # }
/// ```
pub fn create_job_result_event(result: &JobResult) -> crate::nip01::EventTemplate {
    crate::nip01::EventTemplate {
        kind: result.kind,
        tags: result.to_tags(),
        content: result.content.clone(),
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    }
}

/// Create an EventTemplate for job feedback.
///
/// This converts a JobFeedback into an EventTemplate ready to be signed and published.
/// Feedback events use kind 7000.
///
/// # Example
///
/// ```
/// use nostr::nip90::{JobFeedback, JobStatus, create_job_feedback_event};
///
/// # fn example() -> Result<(), nostr::nip90::Nip90Error> {
/// let feedback = JobFeedback::new("request_event_id", "customer_pubkey", JobStatus::Processing)?;
///
/// let event_template = create_job_feedback_event(&feedback);
/// assert_eq!(event_template.kind, 7000);
/// # Ok(())
/// # }
/// ```
pub fn create_job_feedback_event(feedback: &JobFeedback) -> crate::nip01::EventTemplate {
    crate::nip01::EventTemplate {
        kind: KIND_JOB_FEEDBACK,
        tags: feedback.to_tags(),
        content: feedback.content.clone(),
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // Kind validation tests
    // =========================================================================

    #[test]
    fn test_is_job_request_kind() {
        assert!(is_job_request_kind(5000));
        assert!(is_job_request_kind(5001));
        assert!(is_job_request_kind(5500));
        assert!(is_job_request_kind(5999));

        assert!(!is_job_request_kind(4999));
        assert!(!is_job_request_kind(6000));
        assert!(!is_job_request_kind(7000));
    }

    #[test]
    fn test_is_job_result_kind() {
        assert!(is_job_result_kind(6000));
        assert!(is_job_result_kind(6001));
        assert!(is_job_result_kind(6500));
        assert!(is_job_result_kind(6999));

        assert!(!is_job_result_kind(5999));
        assert!(!is_job_result_kind(7000));
        assert!(!is_job_result_kind(7001));
    }

    #[test]
    fn test_is_job_feedback_kind() {
        assert!(is_job_feedback_kind(7000));

        assert!(!is_job_feedback_kind(6999));
        assert!(!is_job_feedback_kind(7001));
    }

    #[test]
    fn test_is_dvm_kind() {
        // Job requests
        assert!(is_dvm_kind(5000));
        assert!(is_dvm_kind(5999));

        // Job results
        assert!(is_dvm_kind(6000));
        assert!(is_dvm_kind(6999));

        // Job feedback
        assert!(is_dvm_kind(7000));

        // Not DVM
        assert!(!is_dvm_kind(4999));
        assert!(!is_dvm_kind(7001));
        assert!(!is_dvm_kind(1));
    }

    #[test]
    fn test_get_result_kind() {
        assert_eq!(get_result_kind(5000), Some(6000));
        assert_eq!(get_result_kind(5001), Some(6001));
        assert_eq!(get_result_kind(5999), Some(6999));

        assert_eq!(get_result_kind(4999), None);
        assert_eq!(get_result_kind(6000), None);
    }

    #[test]
    fn test_get_request_kind() {
        assert_eq!(get_request_kind(6000), Some(5000));
        assert_eq!(get_request_kind(6001), Some(5001));
        assert_eq!(get_request_kind(6999), Some(5999));

        assert_eq!(get_request_kind(5999), None);
        assert_eq!(get_request_kind(7000), None);
    }

    // =========================================================================
    // InputType tests
    // =========================================================================

    #[test]
    fn test_input_type_as_str() {
        assert_eq!(InputType::Url.as_str(), "url");
        assert_eq!(InputType::Event.as_str(), "event");
        assert_eq!(InputType::Job.as_str(), "job");
        assert_eq!(InputType::Text.as_str(), "text");
    }

    #[test]
    fn test_input_type_from_str() {
        assert_eq!(InputType::from_str("url").unwrap(), InputType::Url);
        assert_eq!(InputType::from_str("event").unwrap(), InputType::Event);
        assert_eq!(InputType::from_str("job").unwrap(), InputType::Job);
        assert_eq!(InputType::from_str("text").unwrap(), InputType::Text);

        // Case insensitive
        assert_eq!(InputType::from_str("URL").unwrap(), InputType::Url);
        assert_eq!(InputType::from_str("Text").unwrap(), InputType::Text);

        // Invalid
        assert!(InputType::from_str("invalid").is_err());
    }

    // =========================================================================
    // JobStatus tests
    // =========================================================================

    #[test]
    fn test_job_status_as_str() {
        assert_eq!(JobStatus::PaymentRequired.as_str(), "payment-required");
        assert_eq!(JobStatus::Processing.as_str(), "processing");
        assert_eq!(JobStatus::Error.as_str(), "error");
        assert_eq!(JobStatus::Success.as_str(), "success");
        assert_eq!(JobStatus::Partial.as_str(), "partial");
    }

    #[test]
    fn test_job_status_from_str() {
        assert_eq!(
            JobStatus::from_str("payment-required").unwrap(),
            JobStatus::PaymentRequired
        );
        assert_eq!(
            JobStatus::from_str("processing").unwrap(),
            JobStatus::Processing
        );
        assert_eq!(JobStatus::from_str("error").unwrap(), JobStatus::Error);
        assert_eq!(JobStatus::from_str("success").unwrap(), JobStatus::Success);
        assert_eq!(JobStatus::from_str("partial").unwrap(), JobStatus::Partial);

        // Invalid
        assert!(JobStatus::from_str("invalid").is_err());
    }

    // =========================================================================
    // JobInput tests
    // =========================================================================

    #[test]
    fn test_job_input_text() {
        let input = JobInput::text("What is the capital of France?");
        assert_eq!(input.data, "What is the capital of France?");
        assert_eq!(input.input_type, InputType::Text);
        assert!(input.relay.is_none());
        assert!(input.marker.is_none());
    }

    #[test]
    fn test_job_input_url() {
        let input = JobInput::url("https://example.com/audio.mp3");
        assert_eq!(input.data, "https://example.com/audio.mp3");
        assert_eq!(input.input_type, InputType::Url);
    }

    #[test]
    fn test_job_input_event() {
        let input = JobInput::event("abc123", Some("wss://relay.example.com".to_string()));
        assert_eq!(input.data, "abc123");
        assert_eq!(input.input_type, InputType::Event);
        assert_eq!(input.relay, Some("wss://relay.example.com".to_string()));
    }

    #[test]
    fn test_job_input_job() {
        let input = JobInput::job("def456", Some("wss://relay.example.com".to_string()));
        assert_eq!(input.data, "def456");
        assert_eq!(input.input_type, InputType::Job);
    }

    #[test]
    fn test_job_input_with_marker() {
        let input = JobInput::text("some text").with_marker("source");
        assert_eq!(input.marker, Some("source".to_string()));
    }

    #[test]
    fn test_job_input_to_tag() {
        let input = JobInput::text("Hello");
        let tag = input.to_tag();
        assert_eq!(tag, vec!["i", "Hello", "text"]);

        let input = JobInput::url("https://example.com").with_marker("audio");
        let tag = input.to_tag();
        assert_eq!(tag, vec!["i", "https://example.com", "url", "", "audio"]);

        let input = JobInput::event("abc123", Some("wss://relay.com".to_string()));
        let tag = input.to_tag();
        assert_eq!(tag, vec!["i", "abc123", "event", "wss://relay.com"]);
    }

    #[test]
    fn test_job_input_from_tag() {
        let tag = vec!["i".to_string(), "Hello".to_string(), "text".to_string()];
        let input = JobInput::from_tag(&tag).unwrap();
        assert_eq!(input.data, "Hello");
        assert_eq!(input.input_type, InputType::Text);

        let tag = vec![
            "i".to_string(),
            "abc123".to_string(),
            "event".to_string(),
            "wss://relay.com".to_string(),
            "source".to_string(),
        ];
        let input = JobInput::from_tag(&tag).unwrap();
        assert_eq!(input.data, "abc123");
        assert_eq!(input.input_type, InputType::Event);
        assert_eq!(input.relay, Some("wss://relay.com".to_string()));
        assert_eq!(input.marker, Some("source".to_string()));
    }

    // =========================================================================
    // JobParam tests
    // =========================================================================

    #[test]
    fn test_job_param() {
        let param = JobParam::new("model", "gpt-4");
        assert_eq!(param.key, "model");
        assert_eq!(param.value, "gpt-4");
    }

    #[test]
    fn test_job_param_to_tag() {
        let param = JobParam::new("temperature", "0.7");
        let tag = param.to_tag();
        assert_eq!(tag, vec!["param", "temperature", "0.7"]);
    }

    #[test]
    fn test_job_param_from_tag() {
        let tag = vec![
            "param".to_string(),
            "max_tokens".to_string(),
            "512".to_string(),
        ];
        let param = JobParam::from_tag(&tag).unwrap();
        assert_eq!(param.key, "max_tokens");
        assert_eq!(param.value, "512");
    }

    // =========================================================================
    // JobRequest tests
    // =========================================================================

    #[test]
    fn test_job_request_new() {
        let request = JobRequest::new(5001).unwrap();
        assert_eq!(request.kind, 5001);
        assert!(request.inputs.is_empty());
        assert!(request.params.is_empty());
    }

    #[test]
    fn test_job_request_invalid_kind() {
        assert!(JobRequest::new(4999).is_err());
        assert!(JobRequest::new(6000).is_err());
    }

    #[test]
    fn test_job_request_builder() {
        let request = JobRequest::new(KIND_JOB_TEXT_GENERATION)
            .unwrap()
            .add_input(JobInput::text("What is the capital of France?"))
            .with_output("text/plain")
            .add_param("model", "gpt-4")
            .add_param("temperature", "0.7")
            .with_bid(1000)
            .add_relay("wss://relay.example.com")
            .add_service_provider("abc123");

        assert_eq!(request.kind, 5050);
        assert_eq!(request.inputs.len(), 1);
        assert_eq!(request.output, Some("text/plain".to_string()));
        assert_eq!(request.params.len(), 2);
        assert_eq!(request.bid, Some(1000));
        assert_eq!(request.relays, vec!["wss://relay.example.com"]);
        assert_eq!(request.service_providers, vec!["abc123"]);
    }

    #[test]
    fn test_job_request_result_kind() {
        let request = JobRequest::new(5001).unwrap();
        assert_eq!(request.result_kind(), 6001);

        let request = JobRequest::new(5050).unwrap();
        assert_eq!(request.result_kind(), 6050);
    }

    #[test]
    fn test_job_request_to_tags() {
        let request = JobRequest::new(5001)
            .unwrap()
            .add_input(JobInput::text("Hello"))
            .with_output("text/plain")
            .add_param("lang", "es")
            .with_bid(5000)
            .add_relay("wss://relay1.com")
            .add_relay("wss://relay2.com");

        let tags = request.to_tags();

        assert!(
            tags.iter()
                .any(|t| t[0] == "i" && t[1] == "Hello" && t[2] == "text")
        );
        assert!(
            tags.iter()
                .any(|t| t[0] == "output" && t[1] == "text/plain")
        );
        assert!(
            tags.iter()
                .any(|t| t[0] == "param" && t[1] == "lang" && t[2] == "es")
        );
        assert!(tags.iter().any(|t| t[0] == "bid" && t[1] == "5000"));
        assert!(
            tags.iter()
                .any(|t| t[0] == "relays" && t.contains(&"wss://relay1.com".to_string()))
        );
    }

    // =========================================================================
    // JobResult tests
    // =========================================================================

    #[test]
    fn test_job_result_new() {
        let result =
            JobResult::new(5001, "request123", "customer456", "The capital is Paris.").unwrap();

        assert_eq!(result.kind, 6001);
        assert_eq!(result.request_id, "request123");
        assert_eq!(result.customer_pubkey, "customer456");
        assert_eq!(result.content, "The capital is Paris.");
    }

    #[test]
    fn test_job_result_builder() {
        let result = JobResult::new(5001, "req123", "cust456", "Result content")
            .unwrap()
            .with_request(r#"{"kind":5001}"#)
            .with_request_relay("wss://relay.com")
            .add_input(JobInput::text("Original input"))
            .with_amount(1000, Some("lnbc...".to_string()));

        assert_eq!(result.request, Some(r#"{"kind":5001}"#.to_string()));
        assert_eq!(result.request_relay, Some("wss://relay.com".to_string()));
        assert_eq!(result.inputs.len(), 1);
        assert_eq!(result.amount, Some(1000));
        assert_eq!(result.bolt11, Some("lnbc...".to_string()));
    }

    #[test]
    fn test_job_result_to_tags() {
        let result = JobResult::new(5001, "req123", "cust456", "Result")
            .unwrap()
            .with_request(r#"{"kind":5001}"#)
            .with_request_relay("wss://relay.com")
            .with_amount(1000, None);

        let tags = result.to_tags();

        assert!(tags.iter().any(|t| t[0] == "request"));
        assert!(
            tags.iter()
                .any(|t| t[0] == "e" && t[1] == "req123" && t[2] == "wss://relay.com")
        );
        assert!(tags.iter().any(|t| t[0] == "p" && t[1] == "cust456"));
        assert!(tags.iter().any(|t| t[0] == "amount" && t[1] == "1000"));
    }

    // =========================================================================
    // JobFeedback tests
    // =========================================================================

    #[test]
    fn test_job_feedback_new() {
        let feedback = JobFeedback::new(JobStatus::Processing, "req123", "cust456");

        assert_eq!(feedback.status, JobStatus::Processing);
        assert_eq!(feedback.request_id, "req123");
        assert_eq!(feedback.customer_pubkey, "cust456");
    }

    #[test]
    fn test_job_feedback_builder() {
        let feedback = JobFeedback::new(JobStatus::PaymentRequired, "req123", "cust456")
            .with_status_extra("Please pay to continue")
            .with_request_relay("wss://relay.com")
            .with_amount(5000, Some("lnbc...".to_string()));

        assert_eq!(
            feedback.status_extra,
            Some("Please pay to continue".to_string())
        );
        assert_eq!(feedback.request_relay, Some("wss://relay.com".to_string()));
        assert_eq!(feedback.amount, Some(5000));
        assert_eq!(feedback.bolt11, Some("lnbc...".to_string()));
    }

    #[test]
    fn test_job_feedback_partial_with_content() {
        let feedback = JobFeedback::new(JobStatus::Partial, "req123", "cust456")
            .with_content("Here's a sample of the results...");

        assert_eq!(feedback.status, JobStatus::Partial);
        assert_eq!(feedback.content, "Here's a sample of the results...");
    }

    #[test]
    fn test_job_feedback_to_tags() {
        let feedback = JobFeedback::new(JobStatus::Error, "req123", "cust456")
            .with_status_extra("Out of credits")
            .with_request_relay("wss://relay.com");

        let tags = feedback.to_tags();

        assert!(
            tags.iter()
                .any(|t| t[0] == "status" && t[1] == "error" && t[2] == "Out of credits")
        );
        assert!(tags.iter().any(|t| t[0] == "e" && t[1] == "req123"));
        assert!(tags.iter().any(|t| t[0] == "p" && t[1] == "cust456"));
    }

    // =========================================================================
    // Integration tests - DVM workflow
    // =========================================================================

    #[test]
    fn test_dvm_workflow_text_generation() {
        // 1. Customer creates a job request
        let request = JobRequest::new(KIND_JOB_TEXT_GENERATION)
            .unwrap()
            .add_input(JobInput::text("What is the capital of France?"))
            .add_param("model", "LLaMA-2")
            .add_param("max_tokens", "512")
            .with_bid(1000)
            .add_relay("wss://relay.damus.io");

        assert_eq!(request.kind, 5050);
        assert_eq!(request.result_kind(), 6050);

        // 2. Service provider sends processing feedback
        let feedback = JobFeedback::new(JobStatus::Processing, "req_event_id", "customer_pubkey");
        assert_eq!(feedback.status, JobStatus::Processing);

        // 3. Service provider sends result
        let result = JobResult::new(
            request.kind,
            "req_event_id",
            "customer_pubkey",
            "The capital of France is Paris.",
        )
        .unwrap()
        .with_amount(500, Some("lnbc500n1...".to_string()));

        assert_eq!(result.kind, 6050);
        assert_eq!(result.content, "The capital of France is Paris.");
        assert_eq!(result.amount, Some(500));
    }

    #[test]
    fn test_dvm_workflow_job_chaining() {
        // Job 1: Speech to text
        let job1 = JobRequest::new(KIND_JOB_SPEECH_TO_TEXT)
            .unwrap()
            .add_input(JobInput::url("https://example.com/podcast.mp3"));

        // Job 2: Summarization (uses output of job 1)
        let job2 = JobRequest::new(KIND_JOB_SUMMARIZATION)
            .unwrap()
            .add_input(JobInput::job(
                "job1_event_id",
                Some("wss://relay.com".to_string()),
            ));

        assert_eq!(job1.kind, 5250);
        assert_eq!(job2.kind, 5001);
        assert_eq!(job2.inputs[0].input_type, InputType::Job);
    }

    #[test]
    fn test_common_job_kinds() {
        // Verify common job kinds are in valid range
        assert!(is_job_request_kind(KIND_JOB_TEXT_EXTRACTION));
        assert!(is_job_request_kind(KIND_JOB_SUMMARIZATION));
        assert!(is_job_request_kind(KIND_JOB_TRANSLATION));
        assert!(is_job_request_kind(KIND_JOB_TEXT_GENERATION));
        assert!(is_job_request_kind(KIND_JOB_IMAGE_GENERATION));
        assert!(is_job_request_kind(KIND_JOB_SPEECH_TO_TEXT));

        // Verify corresponding result kinds
        assert_eq!(get_result_kind(KIND_JOB_TEXT_GENERATION), Some(6050));
        assert_eq!(get_result_kind(KIND_JOB_SPEECH_TO_TEXT), Some(6250));
    }

    // =========================================================================
    // Event builder helper tests
    // =========================================================================

    #[test]
    fn test_create_job_request_event() {
        let request = JobRequest::new(KIND_JOB_TEXT_GENERATION)
            .unwrap()
            .add_input(JobInput::text("Write a haiku"))
            .with_bid(1000);

        let event = create_job_request_event(&request);

        assert_eq!(event.kind, KIND_JOB_TEXT_GENERATION);
        assert!(event.tags.iter().any(|t| t[0] == "i"));
        assert!(event.tags.iter().any(|t| t[0] == "bid" && t[1] == "1000"));
        assert_eq!(event.content, "");
    }

    #[test]
    fn test_create_job_result_event() {
        let result = JobResult::new(
            KIND_JOB_TEXT_GENERATION,
            "request123",
            "customer456",
            "Nostr flows free",
        )
        .unwrap()
        .with_amount(1000, Some("lnbc...".to_string()));

        let event = create_job_result_event(&result);

        assert_eq!(event.kind, KIND_JOB_TEXT_GENERATION + 1000);
        assert!(event.tags.iter().any(|t| t[0] == "e" && t[1] == "request123"));
        assert!(event.tags.iter().any(|t| t[0] == "p" && t[1] == "customer456"));
        assert!(event.tags.iter().any(|t| t[0] == "amount" && t[1] == "1000"));
        assert_eq!(event.content, "Nostr flows free");
    }

    #[test]
    fn test_create_job_feedback_event() {
        let feedback = JobFeedback::new(JobStatus::Processing, "request123", "customer456")
            .with_status_extra("Working on it");

        let event = create_job_feedback_event(&feedback);

        assert_eq!(event.kind, KIND_JOB_FEEDBACK);
        assert!(event.tags.iter().any(|t| t[0] == "status" && t[1] == "processing"));
        assert!(event.tags.iter().any(|t| t[0] == "e" && t[1] == "request123"));
        assert!(event.tags.iter().any(|t| t[0] == "p" && t[1] == "customer456"));
        assert_eq!(event.content, "");
    }
}
