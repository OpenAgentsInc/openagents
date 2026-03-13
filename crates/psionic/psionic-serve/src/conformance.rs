use std::{
    collections::BTreeMap,
    fs,
    io::{BufRead, BufReader},
    path::Path,
};

use psionic_catalog::{
    BlobReadPreference, LocalBlobOpenOptions, OllamaLayerKind, OllamaManifest, OllamaModelCatalog,
    OllamaModelConfig,
};
use psionic_models::{
    GgufBlobArtifact, GgufMetadataValue, GoldenPromptRole, ModelIngressSurface,
    ModelInteropBoundary, ModelRuntimeSurface, ModelServingSurface, digest_chat_template,
    golden_prompt_fixture,
};
use psionic_runtime::{EmbeddingParityBudget, compare_embedding_vectors};
use reqwest::blocking::{Client, Response};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Map, Value, json};
use thiserror::Error;

use crate::{
    DecoderModelDescriptor, EmbeddingModelDescriptor, EmbeddingResponse, EmbeddingVector,
    GenerationResponse, TerminationReason, WeightBundleMetadata,
};

/// Surface compared by the Ollama-to-Psionic cutover harness.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConformanceSurface {
    /// Installed model discovery (`tags` / `list_models`).
    Tags,
    /// Model detail inspection (`show` / `show_model`).
    Show,
    /// Loaded-model inspection (`ps` / `loaded_models`).
    Ps,
    /// Non-streaming generation semantics.
    Generate,
    /// Streaming generation semantics.
    GenerateStream,
    /// Embeddings semantics.
    Embed,
}

/// Final result for one conformance check.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConformanceCheckStatus {
    /// Baseline and candidate matched the configured contract.
    Passed,
    /// Baseline and candidate differed in an unapproved way.
    Failed,
    /// The candidate could not run the surface at all.
    Unsupported,
    /// A configured intentional difference was observed explicitly.
    IntentionalDifference,
}

/// Serializable subject output for one surface.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubjectObservation<T> {
    /// The subject produced a comparable observation.
    Supported(T),
    /// The subject does not yet implement the surface honestly.
    Unsupported {
        /// Plain-language reason recorded in the report.
        reason: String,
    },
}

/// Normalized semantic error payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticError {
    /// Stable status code when one exists.
    pub status: u16,
    /// Stable plain-language message.
    pub message: String,
}

impl SemanticError {
    /// Creates a new semantic error payload.
    #[must_use]
    pub fn new(status: u16, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }
}

/// Comparable model summary from `tags` / `list_models`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelSummary {
    /// Stable display name.
    pub name: String,
    /// Stable digest when known.
    pub digest: Option<String>,
    /// Model family when known.
    pub family: Option<String>,
    /// Weight format when known.
    pub format: Option<String>,
    /// Quantization label when known.
    pub quantization: Option<String>,
    /// Total model size when known.
    pub size_bytes: Option<u64>,
    /// Remote upstream host when this is a remote alias.
    pub remote_host: Option<String>,
    /// Remote upstream model when this is a remote alias.
    pub remote_model: Option<String>,
}

impl ModelSummary {
    /// Creates a summary from a loaded decoder descriptor.
    #[must_use]
    pub fn from_decoder_descriptor(
        name: impl Into<String>,
        descriptor: &DecoderModelDescriptor,
    ) -> Self {
        Self {
            name: name.into(),
            digest: Some(descriptor.weights.digest.clone()),
            family: Some(descriptor.model.family.clone()),
            format: Some(serialize_enum_string(&descriptor.weights.format)),
            quantization: Some(serialize_enum_string(&descriptor.weights.quantization)),
            size_bytes: None,
            remote_host: None,
            remote_model: None,
        }
    }

    /// Creates a summary from a loaded embeddings descriptor.
    #[must_use]
    pub fn from_embedding_descriptor(
        name: impl Into<String>,
        descriptor: &EmbeddingModelDescriptor,
    ) -> Self {
        Self {
            name: name.into(),
            digest: Some(descriptor.weights.digest.clone()),
            family: Some(descriptor.model.family.clone()),
            format: Some(serialize_enum_string(&descriptor.weights.format)),
            quantization: Some(serialize_enum_string(&descriptor.weights.quantization)),
            size_bytes: None,
            remote_host: None,
            remote_model: None,
        }
    }
}

/// Comparable `tags` / `list_models` result.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListModelsObservation {
    /// Returned models in stable name order.
    pub models: Vec<ModelSummary>,
    /// Semantic error instead of a model list.
    pub error: Option<SemanticError>,
}

impl ListModelsObservation {
    /// Creates a successful list response and sorts the models by name.
    #[must_use]
    pub fn new(mut models: Vec<ModelSummary>) -> Self {
        models.sort_by(|left, right| left.name.cmp(&right.name));
        Self {
            models,
            error: None,
        }
    }

    /// Creates an error response.
    #[must_use]
    pub fn error(status: u16, message: impl Into<String>) -> Self {
        Self {
            models: Vec::new(),
            error: Some(SemanticError::new(status, message)),
        }
    }
}

/// Comparable `show` response.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShowObservation {
    /// Requested model name.
    pub model: String,
    /// Weight format or outer artifact format.
    pub format: Option<String>,
    /// Primary model family.
    pub family: Option<String>,
    /// Additional family aliases.
    pub families: Vec<String>,
    /// Quantization label.
    pub quantization: Option<String>,
    /// Stable chat-template digest when known.
    pub chat_template_digest: Option<String>,
    /// Stable capability labels.
    pub capabilities: Vec<String>,
    /// Stable fact map for the selected subset of tokenizer/model info.
    pub facts: BTreeMap<String, String>,
    /// Semantic error instead of model details.
    pub error: Option<SemanticError>,
}

impl ShowObservation {
    /// Creates a comparable `show` record from a decoder descriptor.
    #[must_use]
    pub fn from_decoder_descriptor(
        name: impl Into<String>,
        descriptor: &DecoderModelDescriptor,
    ) -> Self {
        Self::from_weight_bundle(
            name,
            Some(descriptor.model.family.clone()),
            vec![descriptor.model.family.clone()],
            &descriptor.weights,
            vec![String::from("generate")],
            &descriptor.interop_boundary(),
        )
    }

    /// Creates a comparable `show` record from an embeddings descriptor.
    #[must_use]
    pub fn from_embedding_descriptor(
        name: impl Into<String>,
        descriptor: &EmbeddingModelDescriptor,
    ) -> Self {
        Self::from_weight_bundle(
            name,
            Some(descriptor.model.family.clone()),
            vec![descriptor.model.family.clone()],
            &descriptor.weights,
            vec![String::from("embed")],
            &descriptor.interop_boundary(),
        )
    }

    #[must_use]
    fn from_weight_bundle(
        name: impl Into<String>,
        family: Option<String>,
        mut families: Vec<String>,
        weights: &WeightBundleMetadata,
        mut capabilities: Vec<String>,
        boundary: &ModelInteropBoundary,
    ) -> Self {
        families.sort();
        families.dedup();
        capabilities.sort();
        capabilities.dedup();

        let mut facts = BTreeMap::new();
        facts.insert(
            String::from("psionic.weight_bundle_digest"),
            weights.digest.clone(),
        );
        facts.insert(
            String::from("psionic.quantization_mode"),
            serialize_enum_string(&weights.quantization),
        );
        facts.insert(
            String::from("psionic.weight_format"),
            serialize_enum_string(&weights.format),
        );
        insert_psionic_interop_boundary_facts(&mut facts, boundary);

        Self {
            model: name.into(),
            format: Some(serialize_enum_string(&weights.format)),
            family,
            families,
            quantization: Some(serialize_enum_string(&weights.quantization)),
            chat_template_digest: None,
            capabilities,
            facts,
            error: None,
        }
    }

    /// Creates an error response.
    #[must_use]
    pub fn error(model: impl Into<String>, status: u16, message: impl Into<String>) -> Self {
        Self {
            model: model.into(),
            format: None,
            family: None,
            families: Vec::new(),
            quantization: None,
            chat_template_digest: None,
            capabilities: Vec::new(),
            facts: BTreeMap::new(),
            error: Some(SemanticError::new(status, message)),
        }
    }
}

fn insert_psionic_interop_boundary_facts(
    facts: &mut BTreeMap<String, String>,
    boundary: &ModelInteropBoundary,
) {
    if let Some(catalog_surface) = boundary.catalog_surface {
        facts.insert(
            String::from("psionic.catalog_surface"),
            serialize_enum_string(&catalog_surface),
        );
    }
    facts.insert(
        String::from("psionic.model_ingress_surface"),
        serialize_enum_string(&boundary.ingress_surface),
    );
    facts.insert(
        String::from("psionic.serving_surface"),
        serialize_enum_string(&boundary.serving_surface),
    );
    facts.insert(
        String::from("psionic.runtime_surface"),
        serialize_enum_string(&boundary.runtime_surface),
    );
}

/// Comparable loaded-model row from `ps` / `loaded_models`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoadedModelSummary {
    /// Stable display name.
    pub model: String,
    /// Model family when known.
    pub family: Option<String>,
    /// Quantization label when known.
    pub quantization: Option<String>,
    /// Stable digest when known.
    pub digest: Option<String>,
    /// Maximum context length when known.
    pub context_length: Option<usize>,
    /// Total size in bytes when known.
    pub size_bytes: Option<u64>,
    /// Device memory residency size when known.
    pub size_vram_bytes: Option<u64>,
    /// Backend label when the candidate can expose it honestly.
    pub backend: Option<String>,
    /// Explicit fallback state when the candidate can expose it honestly.
    pub fallback_state: Option<String>,
}

impl LoadedModelSummary {
    /// Creates a loaded-model row from a decoder descriptor.
    #[must_use]
    pub fn from_decoder_descriptor(
        model: impl Into<String>,
        descriptor: &DecoderModelDescriptor,
    ) -> Self {
        Self {
            model: model.into(),
            family: Some(descriptor.model.family.clone()),
            quantization: Some(serialize_enum_string(&descriptor.weights.quantization)),
            digest: Some(descriptor.weights.digest.clone()),
            context_length: Some(descriptor.config.max_context),
            size_bytes: None,
            size_vram_bytes: None,
            backend: None,
            fallback_state: None,
        }
    }
}

/// Comparable `ps` / `loaded_models` result.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoadedModelsObservation {
    /// Loaded models in stable display-name order.
    pub models: Vec<LoadedModelSummary>,
    /// Semantic error instead of loaded-model data.
    pub error: Option<SemanticError>,
}

impl LoadedModelsObservation {
    /// Creates a successful loaded-model snapshot.
    #[must_use]
    pub fn new(mut models: Vec<LoadedModelSummary>) -> Self {
        models.sort_by(|left, right| left.model.cmp(&right.model));
        Self {
            models,
            error: None,
        }
    }

    /// Creates an error result.
    #[must_use]
    pub fn error(status: u16, message: impl Into<String>) -> Self {
        Self {
            models: Vec::new(),
            error: Some(SemanticError::new(status, message)),
        }
    }
}

/// Reference into the golden prompt corpus.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PromptFixtureCaseRef {
    /// Stable golden fixture identifier.
    pub fixture_id: String,
    /// Stable template-variant identifier.
    pub template_variant_id: String,
    /// Stable render-case identifier.
    pub render_case_id: String,
}

/// Comparable generate case.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GenerateConformanceCase {
    /// Stable case identifier.
    pub id: String,
    /// Model name to call.
    pub model: String,
    /// Prompt text.
    pub prompt: String,
    /// Optional system message.
    pub system: Option<String>,
    /// Optional suffix for insert-style generation.
    pub suffix: Option<String>,
    /// Whether to exercise streaming semantics.
    pub stream: bool,
    /// Whether to request Ollama debug prompt rendering instead of real decode.
    pub debug_render_only: bool,
    /// Output token cap when the subject supports it.
    pub max_output_tokens: Option<usize>,
    /// Explicit stop sequences.
    pub stop_sequences: Vec<String>,
    /// Seed for deterministic decode when the subject supports it.
    pub seed: Option<u64>,
    /// Top-logprobs request when exercising validation/error semantics.
    pub top_logprobs: Option<i32>,
    /// Expected rendered prompt from the golden fixture when known.
    pub expected_rendered_prompt: Option<String>,
    /// Golden fixture reference when the case was fixture-derived.
    pub prompt_fixture: Option<PromptFixtureCaseRef>,
    /// Explicitly allowed candidate difference for this non-streaming case.
    pub expected_candidate_difference: Option<String>,
    /// Explicitly allowed candidate difference for the streaming case.
    pub expected_candidate_stream_difference: Option<String>,
}

impl GenerateConformanceCase {
    /// Creates a generate case from a single-turn golden prompt fixture.
    pub fn from_generate_compatible_prompt_fixture(
        id: impl Into<String>,
        model: impl Into<String>,
        fixture_id: &str,
        template_variant_id: &str,
        render_case_id: &str,
    ) -> Result<Self, ConformanceSubjectError> {
        let fixture = golden_prompt_fixture(fixture_id).ok_or_else(|| {
            ConformanceSubjectError::InvalidCase {
                case_id: render_case_id.to_string(),
                message: format!("unknown prompt fixture `{fixture_id}`"),
            }
        })?;
        let variant = fixture
            .template_variant(template_variant_id)
            .ok_or_else(|| ConformanceSubjectError::InvalidCase {
                case_id: render_case_id.to_string(),
                message: format!(
                    "unknown template variant `{template_variant_id}` in fixture `{fixture_id}`"
                ),
            })?;
        let render_case = variant.render_case(render_case_id).ok_or_else(|| {
            ConformanceSubjectError::InvalidCase {
                case_id: render_case_id.to_string(),
                message: format!(
                    "unknown render case `{render_case_id}` in fixture `{fixture_id}`"
                ),
            }
        })?;
        if !render_case.add_generation_prompt {
            return Err(ConformanceSubjectError::InvalidCase {
                case_id: render_case_id.to_string(),
                message: String::from(
                    "generate-compatible fixture cases must enable add_generation_prompt",
                ),
            });
        }

        let mut system = None;
        let mut prompt = None;
        for (index, message) in render_case.messages.iter().enumerate() {
            match message.role {
                GoldenPromptRole::System if index == 0 && system.is_none() => {
                    system = Some(String::from(message.content));
                }
                GoldenPromptRole::User if prompt.is_none() => {
                    prompt = Some(String::from(message.content));
                }
                _ => {
                    return Err(ConformanceSubjectError::InvalidCase {
                        case_id: render_case_id.to_string(),
                        message: String::from(
                            "only single-turn system-plus-user fixture cases can be mapped to /api/generate today",
                        ),
                    });
                }
            }
        }

        let prompt = prompt.ok_or_else(|| ConformanceSubjectError::InvalidCase {
            case_id: render_case_id.to_string(),
            message: String::from("generate-compatible fixture cases must contain a user turn"),
        })?;

        Ok(Self {
            id: id.into(),
            model: model.into(),
            prompt,
            system,
            suffix: None,
            stream: false,
            debug_render_only: true,
            max_output_tokens: None,
            stop_sequences: variant
                .stop_sequences
                .iter()
                .map(|stop| (*stop).to_string())
                .collect(),
            seed: None,
            top_logprobs: None,
            expected_rendered_prompt: Some(String::from(render_case.expected_rendered)),
            prompt_fixture: Some(PromptFixtureCaseRef {
                fixture_id: fixture_id.to_string(),
                template_variant_id: template_variant_id.to_string(),
                render_case_id: render_case_id.to_string(),
            }),
            expected_candidate_difference: None,
            expected_candidate_stream_difference: None,
        })
    }

