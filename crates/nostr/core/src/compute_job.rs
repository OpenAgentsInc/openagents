//! Compute job types for decentralized compute marketplace.
//!
//! This module provides types for submitting compute jobs, tracking execution,
//! and selecting providers based on requirements.

use crate::provider::{ComputeProvider, Region};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors that can occur when working with compute jobs
#[derive(Error, Debug)]
pub enum ComputeJobError {
    /// Invalid job parameters
    #[error("invalid job parameters: {0}")]
    InvalidParams(String),

    /// No provider meets requirements
    #[error("no provider meets requirements")]
    NoProviderAvailable,

    /// Job execution failed
    #[error("job execution failed: {0}")]
    ExecutionFailed(String),

    /// Budget insufficient
    #[error("budget insufficient: need {0} sats, have {1} sats")]
    InsufficientBudget(u64, u64),
}

/// Parameters for inference/generation
///
/// # Examples
///
/// ```no_run
/// use nostr::compute_job::InferenceParams;
///
/// // Create with validation
/// let params = InferenceParams::new(2048, 0.7)
///     .expect("valid parameters")
///     .with_top_p(0.95)
///     .expect("valid top_p")
///     .add_stop_sequence("END")
///     .add_stop_sequence("STOP");
///
/// assert_eq!(params.max_tokens, 2048);
/// assert_eq!(params.temperature, 0.7);
/// assert_eq!(params.stop_sequences.len(), 2);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceParams {
    /// Maximum number of tokens to generate
    pub max_tokens: u32,

    /// Temperature (0.0-2.0, typically 0.7)
    pub temperature: f32,

    /// Top-p sampling (nucleus sampling)
    pub top_p: Option<f32>,

    /// Sequences that stop generation
    pub stop_sequences: Vec<String>,
}

impl Default for InferenceParams {
    fn default() -> Self {
        Self {
            max_tokens: 1024,
            temperature: 0.7,
            top_p: Some(0.9),
            stop_sequences: vec![],
        }
    }
}

impl InferenceParams {
    /// Create new inference parameters with validation
    pub fn new(max_tokens: u32, temperature: f32) -> Result<Self, ComputeJobError> {
        if max_tokens == 0 {
            return Err(ComputeJobError::InvalidParams(
                "max_tokens must be greater than 0".to_string(),
            ));
        }

        if !(0.0..=2.0).contains(&temperature) {
            return Err(ComputeJobError::InvalidParams(
                "temperature must be between 0.0 and 2.0".to_string(),
            ));
        }

        Ok(Self {
            max_tokens,
            temperature,
            top_p: Some(0.9),
            stop_sequences: vec![],
        })
    }

    /// Set top-p sampling parameter
    pub fn with_top_p(mut self, top_p: f32) -> Result<Self, ComputeJobError> {
        if !(0.0..=1.0).contains(&top_p) {
            return Err(ComputeJobError::InvalidParams(
                "top_p must be between 0.0 and 1.0".to_string(),
            ));
        }
        self.top_p = Some(top_p);
        Ok(self)
    }

    /// Add a stop sequence
    pub fn add_stop_sequence(mut self, sequence: impl Into<String>) -> Self {
        self.stop_sequences.push(sequence.into());
        self
    }
}

/// Requirements for job execution
///
/// # Examples
///
/// ```no_run
/// use nostr::compute_job::JobRequirements;
/// use nostr::provider::Region;
///
/// let reqs = JobRequirements::new()
///     .with_max_latency(500)           // 500ms max
///     .with_region(Region::UsWest)     // US West only
///     .with_min_reputation(0.95);      // 95%+ success rate
///
/// assert_eq!(reqs.max_latency_ms, Some(500));
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct JobRequirements {
    /// Maximum acceptable latency in milliseconds
    pub max_latency_ms: Option<u32>,

    /// Preferred region
    pub region: Option<Region>,

    /// Minimum provider reputation (success rate)
    pub min_reputation: Option<f32>,
}

