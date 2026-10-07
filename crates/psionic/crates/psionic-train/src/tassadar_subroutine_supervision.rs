use std::collections::BTreeSet;

use psionic_data::TassadarSequenceSplit;
use psionic_models::{
    TassadarExecutorSubroutineLibrary, TassadarExecutorSubroutineTarget,
    TassadarExecutorSubroutineWorkloadFamily, tassadar_executor_subroutine_library,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const TASSADAR_EXECUTOR_SUBROUTINE_DATASET_SCHEMA_VERSION: u16 = 1;

/// Supervision target mode for one bounded learned-executor dataset materialization.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarExecutorSupervisionTargetMode {
    FullTrace,
    SubroutineLibrary,
}

impl TassadarExecutorSupervisionTargetMode {
    /// Returns the stable mode label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::FullTrace => "full_trace",
            Self::SubroutineLibrary => "subroutine_library",
        }
    }
}

/// One seeded workload example for the public bounded subroutine-supervision corpus.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorSubroutineCorpusExample {
    pub example_id: String,
    pub workload_family: TassadarExecutorSubroutineWorkloadFamily,
    pub split: TassadarSequenceSplit,
    pub summary: String,
    pub full_trace_targets: Vec<String>,
    pub subroutine_targets: Vec<TassadarExecutorSubroutineTarget>,
}

/// Dataset manifest for one bounded learned-executor supervision mode.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorSubroutineDatasetManifest {
    pub schema_version: u16,
    pub dataset_id: String,
    pub supervision_mode: TassadarExecutorSupervisionTargetMode,
    pub library_digest: String,
    pub workload_families: Vec<TassadarExecutorSubroutineWorkloadFamily>,
    pub train_example_count: u32,
    pub validation_example_count: u32,
    pub test_example_count: u32,
    pub target_vocab_size: u32,
    pub target_count: u32,
    pub examples: Vec<TassadarExecutorSubroutineCorpusExample>,
    pub claim_boundary: String,
    pub manifest_digest: String,
}

impl TassadarExecutorSubroutineDatasetManifest {
    fn new(
        supervision_mode: TassadarExecutorSupervisionTargetMode,
        library: &TassadarExecutorSubroutineLibrary,
        examples: Vec<TassadarExecutorSubroutineCorpusExample>,
    ) -> Self {
        let workload_families = examples
            .iter()
            .map(|example| example.workload_family)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let train_example_count = examples
            .iter()
            .filter(|example| example.split == TassadarSequenceSplit::Train)
            .count() as u32;
        let validation_example_count = examples
            .iter()
            .filter(|example| example.split == TassadarSequenceSplit::Validation)
            .count() as u32;
        let test_example_count = examples
            .iter()
            .filter(|example| example.split == TassadarSequenceSplit::Test)
            .count() as u32;
        let target_vocab = target_vocabulary(examples.as_slice(), supervision_mode);
        let target_count = target_count(examples.as_slice(), supervision_mode);
        let mut manifest = Self {
            schema_version: TASSADAR_EXECUTOR_SUBROUTINE_DATASET_SCHEMA_VERSION,
            dataset_id: format!(
                "tassadar.executor.subroutine_supervision.{}.v0",
                supervision_mode.label()
            ),
            supervision_mode,
            library_digest: library.library_digest.clone(),
            workload_families,
            train_example_count,
            validation_example_count,
            test_example_count,
            target_vocab_size: target_vocab.len() as u32,
            target_count,
            examples,
            claim_boundary: String::from(
                "bounded learned-executor supervision corpus only; freezes the same sort, shortest-path, and sudoku-style examples under either full-trace or reusable subroutine targets and supports OOD label-reuse analysis, not whole-model training claims",
            ),
            manifest_digest: String::new(),
        };
        manifest.manifest_digest =
            stable_digest(b"tassadar_executor_subroutine_dataset_manifest|", &manifest);
        manifest
    }
}

/// Config for one held-out-workload OOD supervision comparison.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorSubroutineOodProxyConfig {
    pub supervision_mode: TassadarExecutorSupervisionTargetMode,
    pub held_out_workload_family: TassadarExecutorSubroutineWorkloadFamily,
}

