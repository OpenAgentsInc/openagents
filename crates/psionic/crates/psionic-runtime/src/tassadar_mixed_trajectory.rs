use psionic_ir::{
    TassadarMixedTrajectory, TassadarMixedTrajectoryEntryKind, TassadarMixedTrajectoryError,
    TassadarMixedTrajectoryLaneKind,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Replay receipt for one mixed language and compute trajectory.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarMixedTrajectoryReplayReceipt {
    /// Stable trajectory identifier.
    pub trajectory_id: String,
    /// Stable case identifier.
    pub case_id: String,
    /// Ordered lane sequence with adjacent duplicates removed.
    pub lane_sequence: Vec<TassadarMixedTrajectoryLaneKind>,
    /// Number of explicit handoffs in the trajectory.
    pub handoff_count: u32,
    /// Number of receipt boundaries in the trajectory.
    pub receipt_boundary_count: u32,
    /// Whether JSON roundtrip kept the trajectory exact.
    pub schema_roundtrip_ok: bool,
    /// Whether lane handoffs stayed correct.
    pub lane_handoff_correct: bool,
    /// Whether the final outputs matched the last output-carrying span.
    pub trajectory_to_outcome_parity: bool,
    /// Final outputs replayed from the trajectory.
    pub final_outputs: Vec<i32>,
    /// Plain-language replay note.
    pub note: String,
    /// Stable digest over the receipt.
    pub receipt_digest: String,
}

impl TassadarMixedTrajectoryReplayReceipt {
    fn new(
        trajectory: &TassadarMixedTrajectory,
        schema_roundtrip_ok: bool,
        lane_handoff_correct: bool,
        trajectory_to_outcome_parity: bool,
    ) -> Self {
        let lane_sequence = lane_sequence(trajectory);
        let handoff_count = trajectory
            .entries
            .iter()
            .filter(|entry| entry.handoff_from_previous.is_some())
            .count() as u32;
        let receipt_boundary_count = trajectory
            .entries
            .iter()
            .filter(|entry| entry.entry_kind == TassadarMixedTrajectoryEntryKind::ReceiptBoundary)
            .count() as u32;
        let mut receipt = Self {
            trajectory_id: trajectory.trajectory_id.clone(),
            case_id: trajectory.case_id.clone(),
            lane_sequence,
            handoff_count,
            receipt_boundary_count,
            schema_roundtrip_ok,
            lane_handoff_correct,
            trajectory_to_outcome_parity,
            final_outputs: trajectory.final_outputs.clone(),
            note: String::from(
                "mixed trajectory replay keeps schema roundtrip, lane handoff, receipt boundaries, and final-output parity explicit without implying accepted-outcome closure",
            ),
            receipt_digest: String::new(),
        };
        receipt.receipt_digest = stable_digest(
            b"psionic_tassadar_mixed_trajectory_replay_receipt|",
            &receipt,
        );
        receipt
    }
}

/// Replay failure for one mixed trajectory.
#[derive(Debug, Error)]
pub enum TassadarMixedTrajectoryReplayError {
    #[error(transparent)]
    Validation(#[from] TassadarMixedTrajectoryError),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

/// Replays one mixed trajectory into a bounded receipt.
pub fn replay_tassadar_mixed_trajectory(
    trajectory: &TassadarMixedTrajectory,
) -> Result<TassadarMixedTrajectoryReplayReceipt, TassadarMixedTrajectoryReplayError> {
    trajectory.validate()?;
    let encoded = serde_json::to_vec(trajectory)?;
    let decoded: TassadarMixedTrajectory = serde_json::from_slice(&encoded)?;
    let schema_roundtrip_ok = decoded == *trajectory;
    let last_outputs = trajectory
        .entries
        .iter()
        .rev()
        .find(|entry| !entry.span_outputs.is_empty())
        .map(|entry| entry.span_outputs.as_slice());
    let trajectory_to_outcome_parity = last_outputs == Some(trajectory.final_outputs.as_slice());
    Ok(TassadarMixedTrajectoryReplayReceipt::new(
        trajectory,
        schema_roundtrip_ok,
        true,
        trajectory_to_outcome_parity,
    ))
}

fn lane_sequence(trajectory: &TassadarMixedTrajectory) -> Vec<TassadarMixedTrajectoryLaneKind> {
    let mut sequence = Vec::new();
    for entry in &trajectory.entries {
        if sequence.last().copied() != Some(entry.lane_kind) {
            sequence.push(entry.lane_kind);
        }
    }
    sequence
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::replay_tassadar_mixed_trajectory;
    use psionic_ir::{
        TassadarMixedTrajectory, TassadarMixedTrajectoryEntry, TassadarMixedTrajectoryEntryKind,
        TassadarMixedTrajectoryHandoffKind, TassadarMixedTrajectoryLaneKind,
        TassadarMixedTrajectoryReceiptBoundary, TassadarMixedTrajectoryReceiptBoundaryKind,
    };

    #[test]
    fn mixed_trajectory_replay_keeps_lane_sequence_and_output_parity() {
        let trajectory = TassadarMixedTrajectory::new(
            "trajectory-runtime-a",
            "case-runtime-a",
            vec![
                TassadarMixedTrajectoryEntry {
                    entry_id: String::from("entry-0"),
                    entry_index: 0,
                    entry_kind: TassadarMixedTrajectoryEntryKind::LanguageSpan,
                    lane_kind: TassadarMixedTrajectoryLaneKind::LanguageReasoning,
                    handoff_from_previous: None,
                    content_summary: String::from("language"),
                    language_text: Some(String::from("plan")),
                    token_count: Some(8),
                    step_count: None,
                    verifier_event_kind: None,
                    tool_name: None,
                    receipt_boundary: None,
                    span_outputs: Vec::new(),
                },
                TassadarMixedTrajectoryEntry {
                    entry_id: String::from("entry-1"),
                    entry_index: 1,
                    entry_kind: TassadarMixedTrajectoryEntryKind::ExactComputeSpan,
                    lane_kind: TassadarMixedTrajectoryLaneKind::InternalExactCompute,
                    handoff_from_previous: Some(
                        TassadarMixedTrajectoryHandoffKind::LanguageToInternalCompute,
                    ),
                    content_summary: String::from("compute"),
                    language_text: None,
                    token_count: None,
                    step_count: Some(9),
                    verifier_event_kind: None,
                    tool_name: None,
                    receipt_boundary: None,
                    span_outputs: vec![9],
                },
                TassadarMixedTrajectoryEntry {
                    entry_id: String::from("entry-2"),
                    entry_index: 2,
                    entry_kind: TassadarMixedTrajectoryEntryKind::ReceiptBoundary,
                    lane_kind: TassadarMixedTrajectoryLaneKind::InternalExactCompute,
                    handoff_from_previous: None,
                    content_summary: String::from("receipt"),
                    language_text: None,
                    token_count: None,
                    step_count: None,
                    verifier_event_kind: None,
                    tool_name: None,
                    receipt_boundary: Some(TassadarMixedTrajectoryReceiptBoundary {
                        boundary_kind:
                            TassadarMixedTrajectoryReceiptBoundaryKind::ExecutorEvidenceBundle,
                        receipt_ref: String::from("receipt.json"),
                        note: String::from("receipt"),
                    }),
                    span_outputs: Vec::new(),
                },
            ],
            vec![9],
            "boundary",
        )
        .expect("trajectory");

        let receipt = replay_tassadar_mixed_trajectory(&trajectory).expect("receipt");
        assert_eq!(receipt.lane_sequence.len(), 2);
        assert_eq!(receipt.handoff_count, 1);
        assert_eq!(receipt.receipt_boundary_count, 1);
        assert!(receipt.schema_roundtrip_ok);
        assert!(receipt.trajectory_to_outcome_parity);
    }
}
