use std::{fs, path::Path};

use psionic_eval::{
    ATTNRES_TWO_PHASE_BENCHMARK_REF, AttnResBenchmarkError, AttnResBenchmarkInputCase,
    benchmark_attnres_two_phase_parity,
};
use psionic_models::{
    AttnResConfig, AttnResCpuReferenceModel, AttnResDiagnosticsSnapshot, AttnResExecutionError,
    AttnResNextTokenSample, TokenId, TokenSequence,
};
use psionic_runtime::AttnResTwoPhaseParityBudget;
use psionic_train::{
    AttnResTinyTrainingConfig, AttnResTinyTrainingCorpus, AttnResTinyTrainingError,
    train_attnres_tiny_next_token,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Canonical output root for the bounded AttnRes residual-vs-AttnRes comparison.
pub const ATTNRES_RESIDUAL_COMPARISON_OUTPUT_DIR: &str =
    "fixtures/attnres/runs/attnres_residual_comparison_v1";
/// Canonical machine-readable comparison report.
pub const ATTNRES_RESIDUAL_COMPARISON_REPORT_FILE: &str = "residual_comparison_report.json";

const TRAINING_FIXTURE: &str = include_str!("../../../fixtures/attnres/tiny_training_cases.json");
const BENCHMARK_FIXTURE: &str =
    include_str!("../../../fixtures/attnres/two_phase_benchmark_cases.json");
const TRAINING_SUMMARY_FILE: &str = "training_summary.json";
const BENCHMARK_RECEIPT_FILE: &str = "two_phase_benchmark_receipt.json";
const ROUTING_DIAGNOSTICS_FILE: &str = "routing_diagnostics.json";
const COMPARISON_REF: &str = "research://openagents/psionic/attnres/residual_vs_attnres/v1";

/// Stable variant family in the residual-vs-AttnRes comparison.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttnResResidualVariantKind {
    /// One block spanning the entire sublayer stack.
    MeanResidual,
    /// The configuration that currently ships as the reference AttnRes lane.
    ReferenceAttnRes,
    /// One block boundary before every sublayer.
    FullAttnRes,
    /// Any other intermediate block layout.
    IntermediateAttnRes,
}

impl AttnResResidualVariantKind {
    fn label(self) -> &'static str {
        match self {
            Self::MeanResidual => "mean_residual",
            Self::ReferenceAttnRes => "reference_attnres",
            Self::FullAttnRes => "full_attnres",
            Self::IntermediateAttnRes => "intermediate_attnres",
        }
    }
}

/// One compared AttnRes block layout.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttnResResidualVariant {
    /// Stable variant identifier used for file paths.
    pub variant_id: String,
    /// Human-readable label.
    pub label: String,
    /// High-level variant family.
    pub kind: AttnResResidualVariantKind,
    /// Number of residual blocks.
    pub num_blocks: usize,
}

impl AttnResResidualVariant {
    fn new(config: &AttnResConfig, num_blocks: usize) -> Self {
        let kind = if num_blocks == 1 {
            AttnResResidualVariantKind::MeanResidual
        } else if num_blocks == config.num_blocks {
            AttnResResidualVariantKind::ReferenceAttnRes
        } else if num_blocks == config.num_layers {
            AttnResResidualVariantKind::FullAttnRes
        } else {
            AttnResResidualVariantKind::IntermediateAttnRes
        };
        let variant_id = if matches!(kind, AttnResResidualVariantKind::IntermediateAttnRes) {
            format!("attnres_{num_blocks}_blocks")
        } else {
            kind.label().to_string()
        };
        let label = match kind {
            AttnResResidualVariantKind::MeanResidual => {
                format!("Mean residual baseline ({num_blocks} block)")
            }
            AttnResResidualVariantKind::ReferenceAttnRes => {
                format!("Reference AttnRes ({num_blocks} blocks)")
            }
            AttnResResidualVariantKind::FullAttnRes => {
                format!("Full AttnRes ({num_blocks} blocks)")
            }
            AttnResResidualVariantKind::IntermediateAttnRes => {
                format!("Intermediate AttnRes ({num_blocks} blocks)")
            }
        };
        Self {
            variant_id,
            label,
            kind,
            num_blocks,
        }
    }
}