    fn difference_reason(&self, surface: ConformanceSurface) -> Option<&str> {
        match surface {
            ConformanceSurface::Generate => self.expected_candidate_difference.as_deref(),
            ConformanceSurface::GenerateStream => {
                self.expected_candidate_stream_difference.as_deref()
            }
            _ => None,
        }
    }
}

/// Comparable non-streaming generation observation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GenerateObservation {
    /// Rendered prompt when the subject can expose it honestly.
    pub rendered_prompt: Option<String>,
    /// Output text.
    pub output_text: String,
    /// Terminal done reason when one exists.
    pub done_reason: Option<String>,
    /// Prompt-token count when exposed by the subject.
    pub prompt_eval_count: Option<usize>,
    /// Output-token count when exposed by the subject.
    pub eval_count: Option<usize>,
    /// Performance metrics when the subject can expose them honestly.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub performance: Option<GeneratePerformanceObservation>,
    /// Semantic error instead of a successful response.
    pub error: Option<SemanticError>,
}

/// Comparable generation performance metrics.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GeneratePerformanceObservation {
    /// End-to-end generation duration in nanoseconds.
    pub total_duration_ns: u64,
    /// Model-load or compile duration attributable to this request.
    pub load_duration_ns: u64,
    /// Prompt token count.
    pub prompt_eval_count: usize,
    /// Prompt-evaluation duration in nanoseconds.
    pub prompt_eval_duration_ns: u64,
    /// Output token count.
    pub eval_count: usize,
    /// Output-generation duration in nanoseconds.
    pub eval_duration_ns: u64,
}

impl GenerateObservation {
    /// Converts a Psionic generation response into a comparable observation.
    #[must_use]
    pub fn from_response(response: &GenerationResponse) -> Self {
        Self {
            rendered_prompt: None,
            output_text: response.output.text.clone(),
            done_reason: Some(termination_reason_label(response.termination)),
            prompt_eval_count: Some(response.usage.input_tokens),
            eval_count: Some(response.usage.output_tokens),
            performance: match (
                response.metrics.total_duration_ns,
                response.metrics.load_duration_ns,
                response.metrics.prompt_eval_count,
                response.metrics.prompt_eval_duration_ns,
                response.metrics.eval_count,
                response.metrics.eval_duration_ns,
            ) {
                (
                    Some(total_duration_ns),
                    Some(load_duration_ns),
                    Some(prompt_eval_count),
                    Some(prompt_eval_duration_ns),
                    Some(eval_count),
                    Some(eval_duration_ns),
                ) => Some(GeneratePerformanceObservation {
                    total_duration_ns,
                    load_duration_ns,
                    prompt_eval_count,
                    prompt_eval_duration_ns,
                    eval_count,
                    eval_duration_ns,
                }),
                _ => None,
            },
            error: None,
        }
    }

    /// Creates an error observation.
    #[must_use]
    pub fn error(status: u16, message: impl Into<String>) -> Self {
        Self {
            rendered_prompt: None,
            output_text: String::new(),
            done_reason: None,
            prompt_eval_count: None,
            eval_count: None,
            performance: None,
            error: Some(SemanticError::new(status, message)),
        }
    }
}

/// Comparable single stream chunk.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GenerateStreamChunk {
    /// Stable chunk index.
    pub index: usize,
    /// Text carried by the chunk.
    pub output_text: String,
    /// Whether this chunk was terminal.
    pub done: bool,
    /// Terminal reason on the chunk when one exists.
    pub done_reason: Option<String>,
    /// Chunk-local semantic error.
    pub error: Option<SemanticError>,
}

/// Comparable streaming generation transcript.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GenerateStreamObservation {
    /// Rendered prompt when the subject can expose it honestly.
    pub rendered_prompt: Option<String>,
    /// Stream transcript in stable order.
    pub chunks: Vec<GenerateStreamChunk>,
    /// Semantic error before any chunk was emitted.
    pub error: Option<SemanticError>,
}

impl GenerateStreamObservation {
    /// Converts a non-streaming Psionic response into a single final stream chunk.
    #[must_use]
    pub fn single_chunk_from_response(response: &GenerationResponse) -> Self {
        Self {
            rendered_prompt: None,
            chunks: vec![GenerateStreamChunk {
                index: 0,
                output_text: response.output.text.clone(),
                done: true,
                done_reason: Some(termination_reason_label(response.termination)),
                error: None,
            }],
            error: None,
        }
    }

    /// Creates a top-level stream error.
    #[must_use]
    pub fn error(status: u16, message: impl Into<String>) -> Self {
        Self {
            rendered_prompt: None,
            chunks: Vec::new(),
            error: Some(SemanticError::new(status, message)),
        }
    }
}

/// Comparable embeddings case.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EmbedConformanceCase {
    /// Stable case identifier.
    pub id: String,
    /// Model name to call.
    pub model: String,
    /// Input batch.
    pub inputs: Vec<String>,
    /// Whether the backend should truncate over-long inputs when it supports that control.
    pub truncate: Option<bool>,
    /// Requested output dimensions when the backend supports that control.
    pub output_dimensions: Option<usize>,
    /// Explicit drift budget to use for the compared vectors.
    pub drift_budget: EmbeddingParityBudget,
    /// Explicitly allowed candidate difference.
    pub expected_candidate_difference: Option<String>,
}

/// Comparable single embedding vector.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EmbedVectorObservation {
    /// Input index.
    pub index: usize,
    /// Embedding values.
    pub values: Vec<f32>,
}

/// Comparable embeddings observation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EmbedObservation {
    /// Returned vectors.
    pub vectors: Vec<EmbedVectorObservation>,
    /// Stable embedding dimensions when known.
    pub dimensions: Option<usize>,
    /// Whether every returned vector is approximately normalized.
    pub normalized: Option<bool>,
    /// Performance metrics when the subject can expose them honestly.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub performance: Option<EmbedPerformanceObservation>,
    /// Semantic error instead of embeddings.
    pub error: Option<SemanticError>,
}

/// Comparable embeddings performance metrics.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmbedPerformanceObservation {
    /// End-to-end embeddings duration in nanoseconds.
    pub total_duration_ns: u64,
    /// Model-load or compile duration attributable to this request.
    pub load_duration_ns: u64,
    /// Prompt token count when exposed honestly.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_eval_count: Option<usize>,
    /// Prompt-evaluation duration in nanoseconds when measured.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_eval_duration_ns: Option<u64>,
}

impl EmbedObservation {
    /// Converts a Psionic embeddings response into a comparable observation.
    #[must_use]
    pub fn from_response(response: &EmbeddingResponse) -> Self {
        let vectors = response
            .embeddings
            .iter()
            .map(EmbedVectorObservation::from_embedding_vector)
            .collect::<Vec<_>>();
        Self {
            dimensions: Some(response.metadata.dimensions),
            normalized: Some(
                response
                    .embeddings
                    .iter()
                    .all(|vector| is_normalized(vector.values.as_slice())),
            ),
            vectors,
            performance: match (
                response.metrics.total_duration_ns,
                response.metrics.load_duration_ns,
            ) {
                (Some(total_duration_ns), Some(load_duration_ns)) => {
                    Some(EmbedPerformanceObservation {
                        total_duration_ns,
                        load_duration_ns,
                        prompt_eval_count: response.metrics.prompt_eval_count,
                        prompt_eval_duration_ns: response.metrics.prompt_eval_duration_ns,
                    })
                }
                _ => None,
            },
            error: None,
        }
    }

    /// Creates an error observation.
    #[must_use]
    pub fn error(status: u16, message: impl Into<String>) -> Self {
        Self {
            vectors: Vec::new(),
            dimensions: None,
            normalized: None,
            performance: None,
            error: Some(SemanticError::new(status, message)),
        }
    }
}

impl EmbedVectorObservation {
    #[must_use]
    fn from_embedding_vector(vector: &EmbeddingVector) -> Self {
        Self {
            index: vector.index,
            values: vector.values.clone(),
        }
    }
}

/// Harness configuration for one cutover review.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ConformanceSuite {
    /// Stable suite identifier.
    pub id: String,
    /// Whether to compare installed-model discovery.
    pub compare_tags: bool,
    /// Whether to compare loaded-model inspection.
    pub compare_ps: bool,
    /// Explicit `show` cases.
    pub show_cases: Vec<ShowConformanceCase>,
    /// Explicit `generate` cases.
    pub generate_cases: Vec<GenerateConformanceCase>,
    /// Explicit `embed` cases.
    pub embed_cases: Vec<EmbedConformanceCase>,
}

impl ConformanceSuite {
    /// Creates an empty suite with `tags` and `ps` checks enabled.
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            compare_tags: true,
            compare_ps: true,
            show_cases: Vec::new(),
            generate_cases: Vec::new(),
            embed_cases: Vec::new(),
        }
    }
}

/// Explicit `show` case in the suite.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShowConformanceCase {
    /// Stable case identifier.
    pub id: String,
    /// Model name to inspect.
    pub model: String,
    /// Explicitly allowed candidate difference.
    pub expected_candidate_difference: Option<String>,
}

/// One check result in the report artifact.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ConformanceCheckResult {
    /// Compared surface.
    pub surface: ConformanceSurface,
    /// Stable case identifier.
    pub case_id: String,
    /// Final check status.
    pub status: ConformanceCheckStatus,
    /// High-signal explanation.
    pub detail: String,
    /// Baseline observation in report-friendly form.
    pub baseline: Value,
    /// Candidate observation in report-friendly form.
    pub candidate: Value,
}

/// Summary counters over all checks in the report.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConformanceSummary {
    /// Passing checks.
    pub passed: usize,
    /// Failing checks.
    pub failed: usize,
    /// Unsupported checks.
    pub unsupported: usize,
    /// Intentional differences.
    pub intentional_differences: usize,
}

impl ConformanceSummary {
    #[must_use]
    fn from_checks(checks: &[ConformanceCheckResult]) -> Self {
        let mut summary = Self::default();
        for check in checks {
            match check.status {
                ConformanceCheckStatus::Passed => summary.passed += 1,
                ConformanceCheckStatus::Failed => summary.failed += 1,
                ConformanceCheckStatus::Unsupported => summary.unsupported += 1,
                ConformanceCheckStatus::IntentionalDifference => {
                    summary.intentional_differences += 1;
                }
            }
        }
        summary
    }
}

/// Structured pass/fail artifact emitted by the harness.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ConformanceReport {
    /// Stable suite identifier.
    pub suite_id: String,
    /// Baseline subject label.
    pub baseline_subject: String,
    /// Candidate subject label.
    pub candidate_subject: String,
    /// Per-surface results.
    pub checks: Vec<ConformanceCheckResult>,
    /// Aggregate counters.
    pub summary: ConformanceSummary,
}

impl ConformanceReport {
    /// Returns whether the suite is honest enough for cutover.
    #[must_use]
    pub fn cutover_ready(&self) -> bool {
        self.summary.failed == 0 && self.summary.unsupported == 0
    }

    /// Evaluates the performance gate for the current report.
    #[must_use]
    pub fn performance_gate(
        &self,
        thresholds: &CutoverPerformanceThresholds,
    ) -> PerformanceGateReport {
        let checks = self
            .checks
            .iter()
            .filter_map(|check| match check.surface {
                ConformanceSurface::Generate => {
                    Some(evaluate_generate_performance_gate(check, thresholds))
                }
                ConformanceSurface::Embed => {
                    Some(evaluate_embed_performance_gate(check, thresholds))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        PerformanceGateReport {
            thresholds: thresholds.clone(),
            summary: PerformanceGateSummary::from_checks(&checks),
            checks,
        }
    }

    /// Returns whether semantic and performance gates are both satisfied.
    #[must_use]
    pub fn cutover_ready_with_performance(
        &self,
        thresholds: &CutoverPerformanceThresholds,
    ) -> bool {
        self.cutover_ready() && self.performance_gate(thresholds).cutover_ready()
    }

    /// Serializes the report as pretty JSON.
    pub fn to_pretty_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}

/// Ratio-based acceptance thresholds for cutover performance.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CutoverPerformanceThresholds {
    /// Maximum allowed candidate-vs-baseline total-duration ratio for generation.
    pub max_generation_total_duration_ratio: f32,
    /// Maximum allowed candidate-vs-baseline load-duration ratio for generation.
    pub max_generation_load_duration_ratio: f32,
    /// Minimum allowed candidate-vs-baseline prompt throughput ratio for generation.
    pub min_generation_prompt_tokens_per_second_ratio: f32,
    /// Minimum allowed candidate-vs-baseline decode throughput ratio for generation.
    pub min_generation_eval_tokens_per_second_ratio: f32,
    /// Maximum allowed candidate-vs-baseline total-duration ratio for embeddings.
    pub max_embedding_total_duration_ratio: f32,
    /// Maximum allowed candidate-vs-baseline load-duration ratio for embeddings.
    pub max_embedding_load_duration_ratio: f32,
}

impl Default for CutoverPerformanceThresholds {
    fn default() -> Self {
        Self {
            max_generation_total_duration_ratio: 1.25,
            max_generation_load_duration_ratio: 1.25,
            min_generation_prompt_tokens_per_second_ratio: 0.80,
            min_generation_eval_tokens_per_second_ratio: 0.80,
            max_embedding_total_duration_ratio: 1.25,
            max_embedding_load_duration_ratio: 1.25,
        }
    }
}

/// Final status for one performance gate check.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PerformanceGateStatus {
    /// Candidate satisfied the configured threshold.
    Passed,
    /// Candidate violated the configured threshold.
    Failed,
    /// The check could not run honestly because evidence was missing.
    InsufficientEvidence,
}

/// One evaluated cutover performance check.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PerformanceGateCheck {
    /// Surface being evaluated.
    pub surface: ConformanceSurface,
    /// Stable case identifier.
    pub case_id: String,
    /// Gate result.
    pub status: PerformanceGateStatus,
    /// Human-readable summary of the decision.
    pub detail: String,
    /// Baseline performance facts used by the gate.
    pub baseline: Value,
    /// Candidate performance facts used by the gate.
    pub candidate: Value,
}

/// Summary counters over all performance checks.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PerformanceGateSummary {
    /// Passing checks.
    pub passed: usize,
    /// Failing checks.
    pub failed: usize,
    /// Checks blocked by missing evidence.
    pub insufficient_evidence: usize,
}

impl PerformanceGateSummary {
    #[must_use]
    fn from_checks(checks: &[PerformanceGateCheck]) -> Self {
        let mut summary = Self::default();
        for check in checks {
            match check.status {
                PerformanceGateStatus::Passed => summary.passed += 1,
                PerformanceGateStatus::Failed => summary.failed += 1,
                PerformanceGateStatus::InsufficientEvidence => summary.insufficient_evidence += 1,
            }
        }
        summary
    }
}

/// Structured performance report emitted from a conformance report.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PerformanceGateReport {
    /// Thresholds applied to the report.
    pub thresholds: CutoverPerformanceThresholds,
    /// Individual evaluated checks.
    pub checks: Vec<PerformanceGateCheck>,
    /// Aggregate counters.
    pub summary: PerformanceGateSummary,
}

impl PerformanceGateReport {
    /// Returns whether every evaluated performance check passed.
    #[must_use]
    pub fn cutover_ready(&self) -> bool {
        !self.checks.is_empty()
            && self.summary.failed == 0
            && self.summary.insufficient_evidence == 0
    }
}