impl JobRequirements {
    /// Create new job requirements
    pub fn new() -> Self {
        Self::default()
    }

    /// Set maximum latency requirement
    pub fn with_max_latency(mut self, max_latency_ms: u32) -> Self {
        self.max_latency_ms = Some(max_latency_ms);
        self
    }

    /// Set region requirement
    pub fn with_region(mut self, region: Region) -> Self {
        self.region = Some(region);
        self
    }

    /// Set minimum reputation requirement
    pub fn with_min_reputation(mut self, min_reputation: f32) -> Self {
        self.min_reputation = Some(min_reputation);
        self
    }

    /// Check if a provider meets these requirements
    pub fn meets_requirements(&self, provider: &ComputeProvider) -> bool {
        // Check region
        if let Some(required_region) = self.region
            && provider.region != required_region
        {
            return false;
        }

        // Check reputation
        if let Some(min_rep) = self.min_reputation
            && provider.reputation.success_rate < min_rep
        {
            return false;
        }

        // Check latency
        if let Some(max_latency) = self.max_latency_ms
            && provider.reputation.avg_latency_ms > max_latency
        {
            return false;
        }

        true
    }
}

/// A compute job request
///
/// # Examples
///
/// ```no_run
/// use nostr::compute_job::{ComputeJobRequest, InferenceParams, JobRequirements};
/// use nostr::provider::Region;
///
/// let params = InferenceParams::new(1024, 0.7).expect("valid params");
/// let requirements = JobRequirements::new()
///     .with_region(Region::UsWest)
///     .with_max_latency(500);
///
/// let request = ComputeJobRequest::new(
///     "job_123",
///     "llama-70b",
///     "Explain quantum computing",
///     params,
///     10_000, // 10k sats budget
/// ).with_requirements(requirements);
///
/// assert_eq!(request.id, "job_123");
/// assert!(request.estimate_input_tokens() > 0);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputeJobRequest {
    /// Unique job identifier
    pub id: String,

    /// Model to use (e.g., "llama-70b", "mistral-7b")
    pub model: String,

    /// Input prompt
    pub prompt: String,

    /// Inference parameters
    pub params: InferenceParams,

    /// Budget in satoshis
    pub budget_sats: u64,

    /// Job requirements
    pub requirements: JobRequirements,
}

impl ComputeJobRequest {
    /// Create a new compute job request
    pub fn new(
        id: impl Into<String>,
        model: impl Into<String>,
        prompt: impl Into<String>,
        params: InferenceParams,
        budget_sats: u64,
    ) -> Self {
        Self {
            id: id.into(),
            model: model.into(),
            prompt: prompt.into(),
            params,
            budget_sats,
            requirements: JobRequirements::default(),
        }
    }

    /// Set job requirements
    pub fn with_requirements(mut self, requirements: JobRequirements) -> Self {
        self.requirements = requirements;
        self
    }

    /// Estimate input token count (rough approximation: 4 chars per token)
    pub fn estimate_input_tokens(&self) -> u64 {
        (self.prompt.len() / 4) as u64
    }

    /// Estimate output token count
    pub fn estimate_output_tokens(&self) -> u64 {
        self.params.max_tokens as u64
    }
}

/// Status of a compute job
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    /// Job submitted to marketplace
    Submitted,
    /// Routing to provider
    Routing,
    /// Executing on provider
    Executing,
    /// Streaming results
    Streaming,
    /// Completed successfully
    Completed,
    /// Failed with error
    Failed,
}

impl JobStatus {
    /// Check if this is a terminal status
    pub fn is_terminal(&self) -> bool {
        matches!(self, JobStatus::Completed | JobStatus::Failed)
    }

    /// Check if job is in progress
    pub fn is_in_progress(&self) -> bool {
        matches!(self, JobStatus::Executing | JobStatus::Streaming)
    }
}

