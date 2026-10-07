use std::collections::BTreeSet;

use psionic_data::TassadarSequenceSplit;
use psionic_models::TassadarExecutorSubroutineWorkloadFamily;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{tassadar_executor_subroutine_corpus, TassadarExecutorSubroutineCorpusExample};

pub const TASSADAR_EXECUTOR_NO_HINT_DATASET_SCHEMA_VERSION: u16 = 1;

/// Explicit hint/no-hint regime for one bounded learned-executor proxy corpus.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarExecutorHintRegime {
    FullHintTrace,
    SubroutineHints,
    NoHintOutputOnly,
    NoHintSelfSupervised,
}

impl TassadarExecutorHintRegime {
    /// Returns the stable regime label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::FullHintTrace => "full_hint_trace",
            Self::SubroutineHints => "subroutine_hints",
            Self::NoHintOutputOnly => "no_hint_output_only",
            Self::NoHintSelfSupervised => "no_hint_self_supervised",
        }
    }
}

/// Self-supervised regularizer family used by the no-hint proxy lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarExecutorSelfSupervisedRegularizerKind {
    TransitionConsistency,
    ProgressMonotonicity,
    AnswerPrefixAlignment,
}

impl TassadarExecutorSelfSupervisedRegularizerKind {
    /// Returns the stable kind label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::TransitionConsistency => "transition_consistency",
            Self::ProgressMonotonicity => "progress_monotonicity",
            Self::AnswerPrefixAlignment => "answer_prefix_alignment",
        }
    }
}

/// One self-supervised regularizer attached to a no-hint corpus example.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorSelfSupervisedRegularizer {
    pub regularizer_id: String,
    pub kind: TassadarExecutorSelfSupervisedRegularizerKind,
    pub summary: String,
    pub shared_across_workloads: bool,
}

/// One bounded executor example enriched with no-hint outputs and regularizers.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorNoHintCorpusExample {
    pub example_id: String,
    pub workload_family: TassadarExecutorSubroutineWorkloadFamily,
    pub split: TassadarSequenceSplit,
    pub summary: String,
    pub full_hint_targets: Vec<String>,
    pub subroutine_hint_targets: Vec<String>,
    pub final_output_targets: Vec<String>,
    pub self_supervised_regularizers: Vec<TassadarExecutorSelfSupervisedRegularizer>,
}

/// Dataset manifest for one public hint/no-hint regime.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorNoHintDatasetManifest {
    pub schema_version: u16,
    pub dataset_id: String,
    pub supervision_regime: TassadarExecutorHintRegime,
    pub workload_families: Vec<TassadarExecutorSubroutineWorkloadFamily>,
    pub train_example_count: u32,
    pub validation_example_count: u32,
    pub test_example_count: u32,
    pub explicit_hint_target_count: u32,
    pub explicit_hint_vocab_size: u32,
    pub output_target_count: u32,
    pub output_target_vocab_size: u32,
    pub self_supervised_regularizer_count: u32,
    pub self_supervised_regularizer_vocab_size: u32,
    pub active_signal_count: u32,
    pub active_signal_vocab_size: u32,
    pub examples: Vec<TassadarExecutorNoHintCorpusExample>,
    pub claim_boundary: String,
    pub manifest_digest: String,
}

