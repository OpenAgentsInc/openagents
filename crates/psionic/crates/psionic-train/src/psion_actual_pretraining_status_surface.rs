use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::PSION_ACTUAL_PRETRAINING_LANE_ID;

/// Stable schema version for the canonical current-run status surface.
pub const PSION_ACTUAL_PRETRAINING_CURRENT_RUN_STATUS_SCHEMA_VERSION: &str =
    "psion.actual_pretraining_current_run_status.v1";

/// Stable schema version for the canonical retained summary surface.
pub const PSION_ACTUAL_PRETRAINING_RETAINED_SUMMARY_SCHEMA_VERSION: &str =
    "psion.actual_pretraining_retained_summary.v1";

/// Stable launcher surface id for starting the actual pretraining lane.
pub const PSION_ACTUAL_PRETRAINING_START_SURFACE_ID: &str = "psion_actual_pretraining.start";

/// Stable launcher surface id for dry-running the actual pretraining lane.
pub const PSION_ACTUAL_PRETRAINING_DRY_RUN_SURFACE_ID: &str = "psion_actual_pretraining.dry_run";

/// Stable launcher surface id for resuming the actual pretraining lane.
pub const PSION_ACTUAL_PRETRAINING_RESUME_SURFACE_ID: &str = "psion_actual_pretraining.resume";

/// Stable launcher surface id for reading current run status.
pub const PSION_ACTUAL_PRETRAINING_STATUS_SURFACE_ID: &str = "psion_actual_pretraining.status";

/// Stable launcher surface names the actual-lane operator contract now reserves.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionActualPretrainingLauncherSurfaces {
    /// Reserved start surface id.
    pub start_surface_id: String,
    /// Reserved dry-run surface id.
    pub dry_run_surface_id: String,
    /// Reserved resume surface id.
    pub resume_surface_id: String,
    /// Reserved status surface id.
    pub status_surface_id: String,
}

/// Current run status artifact for the actual pretraining lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionActualPretrainingCurrentRunStatus {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable actual-lane identifier.
    pub lane_id: String,
    /// Stable run identifier.
    pub run_id: String,
    /// Last known run phase.
    pub phase: String,
    /// Relative path to this status artifact inside the run root.
    pub current_status_path: String,
    /// Relative path to the retained summary surface.
    pub retained_summary_path: String,
    /// Relative path to the latest accepted checkpoint pointer.
    pub latest_checkpoint_pointer_path: String,
    /// Relative path to the accepted-checkpoint continuation handoff.
    pub continuation_handoff_path: String,
    /// Last known accepted checkpoint label.
    pub latest_checkpoint_label: String,
    /// Last completed optimizer step.
    pub last_completed_step: u64,
    /// Reserved launcher surface ids.
    pub launcher_surfaces: PsionActualPretrainingLauncherSurfaces,
    /// Last update timestamp.
    pub updated_at_utc: String,
    /// Short detail.
    pub detail: String,
}

/// Retained summary artifact for the actual pretraining lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionActualPretrainingRetainedSummary {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable actual-lane identifier.
    pub lane_id: String,
    /// Stable run identifier.
    pub run_id: String,
    /// Last known phase.
    pub last_known_phase: String,
    /// Selected git ref for the run.
    pub selected_git_ref: String,
    /// Exact git commit SHA for the run.
    pub git_commit_sha: String,
    /// Dirty-tree admission posture.
    pub dirty_tree_admission: String,
    /// Relative path to the current status surface.
    pub current_status_path: String,
    /// Relative path to the latest accepted checkpoint pointer.
    pub latest_checkpoint_pointer_path: String,
    /// Relative path to the accepted-checkpoint continuation handoff.
    pub continuation_handoff_path: String,
    /// Reserved launcher surface ids.
    pub launcher_surfaces: PsionActualPretrainingLauncherSurfaces,
    /// Claim boundary for the retained summary.
    pub claim_boundary: String,
    /// Short detail.
    pub detail: String,
}