/// Token usage for a completed job
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsage {
    /// Input tokens used
    pub input_tokens: u64,

    /// Output tokens generated
    pub output_tokens: u64,

    /// Total tokens
    pub total_tokens: u64,
}

impl TokenUsage {
    /// Create new token usage
    pub fn new(input_tokens: u64, output_tokens: u64) -> Self {
        Self {
            input_tokens,
            output_tokens,
            total_tokens: input_tokens + output_tokens,
        }
    }
}

/// Result of a compute job execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputeJobResult {
    /// Job identifier
    pub job_id: String,

    /// Provider that executed the job
    pub provider_id: String,

    /// Current status
    pub status: JobStatus,

    /// Generated output (if completed)
    pub output: Option<String>,

    /// Token usage statistics
    pub usage: TokenUsage,

    /// Actual cost in satoshis
    pub cost_sats: u64,

    /// Latency in milliseconds
    pub latency_ms: u32,

    /// Error message (if failed)
    pub error: Option<String>,
}

impl ComputeJobResult {
    /// Create a successful job result
    pub fn success(
        job_id: impl Into<String>,
        provider_id: impl Into<String>,
        output: impl Into<String>,
        usage: TokenUsage,
        cost_sats: u64,
        latency_ms: u32,
    ) -> Self {
        Self {
            job_id: job_id.into(),
            provider_id: provider_id.into(),
            status: JobStatus::Completed,
            output: Some(output.into()),
            usage,
            cost_sats,
            latency_ms,
            error: None,
        }
    }

    /// Create a failed job result
    pub fn failure(
        job_id: impl Into<String>,
        provider_id: impl Into<String>,
        error: impl Into<String>,
    ) -> Self {
        Self {
            job_id: job_id.into(),
            provider_id: provider_id.into(),
            status: JobStatus::Failed,
            output: None,
            usage: TokenUsage::new(0, 0),
            cost_sats: 0,
            latency_ms: 0,
            error: Some(error.into()),
        }
    }
}

/// Provider selection mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SelectionMode {
    /// Optimize for price and quality balance
    BestValue,
    /// Select cheapest provider
    Cheapest,
    /// Select fastest provider (lowest latency)
    Fastest,
    /// Run on top K providers and compare results
    TopK(u8),
}