/// Artifact-writing failure.
#[derive(Debug, Error)]
pub enum ConformanceArtifactError {
    /// Writing the report failed.
    #[error("failed to write conformance report `{path}`: {message}")]
    Write {
        /// Destination path.
        path: String,
        /// Failure summary.
        message: String,
    },
    /// Encoding the report failed.
    #[error("failed to encode conformance report: {0}")]
    Encode(#[from] serde_json::Error),
}

fn evaluate_generate_performance_gate(
    check: &ConformanceCheckResult,
    thresholds: &CutoverPerformanceThresholds,
) -> PerformanceGateCheck {
    if check.status != ConformanceCheckStatus::Passed {
        return insufficient_performance_check(
            check,
            format!("semantic conformance status was {:?}", check.status),
        );
    }
    let baseline = match serde_json::from_value::<GenerateObservation>(check.baseline.clone()) {
        Ok(value) => value,
        Err(error) => {
            return insufficient_performance_check(
                check,
                format!("failed to decode baseline generation observation: {error}"),
            );
        }
    };
    let candidate = match serde_json::from_value::<GenerateObservation>(check.candidate.clone()) {
        Ok(value) => value,
        Err(error) => {
            return insufficient_performance_check(
                check,
                format!("failed to decode candidate generation observation: {error}"),
            );
        }
    };
    let Some(baseline_perf) = baseline.performance else {
        return insufficient_performance_check(
            check,
            String::from("baseline generation observation missing performance metrics"),
        );
    };
    let Some(candidate_perf) = candidate.performance else {
        return insufficient_performance_check(
            check,
            String::from("candidate generation observation missing performance metrics"),
        );
    };

    let Some(baseline_prompt_tps) = tokens_per_second(
        baseline_perf.prompt_eval_count,
        baseline_perf.prompt_eval_duration_ns,
    ) else {
        return insufficient_performance_check(
            check,
            String::from("baseline generation prompt throughput is undefined"),
        );
    };
    let Some(candidate_prompt_tps) = tokens_per_second(
        candidate_perf.prompt_eval_count,
        candidate_perf.prompt_eval_duration_ns,
    ) else {
        return insufficient_performance_check(
            check,
            String::from("candidate generation prompt throughput is undefined"),
        );
    };
    let Some(baseline_eval_tps) =
        tokens_per_second(baseline_perf.eval_count, baseline_perf.eval_duration_ns)
    else {
        return insufficient_performance_check(
            check,
            String::from("baseline generation decode throughput is undefined"),
        );
    };
    let Some(candidate_eval_tps) =
        tokens_per_second(candidate_perf.eval_count, candidate_perf.eval_duration_ns)
    else {
        return insufficient_performance_check(
            check,
            String::from("candidate generation decode throughput is undefined"),
        );
    };

    let mut failures = Vec::new();
    if ratio(
        candidate_perf.total_duration_ns,
        baseline_perf.total_duration_ns,
    ) > f64::from(thresholds.max_generation_total_duration_ratio)
    {
        failures.push(format!(
            "candidate total_duration ratio {:.3} exceeded {:.3}",
            ratio(
                candidate_perf.total_duration_ns,
                baseline_perf.total_duration_ns
            ),
            thresholds.max_generation_total_duration_ratio,
        ));
    }
    if baseline_perf.load_duration_ns == 0 {
        if candidate_perf.load_duration_ns != 0 {
            failures.push(format!(
                "candidate load_duration {}ns must remain 0ns when baseline is 0ns",
                candidate_perf.load_duration_ns
            ));
        }
    } else if ratio(
        candidate_perf.load_duration_ns,
        baseline_perf.load_duration_ns,
    ) > f64::from(thresholds.max_generation_load_duration_ratio)
    {
        failures.push(format!(
            "candidate load_duration ratio {:.3} exceeded {:.3}",
            ratio(
                candidate_perf.load_duration_ns,
                baseline_perf.load_duration_ns
            ),
            thresholds.max_generation_load_duration_ratio,
        ));
    }
    if candidate_prompt_tps
        < baseline_prompt_tps * f64::from(thresholds.min_generation_prompt_tokens_per_second_ratio)
    {
        failures.push(format!(
            "candidate prompt throughput {:.3} tok/s fell below {:.3} tok/s",
            candidate_prompt_tps,
            baseline_prompt_tps
                * f64::from(thresholds.min_generation_prompt_tokens_per_second_ratio),
        ));
    }
    if candidate_eval_tps
        < baseline_eval_tps * f64::from(thresholds.min_generation_eval_tokens_per_second_ratio)
    {
        failures.push(format!(
            "candidate decode throughput {:.3} tok/s fell below {:.3} tok/s",
            candidate_eval_tps,
            baseline_eval_tps * f64::from(thresholds.min_generation_eval_tokens_per_second_ratio),
        ));
    }

    build_performance_check(
        check,
        failures,
        json!({
            "total_duration_ns": baseline_perf.total_duration_ns,
            "load_duration_ns": baseline_perf.load_duration_ns,
            "prompt_tokens_per_second": baseline_prompt_tps,
            "eval_tokens_per_second": baseline_eval_tps,
        }),
        json!({
            "total_duration_ns": candidate_perf.total_duration_ns,
            "load_duration_ns": candidate_perf.load_duration_ns,
            "prompt_tokens_per_second": candidate_prompt_tps,
            "eval_tokens_per_second": candidate_eval_tps,
        }),
        format!(
            "generation total ratio {:.3}, prompt ratio {:.3}, eval ratio {:.3}",
            ratio(
                candidate_perf.total_duration_ns,
                baseline_perf.total_duration_ns
            ),
            candidate_prompt_tps / baseline_prompt_tps,
            candidate_eval_tps / baseline_eval_tps,
        ),
    )
}

fn evaluate_embed_performance_gate(
    check: &ConformanceCheckResult,
    thresholds: &CutoverPerformanceThresholds,
) -> PerformanceGateCheck {
    if check.status != ConformanceCheckStatus::Passed {
        return insufficient_performance_check(
            check,
            format!("semantic conformance status was {:?}", check.status),
        );
    }
    let baseline = match serde_json::from_value::<EmbedObservation>(check.baseline.clone()) {
        Ok(value) => value,
        Err(error) => {
            return insufficient_performance_check(
                check,
                format!("failed to decode baseline embeddings observation: {error}"),
            );
        }
    };
    let candidate = match serde_json::from_value::<EmbedObservation>(check.candidate.clone()) {
        Ok(value) => value,
        Err(error) => {
            return insufficient_performance_check(
                check,
                format!("failed to decode candidate embeddings observation: {error}"),
            );
        }
    };
    let Some(baseline_perf) = baseline.performance else {
        return insufficient_performance_check(
            check,
            String::from("baseline embeddings observation missing performance metrics"),
        );
    };
    let Some(candidate_perf) = candidate.performance else {
        return insufficient_performance_check(
            check,
            String::from("candidate embeddings observation missing performance metrics"),
        );
    };

    let mut failures = Vec::new();
    if ratio(
        candidate_perf.total_duration_ns,
        baseline_perf.total_duration_ns,
    ) > f64::from(thresholds.max_embedding_total_duration_ratio)
    {
        failures.push(format!(
            "candidate embeddings total_duration ratio {:.3} exceeded {:.3}",
            ratio(
                candidate_perf.total_duration_ns,
                baseline_perf.total_duration_ns
            ),
            thresholds.max_embedding_total_duration_ratio,
        ));
    }
    if baseline_perf.load_duration_ns == 0 {
        if candidate_perf.load_duration_ns != 0 {
            failures.push(format!(
                "candidate embeddings load_duration {}ns must remain 0ns when baseline is 0ns",
                candidate_perf.load_duration_ns
            ));
        }
    } else if ratio(
        candidate_perf.load_duration_ns,
        baseline_perf.load_duration_ns,
    ) > f64::from(thresholds.max_embedding_load_duration_ratio)
    {
        failures.push(format!(
            "candidate embeddings load_duration ratio {:.3} exceeded {:.3}",
            ratio(
                candidate_perf.load_duration_ns,
                baseline_perf.load_duration_ns
            ),
            thresholds.max_embedding_load_duration_ratio,
        ));
    }

    build_performance_check(
        check,
        failures,
        json!({
            "total_duration_ns": baseline_perf.total_duration_ns,
            "load_duration_ns": baseline_perf.load_duration_ns,
            "prompt_eval_count": baseline_perf.prompt_eval_count,
            "prompt_eval_duration_ns": baseline_perf.prompt_eval_duration_ns,
        }),
        json!({
            "total_duration_ns": candidate_perf.total_duration_ns,
            "load_duration_ns": candidate_perf.load_duration_ns,
            "prompt_eval_count": candidate_perf.prompt_eval_count,
            "prompt_eval_duration_ns": candidate_perf.prompt_eval_duration_ns,
        }),
        format!(
            "embeddings total ratio {:.3}",
            ratio(
                candidate_perf.total_duration_ns,
                baseline_perf.total_duration_ns
            ),
        ),
    )
}

fn build_performance_check(
    check: &ConformanceCheckResult,
    failures: Vec<String>,
    baseline: Value,
    candidate: Value,
    success_detail: String,
) -> PerformanceGateCheck {
    if failures.is_empty() {
        PerformanceGateCheck {
            surface: check.surface,
            case_id: check.case_id.clone(),
            status: PerformanceGateStatus::Passed,
            detail: success_detail,
            baseline,
            candidate,
        }
    } else {
        PerformanceGateCheck {
            surface: check.surface,
            case_id: check.case_id.clone(),
            status: PerformanceGateStatus::Failed,
            detail: failures.join("; "),
            baseline,
            candidate,
        }
    }
}

fn insufficient_performance_check(
    check: &ConformanceCheckResult,
    detail: String,
) -> PerformanceGateCheck {
    PerformanceGateCheck {
        surface: check.surface,
        case_id: check.case_id.clone(),
        status: PerformanceGateStatus::InsufficientEvidence,
        detail,
        baseline: check.baseline.clone(),
        candidate: check.candidate.clone(),
    }
}

fn tokens_per_second(tokens: usize, duration_ns: u64) -> Option<f64> {
    (duration_ns > 0).then_some((tokens as f64) * 1_000_000_000.0 / (duration_ns as f64))
}

fn ratio(candidate: u64, baseline: u64) -> f64 {
    if baseline == 0 {
        if candidate == 0 { 1.0 } else { f64::INFINITY }
    } else {
        (candidate as f64) / (baseline as f64)
    }
}

/// Comparable subject boundary for the cutover harness.
pub trait ConformanceSubject {
    /// Stable human-readable subject label.
    fn label(&self) -> &str;

    /// Returns the comparable `tags` / `list_models` view.
    fn tags(
        &mut self,
    ) -> Result<SubjectObservation<ListModelsObservation>, ConformanceSubjectError>;

    /// Returns the comparable `show` / `show_model` view.
    fn show(
        &mut self,
        case: &ShowConformanceCase,
    ) -> Result<SubjectObservation<ShowObservation>, ConformanceSubjectError>;

    /// Returns the comparable `ps` / `loaded_models` view.
    fn ps(
        &mut self,
    ) -> Result<SubjectObservation<LoadedModelsObservation>, ConformanceSubjectError>;

    /// Executes a comparable non-streaming generation case.
    fn generate(
        &mut self,
        case: &GenerateConformanceCase,
    ) -> Result<SubjectObservation<GenerateObservation>, ConformanceSubjectError>;

    /// Executes a comparable streaming generation case.
    fn generate_stream(
        &mut self,
        case: &GenerateConformanceCase,
    ) -> Result<SubjectObservation<GenerateStreamObservation>, ConformanceSubjectError>;

    /// Executes a comparable embeddings case.
    fn embed(
        &mut self,
        case: &EmbedConformanceCase,
    ) -> Result<SubjectObservation<EmbedObservation>, ConformanceSubjectError>;
}

/// Harness/adapter failure.
#[derive(Debug, Error)]
pub enum ConformanceSubjectError {
    /// A request case could not be represented honestly.
    #[error("invalid conformance case `{case_id}`: {message}")]
    InvalidCase {
        /// Stable case identifier.
        case_id: String,
        /// Failure summary.
        message: String,
    },
    /// The subject transport failed.
    #[error("subject transport failure for `{context}`: {message}")]
    Transport {
        /// Surface or endpoint description.
        context: String,
        /// Failure summary.
        message: String,
    },
    /// The subject returned a body the adapter could not decode.
    #[error("failed to decode subject response for `{context}`: {message}")]
    Decode {
        /// Surface or endpoint description.
        context: String,
        /// Failure summary.
        message: String,
    },
}

/// Simple recorded subject used by tests and by callers that already have
/// comparable Psionic observations in memory.
pub struct RecordedConformanceSubject {
    label: String,
    tags: SubjectObservation<ListModelsObservation>,
    ps: SubjectObservation<LoadedModelsObservation>,
    show: BTreeMap<String, SubjectObservation<ShowObservation>>,
    generate: BTreeMap<String, SubjectObservation<GenerateObservation>>,
    generate_stream: BTreeMap<String, SubjectObservation<GenerateStreamObservation>>,
    embed: BTreeMap<String, SubjectObservation<EmbedObservation>>,
}

impl RecordedConformanceSubject {
    /// Creates an empty recorded subject.
    #[must_use]
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            tags: SubjectObservation::Unsupported {
                reason: String::from("no tags observation recorded"),
            },
            ps: SubjectObservation::Unsupported {
                reason: String::from("no loaded-model observation recorded"),
            },
            show: BTreeMap::new(),
            generate: BTreeMap::new(),
            generate_stream: BTreeMap::new(),
            embed: BTreeMap::new(),
        }
    }

    /// Records the comparable `tags` observation.
    #[must_use]
    pub fn with_tags(mut self, observation: SubjectObservation<ListModelsObservation>) -> Self {
        self.tags = observation;
        self
    }

    /// Records the comparable `ps` observation.
    #[must_use]
    pub fn with_ps(mut self, observation: SubjectObservation<LoadedModelsObservation>) -> Self {
        self.ps = observation;
        self
    }

    /// Records a `show` case by model name.
    #[must_use]
    pub fn with_show(
        mut self,
        model: impl Into<String>,
        observation: SubjectObservation<ShowObservation>,
    ) -> Self {
        self.show.insert(model.into(), observation);
        self
    }

    /// Records a non-streaming `generate` case by case ID.
    #[must_use]
    pub fn with_generate_case(
        mut self,
        case_id: impl Into<String>,
        observation: SubjectObservation<GenerateObservation>,
    ) -> Self {
        self.generate.insert(case_id.into(), observation);
        self
    }

    /// Records a streaming `generate` case by case ID.
    #[must_use]
    pub fn with_generate_stream_case(
        mut self,
        case_id: impl Into<String>,
        observation: SubjectObservation<GenerateStreamObservation>,
    ) -> Self {
        self.generate_stream.insert(case_id.into(), observation);
        self
    }

    /// Records an `embed` case by case ID.
    #[must_use]
    pub fn with_embed_case(
        mut self,
        case_id: impl Into<String>,
        observation: SubjectObservation<EmbedObservation>,
    ) -> Self {
        self.embed.insert(case_id.into(), observation);
        self
    }
}

impl ConformanceSubject for RecordedConformanceSubject {
    fn label(&self) -> &str {
        self.label.as_str()
    }

    fn tags(
        &mut self,
    ) -> Result<SubjectObservation<ListModelsObservation>, ConformanceSubjectError> {
        Ok(self.tags.clone())
    }

    fn show(
        &mut self,
        case: &ShowConformanceCase,
    ) -> Result<SubjectObservation<ShowObservation>, ConformanceSubjectError> {
        Ok(self
            .show
            .get(case.model.as_str())
            .cloned()
            .unwrap_or_else(|| SubjectObservation::Unsupported {
                reason: format!("no show observation recorded for model `{}`", case.model),
            }))
    }

    fn ps(
        &mut self,
    ) -> Result<SubjectObservation<LoadedModelsObservation>, ConformanceSubjectError> {
        Ok(self.ps.clone())
    }

    fn generate(
        &mut self,
        case: &GenerateConformanceCase,
    ) -> Result<SubjectObservation<GenerateObservation>, ConformanceSubjectError> {
        Ok(self
            .generate
            .get(case.id.as_str())
            .cloned()
            .unwrap_or_else(|| SubjectObservation::Unsupported {
                reason: format!("no generate observation recorded for case `{}`", case.id),
            }))
    }

