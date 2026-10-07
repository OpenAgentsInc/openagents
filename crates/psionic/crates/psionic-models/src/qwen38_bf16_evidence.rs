use std::{collections::BTreeMap, path::Path, str::FromStr};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    QWEN38_27B_MODEL_ID, QWEN38_27B_SERVED_MODEL_ID, QWEN38_27B_UPSTREAM_REVISION,
    Qwen35TextCheckpointError, Qwen35TextTensorRowReadReceipt, Qwen38ForwardAdmissionError,
    Qwen38PromptError, Qwen38PromptMessage, Qwen38PromptOptions, Qwen38PromptReceipt,
    Qwen38PromptRole, Qwen38Tokenizer, Qwen38TokenizerError, qwen35_text_architecture_report,
    qwen35_text_expected_tensor_specs, qwen35_text_read_indexed_tensor_row,
    qwen35_text_weight_index_from_bytes, render_qwen38_prompt, run_qwen38_forward_admission,
};

pub const QWEN38_BF16_EVIDENCE_SCHEMA_VERSION: &str = "psionic.qwen38_27b_bf16_evidence.v1";
pub const QWEN38_BF16_EVIDENCE_PROMPT: &str = "Verify the bounded Qwen3.8 BF16 evidence path.";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Qwen38Bf16EvidenceBackend {
    HeaderAdmission,
    SampledProjection,
    BoundedRowSparseTraversal,
}

impl Qwen38Bf16EvidenceBackend {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::HeaderAdmission => "header_admission",
            Self::SampledProjection => "sampled_projection",
            Self::BoundedRowSparseTraversal => "bounded_row_sparse_traversal",
        }
    }
}