impl TassadarExecutorNoHintDatasetManifest {
    fn new(
        supervision_regime: TassadarExecutorHintRegime,
        examples: Vec<TassadarExecutorNoHintCorpusExample>,
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
        let explicit_hint_vocab = explicit_hint_vocabulary(examples.as_slice(), supervision_regime);
        let output_vocab = output_vocabulary(examples.as_slice());
        let regularizer_vocab = regularizer_vocabulary(examples.as_slice(), supervision_regime);
        let active_signal_vocab = active_signal_vocabulary(examples.as_slice(), supervision_regime);
        let mut manifest = Self {
            schema_version: TASSADAR_EXECUTOR_NO_HINT_DATASET_SCHEMA_VERSION,
            dataset_id: format!(
                "tassadar.executor.no_hint_regime.{}.v0",
                supervision_regime.label()
            ),
            supervision_regime,
            workload_families,
            train_example_count,
            validation_example_count,
            test_example_count,
            explicit_hint_target_count: explicit_hint_count(examples.as_slice(), supervision_regime),
            explicit_hint_vocab_size: explicit_hint_vocab.len() as u32,
            output_target_count: output_count(examples.as_slice()),
            output_target_vocab_size: output_vocab.len() as u32,
            self_supervised_regularizer_count: regularizer_count(
                examples.as_slice(),
                supervision_regime,
            ),
            self_supervised_regularizer_vocab_size: regularizer_vocab.len() as u32,
            active_signal_count: active_signal_count(examples.as_slice(), supervision_regime),
            active_signal_vocab_size: active_signal_vocab.len() as u32,
            examples,
            claim_boundary: String::from(
                "research-only architecture proxy for bounded learned-executor supervision; compares full-hint, lighter-hint, and no-hint plus regularizer regimes on the seeded sort, CLRS-shortest-path, and sudoku-style corpus only and does not imply trained exactness or served-lane capability",
            ),
            manifest_digest: String::new(),
        };
        manifest.manifest_digest =
            stable_digest(b"tassadar_executor_no_hint_dataset_manifest|", &manifest);
        manifest
    }
}

/// Config for one held-out-workload reusable-signal proxy.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorNoHintSignalProxyConfig {
    pub supervision_regime: TassadarExecutorHintRegime,
    pub held_out_workload_family: TassadarExecutorSubroutineWorkloadFamily,
}

/// Deterministic reusable-signal proxy for one held-out workload under one regime.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorNoHintSignalProxyReport {
    pub supervision_regime: TassadarExecutorHintRegime,
    pub held_out_workload_family: TassadarExecutorSubroutineWorkloadFamily,
    pub train_workload_families: Vec<TassadarExecutorSubroutineWorkloadFamily>,
    pub explicit_hint_target_count: u32,
    pub reusable_explicit_hint_target_count: u32,
    pub output_target_count: u32,
    pub reusable_output_target_count: u32,
    pub self_supervised_regularizer_count: u32,
    pub reusable_self_supervised_regularizer_count: u32,
    pub reusable_signal_units: u32,
    pub total_signal_units: u32,
    pub reusable_signal_bps: u32,
    pub detail: String,
}

/// Returns the seeded bounded corpus used by the no-hint/self-supervised lane.
#[must_use]
pub fn tassadar_executor_no_hint_corpus() -> Vec<TassadarExecutorNoHintCorpusExample> {
    tassadar_executor_subroutine_corpus()
        .into_iter()
        .map(enrich_example)
        .collect()
}

/// Materializes one bounded hint/no-hint regime over the seeded executor corpus.
#[must_use]
pub fn build_tassadar_executor_no_hint_dataset_manifest(
    supervision_regime: TassadarExecutorHintRegime,
) -> TassadarExecutorNoHintDatasetManifest {
    TassadarExecutorNoHintDatasetManifest::new(
        supervision_regime,
        tassadar_executor_no_hint_corpus(),
    )
}