/// Deterministic OOD reuse comparison for one supervision mode and held-out workload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorSubroutineOodProxyReport {
    pub supervision_mode: TassadarExecutorSupervisionTargetMode,
    pub held_out_workload_family: TassadarExecutorSubroutineWorkloadFamily,
    pub train_workload_families: Vec<TassadarExecutorSubroutineWorkloadFamily>,
    pub train_target_vocab_size: u32,
    pub held_out_target_count: u32,
    pub reusable_held_out_target_count: u32,
    pub held_out_reuse_bps: u32,
    pub detail: String,
}

/// Dataset validation failures for the bounded subroutine-supervision lane.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum TassadarExecutorSubroutineDatasetError {
    #[error("unknown subroutine id `{subroutine_id}` in example `{example_id}`")]
    UnknownSubroutineId {
        example_id: String,
        subroutine_id: String,
    },
    #[error(
        "subroutine `{subroutine_id}` is not valid for workload `{workload_family}` in example `{example_id}`"
    )]
    UnsupportedSubroutineWorkload {
        example_id: String,
        subroutine_id: String,
        workload_family: String,
    },
}

/// Returns the seeded bounded corpus used by the public subroutine-training lane.
#[must_use]
pub fn tassadar_executor_subroutine_corpus() -> Vec<TassadarExecutorSubroutineCorpusExample> {
    vec![
        TassadarExecutorSubroutineCorpusExample {
            example_id: String::from("sort_train_a"),
            workload_family: TassadarExecutorSubroutineWorkloadFamily::Sort,
            split: TassadarSequenceSplit::Train,
            summary: String::from("bounded length-4 compare/swap pass"),
            full_trace_targets: vec![
                String::from("sort.scan_window[0]"),
                String::from("sort.compare_pair[0,1]"),
                String::from("sort.swap_if_needed[0,1]"),
                String::from("sort.commit_pass[0]"),
            ],
            subroutine_targets: vec![
                target(
                    "tassadar.subroutine.compare_candidates.v1",
                    "sort.compare_pair",
                    TassadarExecutorSubroutineWorkloadFamily::Sort,
                ),
                target(
                    "tassadar.subroutine.commit_update.v1",
                    "sort.swap_commit",
                    TassadarExecutorSubroutineWorkloadFamily::Sort,
                ),
                target(
                    "tassadar.subroutine.advance_cursor.v1",
                    "sort.advance_window",
                    TassadarExecutorSubroutineWorkloadFamily::Sort,
                ),
            ],
        },
        TassadarExecutorSubroutineCorpusExample {
            example_id: String::from("sort_validation_a"),
            workload_family: TassadarExecutorSubroutineWorkloadFamily::Sort,
            split: TassadarSequenceSplit::Validation,
            summary: String::from("bounded length-4 second compare/swap pass"),
            full_trace_targets: vec![
                String::from("sort.scan_window[1]"),
                String::from("sort.compare_pair[1,2]"),
                String::from("sort.swap_if_needed[1,2]"),
                String::from("sort.commit_pass[1]"),
            ],
            subroutine_targets: vec![
                target(
                    "tassadar.subroutine.compare_candidates.v1",
                    "sort.compare_pair",
                    TassadarExecutorSubroutineWorkloadFamily::Sort,
                ),
                target(
                    "tassadar.subroutine.commit_update.v1",
                    "sort.swap_commit",
                    TassadarExecutorSubroutineWorkloadFamily::Sort,
                ),
                target(
                    "tassadar.subroutine.advance_cursor.v1",
                    "sort.advance_window",
                    TassadarExecutorSubroutineWorkloadFamily::Sort,
                ),
            ],
        },
        TassadarExecutorSubroutineCorpusExample {
            example_id: String::from("shortest_path_train_a"),
            workload_family: TassadarExecutorSubroutineWorkloadFamily::ClrsShortestPath,
            split: TassadarSequenceSplit::Train,
            summary: String::from("bounded two-route relaxation with one frontier pop"),
            full_trace_targets: vec![
                String::from("shortest_path.pop_frontier[a]"),
                String::from("shortest_path.relax_edge[a->b]"),
                String::from("shortest_path.commit_distance[b]"),
                String::from("shortest_path.advance_frontier[b]"),
            ],
            subroutine_targets: vec![
                target(
                    "tassadar.subroutine.compare_candidates.v1",
                    "shortest_path.compare_relaxation",
                    TassadarExecutorSubroutineWorkloadFamily::ClrsShortestPath,
                ),
                target(
                    "tassadar.subroutine.prune_candidates.v1",
                    "shortest_path.prune_frontier",
                    TassadarExecutorSubroutineWorkloadFamily::ClrsShortestPath,
                ),
                target(
                    "tassadar.subroutine.commit_update.v1",
                    "shortest_path.commit_distance",
                    TassadarExecutorSubroutineWorkloadFamily::ClrsShortestPath,
                ),
                target(
                    "tassadar.subroutine.advance_cursor.v1",
                    "shortest_path.advance_frontier",
                    TassadarExecutorSubroutineWorkloadFamily::ClrsShortestPath,
                ),
            ],
        },
        TassadarExecutorSubroutineCorpusExample {
            example_id: String::from("shortest_path_test_a"),
            workload_family: TassadarExecutorSubroutineWorkloadFamily::ClrsShortestPath,
            split: TassadarSequenceSplit::Test,
            summary: String::from("bounded alternate two-route relaxation"),
            full_trace_targets: vec![
                String::from("shortest_path.pop_frontier[c]"),
                String::from("shortest_path.relax_edge[c->d]"),
                String::from("shortest_path.commit_distance[d]"),
                String::from("shortest_path.advance_frontier[d]"),
            ],
            subroutine_targets: vec![
                target(
                    "tassadar.subroutine.compare_candidates.v1",
                    "shortest_path.compare_relaxation",
                    TassadarExecutorSubroutineWorkloadFamily::ClrsShortestPath,
                ),
                target(
                    "tassadar.subroutine.prune_candidates.v1",
                    "shortest_path.prune_frontier",
                    TassadarExecutorSubroutineWorkloadFamily::ClrsShortestPath,
                ),
                target(
                    "tassadar.subroutine.commit_update.v1",
                    "shortest_path.commit_distance",
                    TassadarExecutorSubroutineWorkloadFamily::ClrsShortestPath,
                ),
                target(
                    "tassadar.subroutine.advance_cursor.v1",
                    "shortest_path.advance_frontier",
                    TassadarExecutorSubroutineWorkloadFamily::ClrsShortestPath,
                ),
            ],
        },
        TassadarExecutorSubroutineCorpusExample {
            example_id: String::from("sudoku_train_a"),
            workload_family: TassadarExecutorSubroutineWorkloadFamily::SudokuStyle,
            split: TassadarSequenceSplit::Train,
            summary: String::from("bounded candidate-prune plus commit step for one cell"),
            full_trace_targets: vec![
                String::from("sudoku.scan_cell[r0c0]"),
                String::from("sudoku.prune_candidates[r0c0]"),
                String::from("sudoku.commit_digit[r0c0]"),
                String::from("sudoku.advance_cell[r0c1]"),
            ],
            subroutine_targets: vec![
                target(
                    "tassadar.subroutine.prune_candidates.v1",
                    "sudoku.prune_candidates",
                    TassadarExecutorSubroutineWorkloadFamily::SudokuStyle,
                ),
                target(
                    "tassadar.subroutine.commit_update.v1",
                    "sudoku.commit_digit",
                    TassadarExecutorSubroutineWorkloadFamily::SudokuStyle,
                ),
                target(
                    "tassadar.subroutine.advance_cursor.v1",
                    "sudoku.advance_cell",
                    TassadarExecutorSubroutineWorkloadFamily::SudokuStyle,
                ),
            ],
        },
        TassadarExecutorSubroutineCorpusExample {
            example_id: String::from("sudoku_validation_a"),
            workload_family: TassadarExecutorSubroutineWorkloadFamily::SudokuStyle,
            split: TassadarSequenceSplit::Validation,
            summary: String::from("bounded alternate candidate-prune plus commit step"),
            full_trace_targets: vec![
                String::from("sudoku.scan_cell[r1c1]"),
                String::from("sudoku.prune_candidates[r1c1]"),
                String::from("sudoku.commit_digit[r1c1]"),
                String::from("sudoku.advance_cell[r1c2]"),
            ],
            subroutine_targets: vec![
                target(
                    "tassadar.subroutine.prune_candidates.v1",
                    "sudoku.prune_candidates",
                    TassadarExecutorSubroutineWorkloadFamily::SudokuStyle,
                ),
                target(
                    "tassadar.subroutine.commit_update.v1",
                    "sudoku.commit_digit",
                    TassadarExecutorSubroutineWorkloadFamily::SudokuStyle,
                ),
                target(
                    "tassadar.subroutine.advance_cursor.v1",
                    "sudoku.advance_cell",
                    TassadarExecutorSubroutineWorkloadFamily::SudokuStyle,
                ),
            ],
        },
    ]
}

