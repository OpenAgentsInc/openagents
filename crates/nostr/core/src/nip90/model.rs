use super::kinds::{is_job_request_kind, is_job_result_kind};
use crate::nip01::Event;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use thiserror::Error;
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
}

impl std::str::FromStr for InputType {
    type Err = Nip90Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
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
}

impl std::str::FromStr for JobStatus {
    type Err = Nip90Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
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
        request.content.clone_from(&event.content);

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
                    request_id.clone_from(&tag[1]);
                    if tag.len() >= 3 {
                        request_relay = Some(tag[2].clone());
                    }
                }
                "p" if tag.len() >= 2 => {
                    customer_pubkey.clone_from(&tag[1]);
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