/// Builds one deterministic held-out-workload reusable-signal proxy.
#[must_use]
pub fn build_tassadar_executor_no_hint_signal_proxy(
    config: &TassadarExecutorNoHintSignalProxyConfig,
) -> TassadarExecutorNoHintSignalProxyReport {
    let manifest = build_tassadar_executor_no_hint_dataset_manifest(config.supervision_regime);
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
    let train_explicit_hint_vocab =
        explicit_hint_vocabulary(training_examples.as_slice(), config.supervision_regime);
    let train_output_vocab = output_vocabulary(training_examples.as_slice());
    let train_regularizer_vocab =
        regularizer_vocabulary(training_examples.as_slice(), config.supervision_regime);
    let held_out_explicit_hints =
        flattened_explicit_hints(held_out_examples.as_slice(), config.supervision_regime);
    let held_out_outputs = flattened_outputs(held_out_examples.as_slice());
    let held_out_regularizers =
        flattened_regularizers(held_out_examples.as_slice(), config.supervision_regime);
    let reusable_explicit_hint_target_count = held_out_explicit_hints
        .iter()
        .filter(|target| train_explicit_hint_vocab.contains(*target))
        .count() as u32;
    let reusable_output_target_count = held_out_outputs
        .iter()
        .filter(|target| train_output_vocab.contains(*target))
        .count() as u32;
    let reusable_self_supervised_regularizer_count = held_out_regularizers
        .iter()
        .filter(|target| train_regularizer_vocab.contains(*target))
        .count() as u32;
    let explicit_hint_target_count = held_out_explicit_hints.len() as u32;
    let output_target_count = held_out_outputs.len() as u32;
    let self_supervised_regularizer_count = held_out_regularizers.len() as u32;
    let reusable_signal_units = reusable_explicit_hint_target_count
        .saturating_add(reusable_output_target_count)
        .saturating_add(reusable_self_supervised_regularizer_count);
    let total_signal_units = explicit_hint_target_count
        .saturating_add(output_target_count)
        .saturating_add(self_supervised_regularizer_count);
    TassadarExecutorNoHintSignalProxyReport {
        supervision_regime: config.supervision_regime,
        held_out_workload_family: config.held_out_workload_family,
        train_workload_families,
        explicit_hint_target_count,
        reusable_explicit_hint_target_count,
        output_target_count,
        reusable_output_target_count,
        self_supervised_regularizer_count,
        reusable_self_supervised_regularizer_count,
        reusable_signal_units,
        total_signal_units,
        reusable_signal_bps: basis_points(reusable_signal_units, total_signal_units),
        detail: format!(
            "held_out_workload={}, supervision_regime={}, reusable_signal_units={}/{}, reusable_outputs={}/{}, reusable_regularizers={}/{}, reusable_explicit_hints={}/{}",
            config.held_out_workload_family.label(),
            config.supervision_regime.label(),
            reusable_signal_units,
            total_signal_units,
            reusable_output_target_count,
            output_target_count,
            reusable_self_supervised_regularizer_count,
            self_supervised_regularizer_count,
            reusable_explicit_hint_target_count,
            explicit_hint_target_count,
        ),
    }
}

fn enrich_example(
    base: TassadarExecutorSubroutineCorpusExample,
) -> TassadarExecutorNoHintCorpusExample {
    TassadarExecutorNoHintCorpusExample {
        example_id: base.example_id,
        workload_family: base.workload_family,
        split: base.split,
        summary: base.summary,
        full_hint_targets: base.full_trace_targets,
        subroutine_hint_targets: base
            .subroutine_targets
            .into_iter()
            .map(|target| target.subroutine_id)
            .collect(),
        final_output_targets: final_output_targets(base.workload_family),
        self_supervised_regularizers: self_supervised_regularizers(),
    }
}

fn final_output_targets(workload_family: TassadarExecutorSubroutineWorkloadFamily) -> Vec<String> {
    let workload_specific = match workload_family {
        TassadarExecutorSubroutineWorkloadFamily::Sort => "output.sorted_sequence",
        TassadarExecutorSubroutineWorkloadFamily::ClrsShortestPath => "output.shortest_path_answer",
        TassadarExecutorSubroutineWorkloadFamily::SudokuStyle => "output.solved_grid",
    };
    vec![
        String::from("output.primary_answer"),
        String::from(workload_specific),
    ]
}

