//! Served compute product contracts for Rustygrad.

use std::collections::BTreeMap;

use rustygrad_backend_cpu::CpuBackend;
use rustygrad_core::{DType, Device, Shape, TensorId};
use rustygrad_ir::{Graph, GraphBuilder, GraphError};
pub use rustygrad_models::{
    EmbeddingModelDescriptor, EmbeddingNormalization, ModelDescriptor, SmokeByteEmbedder,
};
use rustygrad_runtime::RuntimeError;
use serde::{Deserialize, Serialize};
use thiserror::Error;

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

/// Smoke embeddings execution error.
#[derive(Debug, Error)]
pub enum SmokeEmbeddingsError {
    /// The request targeted the wrong product.
    #[error("unsupported product id `{0}`")]
    UnsupportedProduct(String),
    /// The request targeted the wrong model.
    #[error("unsupported model `{0}`")]
    UnsupportedModel(String),
    /// The request carried no inputs.
    #[error("embedding request must contain at least one input")]
    EmptyInputBatch,
    /// Graph construction failed.
    #[error(transparent)]
    Graph(#[from] GraphError),
    /// CPU runtime execution failed.
    #[error(transparent)]
    Runtime(#[from] RuntimeError),
}

/// CPU-backed embeddings smoke service.
#[derive(Clone, Debug)]
pub struct SmokeEmbeddingsService {
    backend: CpuBackend,
    model: SmokeByteEmbedder,
    graph: Graph,
    input_shape: Shape,
    input_id: TensorId,
    output_id: TensorId,
}

impl SmokeEmbeddingsService {
    /// Creates a new smoke embeddings service.
    pub fn new() -> Result<Self, SmokeEmbeddingsError> {
        let model = SmokeByteEmbedder::new();
        let input_shape = Shape::new(vec![1, model.input_dimensions()]);
        let (graph, input_id, output_id) = build_smoke_graph(&model, input_shape.clone())?;
        Ok(Self {
            backend: CpuBackend::new(),
            model,
            graph,
            input_shape,
            input_id,
            output_id,
        })
    }

    /// Returns the smoke model descriptor.
    #[must_use]
    pub fn model_descriptor(&self) -> &EmbeddingModelDescriptor {
        self.model.descriptor()
    }

    fn embed_one(&mut self, input: &str) -> Result<Vec<f32>, SmokeEmbeddingsError> {
        let mut runtime_inputs = BTreeMap::new();
        runtime_inputs.insert(
            self.input_id,
            self.backend
                .input_buffer(self.input_shape.clone(), self.model.featurize(input))?,
        );
        let result = self.backend.compile_and_execute(&self.graph, &runtime_inputs)?;
        let Some(output) = result.outputs.get(&self.output_id) else {
            return Err(SmokeEmbeddingsError::Runtime(RuntimeError::Backend(
                String::from("missing smoke embedding output"),
            )));
        };
        Ok(output.as_f32_slice().to_vec())
    }
}

impl EmbeddingsExecutor for SmokeEmbeddingsService {
    type Error = SmokeEmbeddingsError;

    fn embed(&mut self, request: &EmbeddingRequest) -> Result<EmbeddingResponse, Self::Error> {
        if request.product_id != EMBEDDINGS_PRODUCT_ID {
            return Err(SmokeEmbeddingsError::UnsupportedProduct(
                request.product_id.clone(),
            ));
        }
        if request.model.model.model_id != self.model.descriptor().model.model_id {
            return Err(SmokeEmbeddingsError::UnsupportedModel(
                request.model.model.model_id.clone(),
            ));
        }
        if request.inputs.is_empty() {
            return Err(SmokeEmbeddingsError::EmptyInputBatch);
        }

        let mut embeddings = Vec::with_capacity(request.inputs.len());
        for (index, input) in request.inputs.iter().enumerate() {
            embeddings.push(EmbeddingVector {
                index,
                values: self.embed_one(input)?,
            });
        }

        Ok(EmbeddingResponse::new(request, embeddings))
    }
}

fn build_smoke_graph(
    model: &SmokeByteEmbedder,
    input_shape: Shape,
) -> Result<(Graph, TensorId, TensorId), GraphError> {
    let mut builder = GraphBuilder::new(Device::cpu());
    let input = builder.input("features", input_shape, DType::F32);
    let weights = builder.constant_f32(
        Shape::new(vec![model.input_dimensions(), model.descriptor().dimensions]),
        model.projection().to_vec(),
    )?;
    let bias = builder.constant_f32(
        Shape::new(vec![1, model.descriptor().dimensions]),
        model.bias().to_vec(),
    )?;
    let projected = builder.matmul(&input, &weights)?;
    let shifted = builder.add(&projected, &bias)?;
    let output_id = shifted.id();
    let input_id = input.id();
    let graph = builder.finish(vec![shifted]);
    Ok((graph, input_id, output_id))
}

#[cfg(test)]
mod tests {
    use super::{
        EmbeddingRequest, EmbeddingResponse, EmbeddingVector, EmbeddingsExecutor,
        SmokeEmbeddingsService,
    };
    use rustygrad_models::{EmbeddingModelDescriptor, EmbeddingNormalization, ModelDescriptor};

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

    #[test]
    fn smoke_embeddings_service_is_deterministic() -> Result<(), Box<dyn std::error::Error>> {
        let mut service = SmokeEmbeddingsService::new()?;
        let request = EmbeddingRequest::new(
            "req-4",
            service.model_descriptor().clone(),
            vec![String::from("hello world")],
        );

        let first = service.embed(&request)?;
        let second = service.embed(&request)?;
        assert_eq!(first, second);
        assert_eq!(first.metadata.dimensions, 8);
        Ok(())
    }
}