    fn generate_stream(
        &mut self,
        case: &GenerateConformanceCase,
    ) -> Result<SubjectObservation<GenerateStreamObservation>, ConformanceSubjectError> {
        Ok(self
            .generate_stream
            .get(case.id.as_str())
            .cloned()
            .unwrap_or_else(|| SubjectObservation::Unsupported {
                reason: format!(
                    "no streaming generate observation recorded for case `{}`",
                    case.id
                ),
            }))
    }

    fn embed(
        &mut self,
        case: &EmbedConformanceCase,
    ) -> Result<SubjectObservation<EmbedObservation>, ConformanceSubjectError> {
        Ok(self
            .embed
            .get(case.id.as_str())
            .cloned()
            .unwrap_or_else(|| SubjectObservation::Unsupported {
                reason: format!("no embed observation recorded for case `{}`", case.id),
            }))
    }
}

/// Local installed-model subject backed directly by the shared Ollama catalog substrate.
pub struct LocalOllamaCatalogSubject {
    label: String,
    catalog: OllamaModelCatalog,
    blob_options: LocalBlobOpenOptions,
}

impl LocalOllamaCatalogSubject {
    /// Creates a local subject rooted at an Ollama models directory.
    #[must_use]
    pub fn new(models_root: impl AsRef<Path>) -> Self {
        let catalog = OllamaModelCatalog::new(models_root);
        Self {
            label: format!("psionic-local@{}", catalog.models_root().display()),
            catalog,
            blob_options: LocalBlobOpenOptions::default(),
        }
    }

    /// Overrides the blob-open options used for local layer and GGUF reads.
    #[must_use]
    pub fn with_blob_options(mut self, blob_options: LocalBlobOpenOptions) -> Self {
        self.blob_options = blob_options;
        self
    }

    /// Returns the local `tags` / `list_models` observation.
    #[must_use]
    pub fn list_models_observation(&self) -> ListModelsObservation {
        let discovery = match self.catalog.discover_models() {
            Ok(discovery) => discovery,
            Err(error) => return ListModelsObservation::error(500, error.to_string()),
        };

        let mut models = Vec::new();
        for manifest in discovery.manifests {
            let config = if manifest.config.is_some() {
                match manifest.load_config(self.small_blob_options()) {
                    Ok(config) => config,
                    Err(_) => continue,
                }
            } else {
                None
            };

            models.push(ModelSummary {
                name: manifest.short_name,
                digest: Some(manifest.manifest_sha256),
                family: config
                    .as_ref()
                    .and_then(OllamaModelConfig::family)
                    .map(str::to_string),
                format: config
                    .as_ref()
                    .and_then(OllamaModelConfig::format)
                    .map(str::to_string),
                quantization: config
                    .as_ref()
                    .and_then(OllamaModelConfig::quantization_level)
                    .map(str::to_string),
                size_bytes: Some(manifest.total_blob_size_bytes),
                remote_host: config
                    .as_ref()
                    .and_then(OllamaModelConfig::remote_host)
                    .map(str::to_string),
                remote_model: config
                    .as_ref()
                    .and_then(OllamaModelConfig::remote_model)
                    .map(str::to_string),
            });
        }

        ListModelsObservation::new(models)
    }

    /// Returns the local `show` / `show_model` observation for one model.
    #[must_use]
    pub fn show_model_observation(&self, model: &str) -> ShowObservation {
        let manifest = match self.catalog.resolve_model(model) {
            Ok(manifest) => manifest,
            Err(error) => return show_catalog_error(model, error),
        };

        let config = match manifest.load_config(self.small_blob_options()) {
            Ok(config) => config,
            Err(error) => return ShowObservation::error(model, 500, error.to_string()),
        };

        match self.build_show_observation(model, &manifest, config.as_ref()) {
            Ok(observation) => observation,
            Err(message) => ShowObservation::error(model, 500, message),
        }
    }

    fn build_show_observation(
        &self,
        requested_model: &str,
        manifest: &OllamaManifest,
        config: Option<&OllamaModelConfig>,
    ) -> Result<ShowObservation, String> {
        let template = manifest
            .load_template(self.small_blob_options())
            .map_err(|error| error.to_string())?;
        let gguf_artifact = manifest
            .primary_model_layer()
            .map(|layer| {
                GgufBlobArtifact::open_ollama_blob(
                    self.catalog.models_root(),
                    layer.digest.as_str(),
                    self.blob_options.clone(),
                )
                .map_err(|error| error.to_string())
            })
            .transpose()?;
        let model_metadata = gguf_artifact
            .as_ref()
            .map(|artifact| artifact.content().metadata());

        let mut format = config
            .and_then(OllamaModelConfig::format)
            .map(str::to_string);
        if format.is_none() && gguf_artifact.is_some() {
            format = Some(String::from("gguf"));
        }

        let mut family = config
            .and_then(OllamaModelConfig::family)
            .map(str::to_string);
        if family.is_none() {
            family = model_metadata
                .and_then(|metadata| metadata.get("general.architecture"))
                .and_then(GgufMetadataValue::as_str)
                .map(str::to_string);
        }

        let mut facts = if let Some(metadata) = model_metadata {
            select_model_info_facts(local_gguf_model_info(metadata))
        } else {
            select_model_info_facts(remote_model_info(config))
        };
        insert_psionic_interop_boundary_facts(
            &mut facts,
            &ModelInteropBoundary {
                catalog_surface: Some(manifest.catalog_surface()),
                ingress_surface: ModelIngressSurface::OllamaCompatManifestImport,
                serving_surface: ModelServingSurface::OllamaCompatMigration,
                runtime_surface: ModelRuntimeSurface::PsionicNative,
            },
        );
        let adapter_policy = manifest.adapter_policy_status();
        if adapter_policy.adapter_layer_count > 0 {
            facts.insert(
                String::from("psionic.ollama_adapter_policy"),
                adapter_policy.policy.to_string(),
            );
            facts.insert(
                String::from("psionic.ollama_adapter_layer_count"),
                adapter_policy.adapter_layer_count.to_string(),
            );
            facts.insert(
                String::from("psionic.ollama_adapter_manifest_supported"),
                adapter_policy.supported.to_string(),
            );
        }

        Ok(ShowObservation {
            model: requested_model.to_string(),
            format,
            family,
            families: config.map_or_else(Vec::new, OllamaModelConfig::families),
            quantization: config
                .and_then(OllamaModelConfig::quantization_level)
                .map(str::to_string),
            chat_template_digest: template
                .as_deref()
                .filter(|value| !value.is_empty())
                .map(digest_chat_template),
            capabilities: derive_local_capabilities(
                requested_model,
                config,
                manifest,
                template.as_deref(),
                model_metadata,
            ),
            facts,
            error: None,
        })
    }

    fn small_blob_options(&self) -> LocalBlobOpenOptions {
        self.blob_options
            .clone()
            .with_read_preference(BlobReadPreference::PreferBuffered)
    }
}

impl ConformanceSubject for LocalOllamaCatalogSubject {
    fn label(&self) -> &str {
        self.label.as_str()
    }

    fn tags(
        &mut self,
    ) -> Result<SubjectObservation<ListModelsObservation>, ConformanceSubjectError> {
        Ok(SubjectObservation::Supported(
            self.list_models_observation(),
        ))
    }

    fn show(
        &mut self,
        case: &ShowConformanceCase,
    ) -> Result<SubjectObservation<ShowObservation>, ConformanceSubjectError> {
        Ok(SubjectObservation::Supported(
            self.show_model_observation(case.model.as_str()),
        ))
    }

    fn ps(
        &mut self,
    ) -> Result<SubjectObservation<LoadedModelsObservation>, ConformanceSubjectError> {
        Ok(SubjectObservation::Unsupported {
            reason: String::from("loaded-model lifecycle is not implemented yet"),
        })
    }

    fn generate(
        &mut self,
        _case: &GenerateConformanceCase,
    ) -> Result<SubjectObservation<GenerateObservation>, ConformanceSubjectError> {
        Ok(SubjectObservation::Unsupported {
            reason: String::from("text generation is not implemented by the catalog subject"),
        })
    }

    fn generate_stream(
        &mut self,
        _case: &GenerateConformanceCase,
    ) -> Result<SubjectObservation<GenerateStreamObservation>, ConformanceSubjectError> {
        Ok(SubjectObservation::Unsupported {
            reason: String::from("streaming generation is not implemented by the catalog subject"),
        })
    }

    fn embed(
        &mut self,
        _case: &EmbedConformanceCase,
    ) -> Result<SubjectObservation<EmbedObservation>, ConformanceSubjectError> {
        Ok(SubjectObservation::Unsupported {
            reason: String::from("embeddings are not implemented by the catalog subject"),
        })
    }
}

/// Live adapter for the subset of Ollama HTTP behavior the desktop depends on.
pub struct OllamaHttpSubject {
    label: String,
    base_url: String,
    client: Client,
}

impl OllamaHttpSubject {
    /// Creates a new live Ollama subject.
    pub fn new(base_url: impl Into<String>) -> Result<Self, ConformanceSubjectError> {
        let base_url = normalize_base_url(base_url.into());
        let client =
            Client::builder()
                .build()
                .map_err(|error| ConformanceSubjectError::Transport {
                    context: String::from("ollama client init"),
                    message: error.to_string(),
                })?;
        Ok(Self {
            label: format!("ollama@{base_url}"),
            base_url,
            client,
        })
    }

    fn get(&self, path: &str) -> Result<Response, ConformanceSubjectError> {
        let url = format!("{}{}", self.base_url, path);
        self.client
            .get(url.as_str())
            .send()
            .map_err(|error| ConformanceSubjectError::Transport {
                context: path.to_string(),
                message: error.to_string(),
            })
    }

    fn post_json(&self, path: &str, payload: &Value) -> Result<Response, ConformanceSubjectError> {
        let url = format!("{}{}", self.base_url, path);
        self.client
            .post(url.as_str())
            .json(payload)
            .send()
            .map_err(|error| ConformanceSubjectError::Transport {
                context: path.to_string(),
                message: error.to_string(),
            })
    }

    fn decode_json<T: DeserializeOwned>(
        &self,
        path: &str,
        response: Response,
    ) -> Result<T, ConformanceSubjectError> {
        response
            .json::<T>()
            .map_err(|error| ConformanceSubjectError::Decode {
                context: path.to_string(),
                message: error.to_string(),
            })
    }

    fn error_from_response(
        &self,
        path: &str,
        response: Response,
    ) -> Result<SemanticError, ConformanceSubjectError> {
        let status = response.status().as_u16();
        let body = response
            .text()
            .map_err(|error| ConformanceSubjectError::Decode {
                context: path.to_string(),
                message: error.to_string(),
            })?;
        Ok(parse_error_body(status, body.as_str()))
    }
}

impl ConformanceSubject for OllamaHttpSubject {
    fn label(&self) -> &str {
        self.label.as_str()
    }

    fn tags(
        &mut self,
    ) -> Result<SubjectObservation<ListModelsObservation>, ConformanceSubjectError> {
        let response = self.get("/api/tags")?;
        if !response.status().is_success() {
            return Ok(SubjectObservation::Supported(ListModelsObservation {
                models: Vec::new(),
                error: Some(self.error_from_response("/api/tags", response)?),
            }));
        }
        let payload: OllamaListResponse = self.decode_json("/api/tags", response)?;
        let models = payload
            .models
            .into_iter()
            .map(|model| ModelSummary {
                name: model.name,
                digest: model.digest,
                family: model.details.family,
                format: model.details.format,
                quantization: model.details.quantization_level,
                size_bytes: model.size,
                remote_host: model.remote_host,
                remote_model: model.remote_model,
            })
            .collect::<Vec<_>>();
        Ok(SubjectObservation::Supported(ListModelsObservation::new(
            models,
        )))
    }

    fn show(
        &mut self,
        case: &ShowConformanceCase,
    ) -> Result<SubjectObservation<ShowObservation>, ConformanceSubjectError> {
        let payload = json!({ "model": case.model });
        let response = self.post_json("/api/show", &payload)?;
        if !response.status().is_success() {
            return Ok(SubjectObservation::Supported(ShowObservation {
                model: case.model.clone(),
                format: None,
                family: None,
                families: Vec::new(),
                quantization: None,
                chat_template_digest: None,
                capabilities: Vec::new(),
                facts: BTreeMap::new(),
                error: Some(self.error_from_response("/api/show", response)?),
            }));
        }
        let payload: OllamaShowResponse = self.decode_json("/api/show", response)?;
        let chat_template_digest = match payload.template.as_deref() {
            Some(template) if !template.is_empty() => Some(digest_chat_template(template)),
            _ => None,
        };
        Ok(SubjectObservation::Supported(ShowObservation {
            model: case.model.clone(),
            format: payload.details.format,
            family: payload.details.family,
            families: sorted_strings(payload.details.families.unwrap_or_default()),
            quantization: payload.details.quantization_level,
            chat_template_digest,
            capabilities: sorted_strings(payload.capabilities.unwrap_or_default()),
            facts: select_model_info_facts(payload.model_info.unwrap_or_default()),
            error: None,
        }))
    }

    fn ps(
        &mut self,
    ) -> Result<SubjectObservation<LoadedModelsObservation>, ConformanceSubjectError> {
        let response = self.get("/api/ps")?;
        if !response.status().is_success() {
            return Ok(SubjectObservation::Supported(LoadedModelsObservation {
                models: Vec::new(),
                error: Some(self.error_from_response("/api/ps", response)?),
            }));
        }
        let payload: OllamaProcessResponse = self.decode_json("/api/ps", response)?;
        let models = payload
            .models
            .into_iter()
            .map(|model| LoadedModelSummary {
                model: model.name,
                family: model.details.family,
                quantization: model.details.quantization_level,
                digest: model.digest,
                context_length: model.context_length,
                size_bytes: model.size.map(|size| size as u64),
                size_vram_bytes: model.size_vram.map(|size| size as u64),
                backend: None,
                fallback_state: None,
            })
            .collect::<Vec<_>>();
        Ok(SubjectObservation::Supported(LoadedModelsObservation::new(
            models,
        )))
    }

    fn generate(
        &mut self,
        case: &GenerateConformanceCase,
    ) -> Result<SubjectObservation<GenerateObservation>, ConformanceSubjectError> {
        if case.stream {
            return Err(ConformanceSubjectError::InvalidCase {
                case_id: case.id.clone(),
                message: String::from(
                    "non-streaming generate comparison cannot run a case flagged as streaming",
                ),
            });
        }
        let payload = build_generate_payload(case, false);
        let response = self.post_json("/api/generate", &payload)?;
        if !response.status().is_success() {
            return Ok(SubjectObservation::Supported(GenerateObservation::error(
                response.status().as_u16(),
                self.error_from_response("/api/generate", response)?.message,
            )));
        }
        let payload: OllamaGenerateResponse = self.decode_json("/api/generate", response)?;
        Ok(SubjectObservation::Supported(GenerateObservation {
            rendered_prompt: payload.debug_info.and_then(|debug| debug.rendered_template),
            output_text: payload.response.unwrap_or_default(),
            done_reason: payload.done_reason,
            prompt_eval_count: payload.prompt_eval_count,
            eval_count: payload.eval_count,
            performance: match (
                payload.total_duration,
                payload.load_duration,
                payload.prompt_eval_count,
                payload.prompt_eval_duration,
                payload.eval_count,
                payload.eval_duration,
            ) {
                (
                    Some(total_duration_ns),
                    Some(load_duration_ns),
                    Some(prompt_eval_count),
                    Some(prompt_eval_duration_ns),
                    Some(eval_count),
                    Some(eval_duration_ns),
                ) => Some(GeneratePerformanceObservation {
                    total_duration_ns,
                    load_duration_ns,
                    prompt_eval_count,
                    prompt_eval_duration_ns,
                    eval_count,
                    eval_duration_ns,
                }),
                _ => None,
            },
            error: None,
        }))
    }

