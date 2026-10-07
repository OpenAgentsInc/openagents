use psionic_core::QuantizationMode;
use serde::{Deserialize, Serialize};

use crate::{
    BackendParityPolicy, EmbeddingParityBudget, EmbeddingParitySummary, LogitParityBudget,
    LogitParitySummary, ParityCheckError, compare_embedding_vectors, compare_logits,
};

/// Stable AttnRes sublayer kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttnResSublayerKind {
    /// Self-attention sublayer.
    Attention,
    /// Feed-forward sublayer.
    FeedForward,
}

/// Immutable AttnRes sublayer routing snapshot for runtime-facing consumers.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AttnResSublayerSnapshot {
    /// Stable sublayer index in `[0, num_layers)`.
    pub sublayer_index: usize,
    /// Transformer-layer index for this sublayer.
    pub transformer_layer_index: usize,
    /// Whether this was the attention or MLP half.
    pub kind: AttnResSublayerKind,
    /// Whether a new block started immediately before the sublayer.
    pub starts_new_block_before: bool,
    /// Completed blocks visible before the boundary update.
    pub completed_blocks_before: usize,
    /// Completed blocks visible after the boundary update.
    pub completed_blocks_after: usize,
    /// Whether a partial block existed before AttnRes routing.
    pub partial_block_present_before: bool,
    /// Whether a partial block existed after the sublayer completed.
    pub partial_block_present_after: bool,
    /// Shape for the flattened routing tensors as `[sources, batch, seq]`.
    pub source_shape: [usize; 3],
    /// Source logits before the depth softmax.
    pub source_logits: Vec<f32>,
    /// Source routing weights after the depth softmax.
    pub routing_weights: Vec<f32>,
    /// L2 norm of the pseudo-query used by the sublayer.
    pub query_norm: f32,
}

/// Immutable AttnRes forward-pass diagnostics snapshot for runtime-facing consumers.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AttnResDiagnosticsSnapshot {
    /// Batch size observed by the forward pass.
    pub batch_size: usize,
    /// Sequence length observed by the forward pass.
    pub sequence_length: usize,
    /// Hidden width observed by the forward pass.
    pub hidden_size: usize,
    /// Final completed-block count after the forward pass.
    pub final_completed_blocks: usize,
    /// Whether a partial block remained after the forward pass.
    pub final_partial_block_present: bool,
    /// Per-sublayer routing and boundary diagnostics.
    pub sublayers: Vec<AttnResSublayerSnapshot>,
}

/// Parity status for standard-vs-two-phase AttnRes comparisons.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttnResTwoPhaseParityStatus {
    /// The compared values matched exactly.
    Exact,
    /// The compared values differed but remained inside the configured budget.
    WithinBudget,
    /// The compared values exceeded the configured budget.
    OutsideBudget,
}

/// Runtime-owned parity budgets for standard-vs-two-phase AttnRes checks.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct AttnResTwoPhaseParityBudget {
    /// Dense hidden-state parity budget.
    pub hidden: EmbeddingParityBudget,
    /// Dense logit parity budget.
    pub logits: LogitParityBudget,
}

impl Default for AttnResTwoPhaseParityBudget {
    fn default() -> Self {
        let policy = BackendParityPolicy::default();
        Self {
            hidden: policy.embedding_budget(QuantizationMode::None),
            logits: policy.logit_budget(QuantizationMode::None),
        }
    }
}

/// Runtime-owned parity report for hidden-state equivalence.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct AttnResHiddenParityReport {
    /// Runtime-owned parity status.
    pub status: AttnResTwoPhaseParityStatus,
    /// Underlying numeric summary.
    pub summary: EmbeddingParitySummary,
}

/// Runtime-owned parity report for logits equivalence.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct AttnResLogitParityReport {
    /// Runtime-owned parity status.
    pub status: AttnResTwoPhaseParityStatus,
    /// Underlying numeric summary.
    pub summary: LogitParitySummary,
}

/// Compares AttnRes hidden states from standard and two-phase execution.
pub fn compare_attnres_hidden_two_phase_parity(
    expected: &[f32],
    actual: &[f32],
    budget: EmbeddingParityBudget,
) -> Result<AttnResHiddenParityReport, ParityCheckError> {
    let summary = compare_embedding_vectors(expected, actual, budget)?;
    Ok(AttnResHiddenParityReport {
        status: parity_status_from_budget(summary.within_budget, expected, actual),
        summary,
    })
}

/// Compares AttnRes logits from standard and two-phase execution.
pub fn compare_attnres_logit_two_phase_parity(
    expected: &[f32],
    actual: &[f32],
    budget: LogitParityBudget,
) -> Result<AttnResLogitParityReport, ParityCheckError> {
    let summary = compare_logits(expected, actual, budget)?;
    Ok(AttnResLogitParityReport {
        status: parity_status_from_budget(summary.within_budget, expected, actual),
        summary,
    })
}

fn parity_status_from_budget(
    within_budget: bool,
    expected: &[f32],
    actual: &[f32],
) -> AttnResTwoPhaseParityStatus {
    if expected == actual {
        AttnResTwoPhaseParityStatus::Exact
    } else if within_budget {
        AttnResTwoPhaseParityStatus::WithinBudget
    } else {
        AttnResTwoPhaseParityStatus::OutsideBudget
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_budget_reuses_dense_backend_policy() {
        let budget = AttnResTwoPhaseParityBudget::default();
        let policy = BackendParityPolicy::default();
        assert_eq!(
            budget.hidden,
            policy.embedding_budget(QuantizationMode::None)
        );
        assert_eq!(budget.logits, policy.logit_budget(QuantizationMode::None));
    }

    #[test]
    fn hidden_parity_reports_exact_on_identical_vectors() -> Result<(), Box<dyn std::error::Error>>
    {
        let budget = AttnResTwoPhaseParityBudget::default();
        let report =
            compare_attnres_hidden_two_phase_parity(&[1.0, 2.0], &[1.0, 2.0], budget.hidden)?;
        assert_eq!(report.status, AttnResTwoPhaseParityStatus::Exact);
        assert!(report.summary.within_budget);
        Ok(())
    }

    #[test]
    fn hidden_parity_reports_within_budget_on_small_drift() -> Result<(), Box<dyn std::error::Error>>
    {
        let budget = AttnResTwoPhaseParityBudget::default();
        let report = compare_attnres_hidden_two_phase_parity(
            &[1.0, 0.0],
            &[0.999_999, 1.0e-6],
            budget.hidden,
        )?;
        assert_eq!(report.status, AttnResTwoPhaseParityStatus::WithinBudget);
        assert!(report.summary.within_budget);
        Ok(())
    }

    #[test]
    fn logit_parity_reports_outside_budget_when_rank_drift_exceeds_budget()
    -> Result<(), Box<dyn std::error::Error>> {
        let budget = AttnResTwoPhaseParityBudget::default();
        let report = compare_attnres_logit_two_phase_parity(
            &[0.51, 0.5, 0.1],
            &[0.5, 0.51, 0.1],
            budget.logits,
        )?;
        assert_eq!(report.status, AttnResTwoPhaseParityStatus::OutsideBudget);
        assert!(!report.summary.within_budget);
        Ok(())
    }
}
