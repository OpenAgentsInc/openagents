use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Stable schema version for mixed language and exact-compute trajectories.
pub const TASSADAR_MIXED_TRAJECTORY_SCHEMA_VERSION: u16 = 1;
/// Coarse claim class for mixed trajectory artifacts.
pub const TASSADAR_MIXED_TRAJECTORY_CLAIM_CLASS: &str = "execution_truth_learned_substrate";

/// Stable lane kinds admitted by the mixed trajectory schema.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarMixedTrajectoryLaneKind {
    LanguageReasoning,
    InternalExactCompute,
    VerifierSearch,
    ExternalTool,
}

impl TassadarMixedTrajectoryLaneKind {
    /// Returns the stable lane label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::LanguageReasoning => "language_reasoning",
            Self::InternalExactCompute => "internal_exact_compute",
            Self::VerifierSearch => "verifier_search",
            Self::ExternalTool => "external_tool",
        }
    }
}

/// Entry kinds admitted by the mixed trajectory schema.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarMixedTrajectoryEntryKind {
    LanguageSpan,
    ExactComputeSpan,
    VerifierEventSpan,
    ExternalToolSpan,
    ReceiptBoundary,
}

/// Explicit lane handoff kinds carried by mixed trajectories.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarMixedTrajectoryHandoffKind {
    LanguageToInternalCompute,
    InternalComputeToVerifier,
    VerifierToLanguage,
    LanguageToExternalTool,
    ExternalToolToLanguage,
    InternalComputeToLanguage,
}

/// Receipt boundary kinds carried by mixed trajectories.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarMixedTrajectoryReceiptBoundaryKind {
    PlannerPolicyReceipt,
    ExecutorEvidenceBundle,
    VerifierCertificate,
    SandboxExecutionReceipt,
    FinalOutcomeReceipt,
}

/// Verifier-event kinds surfaced in mixed trajectories.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarMixedTrajectoryVerifierEventKind {
    Guess,
    Verify,
    Contradiction,
    Backtrack,
    Commit,
}

/// One explicit receipt boundary inside the mixed trajectory stream.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarMixedTrajectoryReceiptBoundary {
    /// Receipt family surfaced by the boundary.
    pub boundary_kind: TassadarMixedTrajectoryReceiptBoundaryKind,
    /// Stable receipt or artifact reference.
    pub receipt_ref: String,
    /// Plain-language note for the boundary.
    pub note: String,
}

/// One entry in the mixed language and compute trajectory.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarMixedTrajectoryEntry {
    /// Stable entry identifier.
    pub entry_id: String,
    /// Zero-based entry index.
    pub entry_index: u32,
    /// Entry kind.
    pub entry_kind: TassadarMixedTrajectoryEntryKind,
    /// Lane that owns the entry.
    pub lane_kind: TassadarMixedTrajectoryLaneKind,
    /// Explicit handoff from the previous entry when the lane changes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub handoff_from_previous: Option<TassadarMixedTrajectoryHandoffKind>,
    /// Short machine-legible summary of the entry content.
    pub content_summary: String,
    /// Language text carried by language spans.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language_text: Option<String>,
    /// Token count for language spans.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token_count: Option<u32>,
    /// Step count for compute, verifier, or external-tool spans.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step_count: Option<u32>,
    /// Verifier-event kind for verifier spans.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verifier_event_kind: Option<TassadarMixedTrajectoryVerifierEventKind>,
    /// Tool name for external-tool spans.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
    /// Explicit receipt boundary when the entry is a receipt marker.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub receipt_boundary: Option<TassadarMixedTrajectoryReceiptBoundary>,
    /// Ordered outputs surfaced by the entry when one span resolves a value.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub span_outputs: Vec<i32>,
}

