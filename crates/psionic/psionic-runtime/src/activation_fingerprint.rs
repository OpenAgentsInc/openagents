use std::time::Instant;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Stable scheme identifier for the quantized activation-fingerprint adapter.
pub const QUANTIZED_ACTIVATION_FINGERPRINT_SCHEME_ID: &str =
    "psionic.activation_fingerprint.quantized.v1";

/// One deterministic vector sample carried into an activation-fingerprint proof.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ActivationFingerprintVectorSample {
    /// Stable sample label such as `embedding:0`.
    pub label: String,
    /// Vector values captured for the sample.
    pub values: Vec<f32>,
}

impl ActivationFingerprintVectorSample {
    /// Creates one labeled vector sample.
    #[must_use]
    pub fn new(label: impl Into<String>, values: Vec<f32>) -> Self {
        Self {
            label: label.into(),
            values,
        }
    }
}

/// Deterministic activation-fingerprint input assembled from one inference output.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ActivationFingerprintInput {
    /// Stable request digest the proof belongs to.
    pub request_digest: String,
    /// Stable compute product identifier.
    pub product_id: String,
    /// Stable model identifier that produced the samples.
    pub model_id: String,
    /// Runtime backend that produced the samples.
    pub runtime_backend: String,
    /// Vector samples carried into the proof.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub samples: Vec<ActivationFingerprintVectorSample>,
}

impl ActivationFingerprintInput {
    /// Creates activation-fingerprint input from explicit request/model/runtime truth.
    #[must_use]
    pub fn new(
        request_digest: impl Into<String>,
        product_id: impl Into<String>,
        model_id: impl Into<String>,
        runtime_backend: impl Into<String>,
    ) -> Self {
        Self {
            request_digest: request_digest.into(),
            product_id: product_id.into(),
            model_id: model_id.into(),
            runtime_backend: runtime_backend.into(),
            samples: Vec::new(),
        }
    }

    /// Appends one deterministic vector sample.
    #[must_use]
    pub fn with_sample(mut self, sample: ActivationFingerprintVectorSample) -> Self {
        self.samples.push(sample);
        self
    }
}

/// Config for the quantized activation-fingerprint adapter.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuantizedActivationFingerprintConfig {
    /// Bucket width in thousandths of one unit.
    pub bucket_width_milli: u32,
    /// Clamp range in thousandths of one unit before quantization.
    pub clamp_abs_milli: u32,
    /// Maximum values sampled from each vector.
    pub max_values_per_sample: usize,
}

impl Default for QuantizedActivationFingerprintConfig {
    fn default() -> Self {
        Self {
            bucket_width_milli: 50,
            clamp_abs_milli: 8_000,
            max_values_per_sample: 128,
        }
    }
}

impl QuantizedActivationFingerprintConfig {
    fn bucket_width(&self) -> f32 {
        self.bucket_width_milli as f32 / 1000.0
    }

    fn clamp_abs(&self) -> f32 {
        self.clamp_abs_milli as f32 / 1000.0
    }
}

/// Compact digest record for one sampled activation vector.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActivationFingerprintSampleArtifact {
    /// Stable label for the sampled vector.
    pub label: String,
    /// Full vector length before deterministic sampling.
    pub input_length: usize,
    /// Number of values retained in the deterministic sample.
    pub sampled_value_count: usize,
    /// Mean quantized bucket across sampled values.
    pub mean_bucket: i64,
    /// Sum of squared quantized buckets across sampled values.
    pub l2_bucket_sum: u64,
    /// Stable digest over the sampled bucket sequence.
    pub digest: String,
}

/// Compact artifact emitted by one activation-fingerprint adapter.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActivationFingerprintProofArtifact {
    /// Stable proof scheme identifier.
    pub scheme_id: String,
    /// Stable artifact schema version.
    pub artifact_version: u16,
    /// Stable request digest the artifact belongs to.
    pub request_digest: String,
    /// Stable compute product identifier.
    pub product_id: String,
    /// Stable model identifier.
    pub model_id: String,
    /// Runtime backend that emitted the samples.
    pub runtime_backend: String,
    /// Quantization config used to build the compact fingerprint.
    pub config: QuantizedActivationFingerprintConfig,
    /// Number of vector samples represented by the artifact.
    pub sample_count: usize,
    /// Total values retained across all deterministic samples.
    pub total_values_sampled: usize,
    /// Compact digest record for each sampled vector.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub samples: Vec<ActivationFingerprintSampleArtifact>,
    /// Stable digest for the whole artifact.
    pub artifact_digest: String,
}

