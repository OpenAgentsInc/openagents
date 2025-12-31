//! NIP-90 event types for the compute marketplace
//!
//! This module provides event types that wrap the core NIP-90 protocol
//! for use in the marketplace compute layer.

use nostr::{
    JobFeedback, JobInput, JobRequest, JobResult, JobStatus, KIND_JOB_IMAGE_GENERATION,
    KIND_JOB_SPEECH_TO_TEXT, KIND_JOB_SUMMARIZATION, KIND_JOB_TEXT_EXTRACTION,
    KIND_JOB_TEXT_GENERATION, KIND_JOB_TRANSLATION,
};
use serde::{Deserialize, Serialize};

/// Re-export core NIP-90 types from nostr crate
pub use nostr::{
    InputType, JobParam, Nip90Error, is_dvm_kind, is_job_feedback_kind, is_job_request_kind,
    is_job_result_kind,
};

/// Compute job request builder for marketplace
#[derive(Debug, Clone)]
pub struct ComputeJobRequest {
    inner: JobRequest,
}

impl ComputeJobRequest {
    /// Create a new text generation job
    pub fn text_generation(prompt: impl Into<String>) -> Result<Self, Nip90Error> {
        let mut req = JobRequest::new(KIND_JOB_TEXT_GENERATION)?;
        req = req.add_input(JobInput::text(prompt));
        Ok(Self { inner: req })
    }

    /// Create a new summarization job
    pub fn summarization(text: impl Into<String>) -> Result<Self, Nip90Error> {
        let mut req = JobRequest::new(KIND_JOB_SUMMARIZATION)?;
        req = req.add_input(JobInput::text(text));
        Ok(Self { inner: req })
    }

    /// Create a new translation job
    pub fn translation(
        text: impl Into<String>,
        target_lang: impl Into<String>,
    ) -> Result<Self, Nip90Error> {
        let mut req = JobRequest::new(KIND_JOB_TRANSLATION)?;
        req = req
            .add_input(JobInput::text(text))
            .add_param("target_lang", target_lang);
        Ok(Self { inner: req })
    }

    /// Create a new text extraction / OCR job
    pub fn text_extraction(url: impl Into<String>) -> Result<Self, Nip90Error> {
        let mut req = JobRequest::new(KIND_JOB_TEXT_EXTRACTION)?;
        req = req.add_input(JobInput::url(url));
        Ok(Self { inner: req })
    }

    /// Create a new image generation job
    pub fn image_generation(prompt: impl Into<String>) -> Result<Self, Nip90Error> {
        let mut req = JobRequest::new(KIND_JOB_IMAGE_GENERATION)?;
        req = req.add_input(JobInput::text(prompt));
        Ok(Self { inner: req })
    }

    /// Create a new speech-to-text job
    pub fn speech_to_text(audio_url: impl Into<String>) -> Result<Self, Nip90Error> {
        let mut req = JobRequest::new(KIND_JOB_SPEECH_TO_TEXT)?;
        req = req.add_input(JobInput::url(audio_url));
        Ok(Self { inner: req })
    }

    /// Create a custom job with the given kind
    pub fn custom(kind: u16) -> Result<Self, Nip90Error> {
        Ok(Self {
            inner: JobRequest::new(kind)?,
        })
    }

    /// Add a text input
    pub fn with_text_input(mut self, text: impl Into<String>) -> Self {
        self.inner = self.inner.add_input(JobInput::text(text));
        self
    }

    /// Add a URL input
    pub fn with_url_input(mut self, url: impl Into<String>) -> Self {
        self.inner = self.inner.add_input(JobInput::url(url));
        self
    }

    /// Add an event input
    pub fn with_event_input(mut self, event_id: impl Into<String>, relay: Option<String>) -> Self {
        self.inner = self.inner.add_input(JobInput::event(event_id, relay));
        self
    }

    /// Add a job input (chaining from previous job)
    pub fn with_job_input(mut self, job_id: impl Into<String>, relay: Option<String>) -> Self {
        self.inner = self.inner.add_input(JobInput::job(job_id, relay));
        self
    }

    /// Set the expected output format
    pub fn with_output(mut self, mime_type: impl Into<String>) -> Self {
        self.inner = self.inner.with_output(mime_type);
        self
    }