/// Materializes the bounded corpus under one explicit supervision target mode.
pub fn build_tassadar_executor_subroutine_dataset_manifest(
    supervision_mode: TassadarExecutorSupervisionTargetMode,
) -> Result<TassadarExecutorSubroutineDatasetManifest, TassadarExecutorSubroutineDatasetError> {
    let library = tassadar_executor_subroutine_library();
    let examples = tassadar_executor_subroutine_corpus();
    validate_corpus(&library, examples.as_slice())?;
    Ok(TassadarExecutorSubroutineDatasetManifest::new(
        supervision_mode,
        &library,
        examples,
    ))
}

/// Builds one deterministic held-out-workload OOD reuse comparison for the
/// bounded full-trace vs subroutine-library ablation.
pub fn build_tassadar_executor_subroutine_ood_proxy(
    config: &TassadarExecutorSubroutineOodProxyConfig,
) -> Result<TassadarExecutorSubroutineOodProxyReport, TassadarExecutorSubroutineDatasetError> {
    let manifest = build_tassadar_executor_subroutine_dataset_manifest(config.supervision_mode)?;
    let training_examples = manifest
        .examples
        .iter()
        .filter(|example| example.workload_family != config.held_out_workload_family)
        .cloned()
        .collect::<Vec<_>>();
    let held_out_examples = manifest
        .examples
        .iter()
        .filter(|example| example.workload_family == config.held_out_workload_family)
        .cloned()
        .collect::<Vec<_>>();
    let train_workload_families = training_examples
        .iter()
        .map(|example| example.workload_family)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let train_target_vocab = target_vocabulary(training_examples.as_slice(), config.supervision_mode);
    let held_out_targets = flattened_targets(held_out_examples.as_slice(), config.supervision_mode);
    let reusable_held_out_target_count = held_out_targets
        .iter()
        .filter(|target| train_target_vocab.contains(*target))
        .count() as u32;
    let held_out_target_count = held_out_targets.len() as u32;
    let held_out_reuse_bps = basis_points(reusable_held_out_target_count, held_out_target_count);
    Ok(TassadarExecutorSubroutineOodProxyReport {
        supervision_mode: config.supervision_mode,
        held_out_workload_family: config.held_out_workload_family,
        train_workload_families,
        train_target_vocab_size: train_target_vocab.len() as u32,
        held_out_target_count,
        reusable_held_out_target_count,
        held_out_reuse_bps,
        detail: format!(
            "held_out_workload={}, supervision_mode={}, reusable_held_out_target_count={}/{}, train_target_vocab_size={}",
            config.held_out_workload_family.label(),
            config.supervision_mode.label(),
            reusable_held_out_target_count,
            held_out_target_count,
            train_target_vocab.len(),
        ),
    })
}