    fn generate_stream(
        &mut self,
        case: &GenerateConformanceCase,
    ) -> Result<SubjectObservation<GenerateStreamObservation>, ConformanceSubjectError> {
        if !case.stream {
            return Err(ConformanceSubjectError::InvalidCase {
                case_id: case.id.clone(),
                message: String::from(
                    "streaming generate comparison requires a case flagged as streaming",
                ),
            });
        }
        if case.debug_render_only {
            return Err(ConformanceSubjectError::InvalidCase {
                case_id: case.id.clone(),
                message: String::from(
                    "debug_render_only is only supported for non-streaming cases",
                ),
            });
        }

        let payload = build_generate_payload(case, true);
        let response = self.post_json("/api/generate", &payload)?;
        let status = response.status().as_u16();
        if !response.status().is_success() {
            return Ok(SubjectObservation::Supported(
                GenerateStreamObservation::error(
                    status,
                    self.error_from_response("/api/generate", response)?.message,
                ),
            ));
        }

        let mut chunks = Vec::new();
        let mut reader = BufReader::new(response);
        let mut line = String::new();
        loop {
            line.clear();
            let read =
                reader
                    .read_line(&mut line)
                    .map_err(|error| ConformanceSubjectError::Decode {
                        context: String::from("/api/generate"),
                        message: error.to_string(),
                    })?;
            if read == 0 {
                break;
            }
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let value: Value =
                serde_json::from_str(trimmed).map_err(|error| ConformanceSubjectError::Decode {
                    context: String::from("/api/generate"),
                    message: error.to_string(),
                })?;
            if let Some(message) = value.get("error").and_then(Value::as_str) {
                if chunks.is_empty() {
                    return Ok(SubjectObservation::Supported(
                        GenerateStreamObservation::error(
                            value
                                .get("status")
                                .and_then(Value::as_u64)
                                .and_then(|status| u16::try_from(status).ok())
                                .unwrap_or(500),
                            message,
                        ),
                    ));
                }
                chunks.push(GenerateStreamChunk {
                    index: chunks.len(),
                    output_text: String::new(),
                    done: true,
                    done_reason: None,
                    error: Some(SemanticError::new(
                        value
                            .get("status")
                            .and_then(Value::as_u64)
                            .and_then(|status| u16::try_from(status).ok())
                            .unwrap_or(500),
                        message,
                    )),
                });
                break;
            }

            let payload: OllamaGenerateResponse =
                serde_json::from_value(value).map_err(|error| ConformanceSubjectError::Decode {
                    context: String::from("/api/generate"),
                    message: error.to_string(),
                })?;
            chunks.push(GenerateStreamChunk {
                index: chunks.len(),
                output_text: payload.response.unwrap_or_default(),
                done: payload.done.unwrap_or(false),
                done_reason: payload.done_reason,
                error: None,
            });
        }

        Ok(SubjectObservation::Supported(GenerateStreamObservation {
            rendered_prompt: None,
            chunks,
            error: None,
        }))
    }

    fn embed(
        &mut self,
        case: &EmbedConformanceCase,
    ) -> Result<SubjectObservation<EmbedObservation>, ConformanceSubjectError> {
        let payload = build_embed_payload(case);
        let response = self.post_json("/api/embed", &payload)?;
        if !response.status().is_success() {
            return Ok(SubjectObservation::Supported(EmbedObservation::error(
                response.status().as_u16(),
                self.error_from_response("/api/embed", response)?.message,
            )));
        }
        let payload: OllamaEmbedResponse = self.decode_json("/api/embed", response)?;
        let vectors = payload
            .embeddings
            .iter()
            .enumerate()
            .map(|(index, values)| EmbedVectorObservation {
                index,
                values: values.clone(),
            })
            .collect::<Vec<_>>();
        Ok(SubjectObservation::Supported(EmbedObservation {
            dimensions: payload.embeddings.first().map(Vec::len),
            normalized: Some(
                payload
                    .embeddings
                    .iter()
                    .all(|values| is_normalized(values)),
            ),
            vectors,
            performance: match (payload.total_duration, payload.load_duration) {
                (Some(total_duration_ns), Some(load_duration_ns)) => {
                    Some(EmbedPerformanceObservation {
                        total_duration_ns,
                        load_duration_ns,
                        prompt_eval_count: payload.prompt_eval_count,
                        prompt_eval_duration_ns: payload.prompt_eval_duration,
                    })
                }
                _ => None,
            },
            error: None,
        }))
    }
}

/// Runs a conformance suite against the baseline and candidate subjects.
pub fn run_conformance_suite(
    suite: &ConformanceSuite,
    baseline: &mut dyn ConformanceSubject,
    candidate: &mut dyn ConformanceSubject,
) -> Result<ConformanceReport, ConformanceSubjectError> {
    let mut checks = Vec::new();

    if suite.compare_tags {
        let baseline_tags = baseline.tags()?;
        let candidate_tags = candidate.tags()?;
        checks.push(compare_observations(
            ConformanceSurface::Tags,
            "tags",
            &baseline_tags,
            &candidate_tags,
            None,
            compare_list_models,
        )?);
    }

    for case in &suite.show_cases {
        let baseline_show = baseline.show(case)?;
        let candidate_show = candidate.show(case)?;
        checks.push(compare_observations(
            ConformanceSurface::Show,
            case.id.as_str(),
            &baseline_show,
            &candidate_show,
            case.expected_candidate_difference.as_deref(),
            compare_show,
        )?);
    }

    if suite.compare_ps {
        let baseline_ps = baseline.ps()?;
        let candidate_ps = candidate.ps()?;
        checks.push(compare_observations(
            ConformanceSurface::Ps,
            "ps",
            &baseline_ps,
            &candidate_ps,
            None,
            compare_loaded_models,
        )?);
    }

    for case in &suite.generate_cases {
        if case.stream {
            let baseline_generate = baseline.generate_stream(case)?;
            let candidate_generate = candidate.generate_stream(case)?;
            checks.push(compare_observations(
                ConformanceSurface::GenerateStream,
                case.id.as_str(),
                &baseline_generate,
                &candidate_generate,
                case.difference_reason(ConformanceSurface::GenerateStream),
                |baseline_value, candidate_value| {
                    compare_stream(case, baseline_value, candidate_value)
                },
            )?);
        } else {
            let baseline_generate = baseline.generate(case)?;
            let candidate_generate = candidate.generate(case)?;
            checks.push(compare_observations(
                ConformanceSurface::Generate,
                case.id.as_str(),
                &baseline_generate,
                &candidate_generate,
                case.difference_reason(ConformanceSurface::Generate),
                |baseline_value, candidate_value| {
                    compare_generate(case, baseline_value, candidate_value)
                },
            )?);
        }
    }

    for case in &suite.embed_cases {
        let baseline_embed = baseline.embed(case)?;
        let candidate_embed = candidate.embed(case)?;
        checks.push(compare_observations(
            ConformanceSurface::Embed,
            case.id.as_str(),
            &baseline_embed,
            &candidate_embed,
            case.expected_candidate_difference.as_deref(),
            |baseline_value, candidate_value| compare_embed(case, baseline_value, candidate_value),
        )?);
    }

    let summary = ConformanceSummary::from_checks(checks.as_slice());
    Ok(ConformanceReport {
        suite_id: suite.id.clone(),
        baseline_subject: baseline.label().to_string(),
        candidate_subject: candidate.label().to_string(),
        checks,
        summary,
    })
}

/// Writes a report artifact as pretty JSON.
pub fn write_conformance_report(
    path: impl AsRef<Path>,
    report: &ConformanceReport,
) -> Result<(), ConformanceArtifactError> {
    let path = path.as_ref();
    let body = report.to_pretty_json()?;
    fs::write(path, format!("{body}\n")).map_err(|error| ConformanceArtifactError::Write {
        path: path.display().to_string(),
        message: error.to_string(),
    })
}

fn compare_observations<T>(
    surface: ConformanceSurface,
    case_id: &str,
    baseline: &SubjectObservation<T>,
    candidate: &SubjectObservation<T>,
    allowed_difference: Option<&str>,
    compare: impl Fn(&T, &T) -> Result<(), String>,
) -> Result<ConformanceCheckResult, ConformanceSubjectError>
where
    T: Serialize,
{
    let baseline_json =
        serde_json::to_value(baseline).map_err(|error| ConformanceSubjectError::Decode {
            context: format!("{surface:?}:{case_id}:baseline"),
            message: error.to_string(),
        })?;
    let candidate_json =
        serde_json::to_value(candidate).map_err(|error| ConformanceSubjectError::Decode {
            context: format!("{surface:?}:{case_id}:candidate"),
            message: error.to_string(),
        })?;

    let (status, detail) = match (baseline, candidate) {
        (
            SubjectObservation::Supported(baseline_value),
            SubjectObservation::Supported(candidate_value),
        ) => match compare(baseline_value, candidate_value) {
            Ok(()) => (
                ConformanceCheckStatus::Passed,
                String::from("baseline and candidate matched"),
            ),
            Err(message) => match allowed_difference {
                Some(reason) => (
                    ConformanceCheckStatus::IntentionalDifference,
                    format!("{reason}; observed difference: {message}"),
                ),
                None => (ConformanceCheckStatus::Failed, message),
            },
        },
        (SubjectObservation::Supported(_), SubjectObservation::Unsupported { reason }) => {
            match allowed_difference {
                Some(allowed) => (
                    ConformanceCheckStatus::IntentionalDifference,
                    format!("{allowed}; candidate marked unsupported: {reason}"),
                ),
                None => (
                    ConformanceCheckStatus::Unsupported,
                    format!("candidate marked unsupported: {reason}"),
                ),
            }
        }
        (SubjectObservation::Unsupported { reason }, SubjectObservation::Supported(_)) => (
            ConformanceCheckStatus::Failed,
            format!("baseline unexpectedly marked unsupported: {reason}"),
        ),
        (
            SubjectObservation::Unsupported {
                reason: baseline_reason,
            },
            SubjectObservation::Unsupported {
                reason: candidate_reason,
            },
        ) => (
            ConformanceCheckStatus::Failed,
            format!(
                "baseline and candidate are both unsupported; baseline=`{baseline_reason}`, candidate=`{candidate_reason}`"
            ),
        ),
    };

    Ok(ConformanceCheckResult {
        surface,
        case_id: case_id.to_string(),
        status,
        detail,
        baseline: baseline_json,
        candidate: candidate_json,
    })
}

fn compare_list_models(
    baseline: &ListModelsObservation,
    candidate: &ListModelsObservation,
) -> Result<(), String> {
    if baseline == candidate {
        Ok(())
    } else {
        Err(format_observation_difference(
            "list-models mismatch",
            baseline,
            candidate,
        ))
    }
}

fn compare_show(baseline: &ShowObservation, candidate: &ShowObservation) -> Result<(), String> {
    let mut candidate = candidate.clone();
    candidate
        .facts
        .retain(|key, _| !key.starts_with("psionic."));
    if baseline == &candidate {
        Ok(())
    } else {
        Err(format_observation_difference(
            "show mismatch",
            baseline,
            &candidate,
        ))
    }
}

fn compare_loaded_models(
    baseline: &LoadedModelsObservation,
    candidate: &LoadedModelsObservation,
) -> Result<(), String> {
    if baseline == candidate {
        Ok(())
    } else {
        Err(format_observation_difference(
            "loaded-model mismatch",
            baseline,
            candidate,
        ))
    }
}

fn compare_generate(
    case: &GenerateConformanceCase,
    baseline: &GenerateObservation,
    candidate: &GenerateObservation,
) -> Result<(), String> {
    if let Some(expected_rendered_prompt) = case.expected_rendered_prompt.as_deref() {
        match baseline.rendered_prompt.as_deref() {
            Some(actual) if actual == expected_rendered_prompt => {}
            _ => {
                return Err(format!(
                    "baseline rendered prompt drifted from fixture truth for case `{}`",
                    case.id
                ));
            }
        }
    }

    if baseline.rendered_prompt == candidate.rendered_prompt
        && baseline.output_text == candidate.output_text
        && baseline.done_reason == candidate.done_reason
        && baseline.prompt_eval_count == candidate.prompt_eval_count
        && baseline.eval_count == candidate.eval_count
        && baseline.error == candidate.error
    {
        Ok(())
    } else {
        Err(format_observation_difference(
            "generate mismatch",
            baseline,
            candidate,
        ))
    }
}

fn compare_stream(
    _case: &GenerateConformanceCase,
    baseline: &GenerateStreamObservation,
    candidate: &GenerateStreamObservation,
) -> Result<(), String> {
    if baseline == candidate {
        Ok(())
    } else {
        Err(format_observation_difference(
            "generate-stream mismatch",
            baseline,
            candidate,
        ))
    }
}

fn compare_embed(
    case: &EmbedConformanceCase,
    baseline: &EmbedObservation,
    candidate: &EmbedObservation,
) -> Result<(), String> {
    if baseline.error != candidate.error {
        return Err(format_observation_difference(
            "embed error mismatch",
            baseline,
            candidate,
        ));
    }
    if baseline.error.is_some() {
        return Ok(());
    }
    if baseline.dimensions != candidate.dimensions || baseline.normalized != candidate.normalized {
        return Err(format_observation_difference(
            "embed metadata mismatch",
            baseline,
            candidate,
        ));
    }
    if baseline.vectors.len() != candidate.vectors.len() {
        return Err(format!(
            "embed vector-count mismatch: baseline={}, candidate={}",
            baseline.vectors.len(),
            candidate.vectors.len()
        ));
    }

    for (vector_index, (baseline_vector, candidate_vector)) in baseline
        .vectors
        .iter()
        .zip(candidate.vectors.iter())
        .enumerate()
    {
        if baseline_vector.index != candidate_vector.index {
            return Err(format!(
                "embed vector index mismatch at position {vector_index}: baseline={}, candidate={}",
                baseline_vector.index, candidate_vector.index
            ));
        }
        let summary = compare_embedding_vectors(
            baseline_vector.values.as_slice(),
            candidate_vector.values.as_slice(),
            case.drift_budget,
        )
        .map_err(|error| {
            format!(
                "embed parity comparison failed for vector {}: {error}",
                baseline_vector.index
            )
        })?;
        if !summary.within_budget {
            return Err(format!(
                "embed vector {} exceeded drift budget: max_abs_delta={}, max_rel_delta={}, cosine_similarity={}, first_failing_index={:?}, budget={:?}",
                baseline_vector.index,
                summary.max_abs_delta,
                summary.max_rel_delta,
                summary.cosine_similarity,
                summary.first_failing_index,
                case.drift_budget,
            ));
        }
    }

    Ok(())
}

fn format_observation_difference<T: Serialize>(label: &str, baseline: &T, candidate: &T) -> String {
    format!(
        "{label}\nbaseline={}\ncandidate={}",
        serde_json::to_string_pretty(baseline).unwrap_or_else(|_| String::from("<encode failed>")),
        serde_json::to_string_pretty(candidate).unwrap_or_else(|_| String::from("<encode failed>")),
    )
}