/// One persisted variant summary inside the comparison report.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AttnResResidualComparisonVariantReport {
    /// Stable compared variant.
    pub variant: AttnResResidualVariant,
    /// Relative directory under the comparison root.
    pub variant_directory: String,
    /// Relative file containing the training summary.
    pub training_summary_file: String,
    /// Stable digest of the training summary.
    pub training_summary_digest: String,
    /// Relative file containing the two-phase benchmark receipt.
    pub benchmark_receipt_file: String,
    /// Stable digest of the benchmark receipt.
    pub benchmark_receipt_digest: String,
    /// Relative file containing the routing diagnostics snapshot.
    pub diagnostics_file: String,
    /// Stable digest of the routing diagnostics snapshot.
    pub diagnostics_digest: String,
    /// Stable model descriptor digest for the trained variant.
    pub model_descriptor_digest: String,
    /// Stable weight digest for the trained variant.
    pub model_weight_digest: String,
    /// Number of sublayer boundaries observed in the diagnostics sample.
    pub boundary_count: u32,
    /// Mean query norm across the diagnostics sample.
    pub mean_query_norm: f32,
    /// Training loss delta (`final - initial`) from the bounded tiny-training lane.
    pub training_loss_delta: f32,
    /// Held-out mean loss for the trained variant.
    pub held_out_mean_loss: f32,
    /// Held-out mean loss delta from the seeded baseline.
    pub held_out_mean_loss_delta: f32,
    /// Held-out cases improved by the trained variant.
    pub held_out_improved_case_count: u32,
    /// Mean routing delta across the held-out split.
    pub held_out_mean_routing_l2_delta: f32,
    /// Aggregate standard-logit throughput from the benchmark receipt.
    pub standard_logit_tokens_per_second: u32,
    /// Aggregate two-phase-logit throughput from the benchmark receipt.
    pub two_phase_logit_tokens_per_second: u32,
}

/// Top-level machine-readable residual-vs-AttnRes comparison report.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AttnResResidualComparisonReport {
    /// Stable comparison reference.
    pub comparison_ref: String,
    /// Training split digest for the shared corpus.
    pub training_dataset_digest: String,
    /// Held-out split digest for the shared corpus.
    pub held_out_dataset_digest: String,
    /// Diagnostics sample used across the variants.
    pub diagnostics_sample_id: String,
    /// Benchmark receipt reference shared by the variants.
    pub benchmark_ref: String,
    /// Parity budget used for all benchmark receipts.
    pub parity_budget: AttnResTwoPhaseParityBudget,
    /// Per-variant result summary.
    pub variants: Vec<AttnResResidualComparisonVariantReport>,
    /// Variant with the lowest held-out mean loss.
    pub best_variant_id: String,
}

impl AttnResResidualComparisonReport {
    /// Returns a stable digest over the comparison report.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        stable_digest(b"psionic_attnres_residual_comparison_report|", self)
    }
}

/// Failure while executing or persisting the bounded residual comparison.
#[derive(Debug, Error)]
pub enum AttnResResidualComparisonError {
    /// The committed training fixture could not be decoded.
    #[error("failed to decode attnres training fixture: {0}")]
    TrainingFixture(serde_json::Error),
    /// The committed benchmark fixture could not be decoded.
    #[error("failed to decode attnres benchmark fixture: {0}")]
    BenchmarkFixture(serde_json::Error),
    /// The bounded training lane failed for one variant.
    #[error("attnres residual comparison training failed for `{variant_id}`: {error}")]
    Training {
        /// Stable variant identifier.
        variant_id: String,
        /// Source error.
        error: AttnResTinyTrainingError,
    },
    /// The benchmark receipt lane failed for one variant.
    #[error("attnres residual comparison benchmark failed for `{variant_id}`: {error}")]
    Benchmark {
        /// Stable variant identifier.
        variant_id: String,
        /// Source error.
        error: AttnResBenchmarkError,
    },
    /// The routing diagnostics sample failed for one variant.
    #[error("attnres residual comparison diagnostics failed for `{variant_id}`: {error}")]
    Diagnostics {
        /// Stable variant identifier.
        variant_id: String,
        /// Source error.
        error: AttnResExecutionError,
    },
    /// Creating an output directory failed.
    #[error("failed to create `{path}`: {error}")]
    CreateDir {
        /// Directory path.
        path: String,
        /// Source error.
        error: std::io::Error,
    },
    /// Writing an output file failed.
    #[error("failed to write `{path}`: {error}")]
    Write {
        /// File path.
        path: String,
        /// Source error.
        error: std::io::Error,
    },
    /// Serializing one JSON artifact failed.
    #[error("failed to serialize `{artifact_kind}`: {error}")]
    Serialize {
        /// Artifact kind.
        artifact_kind: String,
        /// Source error.
        error: serde_json::Error,
    },
}