impl ActivationFingerprintProofArtifact {
    fn digest_without_self(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.scheme_id.as_bytes());
        hasher.update(b"|");
        hasher.update(self.artifact_version.to_string().as_bytes());
        hasher.update(b"|");
        hasher.update(self.request_digest.as_bytes());
        hasher.update(b"|");
        hasher.update(self.product_id.as_bytes());
        hasher.update(b"|");
        hasher.update(self.model_id.as_bytes());
        hasher.update(b"|");
        hasher.update(self.runtime_backend.as_bytes());
        hasher.update(b"|");
        hasher.update(self.config.bucket_width_milli.to_string().as_bytes());
        hasher.update(b"|");
        hasher.update(self.config.clamp_abs_milli.to_string().as_bytes());
        hasher.update(b"|");
        hasher.update(self.config.max_values_per_sample.to_string().as_bytes());
        for sample in &self.samples {
            hasher.update(b"|sample|");
            hasher.update(sample.label.as_bytes());
            hasher.update(b"|");
            hasher.update(sample.input_length.to_string().as_bytes());
            hasher.update(b"|");
            hasher.update(sample.sampled_value_count.to_string().as_bytes());
            hasher.update(b"|");
            hasher.update(sample.mean_bucket.to_string().as_bytes());
            hasher.update(b"|");
            hasher.update(sample.l2_bucket_sum.to_string().as_bytes());
            hasher.update(b"|");
            hasher.update(sample.digest.as_bytes());
        }
        hex::encode(hasher.finalize())
    }

    /// Returns the stable digest for the artifact.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        self.digest_without_self()
    }

    /// Verifies the self-reported artifact digest.
    #[must_use]
    pub fn is_self_consistent(&self) -> bool {
        self.artifact_digest == self.digest_without_self()
    }
}

/// Verification result for one activation-fingerprint artifact.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActivationFingerprintVerification {
    /// Stable proof scheme identifier.
    pub scheme_id: String,
    /// Stable digest of the artifact that was checked.
    pub artifact_digest: String,
    /// Whether the candidate input matched the artifact.
    pub matched: bool,
    /// Number of sample digests that matched.
    pub matched_samples: usize,
    /// Number of sample digests that did not match.
    pub mismatched_samples: usize,
    /// Labels of samples whose compact digests did not match.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mismatch_labels: Vec<String>,
}

/// Simple benchmark report for the quantized activation-fingerprint path.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActivationFingerprintBenchmark {
    /// Stable proof scheme identifier.
    pub scheme_id: String,
    /// Artifact digest measured by the benchmark.
    pub artifact_digest: String,
    /// Number of vector samples benchmarked.
    pub sample_count: usize,
    /// Total deterministic values retained across all samples.
    pub total_values_sampled: usize,
    /// Number of generation iterations measured.
    pub generation_iterations: usize,
    /// Number of verification iterations measured.
    pub verification_iterations: usize,
    /// Mean generation cost per iteration in nanoseconds.
    pub average_generation_ns: u64,
    /// Mean verification cost per iteration in nanoseconds.
    pub average_verification_ns: u64,
}

/// Narrow adapter interface for activation-fingerprint proof schemes.
pub trait ActivationFingerprintProofAdapter {
    /// Stable proof scheme identifier.
    fn scheme_id(&self) -> &'static str;

    /// Generates one compact fingerprint artifact from inference output samples.
    fn generate(&self, input: &ActivationFingerprintInput) -> ActivationFingerprintProofArtifact;

    /// Verifies a compact fingerprint artifact against a candidate inference output.
    fn verify(
        &self,
        input: &ActivationFingerprintInput,
        artifact: &ActivationFingerprintProofArtifact,
    ) -> ActivationFingerprintVerification;
}

/// Quantized compact activation-fingerprint adapter for inference outputs.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuantizedActivationFingerprintAdapter {
    /// Quantization config for the compact fingerprint.
    pub config: QuantizedActivationFingerprintConfig,
}

