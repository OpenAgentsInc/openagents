use std::{
    fs,
    path::{Path, PathBuf},
};

use psionic_core::Shape;
use psionic_models::{TassadarArticleTransformer, TassadarArticleTransformerError};
use psionic_runtime::{
    TassadarArticleTransformerCheckpointLineage,
    TassadarArticleTransformerForwardPassEvidenceBundle, TrainingCheckpointReference,
};
use psionic_transformer::TransformerExecutionMode;
use thiserror::Error;

use crate::{
    read_tassadar_article_transformer_training_evidence_bundle,
    TassadarArticleTransformerTrainingEvidenceBundle,
    TassadarArticleTransformerTrainingEvidenceError,
    TASSADAR_ARTICLE_TRANSFORMER_TRAINING_EVIDENCE_BUNDLE_REF,
};

pub const TASSADAR_ARTICLE_TRANSFORMER_FORWARD_PASS_EVIDENCE_BUNDLE_REF: &str =
    "fixtures/tassadar/runs/tassadar_article_transformer_forward_pass_v1/article_transformer_forward_pass_evidence_bundle.json";

#[derive(Debug, Error)]
pub enum TassadarArticleTransformerForwardPassEvidenceError {
    #[error(transparent)]
    TrainingEvidence(#[from] TassadarArticleTransformerTrainingEvidenceError),
    #[error(transparent)]
    Model(#[from] TassadarArticleTransformerError),
    #[error("article Transformer training evidence is missing example `{field}`")]
    MissingTrainingExample { field: &'static str },
    #[error("article Transformer checkpoint lineage is missing `{field}`")]
    MissingCheckpointField { field: &'static str },
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

pub fn build_tassadar_article_transformer_forward_pass_evidence_bundle(
) -> Result<
    TassadarArticleTransformerForwardPassEvidenceBundle,
    TassadarArticleTransformerForwardPassEvidenceError,
> {
    let training_evidence = read_tassadar_article_transformer_training_evidence_bundle(
        TASSADAR_ARTICLE_TRANSFORMER_TRAINING_EVIDENCE_BUNDLE_REF,
    )?;
    let example = training_evidence
        .training_examples
        .first()
        .ok_or(TassadarArticleTransformerForwardPassEvidenceError::MissingTrainingExample {
            field: "training_examples[0]",
        })?;
    let checkpoint_lineage = checkpoint_lineage(&training_evidence)?;
    let model = TassadarArticleTransformer::canonical_reference()?;
    model
        .forward_with_runtime_evidence(
            "tassadar_article_transformer_forward_pass_v1",
            "tassadar_article_transformer_forward_pass_reference_request",
            "psionic.article_transformer.forward_pass",
            vec![
                String::from("fixtures://tassadar/article_transformer/reference_linear"),
                String::from(TASSADAR_ARTICLE_TRANSFORMER_TRAINING_EVIDENCE_BUNDLE_REF),
            ],
            Shape::new(vec![1, example.source_tokens.len()]),
            example.source_tokens.as_slice(),
            Shape::new(vec![1, example.target_tokens.len()]),
            example.target_tokens.as_slice(),
            TransformerExecutionMode::Eval,
            Some(checkpoint_lineage),
        )
        .map_err(Into::into)
}

#[must_use]
pub fn tassadar_article_transformer_forward_pass_evidence_bundle_path() -> PathBuf {
    repo_root().join(TASSADAR_ARTICLE_TRANSFORMER_FORWARD_PASS_EVIDENCE_BUNDLE_REF)
}

pub fn write_tassadar_article_transformer_forward_pass_evidence_bundle(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarArticleTransformerForwardPassEvidenceBundle,
    TassadarArticleTransformerForwardPassEvidenceError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarArticleTransformerForwardPassEvidenceError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let bundle = build_tassadar_article_transformer_forward_pass_evidence_bundle()?;
    let json = serde_json::to_string_pretty(&bundle)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarArticleTransformerForwardPassEvidenceError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(bundle)
}

pub fn read_tassadar_article_transformer_forward_pass_evidence_bundle(
    relative_path: &str,
) -> Result<
    TassadarArticleTransformerForwardPassEvidenceBundle,
    TassadarArticleTransformerForwardPassEvidenceError,
> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarArticleTransformerForwardPassEvidenceError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarArticleTransformerForwardPassEvidenceError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

fn checkpoint_lineage(
    training_evidence: &TassadarArticleTransformerTrainingEvidenceBundle,
) -> Result<
    TassadarArticleTransformerCheckpointLineage,
    TassadarArticleTransformerForwardPassEvidenceError,
> {
    let checkpoint = &training_evidence.checkpoint;
    if checkpoint.stream_id.trim().is_empty() {
        return Err(TassadarArticleTransformerForwardPassEvidenceError::MissingCheckpointField {
            field: "stream_id",
        });
    }
    if checkpoint.object_digest.trim().is_empty() {
        return Err(TassadarArticleTransformerForwardPassEvidenceError::MissingCheckpointField {
            field: "object_digest",
        });
    }
    if checkpoint.writer_node_id.trim().is_empty() {
        return Err(TassadarArticleTransformerForwardPassEvidenceError::MissingCheckpointField {
            field: "writer_node_id",
        });
    }
    if checkpoint.cluster_state_digest.trim().is_empty() {
        return Err(TassadarArticleTransformerForwardPassEvidenceError::MissingCheckpointField {
            field: "cluster_state_digest",
        });
    }
    if checkpoint.topology_digest.trim().is_empty() {
        return Err(TassadarArticleTransformerForwardPassEvidenceError::MissingCheckpointField {
            field: "topology_digest",
        });
    }
    Ok(TassadarArticleTransformerCheckpointLineage {
        checkpoint: TrainingCheckpointReference::new(
            checkpoint.checkpoint_family.clone(),
            checkpoint.stream_id.clone(),
            checkpoint.manifest_digest.clone(),
            checkpoint.object_digest.clone(),
            checkpoint.writer_node_id.clone(),
            checkpoint.membership_epoch,
            checkpoint.cluster_state_digest.clone(),
            checkpoint.topology_digest.clone(),
            checkpoint.started_at_ms,
        )
        .with_checkpoint_ref(checkpoint.checkpoint_ref.clone())
        .with_step(checkpoint.step)
        .with_durable_at_ms(checkpoint.durable_at_ms),
        parent_checkpoint_ref: checkpoint.parent_checkpoint_ref.clone(),
        parent_manifest_digest: checkpoint.parent_manifest_digest.clone(),
    })
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("psionic-eval should live under <repo>/crates/psionic-eval")
        .to_path_buf()
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_article_transformer_forward_pass_evidence_bundle, read_tassadar_article_transformer_forward_pass_evidence_bundle,
        tassadar_article_transformer_forward_pass_evidence_bundle_path,
        write_tassadar_article_transformer_forward_pass_evidence_bundle,
        TassadarArticleTransformerForwardPassEvidenceBundle,
        TASSADAR_ARTICLE_TRANSFORMER_FORWARD_PASS_EVIDENCE_BUNDLE_REF,
    };

    #[test]
    fn article_transformer_forward_pass_evidence_is_replay_safe_and_checkpoint_bound() {
        let bundle =
            build_tassadar_article_transformer_forward_pass_evidence_bundle().expect("bundle");

        assert_eq!(bundle.tied_requirement_id, "TAS-165");
        assert!(bundle.replay_receipt.deterministic_match);
        assert!(bundle.checkpoint_lineage.is_some());
        assert_eq!(bundle.proof_bundle.failure_reason, None);
        assert_eq!(bundle.trace_artifact.encoder_layer_traces.len(), 2);
        assert_eq!(bundle.trace_artifact.decoder_self_attention_traces.len(), 2);
        assert_eq!(bundle.trace_artifact.decoder_cross_attention_traces.len(), 2);
    }

    #[test]
    fn article_transformer_forward_pass_evidence_matches_committed_truth() {
        let generated =
            build_tassadar_article_transformer_forward_pass_evidence_bundle().expect("bundle");
        let committed: TassadarArticleTransformerForwardPassEvidenceBundle =
            read_tassadar_article_transformer_forward_pass_evidence_bundle(
                TASSADAR_ARTICLE_TRANSFORMER_FORWARD_PASS_EVIDENCE_BUNDLE_REF,
            )
            .expect("committed bundle");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_article_transformer_forward_pass_evidence_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("article_transformer_forward_pass_evidence_bundle.json");
        let written = write_tassadar_article_transformer_forward_pass_evidence_bundle(&output_path)
            .expect("write bundle");
        let persisted: TassadarArticleTransformerForwardPassEvidenceBundle =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read")).expect("decode");
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_article_transformer_forward_pass_evidence_bundle_path()
                .strip_prefix(super::repo_root())
                .expect("repo-relative path")
                .to_string_lossy(),
            TASSADAR_ARTICLE_TRANSFORMER_FORWARD_PASS_EVIDENCE_BUNDLE_REF
        );
    }
}