fn target(
    subroutine_id: &str,
    target_label: &str,
    workload_family: TassadarExecutorSubroutineWorkloadFamily,
) -> TassadarExecutorSubroutineTarget {
    TassadarExecutorSubroutineTarget {
        subroutine_id: String::from(subroutine_id),
        target_label: String::from(target_label),
        workload_family,
    }
}

fn validate_corpus(
    library: &TassadarExecutorSubroutineLibrary,
    examples: &[TassadarExecutorSubroutineCorpusExample],
) -> Result<(), TassadarExecutorSubroutineDatasetError> {
    for example in examples {
        for target in &example.subroutine_targets {
            let Some(entry) = library.entry(target.subroutine_id.as_str()) else {
                return Err(TassadarExecutorSubroutineDatasetError::UnknownSubroutineId {
                    example_id: example.example_id.clone(),
                    subroutine_id: target.subroutine_id.clone(),
                });
            };
            if !entry.supports_workload(target.workload_family) {
                return Err(
                    TassadarExecutorSubroutineDatasetError::UnsupportedSubroutineWorkload {
                        example_id: example.example_id.clone(),
                        subroutine_id: target.subroutine_id.clone(),
                        workload_family: target.workload_family.label().to_string(),
                    },
                );
            }
        }
    }
    Ok(())
}