fn build_generate_payload(case: &GenerateConformanceCase, stream: bool) -> Value {
    let mut request = Map::new();
    request.insert(String::from("model"), Value::String(case.model.clone()));
    request.insert(String::from("prompt"), Value::String(case.prompt.clone()));
    request.insert(String::from("stream"), Value::Bool(stream));
    if let Some(system) = &case.system {
        request.insert(String::from("system"), Value::String(system.clone()));
    }
    if let Some(suffix) = &case.suffix {
        request.insert(String::from("suffix"), Value::String(suffix.clone()));
    }
    if case.debug_render_only {
        request.insert(String::from("_debug_render_only"), Value::Bool(true));
    }
    if let Some(top_logprobs) = case.top_logprobs {
        request.insert(String::from("top_logprobs"), Value::from(top_logprobs));
        request.insert(String::from("logprobs"), Value::Bool(top_logprobs > 0));
    }

    let mut options = Map::new();
    if let Some(max_output_tokens) = case.max_output_tokens {
        options.insert(String::from("num_predict"), Value::from(max_output_tokens));
    }
    if !case.stop_sequences.is_empty() {
        options.insert(String::from("stop"), json!(case.stop_sequences));
    }
    if let Some(seed) = case.seed {
        options.insert(String::from("seed"), Value::from(seed));
    }
    if !options.is_empty() {
        request.insert(String::from("options"), Value::Object(options));
    }

    Value::Object(request)
}

fn build_embed_payload(case: &EmbedConformanceCase) -> Value {
    let mut request = Map::new();
    request.insert(String::from("model"), Value::String(case.model.clone()));
    request.insert(String::from("input"), json!(case.inputs));
    if let Some(truncate) = case.truncate {
        request.insert(String::from("truncate"), Value::Bool(truncate));
    }
    if let Some(output_dimensions) = case.output_dimensions {
        request.insert(String::from("dimensions"), Value::from(output_dimensions));
    }
    Value::Object(request)
}

fn show_catalog_error(model: &str, error: psionic_catalog::CatalogError) -> ShowObservation {
    match error {
        psionic_catalog::CatalogError::InvalidModelName { .. } => {
            ShowObservation::error(model, 400, error.to_string())
        }
        psionic_catalog::CatalogError::MissingManifest { .. } => {
            ShowObservation::error(model, 404, format!("model '{model}' not found"))
        }
        psionic_catalog::CatalogError::InvalidManifestPath { .. }
        | psionic_catalog::CatalogError::ReadManifest { .. }
        | psionic_catalog::CatalogError::DecodeManifest { .. }
        | psionic_catalog::CatalogError::InvalidManifest { .. }
        | psionic_catalog::CatalogError::DecodeLayer { .. }
        | psionic_catalog::CatalogError::Blob(_) => {
            ShowObservation::error(model, 500, error.to_string())
        }
    }
}

fn derive_local_capabilities(
    requested_model: &str,
    config: Option<&OllamaModelConfig>,
    manifest: &OllamaManifest,
    template: Option<&str>,
    metadata: Option<&BTreeMap<String, GgufMetadataValue>>,
) -> Vec<String> {
    let mut capabilities = Vec::new();

    if let Some(metadata) = metadata {
        if metadata_contains_suffix_key(metadata, "pooling_type") {
            capabilities.push(String::from("embedding"));
        } else {
            capabilities.push(String::from("completion"));
        }
        if metadata_contains_suffix_key(metadata, "vision.block_count") {
            capabilities.push(String::from("vision"));
        }
    } else if let Some(config) = config {
        capabilities.extend(config.capabilities());
    }

    if let Some(template) = template {
        if template_contains_variable(template, "tools")
            || config
                .and_then(OllamaModelConfig::parser)
                .is_some_and(parser_has_tool_support)
        {
            capabilities.push(String::from("tools"));
        }
        if template_contains_variable(template, "suffix") {
            capabilities.push(String::from("insert"));
        }
    }

    if manifest
        .first_layer_of_kind(OllamaLayerKind::Projector)
        .is_some()
    {
        capabilities.push(String::from("vision"));
    }

    if !capabilities
        .iter()
        .any(|capability| capability == "thinking")
        && (template_has_thinking_markers(template)
            || config
                .and_then(OllamaModelConfig::parser)
                .is_some_and(parser_has_thinking_support)
            || config
                .and_then(OllamaModelConfig::family)
                .is_some_and(gpt_oss_family)
            || requested_model.contains("gpt-oss"))
    {
        capabilities.push(String::from("thinking"));
    }

    sorted_strings(capabilities)
}

fn remote_model_info(config: Option<&OllamaModelConfig>) -> BTreeMap<String, Value> {
    let mut model_info = BTreeMap::new();
    if let Some(family) = config.and_then(OllamaModelConfig::family) {
        model_info.insert(
            String::from("general.architecture"),
            Value::String(family.to_string()),
        );
    }
    model_info
}

fn local_gguf_model_info(
    metadata: &BTreeMap<String, GgufMetadataValue>,
) -> BTreeMap<String, Value> {
    const KEYS: [&str; 10] = [
        "general.architecture",
        "tokenizer.ggml.model",
        "tokenizer.ggml.pre",
        "tokenizer.ggml.add_bos_token",
        "tokenizer.ggml.add_eos_token",
        "tokenizer.ggml.bos_token_id",
        "tokenizer.ggml.eos_token_id",
        "tokenizer.ggml.eos_token_ids",
        "tokenizer.ggml.padding_token_id",
        "tokenizer.ggml.unknown_token_id",
    ];

    let mut model_info = BTreeMap::new();
    for key in KEYS {
        if let Some(value) = metadata.get(key) {
            model_info.insert(String::from(key), gguf_metadata_to_json(value));
        }
    }
    model_info
}

fn gguf_metadata_to_json(value: &GgufMetadataValue) -> Value {
    match value {
        GgufMetadataValue::U8(value) => Value::from(*value),
        GgufMetadataValue::I8(value) => Value::from(*value),
        GgufMetadataValue::U16(value) => Value::from(*value),
        GgufMetadataValue::I16(value) => Value::from(*value),
        GgufMetadataValue::U32(value) => Value::from(*value),
        GgufMetadataValue::I32(value) => Value::from(*value),
        GgufMetadataValue::U64(value) => Value::from(*value),
        GgufMetadataValue::I64(value) => Value::from(*value),
        GgufMetadataValue::F32(value) => serde_json::Number::from_f64(f64::from(*value))
            .map(Value::Number)
            .unwrap_or(Value::Null),
        GgufMetadataValue::F64(value) => serde_json::Number::from_f64(*value)
            .map(Value::Number)
            .unwrap_or(Value::Null),
        GgufMetadataValue::Bool(value) => Value::Bool(*value),
        GgufMetadataValue::String(value) => Value::String(value.clone()),
        GgufMetadataValue::Array(values) => {
            Value::Array(values.iter().map(gguf_metadata_to_json).collect())
        }
    }
}

fn metadata_contains_suffix_key(
    metadata: &BTreeMap<String, GgufMetadataValue>,
    suffix: &str,
) -> bool {
    metadata.contains_key(suffix)
        || metadata.keys().any(|key| {
            key.strip_suffix(suffix)
                .is_some_and(|prefix| prefix.ends_with('.'))
        })
}

fn template_contains_variable(template: &str, variable: &str) -> bool {
    let quoted_single = format!("'{variable}'");
    let quoted_double = format!("\"{variable}\"");
    let dotted = format!(".{variable}");
    template.contains(quoted_single.as_str())
        || template.contains(quoted_double.as_str())
        || template.contains(dotted.as_str())
        || template.contains(variable)
}

fn template_has_thinking_markers(template: Option<&str>) -> bool {
    template.is_some_and(|template| {
        (template.contains("<think>") && template.contains("</think>"))
            || (template.contains("<thinking>") && template.contains("</thinking>"))
    })
}

fn gpt_oss_family(family: &str) -> bool {
    matches!(family, "gptoss" | "gpt-oss")
}

fn parser_has_tool_support(parser: &str) -> bool {
    matches!(
        parser,
        "cogito"
            | "deepseek3"
            | "functiongemma"
            | "glm-4.7"
            | "glm-ocr"
            | "lfm2"
            | "lfm2-thinking"
            | "ministral"
            | "nemotron-3-nano"
            | "olmo3"
            | "qwen3"
            | "qwen3-thinking"
            | "qwen3-coder"
            | "qwen3-vl-instruct"
            | "qwen3-vl-thinking"
            | "qwen3.5"
    )
}

fn parser_has_thinking_support(parser: &str) -> bool {
    matches!(
        parser,
        "cogito"
            | "deepseek3"
            | "glm-4.7"
            | "lfm2-thinking"
            | "nemotron-3-nano"
            | "olmo3-think"
            | "qwen3-thinking"
            | "qwen3-vl-thinking"
            | "qwen3.5"
    )
}

fn parse_error_body(status: u16, body: &str) -> SemanticError {
    if let Ok(payload) = serde_json::from_str::<OllamaErrorBody>(body)
        && let Some(error) = payload.error
    {
        return SemanticError::new(status, error);
    }
    let message = body.trim();
    if message.is_empty() {
        SemanticError::new(status, "unknown error")
    } else {
        SemanticError::new(status, message)
    }
}

fn select_model_info_facts(model_info: BTreeMap<String, Value>) -> BTreeMap<String, String> {
    const KEYS: [&str; 11] = [
        "general.architecture",
        "general.name",
        "tokenizer.ggml.model",
        "tokenizer.ggml.pre",
        "tokenizer.ggml.add_bos_token",
        "tokenizer.ggml.add_eos_token",
        "tokenizer.ggml.bos_token_id",
        "tokenizer.ggml.eos_token_id",
        "tokenizer.ggml.eos_token_ids",
        "tokenizer.ggml.padding_token_id",
        "tokenizer.ggml.unknown_token_id",
    ];

    let mut facts = BTreeMap::new();
    for key in KEYS {
        if let Some(value) = model_info.get(key) {
            facts.insert(String::from(key), stable_value_string(value));
        }
    }
    facts
}

fn stable_value_string(value: &Value) -> String {
    match value {
        Value::String(value) => value.clone(),
        _ => serde_json::to_string(value).unwrap_or_else(|_| String::from("null")),
    }
}

fn sorted_strings(mut values: Vec<String>) -> Vec<String> {
    values.sort();
    values.dedup();
    values
}

fn is_normalized(values: &[f32]) -> bool {
    let norm_sq = values.iter().map(|value| value * value).sum::<f32>();
    (norm_sq.sqrt() - 1.0).abs() <= 1.0e-3
}

fn termination_reason_label(reason: TerminationReason) -> String {
    match reason {
        TerminationReason::EndOfSequence => String::from("stop"),
        TerminationReason::MaxOutputTokens => String::from("length"),
        TerminationReason::ContextLimit => String::from("context_limit"),
        TerminationReason::Cancelled => String::from("cancelled"),
        TerminationReason::Disconnected => String::from("disconnected"),
        TerminationReason::Error => String::from("error"),
    }
}

fn serialize_enum_string<T: Serialize>(value: &T) -> String {
    serde_json::to_string(value)
        .unwrap_or_else(|_| String::from("\"unknown\""))
        .trim_matches('"')
        .to_string()
}

fn normalize_base_url(base_url: String) -> String {
    base_url.trim_end_matches('/').to_string()
}