/// Executes the bounded residual-vs-AttnRes comparison and writes the
/// machine-readable result bundle under the requested root.
pub fn run_attnres_residual_comparison(
    output_dir: &Path,
) -> Result<AttnResResidualComparisonReport, AttnResResidualComparisonError> {
    let corpus: AttnResTinyTrainingCorpus = serde_json::from_str(TRAINING_FIXTURE)
        .map_err(AttnResResidualComparisonError::TrainingFixture)?;
    let benchmark_fixture: AttnResBenchmarkFixture = serde_json::from_str(BENCHMARK_FIXTURE)
        .map_err(AttnResResidualComparisonError::BenchmarkFixture)?;
    let benchmark_cases = benchmark_fixture.into_cases();
    let parity_budget = AttnResTwoPhaseParityBudget::default();
    let diagnostics_sample = corpus
        .held_out_samples
        .first()
        .cloned()
        .expect("committed attnres held-out fixture should not be empty");
    let variants = comparison_variants(&corpus.config);

    fs::create_dir_all(output_dir).map_err(|error| AttnResResidualComparisonError::CreateDir {
        path: output_dir.display().to_string(),
        error,
    })?;

    let mut reports = Vec::with_capacity(variants.len());
    for variant in variants {
        let variant_dir = output_dir.join(&variant.variant_id);
        fs::create_dir_all(&variant_dir).map_err(|error| {
            AttnResResidualComparisonError::CreateDir {
                path: variant_dir.display().to_string(),
                error,
            }
        })?;

        let mut variant_corpus = corpus.clone();
        variant_corpus.config.num_blocks = variant.num_blocks;
        let training_config = variant_training_config(&variant)?;
        let outcome =
            train_attnres_tiny_next_token(&variant_corpus, &training_config).map_err(|error| {
                AttnResResidualComparisonError::Training {
                    variant_id: variant.variant_id.clone(),
                    error,
                }
            })?;
        let benchmark = benchmark_attnres_two_phase_parity(
            &outcome.trained_model,
            benchmark_cases.as_slice(),
            parity_budget,
        )
        .map_err(|error| AttnResResidualComparisonError::Benchmark {
            variant_id: variant.variant_id.clone(),
            error,
        })?;
        let diagnostics = comparison_diagnostics(
            &outcome.trained_model,
            &diagnostics_sample,
            &variant.variant_id,
        )?;

        let training_summary_path = variant_dir.join(TRAINING_SUMMARY_FILE);
        let benchmark_path = variant_dir.join(BENCHMARK_RECEIPT_FILE);
        let diagnostics_path = variant_dir.join(ROUTING_DIAGNOSTICS_FILE);
        write_json(
            &training_summary_path,
            "attnres_tiny_training_summary",
            &outcome.summary,
        )?;
        write_json(
            &benchmark_path,
            "attnres_two_phase_benchmark_receipt",
            &benchmark,
        )?;
        write_json(
            &diagnostics_path,
            "attnres_diagnostics_snapshot",
            &diagnostics,
        )?;

        reports.push(AttnResResidualComparisonVariantReport {
            variant: variant.clone(),
            variant_directory: variant.variant_id.clone(),
            training_summary_file: String::from(TRAINING_SUMMARY_FILE),
            training_summary_digest: stable_digest(
                b"psionic_attnres_training_summary|",
                &outcome.summary,
            ),
            benchmark_receipt_file: String::from(BENCHMARK_RECEIPT_FILE),
            benchmark_receipt_digest: benchmark.stable_digest(),
            diagnostics_file: String::from(ROUTING_DIAGNOSTICS_FILE),
            diagnostics_digest: stable_digest(
                b"psionic_attnres_routing_diagnostics|",
                &diagnostics,
            ),
            model_descriptor_digest: outcome.trained_model.descriptor().stable_digest(),
            model_weight_digest: outcome.trained_model.descriptor().weights.digest.clone(),
            boundary_count: diagnostics
                .sublayers
                .iter()
                .filter(|snapshot| snapshot.starts_new_block_before)
                .count() as u32,
            mean_query_norm: mean_query_norm(&diagnostics),
            training_loss_delta: outcome.summary.training_loss_delta,
            held_out_mean_loss: outcome.summary.held_out_eval.trained_mean_loss,
            held_out_mean_loss_delta: outcome.summary.held_out_eval.mean_loss_delta,
            held_out_improved_case_count: outcome.summary.held_out_eval.improved_case_count,
            held_out_mean_routing_l2_delta: outcome.summary.held_out_eval.mean_routing_l2_delta,
            standard_logit_tokens_per_second: benchmark.standard_logit_tokens_per_second,
            two_phase_logit_tokens_per_second: benchmark.two_phase_logit_tokens_per_second,
        });
    }

    reports.sort_by_key(|report| report.variant.num_blocks);
    let best_variant_id = reports
        .iter()
        .min_by(|left, right| {
            left.held_out_mean_loss
                .total_cmp(&right.held_out_mean_loss)
                .then_with(|| {
                    left.training_loss_delta
                        .total_cmp(&right.training_loss_delta)
                })
        })
        .map(|report| report.variant.variant_id.clone())
        .expect("bounded residual comparison always emits at least one variant");
    let report = AttnResResidualComparisonReport {
        comparison_ref: String::from(COMPARISON_REF),
        training_dataset_digest: corpus.training_digest(),
        held_out_dataset_digest: corpus.held_out_digest(),
        diagnostics_sample_id: diagnostics_sample.sample_id,
        benchmark_ref: String::from(ATTNRES_TWO_PHASE_BENCHMARK_REF),
        parity_budget,
        variants: reports,
        best_variant_id,
    };
    write_json(
        &output_dir.join(ATTNRES_RESIDUAL_COMPARISON_REPORT_FILE),
        "attnres_residual_comparison_report",
        &report,
    )?;
    Ok(report)
}