impl QuantizedActivationFingerprintAdapter {
    fn sampled_bucket_sequence(&self, values: &[f32]) -> Vec<i64> {
        let sampled_indices = sampled_indices(values.len(), self.config.max_values_per_sample);
        let bucket_width = self.config.bucket_width();
        let clamp_abs = self.config.clamp_abs();
        sampled_indices
            .into_iter()
            .map(|index| {
                let clamped = values[index].clamp(-clamp_abs, clamp_abs);
                (clamped / bucket_width).round() as i64
            })
            .collect()
    }

    fn sample_artifact(
        &self,
        sample: &ActivationFingerprintVectorSample,
    ) -> ActivationFingerprintSampleArtifact {
        let buckets = self.sampled_bucket_sequence(sample.values.as_slice());
        let mut hasher = Sha256::new();
        hasher.update(sample.label.as_bytes());
        hasher.update(b"|");
        hasher.update(sample.values.len().to_string().as_bytes());
        for bucket in &buckets {
            hasher.update(b"|");
            hasher.update(bucket.to_string().as_bytes());
        }
        let sum = buckets.iter().sum::<i64>();
        let l2_bucket_sum = buckets
            .iter()
            .map(|bucket| bucket.unsigned_abs().pow(2))
            .sum::<u64>();
        ActivationFingerprintSampleArtifact {
            label: sample.label.clone(),
            input_length: sample.values.len(),
            sampled_value_count: buckets.len(),
            mean_bucket: if buckets.is_empty() {
                0
            } else {
                sum / buckets.len() as i64
            },
            l2_bucket_sum,
            digest: hex::encode(hasher.finalize()),
        }
    }

    /// Measures generation and verification cost over repeated iterations.
    #[must_use]
    pub fn benchmark(
        &self,
        input: &ActivationFingerprintInput,
        iterations: usize,
    ) -> ActivationFingerprintBenchmark {
        let iterations = iterations.max(1);
        let start = Instant::now();
        let mut artifact = self.generate(input);
        let generation_elapsed = start.elapsed();
        for _ in 1..iterations {
            artifact = self.generate(input);
        }
        let total_generation_ns = generation_elapsed
            .as_nanos()
            .saturating_mul(iterations as u128)
            .min(u128::from(u64::MAX)) as u64;
        let verification_start = Instant::now();
        let _ = self.verify(input, &artifact);
        let verification_elapsed = verification_start.elapsed();
        let total_verification_ns = verification_elapsed
            .as_nanos()
            .saturating_mul(iterations as u128)
            .min(u128::from(u64::MAX)) as u64;
        ActivationFingerprintBenchmark {
            scheme_id: self.scheme_id().to_string(),
            artifact_digest: artifact.artifact_digest.clone(),
            sample_count: artifact.sample_count,
            total_values_sampled: artifact.total_values_sampled,
            generation_iterations: iterations,
            verification_iterations: iterations,
            average_generation_ns: total_generation_ns / iterations as u64,
            average_verification_ns: total_verification_ns / iterations as u64,
        }
    }
}