    /// Add a parameter
    pub fn with_param(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.inner = self.inner.add_param(key, value);
        self
    }

    /// Set the maximum bid in millisats
    pub fn with_bid(mut self, millisats: u64) -> Self {
        self.inner = self.inner.with_bid(millisats);
        self
    }

    /// Add a relay for responses
    pub fn with_relay(mut self, relay: impl Into<String>) -> Self {
        self.inner = self.inner.add_relay(relay);
        self
    }

    /// Add a preferred service provider
    pub fn with_provider(mut self, pubkey: impl Into<String>) -> Self {
        self.inner = self.inner.add_service_provider(pubkey);
        self
    }

    /// Common LLM parameters
    /// Set the model parameter
    pub fn with_model(self, model: impl Into<String>) -> Self {
        self.with_param("model", model)
    }

    /// Set the max_tokens parameter
    pub fn with_max_tokens(self, tokens: u64) -> Self {
        self.with_param("max_tokens", tokens.to_string())
    }

    /// Set the temperature parameter
    pub fn with_temperature(self, temp: f32) -> Self {
        self.with_param("temperature", temp.to_string())
    }

    /// Set the top_p parameter
    pub fn with_top_p(self, top_p: f32) -> Self {
        self.with_param("top_p", top_p.to_string())
    }

    /// Set the top_k parameter
    pub fn with_top_k(self, top_k: u32) -> Self {
        self.with_param("top_k", top_k.to_string())
    }

    /// Set the frequency_penalty parameter
    pub fn with_frequency_penalty(self, penalty: f32) -> Self {
        self.with_param("frequency_penalty", penalty.to_string())
    }

    /// Set the presence_penalty parameter
    pub fn with_presence_penalty(self, penalty: f32) -> Self {
        self.with_param("presence_penalty", penalty.to_string())
    }

    /// Get the inner JobRequest
    pub fn into_inner(self) -> JobRequest {
        self.inner
    }

    /// Get a reference to the inner JobRequest
    pub fn inner(&self) -> &JobRequest {
        &self.inner
    }

    /// Get the job kind
    pub fn kind(&self) -> u16 {
        self.inner.kind
    }

    /// Get the expected result kind
    pub fn result_kind(&self) -> u16 {
        self.inner.result_kind()
    }
}

/// Compute job result builder for marketplace
#[derive(Debug, Clone)]
pub struct ComputeJobResult {
    inner: JobResult,
}

impl ComputeJobResult {
    /// Create a new job result
    pub fn new(
        request_kind: u16,
        request_id: impl Into<String>,
        customer_pubkey: impl Into<String>,
        content: impl Into<String>,
    ) -> Result<Self, Nip90Error> {
        Ok(Self {
            inner: JobResult::new(request_kind, request_id, customer_pubkey, content)?,
        })
    }

    /// Set the original request JSON
    pub fn with_request(mut self, request_json: impl Into<String>) -> Self {
        self.inner = self.inner.with_request(request_json);
        self
    }

    /// Set the request relay hint
    pub fn with_request_relay(mut self, relay: impl Into<String>) -> Self {
        self.inner = self.inner.with_request_relay(relay);
        self
    }

    /// Add an original input
    pub fn with_input(mut self, input: JobInput) -> Self {
        self.inner = self.inner.add_input(input);
        self
    }

    /// Set the payment amount and optional bolt11
    pub fn with_payment(mut self, millisats: u64, bolt11: Option<String>) -> Self {
        self.inner = self.inner.with_amount(millisats, bolt11);
        self
    }

    /// Get the inner JobResult
    pub fn into_inner(self) -> JobResult {
        self.inner
    }

    /// Get a reference to the inner JobResult
    pub fn inner(&self) -> &JobResult {
        &self.inner
    }
}

/// Compute job feedback builder for marketplace
#[derive(Debug, Clone)]
pub struct ComputeJobFeedback {
    inner: JobFeedback,
}