#[derive(Debug, Deserialize)]
struct AttnResBenchmarkFixture {
    cases: Vec<AttnResBenchmarkFixtureCase>,
}

impl AttnResBenchmarkFixture {
    fn into_cases(self) -> Vec<AttnResBenchmarkInputCase> {
        self.cases
            .into_iter()
            .map(AttnResBenchmarkFixtureCase::into_case)
            .collect()
    }
}

#[derive(Debug, Deserialize)]
struct AttnResBenchmarkFixtureCase {
    case_id: String,
    token_ids: Vec<TokenId>,
}

impl AttnResBenchmarkFixtureCase {
    fn into_case(self) -> AttnResBenchmarkInputCase {
        AttnResBenchmarkInputCase::new(self.case_id, TokenSequence::new(self.token_ids))
    }
}

fn comparison_variants(config: &AttnResConfig) -> Vec<AttnResResidualVariant> {
    let mut block_counts = vec![1, config.num_blocks, config.num_layers];
    block_counts.sort_unstable();
    block_counts.dedup();
    block_counts
        .into_iter()
        .filter(|num_blocks| config.num_layers.is_multiple_of(*num_blocks))
        .map(|num_blocks| AttnResResidualVariant::new(config, num_blocks))
        .collect()
}

fn variant_training_config(
    variant: &AttnResResidualVariant,
) -> Result<AttnResTinyTrainingConfig, AttnResResidualComparisonError> {
    let mut config = AttnResTinyTrainingConfig::reference().map_err(|error| {
        AttnResResidualComparisonError::Training {
            variant_id: variant.variant_id.clone(),
            error,
        }
    })?;
    config.model_id = format!("attnres-residual-compare-{}", variant.variant_id);
    config.run_id = format!("attnres-residual-compare-{}", variant.variant_id);
    config.checkpoint_family = String::from("research.attnres.residual_vs_attnres");
    Ok(config)
}