impl TassadarMixedTrajectoryEntry {
    /// Validates one entry in isolation.
    pub fn validate(&self) -> Result<(), TassadarMixedTrajectoryError> {
        if self.entry_id.trim().is_empty() {
            return Err(TassadarMixedTrajectoryError::MissingEntryId {
                entry_index: self.entry_index,
            });
        }
        if self.content_summary.trim().is_empty() {
            return Err(TassadarMixedTrajectoryError::MissingContentSummary {
                entry_index: self.entry_index,
            });
        }
        match self.entry_kind {
            TassadarMixedTrajectoryEntryKind::LanguageSpan => {
                if self.lane_kind != TassadarMixedTrajectoryLaneKind::LanguageReasoning {
                    return Err(TassadarMixedTrajectoryError::EntryLaneMismatch {
                        entry_index: self.entry_index,
                        entry_kind: self.entry_kind,
                        lane_kind: self.lane_kind,
                    });
                }
                if self.language_text.as_deref().is_none() || self.token_count.is_none() {
                    return Err(TassadarMixedTrajectoryError::MalformedLanguageSpan {
                        entry_index: self.entry_index,
                    });
                }
                if self.step_count.is_some()
                    || self.verifier_event_kind.is_some()
                    || self.tool_name.is_some()
                    || self.receipt_boundary.is_some()
                {
                    return Err(TassadarMixedTrajectoryError::MalformedLanguageSpan {
                        entry_index: self.entry_index,
                    });
                }
            }
            TassadarMixedTrajectoryEntryKind::ExactComputeSpan => {
                if self.lane_kind != TassadarMixedTrajectoryLaneKind::InternalExactCompute {
                    return Err(TassadarMixedTrajectoryError::EntryLaneMismatch {
                        entry_index: self.entry_index,
                        entry_kind: self.entry_kind,
                        lane_kind: self.lane_kind,
                    });
                }
                if self.step_count.unwrap_or(0) == 0 || self.receipt_boundary.is_some() {
                    return Err(TassadarMixedTrajectoryError::MalformedComputeSpan {
                        entry_index: self.entry_index,
                    });
                }
                if self.language_text.is_some()
                    || self.token_count.is_some()
                    || self.verifier_event_kind.is_some()
                    || self.tool_name.is_some()
                {
                    return Err(TassadarMixedTrajectoryError::MalformedComputeSpan {
                        entry_index: self.entry_index,
                    });
                }
            }
            TassadarMixedTrajectoryEntryKind::VerifierEventSpan => {
                if self.lane_kind != TassadarMixedTrajectoryLaneKind::VerifierSearch {
                    return Err(TassadarMixedTrajectoryError::EntryLaneMismatch {
                        entry_index: self.entry_index,
                        entry_kind: self.entry_kind,
                        lane_kind: self.lane_kind,
                    });
                }
                if self.step_count.unwrap_or(0) == 0
                    || self.verifier_event_kind.is_none()
                    || self.receipt_boundary.is_some()
                {
                    return Err(TassadarMixedTrajectoryError::MalformedVerifierSpan {
                        entry_index: self.entry_index,
                    });
                }
                if self.language_text.is_some()
                    || self.token_count.is_some()
                    || self.tool_name.is_some()
                {
                    return Err(TassadarMixedTrajectoryError::MalformedVerifierSpan {
                        entry_index: self.entry_index,
                    });
                }
            }
            TassadarMixedTrajectoryEntryKind::ExternalToolSpan => {
                if self.lane_kind != TassadarMixedTrajectoryLaneKind::ExternalTool {
                    return Err(TassadarMixedTrajectoryError::EntryLaneMismatch {
                        entry_index: self.entry_index,
                        entry_kind: self.entry_kind,
                        lane_kind: self.lane_kind,
                    });
                }
                if self.step_count.unwrap_or(0) == 0
                    || self.tool_name.as_deref().is_none()
                    || self.receipt_boundary.is_some()
                {
                    return Err(TassadarMixedTrajectoryError::MalformedExternalToolSpan {
                        entry_index: self.entry_index,
                    });
                }
                if self.language_text.is_some()
                    || self.token_count.is_some()
                    || self.verifier_event_kind.is_some()
                {
                    return Err(TassadarMixedTrajectoryError::MalformedExternalToolSpan {
                        entry_index: self.entry_index,
                    });
                }
            }
            TassadarMixedTrajectoryEntryKind::ReceiptBoundary => {
                if self.receipt_boundary.is_none() {
                    return Err(TassadarMixedTrajectoryError::MissingReceiptBoundary {
                        entry_index: self.entry_index,
                    });
                }
                if self.language_text.is_some()
                    || self.token_count.is_some()
                    || self.step_count.is_some()
                    || self.verifier_event_kind.is_some()
                    || self.tool_name.is_some()
                    || !self.span_outputs.is_empty()
                {
                    return Err(TassadarMixedTrajectoryError::MalformedReceiptBoundary {
                        entry_index: self.entry_index,
                    });
                }
            }
        }
        Ok(())
    }
}