impl PsionActualPretrainingCurrentRunStatus {
    /// Validates the current-run status artifact.
    pub fn validate(&self) -> Result<(), PsionActualPretrainingStatusSurfaceError> {
        ensure_nonempty(
            self.schema_version.as_str(),
            "current_status.schema_version",
        )?;
        if self.schema_version != PSION_ACTUAL_PRETRAINING_CURRENT_RUN_STATUS_SCHEMA_VERSION {
            return Err(PsionActualPretrainingStatusSurfaceError::FieldMismatch {
                field: String::from("schema_version"),
                expected: String::from(PSION_ACTUAL_PRETRAINING_CURRENT_RUN_STATUS_SCHEMA_VERSION),
                actual: self.schema_version.clone(),
            });
        }
        ensure_lane_id(self.lane_id.as_str())?;
        ensure_nonempty(self.run_id.as_str(), "current_status.run_id")?;
        ensure_nonempty(self.phase.as_str(), "current_status.phase")?;
        if self.current_status_path != "status/current_run_status.json" {
            return Err(PsionActualPretrainingStatusSurfaceError::FieldMismatch {
                field: String::from("current_status_path"),
                expected: String::from("status/current_run_status.json"),
                actual: self.current_status_path.clone(),
            });
        }
        if self.retained_summary_path != "status/retained_summary.json" {
            return Err(PsionActualPretrainingStatusSurfaceError::FieldMismatch {
                field: String::from("retained_summary_path"),
                expected: String::from("status/retained_summary.json"),
                actual: self.retained_summary_path.clone(),
            });
        }
        if self.latest_checkpoint_pointer_path
            != "checkpoints/latest_accepted_checkpoint_pointer.json"
        {
            return Err(PsionActualPretrainingStatusSurfaceError::FieldMismatch {
                field: String::from("latest_checkpoint_pointer_path"),
                expected: String::from("checkpoints/latest_accepted_checkpoint_pointer.json"),
                actual: self.latest_checkpoint_pointer_path.clone(),
            });
        }
        if self.continuation_handoff_path != "continuation/accepted_checkpoint_handoff.json" {
            return Err(PsionActualPretrainingStatusSurfaceError::FieldMismatch {
                field: String::from("continuation_handoff_path"),
                expected: String::from("continuation/accepted_checkpoint_handoff.json"),
                actual: self.continuation_handoff_path.clone(),
            });
        }
        ensure_nonempty(
            self.latest_checkpoint_label.as_str(),
            "current_status.latest_checkpoint_label",
        )?;
        let zero_step_phase = matches!(self.phase.as_str(), "dry_run_planned" | "launch_staged");
        if self.last_completed_step == 0 && !zero_step_phase {
            return Err(PsionActualPretrainingStatusSurfaceError::MissingField {
                field: String::from("current_status.last_completed_step"),
            });
        }
        if self.last_completed_step == 0
            && self.latest_checkpoint_label != "pending_first_checkpoint"
        {
            return Err(PsionActualPretrainingStatusSurfaceError::FieldMismatch {
                field: String::from("current_status.latest_checkpoint_label"),
                expected: String::from("pending_first_checkpoint"),
                actual: self.latest_checkpoint_label.clone(),
            });
        }
        self.launcher_surfaces.validate()?;
        ensure_nonempty(
            self.updated_at_utc.as_str(),
            "current_status.updated_at_utc",
        )?;
        ensure_nonempty(self.detail.as_str(), "current_status.detail")?;
        Ok(())
    }
}

impl PsionActualPretrainingRetainedSummary {
    /// Validates the retained summary artifact.
    pub fn validate(&self) -> Result<(), PsionActualPretrainingStatusSurfaceError> {
        ensure_nonempty(
            self.schema_version.as_str(),
            "retained_summary.schema_version",
        )?;
        if self.schema_version != PSION_ACTUAL_PRETRAINING_RETAINED_SUMMARY_SCHEMA_VERSION {
            return Err(PsionActualPretrainingStatusSurfaceError::FieldMismatch {
                field: String::from("schema_version"),
                expected: String::from(PSION_ACTUAL_PRETRAINING_RETAINED_SUMMARY_SCHEMA_VERSION),
                actual: self.schema_version.clone(),
            });
        }
        ensure_lane_id(self.lane_id.as_str())?;
        ensure_nonempty(self.run_id.as_str(), "retained_summary.run_id")?;
        ensure_nonempty(
            self.last_known_phase.as_str(),
            "retained_summary.last_known_phase",
        )?;
        ensure_nonempty(
            self.selected_git_ref.as_str(),
            "retained_summary.selected_git_ref",
        )?;
        ensure_nonempty(
            self.git_commit_sha.as_str(),
            "retained_summary.git_commit_sha",
        )?;
        ensure_nonempty(
            self.dirty_tree_admission.as_str(),
            "retained_summary.dirty_tree_admission",
        )?;
        if self.current_status_path != "status/current_run_status.json" {
            return Err(PsionActualPretrainingStatusSurfaceError::FieldMismatch {
                field: String::from("current_status_path"),
                expected: String::from("status/current_run_status.json"),
                actual: self.current_status_path.clone(),
            });
        }
        if self.latest_checkpoint_pointer_path
            != "checkpoints/latest_accepted_checkpoint_pointer.json"
        {
            return Err(PsionActualPretrainingStatusSurfaceError::FieldMismatch {
                field: String::from("latest_checkpoint_pointer_path"),
                expected: String::from("checkpoints/latest_accepted_checkpoint_pointer.json"),
                actual: self.latest_checkpoint_pointer_path.clone(),
            });
        }
        if self.continuation_handoff_path != "continuation/accepted_checkpoint_handoff.json" {
            return Err(PsionActualPretrainingStatusSurfaceError::FieldMismatch {
                field: String::from("continuation_handoff_path"),
                expected: String::from("continuation/accepted_checkpoint_handoff.json"),
                actual: self.continuation_handoff_path.clone(),
            });
        }
        self.launcher_surfaces.validate()?;
        ensure_nonempty(
            self.claim_boundary.as_str(),
            "retained_summary.claim_boundary",
        )?;
        ensure_nonempty(self.detail.as_str(), "retained_summary.detail")?;
        Ok(())
    }
}