fn comparison_diagnostics(
    model: &AttnResCpuReferenceModel,
    sample: &AttnResNextTokenSample,
    variant_id: &str,
) -> Result<AttnResDiagnosticsSnapshot, AttnResResidualComparisonError> {
    model
        .forward_hidden_with_diagnostics(&[sample.input_tokens.clone()])
        .map(|(_, diagnostics)| diagnostics)
        .map_err(|error| AttnResResidualComparisonError::Diagnostics {
            variant_id: variant_id.to_string(),
            error,
        })
}

fn mean_query_norm(diagnostics: &AttnResDiagnosticsSnapshot) -> f32 {
    if diagnostics.sublayers.is_empty() {
        return 0.0;
    }
    let total = diagnostics
        .sublayers
        .iter()
        .map(|snapshot| snapshot.query_norm)
        .sum::<f32>();
    total / diagnostics.sublayers.len() as f32
}

fn write_json<T>(
    path: &Path,
    artifact_kind: &str,
    value: &T,
) -> Result<(), AttnResResidualComparisonError>
where
    T: Serialize,
{
    let bytes = serde_json::to_vec_pretty(value).map_err(|error| {
        AttnResResidualComparisonError::Serialize {
            artifact_kind: artifact_kind.to_string(),
            error,
        }
    })?;
    fs::write(path, bytes).map_err(|error| AttnResResidualComparisonError::Write {
        path: path.display().to_string(),
        error,
    })
}

fn stable_digest<T>(prefix: &[u8], value: &T) -> String
where
    T: Serialize,
{
    let bytes = serde_json::to_vec(value).expect("stable digest serialization should succeed");
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use tempfile::tempdir;

    use super::{
        ATTNRES_RESIDUAL_COMPARISON_REPORT_FILE, AttnResResidualVariantKind,
        run_attnres_residual_comparison,
    };

    #[test]
    fn residual_comparison_writes_report_and_variant_artifacts() -> Result<(), Box<dyn Error>> {
        let temp = tempdir()?;
        let report = run_attnres_residual_comparison(temp.path())?;

        assert_eq!(
            report.comparison_ref,
            "research://openagents/psionic/attnres/residual_vs_attnres/v1"
        );
        assert_eq!(report.variants.len(), 3);
        assert!(
            temp.path()
                .join(ATTNRES_RESIDUAL_COMPARISON_REPORT_FILE)
                .exists()
        );
        assert!(
            report
                .variants
                .iter()
                .any(|variant| variant.variant.kind == AttnResResidualVariantKind::MeanResidual)
        );
        assert!(
            report
                .variants
                .iter()
                .any(|variant| variant.variant.kind == AttnResResidualVariantKind::ReferenceAttnRes)
        );
        assert!(
            report
                .variants
                .iter()
                .any(|variant| variant.variant.kind == AttnResResidualVariantKind::FullAttnRes)
        );
        for variant in &report.variants {
            assert!(!variant.training_summary_digest.is_empty());
            assert!(!variant.benchmark_receipt_digest.is_empty());
            assert!(!variant.diagnostics_digest.is_empty());
            assert!(
                temp.path()
                    .join(&variant.variant_directory)
                    .join("training_summary.json")
                    .exists()
            );
            assert!(
                temp.path()
                    .join(&variant.variant_directory)
                    .join("two_phase_benchmark_receipt.json")
                    .exists()
            );
            assert!(
                temp.path()
                    .join(&variant.variant_directory)
                    .join("routing_diagnostics.json")
                    .exists()
            );
        }
        assert!(!report.stable_digest().is_empty());
        Ok(())
    }
}