impl ActivationFingerprintProofAdapter for QuantizedActivationFingerprintAdapter {
    fn scheme_id(&self) -> &'static str {
        QUANTIZED_ACTIVATION_FINGERPRINT_SCHEME_ID
    }

    fn generate(&self, input: &ActivationFingerprintInput) -> ActivationFingerprintProofArtifact {
        let samples = input
            .samples
            .iter()
            .map(|sample| self.sample_artifact(sample))
            .collect::<Vec<_>>();
        let total_values_sampled = samples
            .iter()
            .map(|sample| sample.sampled_value_count)
            .sum::<usize>();
        let mut artifact = ActivationFingerprintProofArtifact {
            scheme_id: self.scheme_id().to_string(),
            artifact_version: 1,
            request_digest: input.request_digest.clone(),
            product_id: input.product_id.clone(),
            model_id: input.model_id.clone(),
            runtime_backend: input.runtime_backend.clone(),
            config: self.config.clone(),
            sample_count: samples.len(),
            total_values_sampled,
            samples,
            artifact_digest: String::new(),
        };
        artifact.artifact_digest = artifact.digest_without_self();
        artifact
    }

    fn verify(
        &self,
        input: &ActivationFingerprintInput,
        artifact: &ActivationFingerprintProofArtifact,
    ) -> ActivationFingerprintVerification {
        let candidate = self.generate(input);
        let mut mismatch_labels = Vec::new();
        for (expected, actual) in artifact.samples.iter().zip(candidate.samples.iter()) {
            if expected.digest != actual.digest {
                mismatch_labels.push(expected.label.clone());
            }
        }
        let mismatched_samples = mismatch_labels.len()
            + artifact
                .samples
                .len()
                .max(candidate.samples.len())
                .saturating_sub(artifact.samples.len().min(candidate.samples.len()));
        let matched_samples = artifact.samples.len().saturating_sub(mismatch_labels.len());
        ActivationFingerprintVerification {
            scheme_id: artifact.scheme_id.clone(),
            artifact_digest: artifact.artifact_digest.clone(),
            matched: artifact.artifact_digest == candidate.artifact_digest
                && artifact.is_self_consistent(),
            matched_samples,
            mismatched_samples,
            mismatch_labels,
        }
    }
}

fn sampled_indices(len: usize, max_values: usize) -> Vec<usize> {
    if len == 0 || max_values == 0 {
        return Vec::new();
    }
    if len <= max_values {
        return (0..len).collect();
    }
    if max_values == 1 {
        return vec![0];
    }
    (0..max_values)
        .map(|sample_index| sample_index.saturating_mul(len - 1) / (max_values - 1))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{
        ActivationFingerprintInput, ActivationFingerprintProofAdapter,
        ActivationFingerprintVectorSample, QUANTIZED_ACTIVATION_FINGERPRINT_SCHEME_ID,
        QuantizedActivationFingerprintAdapter,
    };

    fn sample_input() -> ActivationFingerprintInput {
        ActivationFingerprintInput::new("req-digest", "psionic.embeddings", "smoke-embed", "cpu")
            .with_sample(ActivationFingerprintVectorSample::new(
                "embedding:0",
                vec![0.1, 0.12, -0.32, 0.45, 0.46, 0.47, 0.48, 0.49],
            ))
            .with_sample(ActivationFingerprintVectorSample::new(
                "embedding:1",
                vec![0.0, -0.1, -0.2, -0.3, -0.4, 0.5, 0.6, 0.7],
            ))
    }

    #[test]
    fn quantized_adapter_emits_self_consistent_artifact() {
        let adapter = QuantizedActivationFingerprintAdapter::default();
        let artifact = adapter.generate(&sample_input());
        assert_eq!(
            artifact.scheme_id,
            QUANTIZED_ACTIVATION_FINGERPRINT_SCHEME_ID
        );
        assert_eq!(artifact.sample_count, 2);
        assert!(artifact.is_self_consistent());
    }

    #[test]
    fn quantized_adapter_verifies_tolerant_bucket_matches() {
        let adapter = QuantizedActivationFingerprintAdapter::default();
        let input = sample_input();
        let artifact = adapter.generate(&input);
        let candidate = ActivationFingerprintInput::new(
            "req-digest",
            "psionic.embeddings",
            "smoke-embed",
            "cpu",
        )
        .with_sample(ActivationFingerprintVectorSample::new(
            "embedding:0",
            vec![0.101, 0.119, -0.321, 0.449, 0.451, 0.472, 0.481, 0.491],
        ))
        .with_sample(ActivationFingerprintVectorSample::new(
            "embedding:1",
            vec![0.0, -0.102, -0.201, -0.299, -0.401, 0.499, 0.602, 0.699],
        ));
        let verification = adapter.verify(&candidate, &artifact);
        assert!(verification.matched);
        assert!(verification.mismatch_labels.is_empty());
    }

    #[test]
    fn quantized_adapter_benchmark_reports_iterations_and_costs() {
        let adapter = QuantizedActivationFingerprintAdapter::default();
        let benchmark = adapter.benchmark(&sample_input(), 8);
        assert_eq!(benchmark.generation_iterations, 8);
        assert_eq!(benchmark.verification_iterations, 8);
        assert!(benchmark.sample_count >= 2);
    }
}