/// One typed mixed language and compute trajectory.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarMixedTrajectory {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable trajectory identifier.
    pub trajectory_id: String,
    /// Stable case identifier.
    pub case_id: String,
    /// Ordered mixed entries.
    pub entries: Vec<TassadarMixedTrajectoryEntry>,
    /// Final declared outputs for the trajectory.
    pub final_outputs: Vec<i32>,
    /// Coarse claim class.
    pub claim_class: String,
    /// Plain-language claim boundary.
    pub claim_boundary: String,
    /// Stable digest over the trajectory.
    pub trajectory_digest: String,
}

impl TassadarMixedTrajectory {
    /// Creates one validated mixed trajectory with a stable digest.
    pub fn new(
        trajectory_id: impl Into<String>,
        case_id: impl Into<String>,
        entries: Vec<TassadarMixedTrajectoryEntry>,
        final_outputs: Vec<i32>,
        claim_boundary: impl Into<String>,
    ) -> Result<Self, TassadarMixedTrajectoryError> {
        let mut trajectory = Self {
            schema_version: TASSADAR_MIXED_TRAJECTORY_SCHEMA_VERSION,
            trajectory_id: trajectory_id.into(),
            case_id: case_id.into(),
            entries,
            final_outputs,
            claim_class: String::from(TASSADAR_MIXED_TRAJECTORY_CLAIM_CLASS),
            claim_boundary: claim_boundary.into(),
            trajectory_digest: String::new(),
        };
        trajectory.validate()?;
        trajectory.trajectory_digest =
            stable_digest(b"psionic_tassadar_mixed_trajectory|", &trajectory);
        Ok(trajectory)
    }