fn self_supervised_regularizers() -> Vec<TassadarExecutorSelfSupervisedRegularizer> {
    vec![
        regularizer(
            "tassadar.regularizer.transition_consistency.v1",
            TassadarExecutorSelfSupervisedRegularizerKind::TransitionConsistency,
            "keep adjacent hidden-state transitions locally consistent under bounded input-output rollouts",
        ),
        regularizer(
            "tassadar.regularizer.progress_monotonicity.v1",
            TassadarExecutorSelfSupervisedRegularizerKind::ProgressMonotonicity,
            "encourage monotonic bounded progress signals even without explicit intermediate hints",
        ),
        regularizer(
            "tassadar.regularizer.answer_prefix_alignment.v1",
            TassadarExecutorSelfSupervisedRegularizerKind::AnswerPrefixAlignment,
            "align latent state prefixes with the eventual bounded output answer space",
        ),
    ]
}

fn regularizer(
    regularizer_id: &str,
    kind: TassadarExecutorSelfSupervisedRegularizerKind,
    summary: &str,
) -> TassadarExecutorSelfSupervisedRegularizer {
    TassadarExecutorSelfSupervisedRegularizer {
        regularizer_id: String::from(regularizer_id),
        kind,
        summary: String::from(summary),
        shared_across_workloads: true,
    }
}

fn explicit_hint_vocabulary(
    examples: &[TassadarExecutorNoHintCorpusExample],
    supervision_regime: TassadarExecutorHintRegime,
) -> BTreeSet<String> {
    flattened_explicit_hints(examples, supervision_regime)
        .into_iter()
        .collect()
}

fn output_vocabulary(examples: &[TassadarExecutorNoHintCorpusExample]) -> BTreeSet<String> {
    flattened_outputs(examples).into_iter().collect()
}

fn regularizer_vocabulary(
    examples: &[TassadarExecutorNoHintCorpusExample],
    supervision_regime: TassadarExecutorHintRegime,
) -> BTreeSet<String> {
    flattened_regularizers(examples, supervision_regime)
        .into_iter()
        .collect()
}

fn active_signal_vocabulary(
    examples: &[TassadarExecutorNoHintCorpusExample],
    supervision_regime: TassadarExecutorHintRegime,
) -> BTreeSet<String> {
    flattened_active_signals(examples, supervision_regime)
        .into_iter()
        .collect()
}

fn explicit_hint_count(
    examples: &[TassadarExecutorNoHintCorpusExample],
    supervision_regime: TassadarExecutorHintRegime,
) -> u32 {
    flattened_explicit_hints(examples, supervision_regime).len() as u32
}

fn output_count(examples: &[TassadarExecutorNoHintCorpusExample]) -> u32 {
    flattened_outputs(examples).len() as u32
}

fn regularizer_count(
    examples: &[TassadarExecutorNoHintCorpusExample],
    supervision_regime: TassadarExecutorHintRegime,
) -> u32 {
    flattened_regularizers(examples, supervision_regime).len() as u32
}

fn active_signal_count(
    examples: &[TassadarExecutorNoHintCorpusExample],
    supervision_regime: TassadarExecutorHintRegime,
) -> u32 {
    flattened_active_signals(examples, supervision_regime).len() as u32
}

fn flattened_explicit_hints(
    examples: &[TassadarExecutorNoHintCorpusExample],
    supervision_regime: TassadarExecutorHintRegime,
) -> Vec<String> {
    examples
        .iter()
        .flat_map(|example| match supervision_regime {
            TassadarExecutorHintRegime::FullHintTrace => example.full_hint_targets.clone(),
            TassadarExecutorHintRegime::SubroutineHints => example.subroutine_hint_targets.clone(),
            TassadarExecutorHintRegime::NoHintOutputOnly
            | TassadarExecutorHintRegime::NoHintSelfSupervised => Vec::new(),
        })
        .collect()
}

fn flattened_outputs(examples: &[TassadarExecutorNoHintCorpusExample]) -> Vec<String> {
    examples
        .iter()
        .flat_map(|example| example.final_output_targets.clone())
        .collect()
}

