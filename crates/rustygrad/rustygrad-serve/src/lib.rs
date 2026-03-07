//! Served compute product contracts for Rustygrad.

use serde::{Deserialize, Serialize};

pub use rustygrad_models::{EmbeddingModelDescriptor, EmbeddingNormalization, ModelDescriptor};

/// Human-readable crate ownership summary.
pub const CRATE_ROLE: &str = "request and response types for served products";

/// Phase-0 embeddings product identifier.
pub const EMBEDDINGS_PRODUCT_ID: &str = "rustygrad.embeddings";

/// Embeddings request contract.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmbeddingRequest {
    /// Stable client-provided request identifier.
    pub request_id: String,
    /// Product identifier.
    pub product_id: String,
    /// Embeddings model descriptor.
    pub model: EmbeddingModelDescriptor,
    /// UTF-8 text inputs to embed.
    pub inputs: Vec<String>,
}

impl EmbeddingRequest {
    /// Creates an embeddings request for the default Rustygrad product.
    #[must_use]
    pub fn new(
        request_id: impl Into<String>,
        model: EmbeddingModelDescriptor,
        inputs: Vec<String>,
    ) -> Self {
        Self {
            request_id: request_id.into(),
            product_id: String::from(EMBEDDINGS_PRODUCT_ID),
            model,
            inputs,
        }
    }
}

/// Individual embeddings vector payload.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EmbeddingVector {
    /// Input index in the request.
    pub index: usize,
    /// Embedding values.
    pub values: Vec<f32>,
}

/// Response metadata for embeddings execution.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmbeddingResponseMetadata {
    /// Stable output dimensionality.
    pub dimensions: usize,
    /// Number of returned vectors.
    pub vector_count: usize,
    /// Model identifier used during execution.
    pub model_id: String,
    /// Normalization policy applied by the model.
    pub normalization: EmbeddingNormalization,
}

/// Embeddings response contract.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EmbeddingResponse {
    /// Stable request identifier.
    pub request_id: String,
    /// Product identifier.
    pub product_id: String,
    /// Returned embeddings vectors.
    pub embeddings: Vec<EmbeddingVector>,
    /// Metadata describing the outputs.
    pub metadata: EmbeddingResponseMetadata,
}

impl EmbeddingResponse {
    /// Creates an embeddings response from vectors and request metadata.
    #[must_use]
    pub fn new(request: &EmbeddingRequest, embeddings: Vec<EmbeddingVector>) -> Self {
        Self {
            request_id: request.request_id.clone(),
            product_id: request.product_id.clone(),
            metadata: EmbeddingResponseMetadata {
                dimensions: request.model.dimensions,
                vector_count: embeddings.len(),
                model_id: request.model.model.model_id.clone(),
                normalization: request.model.normalization,
            },
            embeddings,
        }
    }
}

/// Minimal embeddings execution interface.
pub trait EmbeddingsExecutor {
    /// Error returned when embedding execution fails.
    type Error;

    /// Executes an embeddings request.
    fn embed(&mut self, request: &EmbeddingRequest) -> Result<EmbeddingResponse, Self::Error>;
}

#[cfg(test)]
mod tests {
    use rustygrad_models::{EmbeddingModelDescriptor, EmbeddingNormalization, ModelDescriptor};

    use super::{EmbeddingRequest, EmbeddingResponse, EmbeddingVector};

    #[test]
    fn embedding_request_json_is_stable() -> Result<(), Box<dyn std::error::Error>> {
        let request = EmbeddingRequest::new(
            "req-1",
            EmbeddingModelDescriptor::new(
                ModelDescriptor::new("smoke-byte-embed-v0", "smoke", "v0"),
                8,
                EmbeddingNormalization::UnitLength,
            ),
            vec![String::from("hello world"), String::from("open agents")],
        );

        let encoded = serde_json::to_string_pretty(&request)?;
        let expected = r#"{
  "request_id": "req-1",
  "product_id": "rustygrad.embeddings",
  "model": {
    "model": {
      "model_id": "smoke-byte-embed-v0",
      "family": "smoke",
      "revision": "v0"
    },
    "dimensions": 8,
    "normalization": "UnitLength"
  },
  "inputs": [
    "hello world",
    "open agents"
  ]
}"#;
        assert_eq!(encoded, expected);
        Ok(())
    }

    #[test]
    fn embedding_response_round_trips() -> Result<(), Box<dyn std::error::Error>> {
        let request = EmbeddingRequest::new(
            "req-2",
            EmbeddingModelDescriptor::new(
                ModelDescriptor::new("smoke-byte-embed-v0", "smoke", "v0"),
                4,
                EmbeddingNormalization::None,
            ),
            vec![String::from("hi")],
        );
        let response = EmbeddingResponse::new(
            &request,
            vec![EmbeddingVector {
                index: 0,
                values: vec![0.0, 1.0, 2.0, 3.0],
            }],
        );

        let encoded = serde_json::to_string(&response)?;
        let decoded: EmbeddingResponse = serde_json::from_str(&encoded)?;
        assert_eq!(decoded, response);
        Ok(())
    }
}
