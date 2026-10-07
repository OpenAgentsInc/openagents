use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use psionic_models::{
    TassadarArticleTransformerArchitectureVariant, TassadarArticleTransformerEmbeddingStrategy,
};
use psionic_transformer::EncoderDecoderTransformerConfig;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const TASSADAR_ARTICLE_TRANSFORMER_TRAINING_EVIDENCE_BUNDLE_REF: &str =
    "fixtures/tassadar/runs/tassadar_article_transformer_training_v1/article_transformer_training_evidence_bundle.json";

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarArticleTransformerToyTaskKind {
    SelectFirst,
    SelectSecond,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleTransformerToyTaskExample {
    pub example_id: String,
    pub task_kind: TassadarArticleTransformerToyTaskKind,
    pub source_tokens: Vec<usize>,
    pub target_tokens: Vec<usize>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarArticleTransformerGradientCheckEvidence {
    pub parameter_id: String,
    pub gradient_len: usize,
    pub max_abs_gradient: f32,
    pub all_finite: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarArticleTransformerTrainingStepEvidence {
    pub global_step: u64,
    pub batch_id: String,
    pub training_mean_loss: f32,
    pub training_exact_match_count: usize,
    pub held_out_mean_loss: f32,
    pub held_out_exact_match_count: usize,
    pub effective_learning_rates: BTreeMap<String, f32>,
    pub scheduler_kinds: BTreeMap<String, String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleTransformerCheckpointEvidence {
    pub checkpoint_ref: String,
    pub checkpoint_family: String,
    #[serde(default)]
    pub stream_id: String,
    pub manifest_digest: String,
    #[serde(default)]
    pub object_digest: String,
    #[serde(default)]
    pub writer_node_id: String,
    #[serde(default)]
    pub membership_epoch: u64,
    #[serde(default)]
    pub cluster_state_digest: String,
    #[serde(default)]
    pub topology_digest: String,
    #[serde(default)]
    pub started_at_ms: u64,
    #[serde(default)]
    pub step: u64,
    #[serde(default)]
    pub durable_at_ms: u64,
    #[serde(default)]
    pub parent_checkpoint_ref: Option<String>,
    #[serde(default)]
    pub parent_manifest_digest: Option<String>,
    pub trained_trainable_parameter_digest: String,
    pub restored_trainable_parameter_digest: String,
    pub restore_matches_trained_state: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarArticleTransformerTrainingEvidenceBundle {
    pub schema_version: u16,
    pub bundle_id: String,
    pub tied_requirement_id: String,
    pub model_module_ref: String,
    pub transformer_module_ref: String,
    pub train_module_ref: String,
    pub run_id: String,
    pub checkpoint_family: String,
    pub architecture_variant: TassadarArticleTransformerArchitectureVariant,
    pub config: EncoderDecoderTransformerConfig,
    pub embedding_strategy: TassadarArticleTransformerEmbeddingStrategy,
    pub trainable_parameter_ids: Vec<String>,
    pub trainable_parameter_scalar_count: usize,
    pub loss_kind: String,
    pub optimizer_kind: String,
    pub scheduler_kind: String,
    pub warmup_steps: u64,
    pub label_smoothing: f32,
    pub finite_difference_epsilon: f32,
    pub training_examples: Vec<TassadarArticleTransformerToyTaskExample>,
    pub held_out_examples: Vec<TassadarArticleTransformerToyTaskExample>,
    pub gradient_checks: Vec<TassadarArticleTransformerGradientCheckEvidence>,
    pub step_evidence: Vec<TassadarArticleTransformerTrainingStepEvidence>,
    pub initial_training_mean_loss: f32,
    pub final_training_mean_loss: f32,
    pub initial_training_exact_match_count: usize,
    pub final_training_exact_match_count: usize,
    pub final_held_out_mean_loss: f32,
    pub final_held_out_exact_match_count: usize,
    pub overfit_training_exact_match: bool,
    pub checkpoint: TassadarArticleTransformerCheckpointEvidence,
    pub claim_boundary: String,
    pub bundle_digest: String,
}

impl TassadarArticleTransformerTrainingEvidenceBundle {
    #[must_use]
    pub fn with_bundle_digest(mut self) -> Self {
        self.bundle_digest.clear();
        self.bundle_digest = stable_digest(
            b"psionic_tassadar_article_transformer_training_evidence_bundle|",
            &self,
        );
        self
    }
}

#[derive(Debug, Error)]
pub enum TassadarArticleTransformerTrainingEvidenceError {
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to decode `{path}`: {error}")]
    Decode {
        path: String,
        error: serde_json::Error,
    },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[must_use]
pub fn tassadar_article_transformer_training_evidence_bundle_path() -> PathBuf {
    repo_root().join(TASSADAR_ARTICLE_TRANSFORMER_TRAINING_EVIDENCE_BUNDLE_REF)
}

pub fn write_tassadar_article_transformer_training_evidence_bundle(
    output_path: impl AsRef<Path>,
    bundle: &TassadarArticleTransformerTrainingEvidenceBundle,
) -> Result<TassadarArticleTransformerTrainingEvidenceBundle, TassadarArticleTransformerTrainingEvidenceError>
{
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarArticleTransformerTrainingEvidenceError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let bundle = bundle.clone().with_bundle_digest();
    let json = serde_json::to_string_pretty(&bundle)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarArticleTransformerTrainingEvidenceError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(bundle)
}

pub fn read_tassadar_article_transformer_training_evidence_bundle(
    relative_path: &str,
) -> Result<TassadarArticleTransformerTrainingEvidenceBundle, TassadarArticleTransformerTrainingEvidenceError>
{
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarArticleTransformerTrainingEvidenceError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarArticleTransformerTrainingEvidenceError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("psionic-eval should live under <repo>/crates/psionic-eval")
        .to_path_buf()
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}