impl ComputeJobFeedback {
    /// Create payment required feedback
    pub fn payment_required(
        request_id: impl Into<String>,
        customer_pubkey: impl Into<String>,
        amount_msats: u64,
        bolt11: Option<String>,
    ) -> Self {
        Self {
            inner: JobFeedback::new(JobStatus::PaymentRequired, request_id, customer_pubkey)
                .with_amount(amount_msats, bolt11),
        }
    }

    /// Create processing feedback
    pub fn processing(request_id: impl Into<String>, customer_pubkey: impl Into<String>) -> Self {
        Self {
            inner: JobFeedback::new(JobStatus::Processing, request_id, customer_pubkey),
        }
    }

    /// Create error feedback
    pub fn error(
        request_id: impl Into<String>,
        customer_pubkey: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            inner: JobFeedback::new(JobStatus::Error, request_id, customer_pubkey)
                .with_status_extra(message),
        }
    }

    /// Create success feedback
    pub fn success(request_id: impl Into<String>, customer_pubkey: impl Into<String>) -> Self {
        Self {
            inner: JobFeedback::new(JobStatus::Success, request_id, customer_pubkey),
        }
    }

    /// Create partial feedback with sample results
    pub fn partial(
        request_id: impl Into<String>,
        customer_pubkey: impl Into<String>,
        sample_content: impl Into<String>,
    ) -> Self {
        Self {
            inner: JobFeedback::new(JobStatus::Partial, request_id, customer_pubkey)
                .with_content(sample_content),
        }
    }

    /// Set extra status information
    pub fn with_extra(mut self, extra: impl Into<String>) -> Self {
        self.inner = self.inner.with_status_extra(extra);
        self
    }

    /// Set the request relay hint
    pub fn with_relay(mut self, relay: impl Into<String>) -> Self {
        self.inner = self.inner.with_request_relay(relay);
        self
    }

    /// Get the inner JobFeedback
    pub fn into_inner(self) -> JobFeedback {
        self.inner
    }

    /// Get a reference to the inner JobFeedback
    pub fn inner(&self) -> &JobFeedback {
        &self.inner
    }
}

/// Helper type for job kind categories
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum JobCategory {
    TextProcessing,
    ImageGeneration,
    AudioProcessing,
    Custom(u16),
}

impl JobCategory {
    /// Get the category for a job kind
    pub fn from_kind(kind: u16) -> Self {
        match kind {
            KIND_JOB_TEXT_EXTRACTION
            | KIND_JOB_SUMMARIZATION
            | KIND_JOB_TRANSLATION
            | KIND_JOB_TEXT_GENERATION => JobCategory::TextProcessing,
            KIND_JOB_IMAGE_GENERATION => JobCategory::ImageGeneration,
            KIND_JOB_SPEECH_TO_TEXT => JobCategory::AudioProcessing,
            _ => JobCategory::Custom(kind),
        }
    }