fn target_vocabulary(
    examples: &[TassadarExecutorSubroutineCorpusExample],
    supervision_mode: TassadarExecutorSupervisionTargetMode,
) -> BTreeSet<String> {
    flattened_targets(examples, supervision_mode).into_iter().collect()
}

fn target_count(
    examples: &[TassadarExecutorSubroutineCorpusExample],
    supervision_mode: TassadarExecutorSupervisionTargetMode,
) -> u32 {
    flattened_targets(examples, supervision_mode).len() as u32
}

fn flattened_targets(
    examples: &[TassadarExecutorSubroutineCorpusExample],
    supervision_mode: TassadarExecutorSupervisionTargetMode,
) -> Vec<String> {
    examples
        .iter()
        .flat_map(|example| match supervision_mode {
            TassadarExecutorSupervisionTargetMode::FullTrace => {
                example.full_trace_targets.clone()
            }
            TassadarExecutorSupervisionTargetMode::SubroutineLibrary => example
                .subroutine_targets
                .iter()
                .map(|target| target.subroutine_id.clone())
                .collect(),
        })
        .collect()
}

fn basis_points(numerator: u32, denominator: u32) -> u32 {
    if denominator == 0 {
        0
    } else {
        ((numerator as u64 * 10_000) / denominator as u64) as u32
    }
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use psionic_models::TassadarExecutorSubroutineWorkloadFamily;

    use super::{
        TassadarExecutorSubroutineOodProxyConfig, TassadarExecutorSupervisionTargetMode,
        build_tassadar_executor_subroutine_dataset_manifest,
        build_tassadar_executor_subroutine_ood_proxy,
    };

    #[test]
    fn subroutine_dataset_manifest_reuses_smaller_target_vocab_than_full_trace() {
        let full_trace = build_tassadar_executor_subroutine_dataset_manifest(
            TassadarExecutorSupervisionTargetMode::FullTrace,
        )
        .expect("full-trace manifest");
        let subroutine = build_tassadar_executor_subroutine_dataset_manifest(
            TassadarExecutorSupervisionTargetMode::SubroutineLibrary,
        )
        .expect("subroutine manifest");
        assert_eq!(full_trace.examples.len(), 6);
        assert_eq!(subroutine.examples.len(), 6);
        assert!(subroutine.target_vocab_size < full_trace.target_vocab_size);
    }

    #[test]
    fn subroutine_mode_improves_held_out_target_reuse_on_all_seeded_workloads() {
        for held_out_workload_family in [
            TassadarExecutorSubroutineWorkloadFamily::Sort,
            TassadarExecutorSubroutineWorkloadFamily::ClrsShortestPath,
            TassadarExecutorSubroutineWorkloadFamily::SudokuStyle,
        ] {
            let full_trace = build_tassadar_executor_subroutine_ood_proxy(
                &TassadarExecutorSubroutineOodProxyConfig {
                    supervision_mode: TassadarExecutorSupervisionTargetMode::FullTrace,
                    held_out_workload_family,
                },
            )
            .expect("full-trace proxy");
            let subroutine = build_tassadar_executor_subroutine_ood_proxy(
                &TassadarExecutorSubroutineOodProxyConfig {
                    supervision_mode: TassadarExecutorSupervisionTargetMode::SubroutineLibrary,
                    held_out_workload_family,
                },
            )
            .expect("subroutine proxy");
            assert!(subroutine.held_out_reuse_bps >= full_trace.held_out_reuse_bps);
            assert!(subroutine.reusable_held_out_target_count > full_trace.reusable_held_out_target_count);
        }
    }
}