fn flattened_regularizers(
    examples: &[TassadarExecutorNoHintCorpusExample],
    supervision_regime: TassadarExecutorHintRegime,
) -> Vec<String> {
    examples
        .iter()
        .flat_map(|example| match supervision_regime {
            TassadarExecutorHintRegime::NoHintSelfSupervised => example
                .self_supervised_regularizers
                .iter()
                .map(|regularizer| regularizer.regularizer_id.clone())
                .collect(),
            TassadarExecutorHintRegime::FullHintTrace
            | TassadarExecutorHintRegime::SubroutineHints
            | TassadarExecutorHintRegime::NoHintOutputOnly => Vec::new(),
        })
        .collect()
}

fn flattened_active_signals(
    examples: &[TassadarExecutorNoHintCorpusExample],
    supervision_regime: TassadarExecutorHintRegime,
) -> Vec<String> {
    let mut signals = flattened_outputs(examples);
    signals.extend(flattened_explicit_hints(examples, supervision_regime));
    signals.extend(flattened_regularizers(examples, supervision_regime));
    signals
}

fn basis_points(numerator: u32, denominator: u32) -> u32 {
    if denominator == 0 {
        0
    } else {
        ((u64::from(numerator) * 10_000) / u64::from(denominator)) as u32
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
        build_tassadar_executor_no_hint_dataset_manifest,
        build_tassadar_executor_no_hint_signal_proxy, TassadarExecutorHintRegime,
        TassadarExecutorNoHintSignalProxyConfig,
    };

    #[test]
    fn no_hint_dataset_manifest_materializes_hintless_and_regularized_variants() {
        let output_only = build_tassadar_executor_no_hint_dataset_manifest(
            TassadarExecutorHintRegime::NoHintOutputOnly,
        );
        let self_supervised = build_tassadar_executor_no_hint_dataset_manifest(
            TassadarExecutorHintRegime::NoHintSelfSupervised,
        );
        assert_eq!(output_only.examples.len(), 6);
        assert_eq!(self_supervised.examples.len(), 6);
        assert_eq!(output_only.explicit_hint_target_count, 0);
        assert_eq!(self_supervised.explicit_hint_target_count, 0);
        assert_eq!(output_only.self_supervised_regularizer_count, 0);
        assert!(self_supervised.self_supervised_regularizer_count > 0);
        assert!(self_supervised.active_signal_count > output_only.active_signal_count);
    }

    #[test]
    fn self_supervised_no_hint_regime_improves_clrs_signal_reuse_vs_lighter_regimes() {
        let held_out_workload_family = TassadarExecutorSubroutineWorkloadFamily::ClrsShortestPath;
        let full_hint = build_tassadar_executor_no_hint_signal_proxy(
            &TassadarExecutorNoHintSignalProxyConfig {
                supervision_regime: TassadarExecutorHintRegime::FullHintTrace,
                held_out_workload_family,
            },
        );
        let subroutine_hints = build_tassadar_executor_no_hint_signal_proxy(
            &TassadarExecutorNoHintSignalProxyConfig {
                supervision_regime: TassadarExecutorHintRegime::SubroutineHints,
                held_out_workload_family,
            },
        );
        let output_only = build_tassadar_executor_no_hint_signal_proxy(
            &TassadarExecutorNoHintSignalProxyConfig {
                supervision_regime: TassadarExecutorHintRegime::NoHintOutputOnly,
                held_out_workload_family,
            },
        );
        let self_supervised = build_tassadar_executor_no_hint_signal_proxy(
            &TassadarExecutorNoHintSignalProxyConfig {
                supervision_regime: TassadarExecutorHintRegime::NoHintSelfSupervised,
                held_out_workload_family,
            },
        );

        assert_eq!(output_only.explicit_hint_target_count, 0);
        assert_eq!(self_supervised.explicit_hint_target_count, 0);
        assert!(self_supervised.reusable_signal_bps > output_only.reusable_signal_bps);
        assert!(self_supervised.reusable_signal_bps > full_hint.reusable_signal_bps);
        assert!(self_supervised.reusable_signal_bps <= subroutine_hints.reusable_signal_bps);
        assert_eq!(self_supervised.reusable_signal_bps, 8000);
    }
}
