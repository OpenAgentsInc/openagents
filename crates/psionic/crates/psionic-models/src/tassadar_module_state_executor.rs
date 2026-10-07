use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::ModelDescriptor;

const TASSADAR_MODULE_STATE_EXECUTOR_PUBLICATION_SCHEMA_VERSION: u16 = 1;
pub const TASSADAR_MODULE_STATE_EXECUTOR_CLAIM_CLASS: &str = "research_only_architecture";
pub const TASSADAR_MODULE_STATE_WORKLOAD_SUITE_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_module_scale_workload_suite_report.json";
pub const TASSADAR_MODULE_STATE_ARCHITECTURE_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_module_state_architecture_report.json";

/// Public repo status for the module-state executor redesign lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarModuleStateExecutorPublicationStatus {
    /// The lane is present as an early public research surface.
    ImplementedEarly,
}

/// Deterministic module-scale family used by the module-state redesign lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarModuleStateProgramFamily {
    /// Fixed-span copy and state-movement modules.
    Memcpy,
    /// Fixed-token parse and decode modules.
    Parsing,
    /// Fixed-span checksum and accumulation modules.
    Checksum,
    /// Multi-export dispatch or VM-style handler modules.
    VmStyle,
}

impl TassadarModuleStateProgramFamily {
    /// Returns the stable family label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Memcpy => "memcpy",
            Self::Parsing => "parsing",
            Self::Checksum => "checksum",
            Self::VmStyle => "vm_style",
        }
    }
}

/// Explicit learned state channel exposed by the redesign lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarModuleStateChannelKind {
    /// Frame-local recurrent state over nested module execution.
    CallFrameState,
    /// Delta-oriented global-state summaries.
    GlobalStateDelta,
    /// Byte-range or slot-range memory-delta summaries.
    MemoryDelta,
    /// Export-boundary summaries reused across multi-export modules.
    ExportBoundaryState,
}

/// One curriculum anchor carried by the public module-state redesign lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarModuleStateCurriculumAnchor {
    /// Stable anchor identifier.
    pub stage_id: String,
    /// Human-readable stage summary.
    pub summary: String,
    /// Families admitted during the stage.
    pub train_families: Vec<TassadarModuleStateProgramFamily>,
    /// Held-out families evaluated but not trained during the stage.
    pub held_out_families: Vec<TassadarModuleStateProgramFamily>,
    /// State channels surfaced during the stage.
    pub enabled_state_channels: Vec<TassadarModuleStateChannelKind>,
    /// Fixed later-window start used by the stage eval.
    pub later_window_start_step: u16,
    /// Fixed target-token cap used by the stage eval.
    pub target_token_cap: u16,
}

/// Public model-facing publication for the module-state executor redesign lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarModuleStateExecutorPublication {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable publication identifier.
    pub publication_id: String,
    /// Repo status vocabulary value for the lane.
    pub status: TassadarModuleStateExecutorPublicationStatus,
    /// Explicit claim class for the lane.
    pub claim_class: String,
    /// Stable candidate descriptor for the lane.
    pub model: ModelDescriptor,
    /// Baseline family refs this redesign compares against.
    pub baseline_family_refs: Vec<String>,
    /// Bound workload-suite report that seeds the module families.
    pub source_workload_suite_ref: String,
    /// Stable module families targeted by the redesign.
    pub source_program_families: Vec<TassadarModuleStateProgramFamily>,
    /// Explicit learned state channels carried by the redesign.
    pub state_channels: Vec<TassadarModuleStateChannelKind>,
    /// Stable curriculum anchors the training lane reuses.
    pub curriculum_anchors: Vec<TassadarModuleStateCurriculumAnchor>,
    /// Stable implementation surfaces.
    pub target_surfaces: Vec<String>,
    /// Stable validation refs for the lane.
    pub validation_refs: Vec<String>,
    /// Explicit support boundaries that remain out of scope.
    pub support_boundaries: Vec<String>,
    /// Stable digest over the publication.
    pub publication_digest: String,
}

impl TassadarModuleStateExecutorPublication {
    fn new() -> Self {
        let mut publication = Self {
            schema_version: TASSADAR_MODULE_STATE_EXECUTOR_PUBLICATION_SCHEMA_VERSION,
            publication_id: String::from("tassadar.module_state_executor.publication.v1"),
            status: TassadarModuleStateExecutorPublicationStatus::ImplementedEarly,
            claim_class: String::from(TASSADAR_MODULE_STATE_EXECUTOR_CLAIM_CLASS),
            model: ModelDescriptor::new(
                "tassadar-module-state-executor-candidate-v0",
                "tassadar_module_state_executor",
                "v0",
            ),
            baseline_family_refs: vec![
                String::from("model-family://openagents/tassadar/windowed_lookup_transformer"),
                String::from("model-family://openagents/tassadar/executor_attention_candidate"),
            ],
            source_workload_suite_ref: String::from(TASSADAR_MODULE_STATE_WORKLOAD_SUITE_REPORT_REF),
            source_program_families: vec![
                TassadarModuleStateProgramFamily::Memcpy,
                TassadarModuleStateProgramFamily::Parsing,
                TassadarModuleStateProgramFamily::Checksum,
                TassadarModuleStateProgramFamily::VmStyle,
            ],
            state_channels: vec![
                TassadarModuleStateChannelKind::CallFrameState,
                TassadarModuleStateChannelKind::GlobalStateDelta,
                TassadarModuleStateChannelKind::MemoryDelta,
                TassadarModuleStateChannelKind::ExportBoundaryState,
            ],
            curriculum_anchors: tassadar_module_state_curriculum_anchors(),
            target_surfaces: vec![
                String::from("crates/psionic-models"),
                String::from("crates/psionic-train"),
                String::from("crates/psionic-research"),
            ],
            validation_refs: vec![
                String::from(TASSADAR_MODULE_STATE_WORKLOAD_SUITE_REPORT_REF),
                String::from(TASSADAR_MODULE_STATE_ARCHITECTURE_REPORT_REF),
            ],
            support_boundaries: vec![
                String::from(
                    "the lane remains a learned bounded research-only architecture and is not a served or benchmark-gated capability publication",
                ),
                String::from(
                    "state channels are explicit call-frame, global-delta, memory-delta, and export-boundary summaries only; arbitrary host state and arbitrary Wasm closure remain out of scope",
                ),
                String::from(
                    "the redesign is anchored to deterministic module-scale memcpy, parsing, checksum, and vm-style workloads rather than arbitrary module execution",
                ),
            ],
            publication_digest: String::new(),
        };
        publication.publication_digest = stable_digest(
            b"psionic_tassadar_module_state_executor_publication|",
            &publication,
        );
        publication
    }
}