    /// Validates the trajectory.
    pub fn validate(&self) -> Result<(), TassadarMixedTrajectoryError> {
        if self.schema_version != TASSADAR_MIXED_TRAJECTORY_SCHEMA_VERSION {
            return Err(TassadarMixedTrajectoryError::UnsupportedSchemaVersion {
                schema_version: self.schema_version,
            });
        }
        if self.trajectory_id.trim().is_empty() {
            return Err(TassadarMixedTrajectoryError::MissingTrajectoryId);
        }
        if self.case_id.trim().is_empty() {
            return Err(TassadarMixedTrajectoryError::MissingCaseId);
        }
        if self.entries.is_empty() {
            return Err(TassadarMixedTrajectoryError::MissingEntries);
        }
        if self.final_outputs.is_empty() {
            return Err(TassadarMixedTrajectoryError::MissingFinalOutputs);
        }
        if self.claim_boundary.trim().is_empty() {
            return Err(TassadarMixedTrajectoryError::MissingClaimBoundary);
        }
        let mut receipt_boundary_count = 0u32;
        let mut last_output_entry = None;
        let mut previous_lane = None;
        for (expected_index, entry) in self.entries.iter().enumerate() {
            if entry.entry_index != expected_index as u32 {
                return Err(TassadarMixedTrajectoryError::NonSequentialEntryIndex {
                    expected: expected_index as u32,
                    actual: entry.entry_index,
                });
            }
            entry.validate()?;
            if entry.entry_kind == TassadarMixedTrajectoryEntryKind::ReceiptBoundary {
                receipt_boundary_count = receipt_boundary_count.saturating_add(1);
            }
            if !entry.span_outputs.is_empty() {
                last_output_entry = Some(entry.span_outputs.as_slice());
            }
            match previous_lane {
                None => {
                    if entry.handoff_from_previous.is_some() {
                        return Err(TassadarMixedTrajectoryError::FirstEntryCannotHaveHandoff);
                    }
                }
                Some(previous_lane) if previous_lane == entry.lane_kind => {
                    if entry.handoff_from_previous.is_some() {
                        return Err(TassadarMixedTrajectoryError::UnexpectedHandoff {
                            entry_index: entry.entry_index,
                        });
                    }
                }
                Some(previous_lane) => {
                    let expected_handoff = expected_handoff(previous_lane, entry.lane_kind).ok_or(
                        TassadarMixedTrajectoryError::UnsupportedLaneTransition {
                            from_lane: previous_lane,
                            to_lane: entry.lane_kind,
                            entry_index: entry.entry_index,
                        },
                    )?;
                    if entry.handoff_from_previous != Some(expected_handoff) {
                        return Err(TassadarMixedTrajectoryError::HandoffMismatch {
                            entry_index: entry.entry_index,
                            expected: expected_handoff,
                            actual: entry.handoff_from_previous,
                        });
                    }
                }
            }
            previous_lane = Some(entry.lane_kind);
        }
        if receipt_boundary_count == 0 {
            return Err(TassadarMixedTrajectoryError::MissingReceiptBoundaries);
        }
        if last_output_entry != Some(self.final_outputs.as_slice()) {
            return Err(TassadarMixedTrajectoryError::FinalOutputMismatch);
        }
        Ok(())
    }
}

/// Mixed trajectory validation failure.
#[derive(Clone, Debug, Error, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TassadarMixedTrajectoryError {
    #[error("unsupported Tassadar mixed trajectory schema version `{schema_version}`")]
    UnsupportedSchemaVersion { schema_version: u16 },
    #[error("mixed trajectory is missing a trajectory id")]
    MissingTrajectoryId,
    #[error("mixed trajectory is missing a case id")]
    MissingCaseId,
    #[error("mixed trajectory is missing entries")]
    MissingEntries,
    #[error("mixed trajectory is missing final outputs")]
    MissingFinalOutputs,
    #[error("mixed trajectory is missing a claim boundary")]
    MissingClaimBoundary,
    #[error("mixed trajectory is missing receipt boundaries")]
    MissingReceiptBoundaries,
    #[error("mixed trajectory final outputs do not match the last output-carrying span")]
    FinalOutputMismatch,
    #[error("entry indices must be sequential: expected {expected}, actual {actual}")]
    NonSequentialEntryIndex { expected: u32, actual: u32 },
    #[error("the first mixed trajectory entry cannot carry a handoff")]
    FirstEntryCannotHaveHandoff,
    #[error("entry {entry_index} changed lane without the expected handoff")]
    HandoffMismatch {
        entry_index: u32,
        expected: TassadarMixedTrajectoryHandoffKind,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        actual: Option<TassadarMixedTrajectoryHandoffKind>,
    },
    #[error("entry {entry_index} carried an unexpected handoff without a lane change")]
    UnexpectedHandoff { entry_index: u32 },
    #[error(
        "entry {entry_index} uses unsupported lane transition `{from_lane:?}` -> `{to_lane:?}`"
    )]
    UnsupportedLaneTransition {
        entry_index: u32,
        from_lane: TassadarMixedTrajectoryLaneKind,
        to_lane: TassadarMixedTrajectoryLaneKind,
    },
    #[error("entry {entry_index} is missing an entry id")]
    MissingEntryId { entry_index: u32 },
    #[error("entry {entry_index} is missing a content summary")]
    MissingContentSummary { entry_index: u32 },
    #[error("entry {entry_index} kind `{entry_kind:?}` cannot use lane `{lane_kind:?}`")]
    EntryLaneMismatch {
        entry_index: u32,
        entry_kind: TassadarMixedTrajectoryEntryKind,
        lane_kind: TassadarMixedTrajectoryLaneKind,
    },
    #[error("entry {entry_index} is not a valid language span")]
    MalformedLanguageSpan { entry_index: u32 },
    #[error("entry {entry_index} is not a valid exact-compute span")]
    MalformedComputeSpan { entry_index: u32 },
    #[error("entry {entry_index} is not a valid verifier span")]
    MalformedVerifierSpan { entry_index: u32 },
    #[error("entry {entry_index} is not a valid external-tool span")]
    MalformedExternalToolSpan { entry_index: u32 },
    #[error("entry {entry_index} is missing a receipt boundary")]
    MissingReceiptBoundary { entry_index: u32 },
    #[error("entry {entry_index} is not a valid receipt boundary")]
    MalformedReceiptBoundary { entry_index: u32 },
}