impl PsionActualPretrainingLauncherSurfaces {
    /// Validates the reserved launcher surface ids.
    pub fn validate(&self) -> Result<(), PsionActualPretrainingStatusSurfaceError> {
        if self.start_surface_id != PSION_ACTUAL_PRETRAINING_START_SURFACE_ID {
            return Err(PsionActualPretrainingStatusSurfaceError::FieldMismatch {
                field: String::from("start_surface_id"),
                expected: String::from(PSION_ACTUAL_PRETRAINING_START_SURFACE_ID),
                actual: self.start_surface_id.clone(),
            });
        }
        if self.dry_run_surface_id != PSION_ACTUAL_PRETRAINING_DRY_RUN_SURFACE_ID {
            return Err(PsionActualPretrainingStatusSurfaceError::FieldMismatch {
                field: String::from("dry_run_surface_id"),
                expected: String::from(PSION_ACTUAL_PRETRAINING_DRY_RUN_SURFACE_ID),
                actual: self.dry_run_surface_id.clone(),
            });
        }
        if self.resume_surface_id != PSION_ACTUAL_PRETRAINING_RESUME_SURFACE_ID {
            return Err(PsionActualPretrainingStatusSurfaceError::FieldMismatch {
                field: String::from("resume_surface_id"),
                expected: String::from(PSION_ACTUAL_PRETRAINING_RESUME_SURFACE_ID),
                actual: self.resume_surface_id.clone(),
            });
        }
        if self.status_surface_id != PSION_ACTUAL_PRETRAINING_STATUS_SURFACE_ID {
            return Err(PsionActualPretrainingStatusSurfaceError::FieldMismatch {
                field: String::from("status_surface_id"),
                expected: String::from(PSION_ACTUAL_PRETRAINING_STATUS_SURFACE_ID),
                actual: self.status_surface_id.clone(),
            });
        }
        Ok(())
    }
}

/// Validation errors for the actual-pretraining status surfaces.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum PsionActualPretrainingStatusSurfaceError {
    #[error("psion actual-pretraining status field `{field}` must not be empty")]
    MissingField { field: String },
    #[error(
        "psion actual-pretraining status field `{field}` mismatch: expected `{expected}`, got `{actual}`"
    )]
    FieldMismatch {
        field: String,
        expected: String,
        actual: String,
    },
}

fn ensure_lane_id(lane_id: &str) -> Result<(), PsionActualPretrainingStatusSurfaceError> {
    if lane_id != PSION_ACTUAL_PRETRAINING_LANE_ID {
        return Err(PsionActualPretrainingStatusSurfaceError::FieldMismatch {
            field: String::from("lane_id"),
            expected: String::from(PSION_ACTUAL_PRETRAINING_LANE_ID),
            actual: String::from(lane_id),
        });
    }
    Ok(())
}

fn ensure_nonempty(
    value: &str,
    field: &str,
) -> Result<(), PsionActualPretrainingStatusSurfaceError> {
    if value.trim().is_empty() {
        return Err(PsionActualPretrainingStatusSurfaceError::MissingField {
            field: String::from(field),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{PsionActualPretrainingCurrentRunStatus, PsionActualPretrainingRetainedSummary};

    fn current_status() -> PsionActualPretrainingCurrentRunStatus {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/pretrain/psion_actual_pretraining_current_run_status_v1.json"
        ))
        .expect("actual pretraining current status fixture should parse")
    }

    fn retained_summary() -> PsionActualPretrainingRetainedSummary {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/pretrain/psion_actual_pretraining_retained_summary_v1.json"
        ))
        .expect("actual pretraining retained summary fixture should parse")
    }

    #[test]
    fn actual_pretraining_current_status_fixture_validates() {
        current_status()
            .validate()
            .expect("actual pretraining current status fixture should validate");
    }

    #[test]
    fn actual_pretraining_retained_summary_fixture_validates() {
        retained_summary()
            .validate()
            .expect("actual pretraining retained summary fixture should validate");
    }

    #[test]
    fn actual_pretraining_status_rejects_missing_status_surface_id() {
        let mut summary = retained_summary();
        summary.launcher_surfaces.status_surface_id = String::from("wrong.status.surface");
        let error = summary
            .validate()
            .expect_err("wrong status surface id should be rejected");
        assert_eq!(
            error,
            super::PsionActualPretrainingStatusSurfaceError::FieldMismatch {
                field: String::from("status_surface_id"),
                expected: String::from(super::PSION_ACTUAL_PRETRAINING_STATUS_SURFACE_ID),
                actual: String::from("wrong.status.surface"),
            }
        );
    }

    #[test]
    fn actual_pretraining_current_status_accepts_precheckpoint_launch_phase() {
        let mut status = current_status();
        status.phase = String::from("launch_staged");
        status.latest_checkpoint_label = String::from("pending_first_checkpoint");
        status.last_completed_step = 0;
        status
            .validate()
            .expect("launch-staged status should allow zero-step pending checkpoint state");
    }
}