    /// Get a human-readable name for the category
    pub fn name(&self) -> &'static str {
        match self {
            JobCategory::TextProcessing => "Text Processing",
            JobCategory::ImageGeneration => "Image Generation",
            JobCategory::AudioProcessing => "Audio Processing",
            JobCategory::Custom(_) => "Custom",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text_generation_request() {
        let req = ComputeJobRequest::text_generation("What is Bitcoin?")
            .unwrap()
            .with_model("gpt-4")
            .with_max_tokens(500)
            .with_temperature(0.7)
            .with_bid(1000);

        assert_eq!(req.kind(), KIND_JOB_TEXT_GENERATION);
        assert_eq!(req.result_kind(), KIND_JOB_TEXT_GENERATION + 1000);

        let inner = req.inner();
        assert_eq!(inner.inputs.len(), 1);
        assert_eq!(inner.params.len(), 3);
        assert_eq!(inner.bid, Some(1000));
    }

    #[test]
    fn test_summarization_request() {
        let req = ComputeJobRequest::summarization("Long text here...")
            .unwrap()
            .with_output("text/plain");

        assert_eq!(req.kind(), KIND_JOB_SUMMARIZATION);
        assert_eq!(req.inner().output, Some("text/plain".to_string()));
    }

    #[test]
    fn test_translation_request() {
        let req = ComputeJobRequest::translation("Hello world", "es").unwrap();

        assert_eq!(req.kind(), KIND_JOB_TRANSLATION);
        let inner = req.inner();
        assert!(
            inner
                .params
                .iter()
                .any(|p| p.key == "target_lang" && p.value == "es")
        );
    }

    #[test]
    fn test_image_generation_request() {
        let req = ComputeJobRequest::image_generation("A sunset over the ocean")
            .unwrap()
            .with_param("size", "1024x1024")
            .with_param("style", "photorealistic");

        assert_eq!(req.kind(), KIND_JOB_IMAGE_GENERATION);
        assert_eq!(req.inner().params.len(), 2);
    }

    #[test]
    fn test_speech_to_text_request() {
        let req = ComputeJobRequest::speech_to_text("https://example.com/audio.mp3")
            .unwrap()
            .with_param("language", "en");

        assert_eq!(req.kind(), KIND_JOB_SPEECH_TO_TEXT);
    }

    #[test]
    fn test_job_chaining() {
        let req = ComputeJobRequest::summarization("dummy")
            .unwrap()
            .with_job_input("previous_job_id", Some("wss://relay.com".to_string()));

        let inner = req.inner();
        assert_eq!(inner.inputs.len(), 2); // Original dummy + job input
        assert!(inner.inputs.iter().any(|i| i.input_type == InputType::Job));
    }

    #[test]
    fn test_job_result() {
        let result = ComputeJobResult::new(
            KIND_JOB_TEXT_GENERATION,
            "request_id",
            "customer_pubkey",
            "The answer is 42",
        )
        .unwrap()
        .with_payment(500, Some("lnbc500n...".to_string()));

        let inner = result.inner();
        assert_eq!(inner.kind, KIND_JOB_TEXT_GENERATION + 1000);
        assert_eq!(inner.content, "The answer is 42");
        assert_eq!(inner.amount, Some(500));
    }

    #[test]
    fn test_payment_required_feedback() {
        let feedback = ComputeJobFeedback::payment_required(
            "request_id",
            "customer_pubkey",
            1000,
            Some("lnbc1000n...".to_string()),
        );

        let inner = feedback.inner();
        assert_eq!(inner.status, JobStatus::PaymentRequired);
        assert_eq!(inner.amount, Some(1000));
        assert!(inner.bolt11.is_some());
    }

    #[test]
    fn test_processing_feedback() {
        let feedback = ComputeJobFeedback::processing("request_id", "customer_pubkey")
            .with_extra("Processing started");

        let inner = feedback.inner();
        assert_eq!(inner.status, JobStatus::Processing);
        assert_eq!(inner.status_extra, Some("Processing started".to_string()));
    }

    #[test]
    fn test_error_feedback() {
        let feedback = ComputeJobFeedback::error("request_id", "customer_pubkey", "Out of credits");

        let inner = feedback.inner();
        assert_eq!(inner.status, JobStatus::Error);
        assert_eq!(inner.status_extra, Some("Out of credits".to_string()));
    }

    #[test]
    fn test_partial_feedback() {
        let feedback =
            ComputeJobFeedback::partial("request_id", "customer_pubkey", "Here's a preview: ...");

        let inner = feedback.inner();
        assert_eq!(inner.status, JobStatus::Partial);
        assert_eq!(inner.content, "Here's a preview: ...");
    }

    #[test]
    fn test_job_category() {
        assert_eq!(
            JobCategory::from_kind(KIND_JOB_TEXT_GENERATION),
            JobCategory::TextProcessing
        );
        assert_eq!(
            JobCategory::from_kind(KIND_JOB_IMAGE_GENERATION),
            JobCategory::ImageGeneration
        );
        assert_eq!(
            JobCategory::from_kind(KIND_JOB_SPEECH_TO_TEXT),
            JobCategory::AudioProcessing
        );
        assert_eq!(JobCategory::from_kind(9999), JobCategory::Custom(9999));
    }

    #[test]
    fn test_job_category_names() {
        assert_eq!(JobCategory::TextProcessing.name(), "Text Processing");
        assert_eq!(JobCategory::ImageGeneration.name(), "Image Generation");
        assert_eq!(JobCategory::AudioProcessing.name(), "Audio Processing");
        assert_eq!(JobCategory::Custom(9999).name(), "Custom");
    }
}