/// Select the best provider for a job request
///
/// # Arguments
/// * `request` - The compute job request
/// * `providers` - Available providers
/// * `mode` - Selection strategy
///
/// # Examples
///
/// ```no_run
/// use nostr::compute_job::{ComputeJobRequest, InferenceParams, SelectionMode, select_provider};
///
/// # fn example() -> Option<()> {
/// let params = InferenceParams::default();
/// let request = ComputeJobRequest::new(
///     "job_123",
///     "llama-70b",
///     "Write a poem",
///     params,
///     5000,
/// );
///
/// let providers = vec![]; // Fetch from marketplace
/// let selected = select_provider(&request, &providers, SelectionMode::BestValue)?;
/// println!("Selected provider: {}", selected.lightning_address);
/// # Some(())
/// # }
/// ```
pub fn select_provider(
    request: &ComputeJobRequest,
    providers: &[ComputeProvider],
    mode: SelectionMode,
) -> Option<ComputeProvider> {
    // Filter providers that meet requirements
    let mut candidates: Vec<_> = providers
        .iter()
        .filter(|p| {
            // Must be online
            if !p.online {
                return false;
            }

            // Must support the model
            if !p.supports_model(&request.model) {
                return false;
            }

            // Must meet job requirements
            if !request.requirements.meets_requirements(p) {
                return false;
            }

            // Check if provider can handle the job within budget
            let estimated_cost = p.calculate_job_cost(
                request.estimate_input_tokens(),
                request.estimate_output_tokens(),
            );

            if estimated_cost > request.budget_sats {
                return false;
            }

            true
        })
        .collect();

    if candidates.is_empty() {
        return None;
    }

    match mode {
        SelectionMode::Cheapest => {
            // Find provider with lowest cost
            candidates.sort_by_key(|p| {
                p.calculate_job_cost(
                    request.estimate_input_tokens(),
                    request.estimate_output_tokens(),
                )
            });
            candidates.first().map(|&p| p.clone())
        }

        SelectionMode::Fastest => {
            // Find provider with lowest latency
            candidates.sort_by_key(|p| p.reputation.avg_latency_ms);
            candidates.first().map(|&p| p.clone())
        }

        SelectionMode::BestValue => {
            // Score based on cost and quality
            candidates.sort_by(|a, b| {
                let score_a = calculate_value_score(a, request);
                let score_b = calculate_value_score(b, request);
                score_b
                    .partial_cmp(&score_a)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            candidates.first().map(|&p| p.clone())
        }

        SelectionMode::TopK(_k) => {
            // Return highest reputation provider from top K
            candidates.sort_by(|a, b| {
                b.reputation
                    .success_rate
                    .partial_cmp(&a.reputation.success_rate)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            candidates.first().map(|&p| p.clone())
        }
    }
}

/// Calculate a value score for provider selection (higher is better)
fn calculate_value_score(provider: &ComputeProvider, request: &ComputeJobRequest) -> f32 {
    let cost = provider.calculate_job_cost(
        request.estimate_input_tokens(),
        request.estimate_output_tokens(),
    ) as f32;

    let reputation = provider.reputation.success_rate;
    let latency_factor = 1.0 / (provider.reputation.avg_latency_ms as f32 + 1.0);

    // Score formula: (reputation * latency_factor) / (cost + 1)
    // Higher reputation and lower latency increase score
    // Higher cost decreases score
    (reputation * latency_factor * 1000.0) / (cost + 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::NostrIdentity;
    use crate::provider::{ComputeCapabilities, ComputePricing, ProviderReputation};

    fn create_test_provider(
        region: Region,
        pricing: ComputePricing,
        latency_ms: u32,
        success_rate: f32,
    ) -> ComputeProvider {
        let pubkey = "a".repeat(64);
        let identity = NostrIdentity::new(&pubkey).unwrap();
        let capabilities =
            ComputeCapabilities::new(vec!["llama-70b".to_string()], 8192, 2048).unwrap();

        let mut provider = ComputeProvider::new(
            identity,
            "provider@domain.com",
            region,
            pricing,
            capabilities,
        )
        .unwrap();

        provider.set_online(true);
        provider.reputation = ProviderReputation {
            jobs_completed: 1000,
            success_rate,
            avg_latency_ms: latency_ms,
            uptime_pct: 0.99,
        };

        provider
    }

    #[test]
    fn test_inference_params() {
        let params = InferenceParams::new(1024, 0.7).unwrap();
        assert_eq!(params.max_tokens, 1024);
        assert_eq!(params.temperature, 0.7);

        assert!(InferenceParams::new(0, 0.7).is_err());
        assert!(InferenceParams::new(1024, 2.5).is_err());

        let params = params.with_top_p(0.95).unwrap().add_stop_sequence("END");
        assert_eq!(params.top_p, Some(0.95));
        assert_eq!(params.stop_sequences.len(), 1);
    }

    #[test]
    fn test_job_requirements() {
        let reqs = JobRequirements::new()
            .with_max_latency(500)
            .with_region(Region::UsWest)
            .with_min_reputation(0.95);

        assert_eq!(reqs.max_latency_ms, Some(500));
        assert_eq!(reqs.region, Some(Region::UsWest));
        assert_eq!(reqs.min_reputation, Some(0.95));
    }

    #[test]
    fn test_requirements_checking() {
        let pricing = ComputePricing::new(10, 20, 100).unwrap();
        let provider = create_test_provider(Region::UsWest, pricing, 300, 0.97);

        let reqs = JobRequirements::new().with_region(Region::UsWest);
        assert!(reqs.meets_requirements(&provider));

        let reqs = JobRequirements::new().with_region(Region::EuWest);
        assert!(!reqs.meets_requirements(&provider));

        let reqs = JobRequirements::new().with_min_reputation(0.99);
        assert!(!reqs.meets_requirements(&provider));

        let reqs = JobRequirements::new().with_max_latency(200);
        assert!(!reqs.meets_requirements(&provider));
    }

    #[test]
    fn test_compute_job_request() {
        let params = InferenceParams::default();
        let request = ComputeJobRequest::new("job_123", "llama-70b", "Hello, world!", params, 1000);

        assert_eq!(request.id, "job_123");
        assert_eq!(request.model, "llama-70b");
        assert!(request.estimate_input_tokens() > 0);
    }

    #[test]
    fn test_job_status() {
        assert!(JobStatus::Completed.is_terminal());
        assert!(JobStatus::Failed.is_terminal());
        assert!(!JobStatus::Executing.is_terminal());

        assert!(JobStatus::Executing.is_in_progress());
        assert!(!JobStatus::Submitted.is_in_progress());
    }

    #[test]
    fn test_token_usage() {
        let usage = TokenUsage::new(100, 50);
        assert_eq!(usage.input_tokens, 100);
        assert_eq!(usage.output_tokens, 50);
        assert_eq!(usage.total_tokens, 150);
    }

    #[test]
    fn test_compute_job_result() {
        let usage = TokenUsage::new(100, 50);
        let result =
            ComputeJobResult::success("job_123", "provider_1", "Output text", usage, 200, 350);

        assert_eq!(result.status, JobStatus::Completed);
        assert!(result.output.is_some());
        assert!(result.error.is_none());

        let failed = ComputeJobResult::failure("job_456", "provider_2", "Timeout");
        assert_eq!(failed.status, JobStatus::Failed);
        assert!(failed.error.is_some());
    }

    #[test]
    fn test_select_cheapest_provider() {
        let cheap_pricing = ComputePricing::new(5, 10, 50).unwrap();
        let expensive_pricing = ComputePricing::new(20, 40, 100).unwrap();

        let provider1 = create_test_provider(Region::UsWest, cheap_pricing, 500, 0.95);
        let provider2 = create_test_provider(Region::UsWest, expensive_pricing, 300, 0.99);

        let params = InferenceParams::default();
        let request = ComputeJobRequest::new("job_123", "llama-70b", "Test prompt", params, 10000);

        let providers = vec![provider1, provider2];
        let selected = select_provider(&request, &providers, SelectionMode::Cheapest).unwrap();

        assert_eq!(selected.pricing.per_1k_input_sats, 5);
    }

    #[test]
    fn test_select_fastest_provider() {
        let pricing = ComputePricing::new(10, 20, 100).unwrap();

        let fast_provider = create_test_provider(Region::UsWest, pricing.clone(), 200, 0.95);
        let slow_provider = create_test_provider(Region::UsWest, pricing, 800, 0.99);

        let params = InferenceParams::default();
        let request = ComputeJobRequest::new("job_123", "llama-70b", "Test prompt", params, 10000);

        let providers = vec![slow_provider, fast_provider];
        let selected = select_provider(&request, &providers, SelectionMode::Fastest).unwrap();

        assert_eq!(selected.reputation.avg_latency_ms, 200);
    }

    #[test]
    fn test_no_provider_available() {
        let pricing = ComputePricing::new(10, 20, 100).unwrap();
        let mut provider = create_test_provider(Region::UsWest, pricing, 300, 0.95);
        provider.set_online(false); // Offline

        let params = InferenceParams::default();
        let request = ComputeJobRequest::new("job_123", "llama-70b", "Test prompt", params, 10000);

        let providers = vec![provider];
        let selected = select_provider(&request, &providers, SelectionMode::BestValue);

        assert!(selected.is_none());
    }
}