/// Returns the stable curriculum anchors for the module-state redesign lane.
#[must_use]
pub fn tassadar_module_state_curriculum_anchors() -> Vec<TassadarModuleStateCurriculumAnchor> {
    vec![
        TassadarModuleStateCurriculumAnchor {
            stage_id: String::from("memory_copy_bootstrap"),
            summary: String::from(
                "bootstrap memory-delta and global-state channels on memcpy and checksum before wider dispatch families",
            ),
            train_families: vec![
                TassadarModuleStateProgramFamily::Memcpy,
                TassadarModuleStateProgramFamily::Checksum,
            ],
            held_out_families: vec![
                TassadarModuleStateProgramFamily::Parsing,
                TassadarModuleStateProgramFamily::VmStyle,
            ],
            enabled_state_channels: vec![
                TassadarModuleStateChannelKind::GlobalStateDelta,
                TassadarModuleStateChannelKind::MemoryDelta,
            ],
            later_window_start_step: 8,
            target_token_cap: 64,
        },
        TassadarModuleStateCurriculumAnchor {
            stage_id: String::from("frame_parse_alignment"),
            summary: String::from(
                "introduce frame-local recurrent state on parsing while keeping vm-style dispatch held out",
            ),
            train_families: vec![
                TassadarModuleStateProgramFamily::Memcpy,
                TassadarModuleStateProgramFamily::Checksum,
                TassadarModuleStateProgramFamily::Parsing,
            ],
            held_out_families: vec![TassadarModuleStateProgramFamily::VmStyle],
            enabled_state_channels: vec![
                TassadarModuleStateChannelKind::CallFrameState,
                TassadarModuleStateChannelKind::GlobalStateDelta,
                TassadarModuleStateChannelKind::MemoryDelta,
            ],
            later_window_start_step: 12,
            target_token_cap: 96,
        },
        TassadarModuleStateCurriculumAnchor {
            stage_id: String::from("dispatch_holdout_replay"),
            summary: String::from(
                "freeze explicit export-boundary state and measure held-out vm-style dispatch after parsing and memory stages",
            ),
            train_families: vec![
                TassadarModuleStateProgramFamily::Memcpy,
                TassadarModuleStateProgramFamily::Checksum,
                TassadarModuleStateProgramFamily::Parsing,
            ],
            held_out_families: vec![TassadarModuleStateProgramFamily::VmStyle],
            enabled_state_channels: vec![
                TassadarModuleStateChannelKind::CallFrameState,
                TassadarModuleStateChannelKind::GlobalStateDelta,
                TassadarModuleStateChannelKind::MemoryDelta,
                TassadarModuleStateChannelKind::ExportBoundaryState,
            ],
            later_window_start_step: 16,
            target_token_cap: 128,
        },
    ]
}

/// Returns the canonical public publication for the module-state executor redesign lane.
#[must_use]
pub fn tassadar_module_state_executor_publication() -> TassadarModuleStateExecutorPublication {
    TassadarModuleStateExecutorPublication::new()
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
        TassadarModuleStateChannelKind, TassadarModuleStateExecutorPublicationStatus,
        TassadarModuleStateProgramFamily, TASSADAR_MODULE_STATE_ARCHITECTURE_REPORT_REF,
        tassadar_module_state_executor_publication,
    };

    #[test]
    fn module_state_executor_publication_is_machine_legible() {
        let publication = tassadar_module_state_executor_publication();

        assert_eq!(
            publication.status,
            TassadarModuleStateExecutorPublicationStatus::ImplementedEarly
        );
        assert_eq!(publication.model.family, "tassadar_module_state_executor");
        assert_eq!(
            publication.validation_refs,
            vec![
                String::from("fixtures/tassadar/reports/tassadar_module_scale_workload_suite_report.json"),
                String::from(TASSADAR_MODULE_STATE_ARCHITECTURE_REPORT_REF),
            ]
        );
        assert!(!publication.publication_digest.is_empty());
    }

    #[test]
    fn module_state_executor_publication_carries_memory_delta_holdout_curriculum() {
        let publication = tassadar_module_state_executor_publication();
        assert!(
            publication
                .state_channels
                .contains(&TassadarModuleStateChannelKind::MemoryDelta)
        );
        let stage = publication
            .curriculum_anchors
            .iter()
            .find(|stage| stage.stage_id == "dispatch_holdout_replay")
            .expect("dispatch stage");
        assert!(
            stage.held_out_families
                .contains(&TassadarModuleStateProgramFamily::VmStyle)
        );
    }
}
