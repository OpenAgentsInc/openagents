//! Provenance tracking for job execution.
//!
//! Every job response includes provenance information that captures:
//! - Which model was used
//! - What sampling parameters were applied
//! - Hashes of input and output for verification
//! - Provider identity and execution timestamp

use serde::{Deserialize, Serialize};

/// Sampling parameters used for LLM inference.
///
/// These affect the reproducibility of results.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SamplingParams {
    /// Temperature for sampling (0.0 = deterministic, higher = more random).
    #[serde(default)]
    pub temperature: f32,

    /// Top-p (nucleus) sampling threshold.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,

    /// Top-k sampling (number of top tokens to consider).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_k: Option<u32>,

    /// Random seed for reproducibility.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed: Option<u64>,

    /// Maximum tokens to generate.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,

    /// Stop sequences.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stop: Vec<String>,
}

impl Default for SamplingParams {
    fn default() -> Self {
        Self {
            temperature: 0.0,
            top_p: None,
            top_k: None,
            seed: None,
            max_tokens: None,
            stop: Vec::new(),
        }
    }
}

impl SamplingParams {
    /// Create deterministic sampling params (temperature=0, fixed seed).
    pub fn deterministic(seed: u64) -> Self {
        Self {
            temperature: 0.0,
            top_p: None,
            top_k: None,
            seed: Some(seed),
            max_tokens: None,
            stop: Vec::new(),
        }
    }

    /// Create creative sampling params.
    pub fn creative(temperature: f32) -> Self {
        Self {
            temperature,
            top_p: Some(0.95),
            top_k: None,
            seed: None,
            max_tokens: None,
            stop: Vec::new(),
        }
    }

    /// Set max tokens.
    pub fn with_max_tokens(mut self, max: u32) -> Self {
        self.max_tokens = Some(max);
        self
    }

    /// Add stop sequences.
    pub fn with_stop(mut self, stop: Vec<String>) -> Self {
        self.stop = stop;
        self
    }
}

/// Provenance information for a job execution.
///
/// This captures everything needed to:
/// - Reproduce the result (model + sampling + input)
/// - Verify the result (input/output hashes)
/// - Attribute the work (provider + timestamp)
///
/// # Example
///
/// ```
/// use protocol::provenance::{Provenance, SamplingParams};
///
/// let provenance = Provenance::new("claude-3-sonnet")
///     .with_sampling(SamplingParams::deterministic(42))
///     .with_input_hash("abc123...")
///     .with_output_hash("def456...");
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Provenance {
    /// Model identifier used for inference.
    pub model_id: String,

    /// Sampling parameters used.
    #[serde(default)]
    pub sampling: SamplingParams,

    /// SHA-256 hash of the canonical JSON input.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_sha256: Option<String>,

    /// SHA-256 hash of the canonical JSON output.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_sha256: Option<String>,

    /// Nostr public key of the provider (hex-encoded).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_pubkey: Option<String>,

    /// Unix timestamp when the job was executed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub executed_at: Option<u64>,

    /// Duration of execution in milliseconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,

    /// Token counts for billing/analysis.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokens: Option<TokenCounts>,
}

/// Token usage counts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenCounts {
    /// Number of input/prompt tokens.
    pub input: u32,
    /// Number of output/completion tokens.
    pub output: u32,
}

impl TokenCounts {
    /// Create new token counts.
    pub fn new(input: u32, output: u32) -> Self {
        Self { input, output }
    }

    /// Total tokens (input + output).
    pub fn total(&self) -> u32 {
        self.input + self.output
    }
}

impl Provenance {
    /// Create provenance with a model ID.
    pub fn new(model_id: impl Into<String>) -> Self {
        Self {
            model_id: model_id.into(),
            sampling: SamplingParams::default(),
            input_sha256: None,
            output_sha256: None,
            provider_pubkey: None,
            executed_at: None,
            duration_ms: None,
            tokens: None,
        }
    }

    /// Set sampling parameters.
    pub fn with_sampling(mut self, sampling: SamplingParams) -> Self {
        self.sampling = sampling;
        self
    }

    /// Set input hash.
    pub fn with_input_hash(mut self, hash: impl Into<String>) -> Self {
        self.input_sha256 = Some(hash.into());
        self
    }

    /// Set output hash.
    pub fn with_output_hash(mut self, hash: impl Into<String>) -> Self {
        self.output_sha256 = Some(hash.into());
        self
    }

    /// Set provider public key.
    pub fn with_provider(mut self, pubkey: impl Into<String>) -> Self {
        self.provider_pubkey = Some(pubkey.into());
        self
    }

    /// Set execution timestamp.
    pub fn with_executed_at(mut self, timestamp: u64) -> Self {
        self.executed_at = Some(timestamp);
        self
    }

    /// Set execution duration.
    pub fn with_duration(mut self, duration_ms: u64) -> Self {
        self.duration_ms = Some(duration_ms);
        self
    }

    /// Set token counts.
    pub fn with_tokens(mut self, input: u32, output: u32) -> Self {
        self.tokens = Some(TokenCounts::new(input, output));
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sampling_deterministic() {
        let s = SamplingParams::deterministic(42);
        assert_eq!(s.temperature, 0.0);
        assert_eq!(s.seed, Some(42));
    }

    #[test]
    fn test_sampling_creative() {
        let s = SamplingParams::creative(0.8);
        assert_eq!(s.temperature, 0.8);
        assert_eq!(s.top_p, Some(0.95));
    }

    #[test]
    fn test_provenance_builder() {
        let p = Provenance::new("gpt-4")
            .with_sampling(SamplingParams::deterministic(42))
            .with_input_hash("abc123")
            .with_output_hash("def456")
            .with_provider("npub1...")
            .with_executed_at(1700000000)
            .with_duration(150)
            .with_tokens(100, 50);

        assert_eq!(p.model_id, "gpt-4");
        assert_eq!(p.sampling.seed, Some(42));
        assert_eq!(p.input_sha256, Some("abc123".to_string()));
        assert_eq!(p.output_sha256, Some("def456".to_string()));
        assert_eq!(p.provider_pubkey, Some("npub1...".to_string()));
        assert_eq!(p.executed_at, Some(1700000000));
        assert_eq!(p.duration_ms, Some(150));
        assert_eq!(p.tokens.as_ref().unwrap().total(), 150);
    }

    #[test]
    fn test_token_counts() {
        let t = TokenCounts::new(100, 50);
        assert_eq!(t.input, 100);
        assert_eq!(t.output, 50);
        assert_eq!(t.total(), 150);
    }

    #[test]
    fn test_serde_roundtrip() {
        let p = Provenance::new("claude-3")
            .with_sampling(SamplingParams::creative(0.7))
            .with_input_hash("hash1")
            .with_tokens(200, 100);

        let json = serde_json::to_string(&p).unwrap();
        let parsed: Provenance = serde_json::from_str(&json).unwrap();
        assert_eq!(p, parsed);
    }

    #[test]
    fn test_minimal_provenance_serde() {
        // Minimal provenance should serialize without optional fields
        let p = Provenance::new("model");
        let json = serde_json::to_string(&p).unwrap();
        assert!(!json.contains("input_sha256"));
        assert!(!json.contains("provider_pubkey"));
    }
}