fn expected_handoff(
    from_lane: TassadarMixedTrajectoryLaneKind,
    to_lane: TassadarMixedTrajectoryLaneKind,
) -> Option<TassadarMixedTrajectoryHandoffKind> {
    match (from_lane, to_lane) {
        (
            TassadarMixedTrajectoryLaneKind::LanguageReasoning,
            TassadarMixedTrajectoryLaneKind::InternalExactCompute,
        ) => Some(TassadarMixedTrajectoryHandoffKind::LanguageToInternalCompute),
        (
            TassadarMixedTrajectoryLaneKind::InternalExactCompute,
            TassadarMixedTrajectoryLaneKind::VerifierSearch,
        ) => Some(TassadarMixedTrajectoryHandoffKind::InternalComputeToVerifier),
        (
            TassadarMixedTrajectoryLaneKind::VerifierSearch,
            TassadarMixedTrajectoryLaneKind::LanguageReasoning,
        ) => Some(TassadarMixedTrajectoryHandoffKind::VerifierToLanguage),
        (
            TassadarMixedTrajectoryLaneKind::LanguageReasoning,
            TassadarMixedTrajectoryLaneKind::ExternalTool,
        ) => Some(TassadarMixedTrajectoryHandoffKind::LanguageToExternalTool),
        (
            TassadarMixedTrajectoryLaneKind::ExternalTool,
            TassadarMixedTrajectoryLaneKind::LanguageReasoning,
        ) => Some(TassadarMixedTrajectoryHandoffKind::ExternalToolToLanguage),
        (
            TassadarMixedTrajectoryLaneKind::InternalExactCompute,
            TassadarMixedTrajectoryLaneKind::LanguageReasoning,
        ) => Some(TassadarMixedTrajectoryHandoffKind::InternalComputeToLanguage),
        _ => None,
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
    use super::{
        TassadarMixedTrajectory, TassadarMixedTrajectoryEntry, TassadarMixedTrajectoryEntryKind,
        TassadarMixedTrajectoryError, TassadarMixedTrajectoryHandoffKind,
        TassadarMixedTrajectoryLaneKind, TassadarMixedTrajectoryReceiptBoundary,
        TassadarMixedTrajectoryReceiptBoundaryKind, TassadarMixedTrajectoryVerifierEventKind,
    };

    fn language_entry(index: u32, text: &str) -> TassadarMixedTrajectoryEntry {
        TassadarMixedTrajectoryEntry {
            entry_id: format!("entry-{index}"),
            entry_index: index,
            entry_kind: TassadarMixedTrajectoryEntryKind::LanguageSpan,
            lane_kind: TassadarMixedTrajectoryLaneKind::LanguageReasoning,
            handoff_from_previous: None,
            content_summary: String::from("language_reasoning"),
            language_text: Some(String::from(text)),
            token_count: Some(12),
            step_count: None,
            verifier_event_kind: None,
            tool_name: None,
            receipt_boundary: None,
            span_outputs: Vec::new(),
        }
    }

    #[test]
    fn mixed_trajectory_round_trips_with_explicit_handoffs_and_receipts() {
        let mut compute = TassadarMixedTrajectoryEntry {
            entry_id: String::from("entry-1"),
            entry_index: 1,
            entry_kind: TassadarMixedTrajectoryEntryKind::ExactComputeSpan,
            lane_kind: TassadarMixedTrajectoryLaneKind::InternalExactCompute,
            handoff_from_previous: Some(
                TassadarMixedTrajectoryHandoffKind::LanguageToInternalCompute,
            ),
            content_summary: String::from("exact_compute"),
            language_text: None,
            token_count: None,
            step_count: Some(18),
            verifier_event_kind: None,
            tool_name: None,
            receipt_boundary: None,
            span_outputs: vec![42],
        };
        let receipt = TassadarMixedTrajectoryEntry {
            entry_id: String::from("entry-2"),
            entry_index: 2,
            entry_kind: TassadarMixedTrajectoryEntryKind::ReceiptBoundary,
            lane_kind: TassadarMixedTrajectoryLaneKind::InternalExactCompute,
            handoff_from_previous: None,
            content_summary: String::from("executor_receipt"),
            language_text: None,
            token_count: None,
            step_count: None,
            verifier_event_kind: None,
            tool_name: None,
            receipt_boundary: Some(TassadarMixedTrajectoryReceiptBoundary {
                boundary_kind: TassadarMixedTrajectoryReceiptBoundaryKind::ExecutorEvidenceBundle,
                receipt_ref: String::from(
                    "fixtures/tassadar/reports/example_executor_receipt.json",
                ),
                note: String::from("executor receipt"),
            }),
            span_outputs: Vec::new(),
        };
        let mut final_language = language_entry(3, "answer");
        final_language.handoff_from_previous =
            Some(TassadarMixedTrajectoryHandoffKind::InternalComputeToLanguage);

        let trajectory = TassadarMixedTrajectory::new(
            "trajectory-a",
            "case-a",
            vec![
                language_entry(0, "plan"),
                compute.clone(),
                receipt,
                final_language,
            ],
            vec![42],
            "mixed trajectory stays at execution-truth scope and keeps receipt boundaries explicit",
        )
        .expect("trajectory");

        let encoded = serde_json::to_vec(&trajectory).expect("encode");
        let decoded: TassadarMixedTrajectory = serde_json::from_slice(&encoded).expect("decode");
        assert_eq!(decoded, trajectory);

        compute.handoff_from_previous = None;
        let err = TassadarMixedTrajectory::new(
            "trajectory-b",
            "case-b",
            vec![language_entry(0, "plan"), compute],
            vec![42],
            "boundary",
        )
        .expect_err("handoff should be required");
        assert!(matches!(
            err,
            TassadarMixedTrajectoryError::HandoffMismatch { .. }
        ));
    }

    #[test]
    fn mixed_trajectory_requires_matching_lane_and_entry_kinds() {
        let bad_verifier = TassadarMixedTrajectoryEntry {
            entry_id: String::from("entry-0"),
            entry_index: 0,
            entry_kind: TassadarMixedTrajectoryEntryKind::VerifierEventSpan,
            lane_kind: TassadarMixedTrajectoryLaneKind::LanguageReasoning,
            handoff_from_previous: None,
            content_summary: String::from("verifier"),
            language_text: None,
            token_count: None,
            step_count: Some(2),
            verifier_event_kind: Some(TassadarMixedTrajectoryVerifierEventKind::Verify),
            tool_name: None,
            receipt_boundary: None,
            span_outputs: Vec::new(),
        };
        let err = TassadarMixedTrajectory::new(
            "trajectory-c",
            "case-c",
            vec![bad_verifier],
            vec![1],
            "boundary",
        )
        .expect_err("lane mismatch should fail");
        assert!(matches!(
            err,
            TassadarMixedTrajectoryError::EntryLaneMismatch { .. }
        ));
    }
}