#[derive(Debug, Deserialize)]
struct OllamaErrorBody {
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OllamaModelDetails {
    format: Option<String>,
    family: Option<String>,
    #[serde(default)]
    families: Option<Vec<String>>,
    quantization_level: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OllamaListModel {
    name: String,
    digest: Option<String>,
    size: Option<u64>,
    remote_host: Option<String>,
    remote_model: Option<String>,
    details: OllamaModelDetails,
}

#[derive(Debug, Deserialize)]
struct OllamaListResponse {
    models: Vec<OllamaListModel>,
}

#[derive(Debug, Deserialize)]
struct OllamaShowResponse {
    template: Option<String>,
    #[serde(default)]
    capabilities: Option<Vec<String>>,
    #[serde(default)]
    model_info: Option<BTreeMap<String, Value>>,
    details: OllamaModelDetails,
}

#[derive(Debug, Deserialize)]
struct OllamaProcessModel {
    name: String,
    digest: Option<String>,
    size: Option<i64>,
    size_vram: Option<i64>,
    context_length: Option<usize>,
    details: OllamaModelDetails,
}

#[derive(Debug, Deserialize)]
struct OllamaProcessResponse {
    models: Vec<OllamaProcessModel>,
}

#[derive(Debug, Deserialize)]
struct OllamaDebugInfo {
    rendered_template: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OllamaGenerateResponse {
    response: Option<String>,
    done: Option<bool>,
    done_reason: Option<String>,
    total_duration: Option<u64>,
    load_duration: Option<u64>,
    prompt_eval_count: Option<usize>,
    prompt_eval_duration: Option<u64>,
    eval_count: Option<usize>,
    eval_duration: Option<u64>,
    #[serde(rename = "_debug_info")]
    debug_info: Option<OllamaDebugInfo>,
}

#[derive(Debug, Deserialize)]
struct OllamaEmbedResponse {
    embeddings: Vec<Vec<f32>>,
    total_duration: Option<u64>,
    load_duration: Option<u64>,
    prompt_eval_count: Option<usize>,
    prompt_eval_duration: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        io::{Read, Write},
        net::{Shutdown, TcpListener, TcpStream},
        path::Path,
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
        thread,
    };

    use psionic_core::QuantizationMode;
    use psionic_runtime::BackendParityPolicy;
    use sha2::{Digest, Sha256};

    #[test]
    fn generate_case_builder_uses_real_qwen2_fixture() -> Result<(), Box<dyn std::error::Error>> {
        let case = GenerateConformanceCase::from_generate_compatible_prompt_fixture(
            "qwen2-render",
            "qwen2",
            "qwen2",
            "qwen2.default",
            "qwen2.default_system",
        )?;

        assert_eq!(case.model, "qwen2");
        assert_eq!(case.prompt, "Summarize the roadmap.");
        assert_eq!(case.system, None);
        assert!(case.debug_render_only);
        assert!(case.stop_sequences.is_empty());
        assert_eq!(
            case.expected_rendered_prompt.as_deref(),
            Some(
                "<|im_start|>system\nYou are a helpful assistant<|im_end|>\n<|im_start|>user\nSummarize the roadmap.<|im_end|>\n<|im_start|>assistant\n"
            )
        );
        Ok(())
    }

    #[test]
    fn conformance_suite_accepts_matching_prompt_render_candidate()
    -> Result<(), Box<dyn std::error::Error>> {
        let case = GenerateConformanceCase::from_generate_compatible_prompt_fixture(
            "phi3-render",
            "phi3",
            "phi3",
            "phi3.default",
            "phi3.user_only",
        )?;
        let expected_rendered_prompt = match &case.expected_rendered_prompt {
            Some(value) => value.clone(),
            None => return Err("expected fixture render".into()),
        };

        let suite = ConformanceSuite {
            id: String::from("fixture-qwen-suite"),
            compare_tags: false,
            compare_ps: false,
            show_cases: Vec::new(),
            generate_cases: vec![case],
            embed_cases: Vec::new(),
        };

        let baseline = RecordedConformanceSubject::new("ollama-baseline").with_generate_case(
            "phi3-render",
            SubjectObservation::Supported(GenerateObservation {
                rendered_prompt: Some(expected_rendered_prompt.clone()),
                output_text: String::new(),
                done_reason: None,
                prompt_eval_count: None,
                eval_count: None,
                performance: None,
                error: None,
            }),
        );
        let candidate = RecordedConformanceSubject::new("psionic-candidate").with_generate_case(
            "phi3-render",
            SubjectObservation::Supported(GenerateObservation {
                rendered_prompt: Some(expected_rendered_prompt),
                output_text: String::new(),
                done_reason: None,
                prompt_eval_count: None,
                eval_count: None,
                performance: None,
                error: None,
            }),
        );

        let mut baseline = baseline;
        let mut candidate = candidate;
        let report = run_conformance_suite(&suite, &mut baseline, &mut candidate)?;

        assert_eq!(report.summary.passed, 1);
        assert_eq!(report.summary.failed, 0);
        assert_eq!(report.summary.unsupported, 0);
        assert_eq!(report.summary.intentional_differences, 0);
        assert!(report.cutover_ready());
        Ok(())
    }

    #[test]
    fn conformance_suite_fails_on_embed_drift() -> Result<(), Box<dyn std::error::Error>> {
        let suite = ConformanceSuite {
            id: String::from("embed-drift"),
            compare_tags: false,
            compare_ps: false,
            show_cases: Vec::new(),
            generate_cases: Vec::new(),
            embed_cases: vec![EmbedConformanceCase {
                id: String::from("embed-1"),
                model: String::from("embedder"),
                inputs: vec![String::from("hello")],
                truncate: None,
                output_dimensions: None,
                drift_budget: BackendParityPolicy::default()
                    .embedding_budget(QuantizationMode::None),
                expected_candidate_difference: None,
            }],
        };
        let baseline = RecordedConformanceSubject::new("baseline").with_embed_case(
            "embed-1",
            SubjectObservation::Supported(EmbedObservation {
                vectors: vec![EmbedVectorObservation {
                    index: 0,
                    values: vec![1.0, 0.0],
                }],
                dimensions: Some(2),
                normalized: Some(true),
                performance: None,
                error: None,
            }),
        );
        let candidate = RecordedConformanceSubject::new("candidate").with_embed_case(
            "embed-1",
            SubjectObservation::Supported(EmbedObservation {
                vectors: vec![EmbedVectorObservation {
                    index: 0,
                    values: vec![0.9, 0.2],
                }],
                dimensions: Some(2),
                normalized: Some(false),
                performance: None,
                error: None,
            }),
        );

        let mut baseline = baseline;
        let mut candidate = candidate;
        let report = run_conformance_suite(&suite, &mut baseline, &mut candidate)?;

        assert_eq!(report.summary.failed, 1);
        assert!(!report.cutover_ready());
        Ok(())
    }

    #[test]
    fn performance_gate_accepts_candidate_within_generation_budget()
    -> Result<(), Box<dyn std::error::Error>> {
        let report = ConformanceReport {
            suite_id: String::from("perf-ok"),
            baseline_subject: String::from("ollama"),
            candidate_subject: String::from("psionic"),
            checks: vec![ConformanceCheckResult {
                surface: ConformanceSurface::Generate,
                case_id: String::from("generate-1"),
                status: ConformanceCheckStatus::Passed,
                detail: String::from("semantic match"),
                baseline: serde_json::to_value(GenerateObservation {
                    rendered_prompt: None,
                    output_text: String::from("hello"),
                    done_reason: Some(String::from("stop")),
                    prompt_eval_count: Some(16),
                    eval_count: Some(8),
                    performance: Some(GeneratePerformanceObservation {
                        total_duration_ns: 2_000_000_000,
                        load_duration_ns: 500_000_000,
                        prompt_eval_count: 16,
                        prompt_eval_duration_ns: 400_000_000,
                        eval_count: 8,
                        eval_duration_ns: 1_200_000_000,
                    }),
                    error: None,
                })?,
                candidate: serde_json::to_value(GenerateObservation {
                    rendered_prompt: None,
                    output_text: String::from("hello"),
                    done_reason: Some(String::from("stop")),
                    prompt_eval_count: Some(16),
                    eval_count: Some(8),
                    performance: Some(GeneratePerformanceObservation {
                        total_duration_ns: 2_200_000_000,
                        load_duration_ns: 550_000_000,
                        prompt_eval_count: 16,
                        prompt_eval_duration_ns: 440_000_000,
                        eval_count: 8,
                        eval_duration_ns: 1_250_000_000,
                    }),
                    error: None,
                })?,
            }],
            summary: ConformanceSummary {
                passed: 1,
                failed: 0,
                unsupported: 0,
                intentional_differences: 0,
            },
        };

        let performance = report.performance_gate(&CutoverPerformanceThresholds::default());
        assert_eq!(performance.summary.passed, 1);
        assert_eq!(performance.summary.failed, 0);
        assert_eq!(performance.summary.insufficient_evidence, 0);
        assert!(performance.cutover_ready());
        assert!(report.cutover_ready_with_performance(&CutoverPerformanceThresholds::default()));
        Ok(())
    }

    #[test]
    fn performance_gate_refuses_cutover_when_metrics_are_missing()
    -> Result<(), Box<dyn std::error::Error>> {
        let report = ConformanceReport {
            suite_id: String::from("perf-missing"),
            baseline_subject: String::from("ollama"),
            candidate_subject: String::from("psionic"),
            checks: vec![ConformanceCheckResult {
                surface: ConformanceSurface::Embed,
                case_id: String::from("embed-1"),
                status: ConformanceCheckStatus::Passed,
                detail: String::from("semantic match"),
                baseline: serde_json::to_value(EmbedObservation {
                    vectors: vec![EmbedVectorObservation {
                        index: 0,
                        values: vec![1.0, 0.0],
                    }],
                    dimensions: Some(2),
                    normalized: Some(true),
                    performance: Some(EmbedPerformanceObservation {
                        total_duration_ns: 1_000_000_000,
                        load_duration_ns: 250_000_000,
                        prompt_eval_count: Some(4),
                        prompt_eval_duration_ns: Some(800_000_000),
                    }),
                    error: None,
                })?,
                candidate: serde_json::to_value(EmbedObservation {
                    vectors: vec![EmbedVectorObservation {
                        index: 0,
                        values: vec![1.0, 0.0],
                    }],
                    dimensions: Some(2),
                    normalized: Some(true),
                    performance: None,
                    error: None,
                })?,
            }],
            summary: ConformanceSummary {
                passed: 1,
                failed: 0,
                unsupported: 0,
                intentional_differences: 0,
            },
        };

        let performance = report.performance_gate(&CutoverPerformanceThresholds::default());
        assert_eq!(performance.summary.passed, 0);
        assert_eq!(performance.summary.failed, 0);
        assert_eq!(performance.summary.insufficient_evidence, 1);
        assert!(!performance.cutover_ready());
        assert!(!report.cutover_ready_with_performance(&CutoverPerformanceThresholds::default()));
        Ok(())
    }

    #[test]
    fn report_writer_emits_pretty_json() -> Result<(), Box<dyn std::error::Error>> {
        let report = ConformanceReport {
            suite_id: String::from("demo"),
            baseline_subject: String::from("baseline"),
            candidate_subject: String::from("candidate"),
            checks: vec![ConformanceCheckResult {
                surface: ConformanceSurface::Tags,
                case_id: String::from("tags"),
                status: ConformanceCheckStatus::Passed,
                detail: String::from("ok"),
                baseline: json!({"models": []}),
                candidate: json!({"models": []}),
            }],
            summary: ConformanceSummary {
                passed: 1,
                failed: 0,
                unsupported: 0,
                intentional_differences: 0,
            },
        };
        let temp = tempfile::tempdir()?;
        let path = temp.path().join("report.json");
        write_conformance_report(&path, &report)?;
        let body = fs::read_to_string(path)?;
        assert!(body.contains("\"suite_id\": \"demo\""));
        Ok(())
    }

    #[test]
    fn ollama_http_subject_normalizes_live_http_responses() -> Result<(), Box<dyn std::error::Error>>
    {
        let server = TestServer::spawn()?;
        let mut subject = OllamaHttpSubject::new(server.base_url.clone())?;

        let tags = subject.tags()?;
        match tags {
            SubjectObservation::Supported(tags) => {
                assert_eq!(tags.models.len(), 1);
                assert_eq!(tags.models[0].name, "qwen2:latest");
            }
            SubjectObservation::Unsupported { reason } => {
                return Err(format!("unexpected tags unsupported: {reason}").into());
            }
        }

        let show = subject.show(&ShowConformanceCase {
            id: String::from("show-qwen2"),
            model: String::from("qwen2:latest"),
            expected_candidate_difference: None,
        })?;
        match show {
            SubjectObservation::Supported(show) => {
                assert_eq!(show.family.as_deref(), Some("qwen2"));
                assert_eq!(
                    show.chat_template_digest.as_deref(),
                    Some("af9c0233881b083b52ff773580215222b5440ac3d0beeeca99b76329b048f8db")
                );
            }
            SubjectObservation::Unsupported { reason } => {
                return Err(format!("unexpected show unsupported: {reason}").into());
            }
        }

        let stream = subject.generate_stream(&GenerateConformanceCase {
            id: String::from("stream-ok"),
            model: String::from("qwen2:latest"),
            prompt: String::from("hello"),
            system: None,
            suffix: None,
            stream: true,
            debug_render_only: false,
            max_output_tokens: None,
            stop_sequences: Vec::new(),
            seed: None,
            top_logprobs: None,
            expected_rendered_prompt: None,
            prompt_fixture: None,
            expected_candidate_difference: None,
            expected_candidate_stream_difference: None,
        })?;
        match stream {
            SubjectObservation::Supported(stream) => {
                assert_eq!(stream.chunks.len(), 2);
                assert_eq!(stream.chunks[0].output_text, "hel");
                assert_eq!(stream.chunks[1].done_reason.as_deref(), Some("stop"));
            }
            SubjectObservation::Unsupported { reason } => {
                return Err(format!("unexpected stream unsupported: {reason}").into());
            }
        }

        let embed = subject.embed(&EmbedConformanceCase {
            id: String::from("embed-ok"),
            model: String::from("qwen2-embed"),
            inputs: vec![String::from("hello"), String::from("world")],
            truncate: Some(true),
            output_dimensions: None,
            drift_budget: BackendParityPolicy::default().embedding_budget(QuantizationMode::None),
            expected_candidate_difference: None,
        })?;
        match embed {
            SubjectObservation::Supported(embed) => {
                assert_eq!(embed.dimensions, Some(2));
                assert!(embed.normalized.unwrap_or(false));
                assert_eq!(embed.vectors.len(), 2);
            }
            SubjectObservation::Unsupported { reason } => {
                return Err(format!("unexpected embed unsupported: {reason}").into());
            }
        }

        Ok(())
    }

    #[test]
    fn local_ollama_catalog_subject_lists_and_shows_models()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let model_bytes = build_test_gguf(&[
            (
                String::from("general.architecture"),
                GgufMetadataValue::String(String::from("qwen2")),
            ),
            (
                String::from("general.name"),
                GgufMetadataValue::String(String::from("Tiny Qwen2")),
            ),
            (
                String::from("tokenizer.ggml.model"),
                GgufMetadataValue::String(String::from("gpt2")),
            ),
            (
                String::from("tokenizer.ggml.pre"),
                GgufMetadataValue::String(String::from("qwen2")),
            ),
            (
                String::from("tokenizer.ggml.add_bos_token"),
                GgufMetadataValue::Bool(false),
            ),
            (
                String::from("tokenizer.ggml.add_eos_token"),
                GgufMetadataValue::Bool(false),
            ),
            (
                String::from("tokenizer.ggml.eos_token_id"),
                GgufMetadataValue::U32(151645),
            ),
        ])?;
        let model_digest = write_blob(temp.path(), model_bytes.as_slice())?;
        let config_digest = write_blob(
            temp.path(),
            br#"{
                "model_format":"gguf",
                "model_family":"qwen2",
                "model_families":["qwen2"],
                "model_type":"7B",
                "file_type":"Q4_0"
            }"#,
        )?;
        let template_digest = write_blob(
            temp.path(),
            b"{% for message in messages %}{% if loop.first and messages[0]['role'] != 'system' %}{{ '<|im_start|>system\nYou are a helpful assistant<|im_end|>\n' }}{% endif %}{{'<|im_start|>' + message['role'] + '\n' + message['content'] + '<|im_end|>' + '\n'}}{% endfor %}{% if add_generation_prompt %}{{ '<|im_start|>assistant\n' }}{% endif %}",
        )?;
        let bad_config_digest = write_blob(temp.path(), b"{")?;

        write_manifest(
            temp.path(),
            "registry.ollama.ai/library/qwen2/latest",
            json!({
                "schemaVersion": 2,
                "mediaType": "application/vnd.docker.distribution.manifest.v2+json",
                "config": {
                    "mediaType": "application/vnd.docker.container.image.v1+json",
                    "digest": config_digest,
                    "size": 123
                },
                "layers": [
                    {
                        "mediaType": "application/vnd.ollama.image.model",
                        "digest": model_digest,
                        "size": 456
                    },
                    {
                        "mediaType": "application/vnd.ollama.image.template",
                        "digest": template_digest,
                        "size": 42
                    }
                ]
            }),
        )?;
        write_manifest(
            temp.path(),
            "registry.ollama.ai/library/bad/latest",
            json!({
                "schemaVersion": 2,
                "mediaType": "application/vnd.docker.distribution.manifest.v2+json",
                "config": {
                    "mediaType": "application/vnd.docker.container.image.v1+json",
                    "digest": bad_config_digest,
                    "size": 1
                },
                "layers": []
            }),
        )?;

        let mut subject = LocalOllamaCatalogSubject::new(temp.path());

        let tags = subject.tags()?;
        match tags {
            SubjectObservation::Supported(tags) => {
                assert_eq!(tags.models.len(), 1);
                assert_eq!(tags.models[0].name, "qwen2:latest");
                assert_eq!(tags.models[0].family.as_deref(), Some("qwen2"));
                assert_eq!(tags.models[0].format.as_deref(), Some("gguf"));
                assert_eq!(tags.models[0].quantization.as_deref(), Some("Q4_0"));
                assert_eq!(tags.models[0].size_bytes, Some(621));
                assert_eq!(tags.models[0].digest.as_ref().map(String::len), Some(64));
            }
            SubjectObservation::Unsupported { reason } => {
                return Err(format!("unexpected tags unsupported: {reason}").into());
            }
        }

        let show = subject.show(&ShowConformanceCase {
            id: String::from("local-show"),
            model: String::from("qwen2"),
            expected_candidate_difference: None,
        })?;
        match show {
            SubjectObservation::Supported(show) => {
                assert_eq!(show.family.as_deref(), Some("qwen2"));
                assert_eq!(show.format.as_deref(), Some("gguf"));
                assert_eq!(show.quantization.as_deref(), Some("Q4_0"));
                assert_eq!(
                    show.chat_template_digest.as_deref(),
                    Some("af9c0233881b083b52ff773580215222b5440ac3d0beeeca99b76329b048f8db")
                );
                assert_eq!(show.capabilities, vec![String::from("completion")]);
                assert_eq!(
                    show.facts.get("general.architecture").map(String::as_str),
                    Some("qwen2")
                );
                assert_eq!(
                    show.facts.get("tokenizer.ggml.model").map(String::as_str),
                    Some("gpt2")
                );
                assert_eq!(
                    show.facts.get("tokenizer.ggml.pre").map(String::as_str),
                    Some("qwen2")
                );
                assert_eq!(
                    show.facts
                        .get("tokenizer.ggml.add_bos_token")
                        .map(String::as_str),
                    Some("false")
                );
                assert_eq!(
                    show.facts
                        .get("tokenizer.ggml.add_eos_token")
                        .map(String::as_str),
                    Some("false")
                );
                assert_eq!(
                    show.facts
                        .get("tokenizer.ggml.eos_token_id")
                        .map(String::as_str),
                    Some("151645")
                );
                assert_eq!(
                    show.facts
                        .get("psionic.catalog_surface")
                        .map(String::as_str),
                    Some("ollama_compat_migration")
                );
                assert_eq!(
                    show.facts
                        .get("psionic.model_ingress_surface")
                        .map(String::as_str),
                    Some("ollama_compat_manifest_import")
                );
                assert_eq!(
                    show.facts
                        .get("psionic.serving_surface")
                        .map(String::as_str),
                    Some("ollama_compat_migration")
                );
                assert_eq!(
                    show.facts
                        .get("psionic.runtime_surface")
                        .map(String::as_str),
                    Some("psionic_native")
                );
                assert!(!show.facts.contains_key("general.name"));
            }
            SubjectObservation::Unsupported { reason } => {
                return Err(format!("unexpected show unsupported: {reason}").into());
            }
        }

        Ok(())
    }