impl FromStr for Qwen38Bf16EvidenceBackend {
    type Err = Qwen38Bf16EvidenceError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "header-admission" | "header_admission" => Ok(Self::HeaderAdmission),
            "sampled-projection" | "sampled_projection" => Ok(Self::SampledProjection),
            "bounded-row-sparse-traversal" | "bounded_row_sparse_traversal" => {
                Ok(Self::BoundedRowSparseTraversal)
            }
            other => Err(Qwen38Bf16EvidenceError::UnsupportedBackend(String::from(
                other,
            ))),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Qwen38Bf16RuntimeIdentity {
    pub engine: String,
    pub crate_version: String,
    pub operating_system: String,
    pub architecture: String,
    pub execution_device: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Qwen38Bf16CapabilityClaims {
    pub full_width_attention: bool,
    pub full_width_mlp: bool,
    pub full_vocabulary_logits: bool,
    pub token_generation: bool,
    pub training_gradients: bool,
    pub media_execution: bool,
}

impl Default for Qwen38Bf16CapabilityClaims {
    fn default() -> Self {
        Self {
            full_width_attention: false,
            full_width_mlp: false,
            full_vocabulary_logits: false,
            token_generation: false,
            training_gradients: false,
            media_execution: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Qwen38SampledLogitEvidence {
    pub token_id: u32,
    pub logit: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Qwen38SampledProjectionEvidence {
    pub input_token_id: u32,
    pub candidate_token_ids: Vec<u32>,
    pub sampled_logits: Vec<Qwen38SampledLogitEvidence>,
    pub sampled_logits_sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Qwen38LayerTraversalEvidence {
    pub layer_index: usize,
    pub layer_type: String,
    pub tensor_read_count: usize,
    pub tensor_reads_sha256: String,
    pub traversal_state_sha256: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Qwen38Bf16EvidenceReport {
    pub schema_version: String,
    pub model_id: String,
    pub served_model_id: String,
    pub upstream_revision: String,
    pub backend: Qwen38Bf16EvidenceBackend,
    pub runtime: Qwen38Bf16RuntimeIdentity,
    pub command_line: Vec<String>,
    pub config_sha256: String,
    pub tokenizer_sha256: String,
    pub template_sha256: String,
    pub index_sha256: String,
    pub prompt_receipt: Qwen38PromptReceipt,
    pub prompt_token_ids_sha256: String,
    pub checkpoint_tensor_admission_sha256: String,
    pub indexed_shard_count: usize,
    pub indexed_tensor_count: usize,
    pub admitted_decoder_tensor_count: usize,
    pub admitted_mtp_tensor_count: usize,
    pub non_text_tensor_count: usize,
    pub tensor_reads: Vec<Qwen35TextTensorRowReadReceipt>,
    pub tensor_reads_sha256: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sampled_projection: Option<Qwen38SampledProjectionEvidence>,
    pub layer_traversal: Vec<Qwen38LayerTraversalEvidence>,
    pub visited_decoder_layer_count: usize,
    pub visited_mtp_layer_count: usize,
    pub output_sha256: String,
    pub capability_claims: Qwen38Bf16CapabilityClaims,
    pub claim_boundary: String,
}

#[derive(Debug, Error)]
pub enum Qwen38Bf16EvidenceError {
    #[error("unsupported Qwen3.8 BF16 evidence backend `{0}`")]
    UnsupportedBackend(String),
    #[error("Qwen3.8 BF16 evidence prompt produced no token ids")]
    EmptyPromptTokens,
    #[error(transparent)]
    ForwardAdmission(#[from] Qwen38ForwardAdmissionError),
    #[error(transparent)]
    Checkpoint(#[from] Qwen35TextCheckpointError),
    #[error(transparent)]
    Prompt(#[from] Qwen38PromptError),
    #[error(transparent)]
    Tokenizer(#[from] Qwen38TokenizerError),
    #[error("Qwen3.8 BF16 evidence I/O failed at `{path}`: {source}")]
    Io {
        path: std::path::PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to serialize Qwen3.8 BF16 evidence: {0}")]
    Serialization(#[from] serde_json::Error),
}

pub fn run_qwen38_bf16_evidence(
    model_dir: impl AsRef<Path>,
    backend: Qwen38Bf16EvidenceBackend,
    command_line: Vec<String>,
) -> Result<Qwen38Bf16EvidenceReport, Qwen38Bf16EvidenceError> {
    let model_dir = model_dir.as_ref();
    let checkpoint = run_qwen38_forward_admission(model_dir)?;
    let config_path = model_dir.join("config.json");
    let tokenizer_path = model_dir.join("tokenizer.json");
    let index_path = model_dir.join("model.safetensors.index.json");
    let config_bytes = read_bytes(&config_path)?;
    let index_bytes = read_bytes(&index_path)?;
    let architecture = qwen35_text_architecture_report(config_bytes.as_slice())?;
    let index = qwen35_text_weight_index_from_bytes(index_bytes.as_slice())?;
    let rendered = render_qwen38_prompt(
        &[Qwen38PromptMessage::text(
            Qwen38PromptRole::User,
            QWEN38_BF16_EVIDENCE_PROMPT,
        )],
        &Qwen38PromptOptions::default(),
    )?;
    let tokenizer = Qwen38Tokenizer::from_official_file(&tokenizer_path)?;
    let tokenized = tokenizer.tokenize(&rendered)?;
    let input_token_id = tokenized
        .token_ids
        .last()
        .copied()
        .ok_or(Qwen38Bf16EvidenceError::EmptyPromptTokens)?;
    let prompt_token_ids_sha256 = sha256_json(&tokenized.token_ids)?;

    let mut tensor_reads = Vec::new();
    let mut sampled_projection = None;
    let mut layer_traversal = Vec::new();
    let mut visited_decoder_layer_count = 0usize;
    let mut visited_mtp_layer_count = 0usize;
    let output_sha256 = match backend {
        Qwen38Bf16EvidenceBackend::HeaderAdmission => checkpoint.tensor_admission_sha256.clone(),
        Qwen38Bf16EvidenceBackend::SampledProjection => {
            let evidence = sampled_projection_evidence(
                model_dir,
                &index.weight_map,
                architecture.vocab_size,
                architecture.hidden_size,
                input_token_id,
                &mut tensor_reads,
            )?;
            let output = evidence.sampled_logits_sha256.clone();
            sampled_projection = Some(evidence);
            output
        }
        Qwen38Bf16EvidenceBackend::BoundedRowSparseTraversal => {
            let expected = qwen35_text_expected_tensor_specs(&architecture)?;
            let expected_by_name = expected
                .into_iter()
                .map(|spec| (spec.name.clone(), spec))
                .collect::<BTreeMap<_, _>>();
            let mut state = sha256_hex(checkpoint.tensor_admission_sha256.as_bytes());

            for (layer_index, layer_type) in architecture.layer_types.iter().enumerate() {
                let prefix = format!("model.language_model.layers.{layer_index}.");
                let specs = expected_by_name
                    .values()
                    .filter(|spec| spec.name.starts_with(&prefix))
                    .collect::<Vec<_>>();
                state = traverse_specs(
                    model_dir,
                    &index.weight_map,
                    specs.as_slice(),
                    layer_index,
                    layer_type,
                    state,
                    &mut tensor_reads,
                    &mut layer_traversal,
                )?;
                visited_decoder_layer_count += 1;
            }

            if architecture.mtp_num_hidden_layers > 0 {
                let specs = expected_by_name
                    .values()
                    .filter(|spec| spec.name.starts_with("mtp."))
                    .collect::<Vec<_>>();
                state = traverse_specs(
                    model_dir,
                    &index.weight_map,
                    specs.as_slice(),
                    architecture.num_hidden_layers,
                    "mtp_full_attention",
                    state,
                    &mut tensor_reads,
                    &mut layer_traversal,
                )?;
                visited_mtp_layer_count = architecture.mtp_num_hidden_layers;
            }

            let globals = expected_by_name
                .values()
                .filter(|spec| {
                    !spec.name.starts_with("model.language_model.layers.")
                        && !spec.name.starts_with("mtp.")
                })
                .collect::<Vec<_>>();
            for (index_seed, spec) in globals.iter().enumerate() {
                let row = qwen35_text_read_indexed_tensor_row(
                    model_dir,
                    &index.weight_map,
                    spec.name.as_str(),
                    deterministic_row_index(spec.shape.as_slice(), index_seed),
                )?;
                state = hash_chain(state.as_str(), &row.receipt)?;
                tensor_reads.push(row.receipt);
            }
            state
        }
    };
    let tensor_reads_sha256 = sha256_json(&tensor_reads)?;

    Ok(Qwen38Bf16EvidenceReport {
        schema_version: String::from(QWEN38_BF16_EVIDENCE_SCHEMA_VERSION),
        model_id: String::from(QWEN38_27B_MODEL_ID),
        served_model_id: String::from(QWEN38_27B_SERVED_MODEL_ID),
        upstream_revision: String::from(QWEN38_27B_UPSTREAM_REVISION),
        backend,
        runtime: Qwen38Bf16RuntimeIdentity {
            engine: String::from("psionic-models"),
            crate_version: String::from(env!("CARGO_PKG_VERSION")),
            operating_system: String::from(std::env::consts::OS),
            architecture: String::from(std::env::consts::ARCH),
            execution_device: String::from("cpu_bounded_row_reads"),
        },
        command_line,
        config_sha256: checkpoint.config_sha256,
        tokenizer_sha256: tokenized.tokenizer_sha256.clone(),
        template_sha256: rendered.receipt.template_sha256.clone(),
        index_sha256: checkpoint.index_sha256,
        prompt_receipt: rendered.receipt,
        prompt_token_ids_sha256,
        checkpoint_tensor_admission_sha256: checkpoint.tensor_admission_sha256,
        indexed_shard_count: checkpoint.indexed_shard_count,
        indexed_tensor_count: checkpoint.indexed_tensor_count,
        admitted_decoder_tensor_count: checkpoint.tensor_admission.admitted_decoder_tensor_count,
        admitted_mtp_tensor_count: checkpoint.tensor_admission.admitted_mtp_tensor_count,
        non_text_tensor_count: checkpoint
            .tensor_admission
            .visual_or_other_observed_tensor_count,
        tensor_reads,
        tensor_reads_sha256,
        sampled_projection,
        layer_traversal,
        visited_decoder_layer_count,
        visited_mtp_layer_count,
        output_sha256,
        capability_claims: Qwen38Bf16CapabilityClaims::default(),
        claim_boundary: String::from(
            "This evidence lane validates headers, reads bounded real BF16 tensor rows, and optionally computes sampled embedding/LM-head dots or an ordered row-digest traversal. It does not execute full-width attention or MLPs, materialize full-vocabulary logits, generate tokens, compute training gradients, or execute image/video inputs.",
        ),
    })
}

fn sampled_projection_evidence(
    model_dir: &Path,
    weight_map: &BTreeMap<String, String>,
    vocab_size: usize,
    hidden_size: usize,
    input_token_id: u32,
    tensor_reads: &mut Vec<Qwen35TextTensorRowReadReceipt>,
) -> Result<Qwen38SampledProjectionEvidence, Qwen38Bf16EvidenceError> {
    let embed = qwen35_text_read_indexed_tensor_row(
        model_dir,
        weight_map,
        "model.language_model.embed_tokens.weight",
        input_token_id as usize,
    )?;
    if embed.receipt.shape != [vocab_size, hidden_size] {
        return Err(Qwen35TextCheckpointError::InvalidConfig(format!(
            "unexpected embedding shape {:?}",
            embed.receipt.shape
        ))
        .into());
    }
    let mut candidate_token_ids = vec![0, 1, input_token_id];
    candidate_token_ids.sort_unstable();
    candidate_token_ids.dedup();
    tensor_reads.push(embed.receipt);
    let mut sampled_logits = Vec::new();
    for token_id in &candidate_token_ids {
        let row = qwen35_text_read_indexed_tensor_row(
            model_dir,
            weight_map,
            "lm_head.weight",
            *token_id as usize,
        )?;
        let logit = embed
            .values
            .iter()
            .zip(&row.values)
            .map(|(left, right)| f64::from(*left) * f64::from(*right))
            .sum();
        tensor_reads.push(row.receipt);
        sampled_logits.push(Qwen38SampledLogitEvidence {
            token_id: *token_id,
            logit,
        });
    }
    Ok(Qwen38SampledProjectionEvidence {
        input_token_id,
        candidate_token_ids,
        sampled_logits_sha256: sha256_json(&sampled_logits)?,
        sampled_logits,
    })
}

#[allow(clippy::too_many_arguments)]
fn traverse_specs(
    model_dir: &Path,
    weight_map: &BTreeMap<String, String>,
    specs: &[&crate::Qwen35TextTensorSpec],
    layer_index: usize,
    layer_type: &str,
    mut state: String,
    tensor_reads: &mut Vec<Qwen35TextTensorRowReadReceipt>,
    layer_traversal: &mut Vec<Qwen38LayerTraversalEvidence>,
) -> Result<String, Qwen38Bf16EvidenceError> {
    let start = tensor_reads.len();
    for (tensor_index, spec) in specs.iter().enumerate() {
        let row = qwen35_text_read_indexed_tensor_row(
            model_dir,
            weight_map,
            spec.name.as_str(),
            deterministic_row_index(spec.shape.as_slice(), layer_index + tensor_index),
        )?;
        state = hash_chain(state.as_str(), &row.receipt)?;
        tensor_reads.push(row.receipt);
    }
    let layer_reads = &tensor_reads[start..];
    layer_traversal.push(Qwen38LayerTraversalEvidence {
        layer_index,
        layer_type: String::from(layer_type),
        tensor_read_count: layer_reads.len(),
        tensor_reads_sha256: sha256_json(layer_reads)?,
        traversal_state_sha256: state.clone(),
    });
    Ok(state)
}

fn deterministic_row_index(shape: &[usize], seed: usize) -> usize {
    if shape.len() == 1 { 0 } else { seed % shape[0] }
}

fn hash_chain<T: Serialize>(state: &str, value: &T) -> Result<String, serde_json::Error> {
    let mut hasher = Sha256::new();
    hasher.update(state.as_bytes());
    hasher.update(serde_json::to_vec(value)?);
    Ok(hex::encode(hasher.finalize()))
}

fn read_bytes(path: &Path) -> Result<Vec<u8>, Qwen38Bf16EvidenceError> {
    std::fs::read(path).map_err(|source| Qwen38Bf16EvidenceError::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn sha256_json<T: Serialize + ?Sized>(value: &T) -> Result<String, serde_json::Error> {
    Ok(sha256_hex(serde_json::to_vec(value)?.as_slice()))
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    const HEADER_REPORT: &str =
        include_str!("../../../fixtures/qwen38/reports/qwen38_bf16_header_admission_v1.json");
    const PROJECTION_REPORT: &str =
        include_str!("../../../fixtures/qwen38/reports/qwen38_bf16_sampled_projection_v1.json");
    const TRAVERSAL_REPORT: &str =
        include_str!("../../../fixtures/qwen38/reports/qwen38_bf16_bounded_traversal_v1.json");

    #[test]
    fn qwen38_bf16_evidence_backends_are_explicit() {
        assert!(matches!(
            "header-admission".parse::<Qwen38Bf16EvidenceBackend>(),
            Ok(Qwen38Bf16EvidenceBackend::HeaderAdmission)
        ));
        assert!("generation".parse::<Qwen38Bf16EvidenceBackend>().is_err());
    }

    #[test]
    fn qwen38_bf16_official_evidence_is_deterministic_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let model_dir = std::env::var("PSIONIC_QWEN38_MODEL_DIR")
            .unwrap_or_else(|_| String::from("target/models/qwen/Qwen3.8-27B"));
        if !Path::new(model_dir.as_str()).join("config.json").is_file() {
            return Ok(());
        }
        for backend in [
            Qwen38Bf16EvidenceBackend::HeaderAdmission,
            Qwen38Bf16EvidenceBackend::SampledProjection,
            Qwen38Bf16EvidenceBackend::BoundedRowSparseTraversal,
        ] {
            let first = run_qwen38_bf16_evidence(&model_dir, backend, Vec::new())?;
            let second = run_qwen38_bf16_evidence(&model_dir, backend, Vec::new())?;
            let retained = match backend {
                Qwen38Bf16EvidenceBackend::HeaderAdmission => HEADER_REPORT,
                Qwen38Bf16EvidenceBackend::SampledProjection => PROJECTION_REPORT,
                Qwen38Bf16EvidenceBackend::BoundedRowSparseTraversal => TRAVERSAL_REPORT,
            };
            let retained = serde_json::from_str::<Qwen38Bf16EvidenceReport>(retained)?;
            assert_eq!(first.output_sha256, second.output_sha256);
            assert_eq!(first.tensor_reads_sha256, second.tensor_reads_sha256);
            assert_eq!(first.output_sha256, retained.output_sha256);
            assert_eq!(first.tensor_reads_sha256, retained.tensor_reads_sha256);
            assert_eq!(first.backend, retained.backend);
            assert_eq!(first.indexed_shard_count, 18);
            assert_eq!(first.indexed_tensor_count, 1_199);
            assert!(!first.capability_claims.full_width_attention);
            assert!(!first.capability_claims.full_vocabulary_logits);
            assert!(!first.capability_claims.token_generation);
            assert!(!first.capability_claims.training_gradients);
            if backend == Qwen38Bf16EvidenceBackend::BoundedRowSparseTraversal {
                assert_eq!(first.visited_decoder_layer_count, 64);
                assert_eq!(first.visited_mtp_layer_count, 1);
                assert_eq!(first.layer_traversal.len(), 65);
                assert_eq!(first.tensor_reads.len(), 866);
            }
        }
        Ok(())
    }
}
