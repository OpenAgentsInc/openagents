//! Model abstractions for Rustygrad.

use serde::{Deserialize, Serialize};

/// Human-readable crate ownership summary.
pub const CRATE_ROLE: &str = "reusable model definitions and metadata";

/// Embedding vector normalization policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EmbeddingNormalization {
    /// Return raw vectors without normalization.
    None,
    /// Normalize each vector to unit length.
    UnitLength,
}

/// Shared model descriptor.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelDescriptor {
    /// Stable model identifier.
    pub model_id: String,
    /// Model family label such as `smoke`, `llama`, or `bert`.
    pub family: String,
    /// Revision string or version tag.
    pub revision: String,
}

impl ModelDescriptor {
    /// Creates a model descriptor.
    #[must_use]
    pub fn new(
        model_id: impl Into<String>,
        family: impl Into<String>,
        revision: impl Into<String>,
    ) -> Self {
        Self {
            model_id: model_id.into(),
            family: family.into(),
            revision: revision.into(),
        }
    }
}

/// Embeddings-specific model descriptor.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmbeddingModelDescriptor {
    /// Shared model metadata.
    pub model: ModelDescriptor,
    /// Stable vector dimension.
    pub dimensions: usize,
    /// Normalization policy applied to results.
    pub normalization: EmbeddingNormalization,
}

impl EmbeddingModelDescriptor {
    /// Creates an embeddings model descriptor.
    #[must_use]
    pub fn new(
        model: ModelDescriptor,
        dimensions: usize,
        normalization: EmbeddingNormalization,
    ) -> Self {
        Self {
            model,
            dimensions,
            normalization,
        }
    }
}

/// Deterministic embeddings smoke model used for the phase-0 end-to-end flow.
#[derive(Clone, Debug, PartialEq)]
pub struct SmokeByteEmbedder {
    descriptor: EmbeddingModelDescriptor,
    input_dimensions: usize,
    projection: Vec<f32>,
    bias: Vec<f32>,
}

impl Default for SmokeByteEmbedder {
    fn default() -> Self {
        Self::new()
    }
}

impl SmokeByteEmbedder {
    /// Stable smoke model identifier.
    pub const MODEL_ID: &str = "smoke-byte-embed-v0";

    /// Creates the default smoke embeddings model.
    #[must_use]
    pub fn new() -> Self {
        let input_dimensions = 16;
        let dimensions = 8;
        let descriptor = EmbeddingModelDescriptor::new(
            ModelDescriptor::new(Self::MODEL_ID, "smoke", "v0"),
            dimensions,
            EmbeddingNormalization::None,
        );
        let projection = (0..input_dimensions)
            .flat_map(|row| {
                (0..dimensions).map(move |column| {
                    let seed = ((row + 3) * (column + 5)) % 17;
                    ((seed as f32) - 8.0) / 8.0
                })
            })
            .collect();
        let bias = (0..dimensions)
            .map(|column| {
                let seed = ((column + 1) * 3) % 7;
                ((seed as f32) - 3.0) / 10.0
            })
            .collect();

        Self {
            descriptor,
            input_dimensions,
            projection,
            bias,
        }
    }

    /// Returns the public model descriptor.
    #[must_use]
    pub fn descriptor(&self) -> &EmbeddingModelDescriptor {
        &self.descriptor
    }

    /// Returns the fixed input feature dimension.
    #[must_use]
    pub const fn input_dimensions(&self) -> usize {
        self.input_dimensions
    }

    /// Returns the projection matrix in row-major form.
    #[must_use]
    pub fn projection(&self) -> &[f32] {
        &self.projection
    }

    /// Returns the bias vector.
    #[must_use]
    pub fn bias(&self) -> &[f32] {
        &self.bias
    }

    /// Converts input text into a deterministic feature vector.
    #[must_use]
    pub fn featurize(&self, input: &str) -> Vec<f32> {
        let mut buckets = vec![0.0; self.input_dimensions];
        let bytes = input.as_bytes();
        if bytes.is_empty() {
            return buckets;
        }

        for (index, byte) in bytes.iter().enumerate() {
            let bucket = (usize::from(*byte) + index) % self.input_dimensions;
            buckets[bucket] += f32::from(*byte) / 255.0;
        }

        let scale = 1.0 / (bytes.len() as f32);
        for value in &mut buckets {
            *value *= scale;
        }

        buckets
    }
}

#[cfg(test)]
mod tests {
    use super::SmokeByteEmbedder;

    #[test]
    fn smoke_featurize_is_deterministic() {
        let model = SmokeByteEmbedder::new();
        let first = model.featurize("hello world");
        let second = model.featurize("hello world");
        assert_eq!(first, second);
    }

    #[test]
    fn smoke_model_exposes_stable_dimensions() {
        let model = SmokeByteEmbedder::new();
        assert_eq!(model.input_dimensions(), 16);
        assert_eq!(model.descriptor().dimensions, 8);
        assert_eq!(model.descriptor().model.model_id, SmokeByteEmbedder::MODEL_ID);
    }
}