    #[test]
    fn local_ollama_catalog_subject_reports_missing_model() -> Result<(), Box<dyn std::error::Error>>
    {
        let temp = tempfile::tempdir()?;
        let mut subject = LocalOllamaCatalogSubject::new(temp.path());

        let show = subject.show(&ShowConformanceCase {
            id: String::from("missing"),
            model: String::from("missing-model"),
            expected_candidate_difference: None,
        })?;
        match show {
            SubjectObservation::Supported(show) => {
                let error = show.error.expect("missing-model error");
                assert_eq!(error.status, 404);
                assert_eq!(error.message, "model 'missing-model' not found");
            }
            SubjectObservation::Unsupported { reason } => {
                return Err(format!("unexpected show unsupported: {reason}").into());
            }
        }

        Ok(())
    }

    #[test]
    fn compare_show_ignores_candidate_only_psionic_facts() {
        let baseline = ShowObservation {
            model: String::from("qwen2"),
            format: Some(String::from("gguf")),
            family: Some(String::from("qwen2")),
            families: vec![String::from("qwen2")],
            quantization: Some(String::from("Q4_0")),
            chat_template_digest: None,
            capabilities: vec![String::from("completion")],
            facts: BTreeMap::from([(String::from("general.architecture"), String::from("qwen2"))]),
            error: None,
        };
        let mut candidate = baseline.clone();
        candidate.facts.insert(
            String::from("psionic.ollama_adapter_policy"),
            String::from("refuse_manifest_with_adapters"),
        );

        assert!(compare_show(&baseline, &candidate).is_ok());
    }

    #[test]
    fn show_observation_from_descriptor_reports_native_boundary_facts() {
        let model = crate::ReferenceWordDecoder::new();
        let show = ShowObservation::from_decoder_descriptor(
            model.descriptor().model.model_id.clone(),
            model.descriptor(),
        );

        assert_eq!(
            show.facts
                .get("psionic.model_ingress_surface")
                .map(String::as_str),
            Some("fixture")
        );
        assert_eq!(
            show.facts
                .get("psionic.serving_surface")
                .map(String::as_str),
            Some("psionic_native")
        );
        assert_eq!(
            show.facts
                .get("psionic.runtime_surface")
                .map(String::as_str),
            Some("psionic_native")
        );
        assert!(!show.facts.contains_key("psionic.catalog_surface"));
    }

    #[test]
    fn local_ollama_catalog_subject_show_reports_adapter_policy_facts()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let model_bytes = build_test_gguf(&[
            (
                String::from("general.architecture"),
                GgufMetadataValue::String(String::from("qwen2")),
            ),
            (
                String::from("tokenizer.ggml.model"),
                GgufMetadataValue::String(String::from("gpt2")),
            ),
            (
                String::from("tokenizer.ggml.pre"),
                GgufMetadataValue::String(String::from("qwen2")),
            ),
        ])?;
        let model_digest = write_blob(temp.path(), model_bytes.as_slice())?;
        let adapter_digest = write_blob(temp.path(), b"adapter-gguf")?;
        let config_digest = write_blob(
            temp.path(),
            br#"{
                "model_format":"gguf",
                "model_family":"qwen2",
                "model_families":["qwen2"],
                "file_type":"Q4_0"
            }"#,
        )?;

        write_manifest(
            temp.path(),
            "registry.ollama.ai/library/qwen2-adapter/latest",
            json!({
                "schemaVersion": 2,
                "mediaType": "application/vnd.docker.distribution.manifest.v2+json",
                "config": {
                    "mediaType": "application/vnd.docker.container.image.v1+json",
                    "digest": config_digest,
                    "size": 1
                },
                "layers": [
                    {
                        "mediaType": "application/vnd.ollama.image.model",
                        "digest": model_digest,
                        "size": 456
                    },
                    {
                        "mediaType": "application/vnd.ollama.image.adapter",
                        "digest": adapter_digest,
                        "size": 12
                    }
                ]
            }),
        )?;

        let mut subject = LocalOllamaCatalogSubject::new(temp.path());
        let show = subject.show(&ShowConformanceCase {
            id: String::from("local-show-adapter"),
            model: String::from("qwen2-adapter"),
            expected_candidate_difference: None,
        })?;
        match show {
            SubjectObservation::Supported(show) => {
                assert_eq!(
                    show.facts
                        .get("psionic.ollama_adapter_policy")
                        .map(String::as_str),
                    Some("refuse_manifest_with_adapters")
                );
                assert_eq!(
                    show.facts
                        .get("psionic.ollama_adapter_layer_count")
                        .map(String::as_str),
                    Some("1")
                );
                assert_eq!(
                    show.facts
                        .get("psionic.ollama_adapter_manifest_supported")
                        .map(String::as_str),
                    Some("false")
                );
            }
            SubjectObservation::Unsupported { reason } => {
                return Err(format!("unexpected show unsupported: {reason}").into());
            }
        }

        Ok(())
    }

    struct TestServer {
        base_url: String,
        alive: Arc<AtomicBool>,
        handle: Option<thread::JoinHandle<()>>,
    }

    impl TestServer {
        fn spawn() -> Result<Self, Box<dyn std::error::Error>> {
            let listener = TcpListener::bind("127.0.0.1:0")?;
            let address = listener.local_addr()?;
            let alive = Arc::new(AtomicBool::new(true));
            let server_alive = Arc::clone(&alive);
            let handle = thread::spawn(move || {
                while server_alive.load(Ordering::SeqCst) {
                    let Ok((mut stream, _)) = listener.accept() else {
                        continue;
                    };
                    if !server_alive.load(Ordering::SeqCst) {
                        break;
                    }
                    let Ok((method, path, body)) = read_request(&mut stream) else {
                        let _ = stream.shutdown(Shutdown::Both);
                        continue;
                    };
                    let (status, content_type, response_body) =
                        respond(method.as_str(), path.as_str(), body.as_str());
                    let _ =
                        write_response(&mut stream, status, content_type, response_body.as_str());
                    let _ = stream.shutdown(Shutdown::Both);
                }
            });

            Ok(Self {
                base_url: format!("http://{address}"),
                alive,
                handle: Some(handle),
            })
        }
    }

    impl Drop for TestServer {
        fn drop(&mut self) {
            self.alive.store(false, Ordering::SeqCst);
            let _ = TcpStream::connect(self.base_url.trim_start_matches("http://"));
            if let Some(handle) = self.handle.take() {
                let _ = handle.join();
            }
        }
    }

    fn respond(method: &str, path: &str, body: &str) -> (u16, &'static str, String) {
        match (method, path) {
            ("GET", "/api/tags") => (
                200,
                "application/json",
                json!({
                    "models": [{
                        "name": "qwen2:latest",
                        "digest": "sha256:qwen2",
                        "size": 1234,
                        "details": {
                            "format": "gguf",
                            "family": "qwen2",
                            "families": ["qwen2"],
                            "quantization_level": "Q4_0"
                        }
                    }]
                })
                .to_string(),
            ),
            ("POST", "/api/show") => (
                200,
                "application/json",
                json!({
                    "template": "{% for message in messages %}{% if loop.first and messages[0]['role'] != 'system' %}{{ '<|im_start|>system\nYou are a helpful assistant<|im_end|>\n' }}{% endif %}{{'<|im_start|>' + message['role'] + '\n' + message['content'] + '<|im_end|>' + '\n'}}{% endfor %}{% if add_generation_prompt %}{{ '<|im_start|>assistant\n' }}{% endif %}",
                    "capabilities": ["completion"],
                    "model_info": {
                        "general.architecture": "qwen2",
                        "tokenizer.ggml.model": "gpt2",
                        "tokenizer.ggml.add_bos_token": false,
                        "tokenizer.ggml.add_eos_token": false,
                        "tokenizer.ggml.eos_token_id": 151645
                    },
                    "details": {
                        "format": "gguf",
                        "family": "qwen2",
                        "families": ["qwen2"],
                        "quantization_level": "Q4_0"
                    }
                })
                .to_string(),
            ),
            ("GET", "/api/ps") => (
                200,
                "application/json",
                json!({
                    "models": [{
                        "name": "qwen2:latest",
                        "digest": "sha256:qwen2",
                        "size": 2048,
                        "size_vram": 1024,
                        "context_length": 32768,
                        "details": {
                            "format": "gguf",
                            "family": "qwen2",
                            "families": ["qwen2"],
                            "quantization_level": "Q4_0"
                        }
                    }]
                })
                .to_string(),
            ),
            ("POST", "/api/generate") if body.contains("\"stream\":true") => (
                200,
                "application/x-ndjson",
                [
                    json!({"response": "hel", "done": false}),
                    json!({"response": "lo", "done": true, "done_reason": "stop"}),
                ]
                .iter()
                .map(Value::to_string)
                .collect::<Vec<_>>()
                .join("\n")
                    + "\n",
            ),
            ("POST", "/api/generate") => (
                200,
                "application/json",
                json!({
                    "response": "",
                    "done": true,
                    "_debug_info": {
                        "rendered_template": "<|im_start|>system\nYou are a helpful assistant<|im_end|>\n<|im_start|>user\nSummarize the roadmap.<|im_end|>\n<|im_start|>assistant\n"
                    }
                })
                .to_string(),
            ),
            ("POST", "/api/embed") => (
                200,
                "application/json",
                json!({
                    "embeddings": [[1.0, 0.0], [0.0, 1.0]]
                })
                .to_string(),
            ),
            _ => (
                404,
                "application/json",
                json!({"error": "not found"}).to_string(),
            ),
        }
    }

    fn read_request(
        stream: &mut TcpStream,
    ) -> Result<(String, String, String), Box<dyn std::error::Error>> {
        let mut buffer = Vec::new();
        let mut chunk = [0_u8; 1024];
        loop {
            let read = stream.read(&mut chunk)?;
            if read == 0 {
                break;
            }
            buffer.extend_from_slice(&chunk[..read]);
            if buffer.windows(4).any(|window| window == b"\r\n\r\n") {
                break;
            }
        }

        let header_end = buffer
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .ok_or("missing header terminator")?
            + 4;
        let header = String::from_utf8(buffer[..header_end].to_vec())?;
        let mut body = buffer[header_end..].to_vec();
        let content_length = header
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                if name.eq_ignore_ascii_case("content-length") {
                    value.trim().parse::<usize>().ok()
                } else {
                    None
                }
            })
            .unwrap_or(0);
        while body.len() < content_length {
            let read = stream.read(&mut chunk)?;
            if read == 0 {
                break;
            }
            body.extend_from_slice(&chunk[..read]);
        }

        let request_line = header.lines().next().ok_or("missing request line")?;
        let mut parts = request_line.split_whitespace();
        let method = parts.next().ok_or("missing method")?.to_string();
        let path = parts.next().ok_or("missing path")?.to_string();
        let body = String::from_utf8(body)?;
        Ok((method, path, body))
    }

    fn write_response(
        stream: &mut TcpStream,
        status: u16,
        content_type: &str,
        body: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let reason = match status {
            200 => "OK",
            404 => "Not Found",
            _ => "OK",
        };
        write!(
            stream,
            "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )?;
        stream.flush()?;
        Ok(())
    }

    fn write_manifest(
        models_root: &Path,
        relpath: &str,
        json: Value,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = models_root.join("manifests").join(relpath);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, serde_json::to_vec(&json)?)?;
        Ok(())
    }

    fn write_blob(models_root: &Path, bytes: &[u8]) -> Result<String, Box<dyn std::error::Error>> {
        let digest = format!("sha256:{:x}", Sha256::digest(bytes));
        let path = models_root.join("blobs").join(digest.replace(':', "-"));
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, bytes)?;
        Ok(digest)
    }

    fn build_test_gguf(
        metadata: &[(String, GgufMetadataValue)],
    ) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let alignment = metadata
            .iter()
            .find(|(key, _)| key == "general.alignment")
            .and_then(|(_, value)| match value {
                GgufMetadataValue::U64(value) => Some(*value),
                GgufMetadataValue::U32(value) => Some(u64::from(*value)),
                _ => None,
            })
            .unwrap_or(32);

        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"GGUF");
        push_u32(&mut bytes, 3);
        push_u64(&mut bytes, 0);
        push_u64(&mut bytes, u64::try_from(metadata.len())?);

        for (key, value) in metadata {
            push_gguf_string(&mut bytes, key)?;
            push_u32(&mut bytes, gguf_metadata_value_type(value));
            push_gguf_value(&mut bytes, value)?;
        }

        let aligned = align_offset_local(bytes.len() as u64, alignment);
        bytes.resize(aligned as usize, 0);
        Ok(bytes)
    }

    fn push_gguf_string(
        bytes: &mut Vec<u8>,
        value: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        push_u64(bytes, u64::try_from(value.len())?);
        bytes.extend_from_slice(value.as_bytes());
        Ok(())
    }

    fn push_gguf_value(
        bytes: &mut Vec<u8>,
        value: &GgufMetadataValue,
    ) -> Result<(), Box<dyn std::error::Error>> {
        match value {
            GgufMetadataValue::U8(value) => bytes.push(*value),
            GgufMetadataValue::I8(value) => bytes.push(value.to_le_bytes()[0]),
            GgufMetadataValue::U16(value) => bytes.extend(value.to_le_bytes()),
            GgufMetadataValue::I16(value) => bytes.extend(value.to_le_bytes()),
            GgufMetadataValue::U32(value) => bytes.extend(value.to_le_bytes()),
            GgufMetadataValue::I32(value) => bytes.extend(value.to_le_bytes()),
            GgufMetadataValue::U64(value) => bytes.extend(value.to_le_bytes()),
            GgufMetadataValue::I64(value) => bytes.extend(value.to_le_bytes()),
            GgufMetadataValue::F32(value) => bytes.extend(value.to_le_bytes()),
            GgufMetadataValue::F64(value) => bytes.extend(value.to_le_bytes()),
            GgufMetadataValue::Bool(value) => bytes.push(u8::from(*value)),
            GgufMetadataValue::String(value) => push_gguf_string(bytes, value)?,
            GgufMetadataValue::Array(values) => {
                let value_type = values.first().map_or(4, gguf_metadata_value_type);
                push_u32(bytes, value_type);
                push_u64(bytes, u64::try_from(values.len())?);
                for value in values {
                    push_gguf_value(bytes, value)?;
                }
            }
        }
        Ok(())
    }

    fn gguf_metadata_value_type(value: &GgufMetadataValue) -> u32 {
        match value {
            GgufMetadataValue::U8(_) => 0,
            GgufMetadataValue::I8(_) => 1,
            GgufMetadataValue::U16(_) => 2,
            GgufMetadataValue::I16(_) => 3,
            GgufMetadataValue::U32(_) => 4,
            GgufMetadataValue::I32(_) => 5,
            GgufMetadataValue::F32(_) => 6,
            GgufMetadataValue::Bool(_) => 7,
            GgufMetadataValue::String(_) => 8,
            GgufMetadataValue::Array(_) => 9,
            GgufMetadataValue::U64(_) => 10,
            GgufMetadataValue::I64(_) => 11,
            GgufMetadataValue::F64(_) => 12,
        }
    }

    fn push_u32(bytes: &mut Vec<u8>, value: u32) {
        bytes.extend(value.to_le_bytes());
    }

    fn push_u64(bytes: &mut Vec<u8>, value: u64) {
        bytes.extend(value.to_le_bytes());
    }

    fn align_offset_local(value: u64, alignment: u64) -> u64 {
        if alignment <= 1 {
            value
        } else {
            value.div_ceil(alignment) * alignment
        }
    }
}
