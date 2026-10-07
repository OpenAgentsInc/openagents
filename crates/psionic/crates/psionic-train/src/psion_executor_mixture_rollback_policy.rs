use std::{fs, path::Path};

use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    build_executor_source_family_contribution_report, builtin_executor_canonical_mixture_packet,
    PsionExecutorCanonicalMixtureError, PsionExecutorSourceFamilyContributionError,
    PSION_EXECUTOR_CANONICAL_MIXTURE_DOC_PATH, PSION_EXECUTOR_CANONICAL_MIXTURE_FIXTURE_PATH,
    PSION_EXECUTOR_SOURCE_FAMILY_CONTRIBUTION_DOC_PATH,
    PSION_EXECUTOR_SOURCE_FAMILY_CONTRIBUTION_FIXTURE_PATH,
};

pub const PSION_EXECUTOR_MIXTURE_ROLLBACK_POLICY_SCHEMA_VERSION: &str =
    "psion.executor.mixture_rollback_policy.v1";
pub const PSION_EXECUTOR_MIXTURE_ROLLBACK_POLICY_FIXTURE_PATH: &str =
    "fixtures/psion/executor/psion_executor_mixture_rollback_policy_v1.json";
pub const PSION_EXECUTOR_MIXTURE_ROLLBACK_POLICY_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_MIXTURE_ROLLBACK_POLICY.md";

const POLICY_ID: &str = "psion_executor_mixture_rollback_policy_v1";
const PSION_EXECUTOR_PROGRAM_DOC_PATH: &str = "docs/PSION_EXECUTOR_PROGRAM.md";
const PSION_EXECUTOR_LOCAL_CLUSTER_REVIEW_WORKFLOW_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_LOCAL_CLUSTER_REVIEW_WORKFLOW.md";
const MAX_CHANGED_LEVERS_AFTER_ROLLBACK: u32 = 1;

#[derive(Debug, Error)]
pub enum PsionExecutorMixtureRollbackPolicyError {
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("failed to parse `{path}`: {error}")]
    Parse {
        path: String,
        error: serde_json::Error,
    },
    #[error("missing required field `{field}`")]
    MissingField { field: String },
    #[error("invalid value for `{field}`: {detail}")]
    InvalidValue { field: String, detail: String },
    #[error("schema version mismatch: expected `{expected}` but found `{actual}`")]
    SchemaVersionMismatch { expected: String, actual: String },
    #[error("digest mismatch for `{field}`")]
    DigestMismatch { field: String },
    #[error("fixture `{path}` drifted from the canonical generator output")]
    FixtureDrift { path: String },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Mixture(#[from] PsionExecutorCanonicalMixtureError),
    #[error(transparent)]
    Contribution(#[from] PsionExecutorSourceFamilyContributionError),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionExecutorMixtureRollbackTriggerRow {
    pub trigger_id: String,
    pub same_budget_win_required: bool,
    pub delta_scope: String,
    pub negative_delta_count: u32,
    pub trigger_active: bool,
    pub rollback_action: String,
    pub detail: String,
    pub row_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionExecutorMixtureRetryConstraint {
    pub max_changed_levers: u32,
    pub allowed_lever_classes: Vec<String>,
    pub detail: String,
    pub row_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionExecutorMixtureRollbackPolicyPacket {
    pub schema_version: String,
    pub policy_id: String,
    pub canonical_mixture_ref: String,
    pub canonical_mixture_digest: String,
    pub active_mixture_version_id: String,
    pub source_family_contribution_ref: String,
    pub source_family_contribution_digest: String,
    pub baseline_row_id: String,
    pub candidate_row_id: String,
    pub same_budget_win_claim_present: bool,
    pub rollback_triggered: bool,
    pub rollback_decision: String,
    pub rollback_status: String,
    pub trigger_rows: Vec<PsionExecutorMixtureRollbackTriggerRow>,
    pub retry_constraint: PsionExecutorMixtureRetryConstraint,
    pub support_refs: Vec<String>,
    pub summary: String,
    pub packet_digest: String,
}

impl PsionExecutorMixtureRollbackTriggerRow {
    fn validate(&self) -> Result<(), PsionExecutorMixtureRollbackPolicyError> {
        for (field, value) in [
            (
                "psion_executor_mixture_rollback_policy.trigger_rows[].trigger_id",
                self.trigger_id.as_str(),
            ),
            (
                "psion_executor_mixture_rollback_policy.trigger_rows[].delta_scope",
                self.delta_scope.as_str(),
            ),
            (
                "psion_executor_mixture_rollback_policy.trigger_rows[].rollback_action",
                self.rollback_action.as_str(),
            ),
            (
                "psion_executor_mixture_rollback_policy.trigger_rows[].detail",
                self.detail.as_str(),
            ),
            (
                "psion_executor_mixture_rollback_policy.trigger_rows[].row_digest",
                self.row_digest.as_str(),
            ),
        ] {
            ensure_nonempty(value, field)?;
        }
        if stable_trigger_row_digest(self) != self.row_digest {
            return Err(PsionExecutorMixtureRollbackPolicyError::DigestMismatch {
                field: String::from(
                    "psion_executor_mixture_rollback_policy.trigger_rows[].row_digest",
                ),
            });
        }
        Ok(())
    }
}

impl PsionExecutorMixtureRetryConstraint {
    fn validate(&self) -> Result<(), PsionExecutorMixtureRollbackPolicyError> {
        ensure_nonempty(
            self.detail.as_str(),
            "psion_executor_mixture_rollback_policy.retry_constraint.detail",
        )?;
        ensure_nonempty(
            self.row_digest.as_str(),
            "psion_executor_mixture_rollback_policy.retry_constraint.row_digest",
        )?;
        if self.max_changed_levers != MAX_CHANGED_LEVERS_AFTER_ROLLBACK {
            return Err(PsionExecutorMixtureRollbackPolicyError::InvalidValue {
                field: String::from(
                    "psion_executor_mixture_rollback_policy.retry_constraint.max_changed_levers",
                ),
                detail: String::from("rollback retry must stay limited to one lever"),
            });
        }
        if self.allowed_lever_classes.is_empty() {
            return Err(PsionExecutorMixtureRollbackPolicyError::MissingField {
                field: String::from(
                    "psion_executor_mixture_rollback_policy.retry_constraint.allowed_lever_classes",
                ),
            });
        }
        for lever in &self.allowed_lever_classes {
            ensure_nonempty(
                lever.as_str(),
                "psion_executor_mixture_rollback_policy.retry_constraint.allowed_lever_classes[]",
            )?;
        }
        if stable_retry_constraint_digest(self) != self.row_digest {
            return Err(PsionExecutorMixtureRollbackPolicyError::DigestMismatch {
                field: String::from(
                    "psion_executor_mixture_rollback_policy.retry_constraint.row_digest",
                ),
            });
        }
        Ok(())
    }
}

impl PsionExecutorMixtureRollbackPolicyPacket {
    pub fn validate(&self) -> Result<(), PsionExecutorMixtureRollbackPolicyError> {
        if self.schema_version != PSION_EXECUTOR_MIXTURE_ROLLBACK_POLICY_SCHEMA_VERSION {
            return Err(PsionExecutorMixtureRollbackPolicyError::SchemaVersionMismatch {
                expected: String::from(PSION_EXECUTOR_MIXTURE_ROLLBACK_POLICY_SCHEMA_VERSION),
                actual: self.schema_version.clone(),
            });
        }
        for (field, value) in [
            (
                "psion_executor_mixture_rollback_policy.policy_id",
                self.policy_id.as_str(),
            ),
            (
                "psion_executor_mixture_rollback_policy.canonical_mixture_ref",
                self.canonical_mixture_ref.as_str(),
            ),
            (
                "psion_executor_mixture_rollback_policy.canonical_mixture_digest",
                self.canonical_mixture_digest.as_str(),
            ),
            (
                "psion_executor_mixture_rollback_policy.active_mixture_version_id",
                self.active_mixture_version_id.as_str(),
            ),
            (
                "psion_executor_mixture_rollback_policy.source_family_contribution_ref",
                self.source_family_contribution_ref.as_str(),
            ),
            (
                "psion_executor_mixture_rollback_policy.source_family_contribution_digest",
                self.source_family_contribution_digest.as_str(),
            ),
            (
                "psion_executor_mixture_rollback_policy.baseline_row_id",
                self.baseline_row_id.as_str(),
            ),
            (
                "psion_executor_mixture_rollback_policy.candidate_row_id",
                self.candidate_row_id.as_str(),
            ),
            (
                "psion_executor_mixture_rollback_policy.rollback_decision",
                self.rollback_decision.as_str(),
            ),
            (
                "psion_executor_mixture_rollback_policy.rollback_status",
                self.rollback_status.as_str(),
            ),
            (
                "psion_executor_mixture_rollback_policy.summary",
                self.summary.as_str(),
            ),
            (
                "psion_executor_mixture_rollback_policy.packet_digest",
                self.packet_digest.as_str(),
            ),
        ] {
            ensure_nonempty(value, field)?;
        }
        if self.trigger_rows.len() != 2 || self.support_refs.is_empty() {
            return Err(PsionExecutorMixtureRollbackPolicyError::MissingField {
                field: String::from("psion_executor_mixture_rollback_policy.required_arrays"),
            });
        }
        for row in &self.trigger_rows {
            row.validate()?;
            if row.trigger_active && !self.same_budget_win_claim_present {
                return Err(PsionExecutorMixtureRollbackPolicyError::InvalidValue {
                    field: String::from(
                        "psion_executor_mixture_rollback_policy.same_budget_win_claim_present",
                    ),
                    detail: String::from(
                        "rollback trigger cannot activate without a provisional same-budget win claim",
                    ),
                });
            }
        }
        self.retry_constraint.validate()?;
        let any_trigger_active = self.trigger_rows.iter().any(|row| row.trigger_active);
        if any_trigger_active != self.rollback_triggered {
            return Err(PsionExecutorMixtureRollbackPolicyError::InvalidValue {
                field: String::from(
                    "psion_executor_mixture_rollback_policy.rollback_triggered",
                ),
                detail: String::from(
                    "rollback_triggered must match the trigger rows' active state",
                ),
            });
        }
        if self.rollback_triggered {
            if self.rollback_decision != "rollback_misleading_mixture_win"
                || self.rollback_status != "rollback_required_single_lever_retry"
            {
                return Err(PsionExecutorMixtureRollbackPolicyError::InvalidValue {
                    field: String::from(
                        "psion_executor_mixture_rollback_policy.rollback_decision",
                    ),
                    detail: String::from(
                        "triggered rollback must record the canonical rollback decision and status",
                    ),
                });
            }
        } else if self.rollback_decision != "hold_no_misleading_mixture_win"
            || self.rollback_status != "no_misleading_win_current_week"
        {
            return Err(PsionExecutorMixtureRollbackPolicyError::InvalidValue {
                field: String::from(
                    "psion_executor_mixture_rollback_policy.rollback_decision",
                ),
                detail: String::from(
                    "non-triggered rollback policy must record the canonical no-trigger decision and status",
                ),
            });
        }
        if stable_packet_digest(self) != self.packet_digest {
            return Err(PsionExecutorMixtureRollbackPolicyError::DigestMismatch {
                field: String::from("psion_executor_mixture_rollback_policy.packet_digest"),
            });
        }
        Ok(())
    }
}

pub fn builtin_executor_mixture_rollback_policy_packet(
    workspace_root: &Path,
) -> Result<PsionExecutorMixtureRollbackPolicyPacket, PsionExecutorMixtureRollbackPolicyError> {
    let canonical_mixture = builtin_executor_canonical_mixture_packet(workspace_root)?;
    let report = build_executor_source_family_contribution_report(workspace_root)?;

    let same_budget_win_claim_present = report.source_family_rows.iter().any(|row| {
        row.exactness_delta_bps > 0
            || row
                .held_out_slice_deltas
                .iter()
                .any(|slice| slice.delta_bps > 0)
            || row
                .adversarial_slice_deltas
                .iter()
                .any(|slice| slice.delta_bps > 0)
    });
    let negative_held_out_delta_count = report
        .source_family_rows
        .iter()
        .flat_map(|row| row.held_out_slice_deltas.iter())
        .filter(|slice| slice.delta_bps < 0)
        .count() as u32;
    let negative_adversarial_delta_count = report
        .source_family_rows
        .iter()
        .flat_map(|row| row.adversarial_slice_deltas.iter())
        .filter(|slice| slice.delta_bps < 0)
        .count() as u32;

    let trigger_rows = vec![
        build_trigger_row(
            "held_out_negative_delta_after_provisional_win",
            "held_out",
            negative_held_out_delta_count,
            same_budget_win_claim_present,
            "Rollback any provisional same-budget mixture winner immediately when a held-out slice regresses, then keep promotion on hold until the next constrained retry clears frozen-pack review.",
        ),
        build_trigger_row(
            "adversarial_negative_delta_after_provisional_win",
            "adversarial",
            negative_adversarial_delta_count,
            same_budget_win_claim_present,
            "Rollback any provisional same-budget mixture winner immediately when an adversarial slice regresses, then keep promotion on hold until the next constrained retry clears frozen-pack review.",
        ),
    ];
    let rollback_triggered = trigger_rows.iter().any(|row| row.trigger_active);
    let retry_constraint = build_retry_constraint();

    let mut packet = PsionExecutorMixtureRollbackPolicyPacket {
        schema_version: String::from(PSION_EXECUTOR_MIXTURE_ROLLBACK_POLICY_SCHEMA_VERSION),
        policy_id: String::from(POLICY_ID),
        canonical_mixture_ref: String::from(PSION_EXECUTOR_CANONICAL_MIXTURE_FIXTURE_PATH),
        canonical_mixture_digest: canonical_mixture.packet_digest,
        active_mixture_version_id: canonical_mixture.mixture_id,
        source_family_contribution_ref: String::from(
            PSION_EXECUTOR_SOURCE_FAMILY_CONTRIBUTION_FIXTURE_PATH,
        ),
        source_family_contribution_digest: report.report_digest.clone(),
        baseline_row_id: report.baseline_row_id.clone(),
        candidate_row_id: report.candidate_row_id.clone(),
        same_budget_win_claim_present,
        rollback_triggered,
        rollback_decision: if rollback_triggered {
            String::from("rollback_misleading_mixture_win")
        } else {
            String::from("hold_no_misleading_mixture_win")
        },
        rollback_status: if rollback_triggered {
            String::from("rollback_required_single_lever_retry")
        } else {
            String::from("no_misleading_win_current_week")
        },
        trigger_rows,
        retry_constraint,
        support_refs: vec![
            String::from(PSION_EXECUTOR_PROGRAM_DOC_PATH),
            String::from(PSION_EXECUTOR_CANONICAL_MIXTURE_DOC_PATH),
            String::from(PSION_EXECUTOR_SOURCE_FAMILY_CONTRIBUTION_DOC_PATH),
            String::from(PSION_EXECUTOR_LOCAL_CLUSTER_REVIEW_WORKFLOW_DOC_PATH),
        ],
        summary: if rollback_triggered {
            String::from(
                "The executor lane now has one canonical mixture rollback policy packet. A provisional same-budget mixture win regressed at least one held-out or adversarial slice, so the lane records an immediate rollback and limits the next retry to one lever.",
            )
        } else {
            String::from(
                "The executor lane now has one canonical mixture rollback policy packet. No provisional same-budget mixture win is retained this week, but any future held-out or adversarial regression under a provisional win will trigger rollback and limit the next retry to one lever.",
            )
        },
        packet_digest: String::new(),
    };
    packet.packet_digest = stable_packet_digest(&packet);
    packet.validate()?;
    Ok(packet)
}

pub fn write_builtin_executor_mixture_rollback_policy_packet(
    workspace_root: &Path,
) -> Result<PsionExecutorMixtureRollbackPolicyPacket, PsionExecutorMixtureRollbackPolicyError> {
    let packet = builtin_executor_mixture_rollback_policy_packet(workspace_root)?;
    write_json_fixture(
        workspace_root,
        PSION_EXECUTOR_MIXTURE_ROLLBACK_POLICY_FIXTURE_PATH,
        &packet,
    )?;
    Ok(packet)
}

fn build_trigger_row(
    trigger_id: &str,
    delta_scope: &str,
    negative_delta_count: u32,
    same_budget_win_claim_present: bool,
    detail: &str,
) -> PsionExecutorMixtureRollbackTriggerRow {
    let mut row = PsionExecutorMixtureRollbackTriggerRow {
        trigger_id: String::from(trigger_id),
        same_budget_win_required: true,
        delta_scope: String::from(delta_scope),
        negative_delta_count,
        trigger_active: same_budget_win_claim_present && negative_delta_count > 0,
        rollback_action: String::from("rollback_candidate_and_hold_promotion"),
        detail: String::from(detail),
        row_digest: String::new(),
    };
    row.row_digest = stable_trigger_row_digest(&row);
    row
}

fn build_retry_constraint() -> PsionExecutorMixtureRetryConstraint {
    let mut constraint = PsionExecutorMixtureRetryConstraint {
        max_changed_levers: MAX_CHANGED_LEVERS_AFTER_ROLLBACK,
        allowed_lever_classes: vec![
            String::from("source_family_weight_bps"),
            String::from("held_out_exclusion_boundary"),
            String::from("curriculum_stage_boundary"),
        ],
        detail: String::from(
            "After a misleading mixture win rolls back, the next admitted retry may change exactly one lever: one family-weight adjustment, one exclusion-boundary adjustment, or one curriculum-boundary adjustment. Multi-lever retries remain out of policy until a clean frozen-pack review exists.",
        ),
        row_digest: String::new(),
    };
    constraint.row_digest = stable_retry_constraint_digest(&constraint);
    constraint
}

fn stable_trigger_row_digest(row: &PsionExecutorMixtureRollbackTriggerRow) -> String {
    let mut clone = row.clone();
    clone.row_digest.clear();
    stable_json_digest("psion_executor_mixture_rollback_policy_trigger", &clone)
}

fn stable_retry_constraint_digest(row: &PsionExecutorMixtureRetryConstraint) -> String {
    let mut clone = row.clone();
    clone.row_digest.clear();
    stable_json_digest("psion_executor_mixture_rollback_policy_retry_constraint", &clone)
}

fn stable_packet_digest(packet: &PsionExecutorMixtureRollbackPolicyPacket) -> String {
    let mut clone = packet.clone();
    clone.packet_digest.clear();
    stable_json_digest("psion_executor_mixture_rollback_policy_packet", &clone)
}

fn stable_json_digest<T: Serialize>(label: &str, value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(label.as_bytes());
    hasher.update(b"|");
    hasher.update(serde_json::to_vec(value).expect("stable json"));
    hex::encode(hasher.finalize())
}

fn write_json_fixture<T: Serialize>(
    workspace_root: &Path,
    relative_path: &str,
    value: &T,
) -> Result<(), PsionExecutorMixtureRollbackPolicyError> {
    let path = workspace_root.join(relative_path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            PsionExecutorMixtureRollbackPolicyError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let body = serde_json::to_vec_pretty(value)?;
    fs::write(&path, body).map_err(|error| PsionExecutorMixtureRollbackPolicyError::Write {
        path: path.display().to_string(),
        error,
    })?;
    Ok(())
}

fn read_json<T: DeserializeOwned>(
    workspace_root: &Path,
    relative_path: &str,
) -> Result<T, PsionExecutorMixtureRollbackPolicyError> {
    let path = workspace_root.join(relative_path);
    let bytes = fs::read(&path).map_err(|error| PsionExecutorMixtureRollbackPolicyError::Read {
        path: path.display().to_string(),
        error,
    })?;
    serde_json::from_slice(&bytes).map_err(|error| PsionExecutorMixtureRollbackPolicyError::Parse {
        path: path.display().to_string(),
        error,
    })
}

fn ensure_nonempty(value: &str, field: &str) -> Result<(), PsionExecutorMixtureRollbackPolicyError> {
    if value.trim().is_empty() {
        return Err(PsionExecutorMixtureRollbackPolicyError::MissingField {
            field: String::from(field),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workspace_root() -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|path| path.parent())
            .expect("workspace root")
            .to_path_buf()
    }

    #[test]
    fn builtin_executor_mixture_rollback_policy_packet_is_valid(
    ) -> Result<(), PsionExecutorMixtureRollbackPolicyError> {
        let root = workspace_root();
        let packet = builtin_executor_mixture_rollback_policy_packet(root.as_path())?;
        packet.validate()?;
        assert_eq!(packet.trigger_rows.len(), 2);
        assert_eq!(packet.retry_constraint.max_changed_levers, 1);
        Ok(())
    }

    #[test]
    fn executor_mixture_rollback_policy_fixture_matches_committed_truth(
    ) -> Result<(), PsionExecutorMixtureRollbackPolicyError> {
        let root = workspace_root();
        let expected: PsionExecutorMixtureRollbackPolicyPacket =
            read_json(root.as_path(), PSION_EXECUTOR_MIXTURE_ROLLBACK_POLICY_FIXTURE_PATH)?;
        let actual = builtin_executor_mixture_rollback_policy_packet(root.as_path())?;
        if expected != actual {
            return Err(PsionExecutorMixtureRollbackPolicyError::FixtureDrift {
                path: String::from(PSION_EXECUTOR_MIXTURE_ROLLBACK_POLICY_FIXTURE_PATH),
            });
        }
        Ok(())
    }

    #[test]
    fn rollback_retry_stays_single_lever(
    ) -> Result<(), PsionExecutorMixtureRollbackPolicyError> {
        let root = workspace_root();
        let packet = builtin_executor_mixture_rollback_policy_packet(root.as_path())?;
        assert_eq!(packet.retry_constraint.max_changed_levers, 1);
        assert!(!packet.retry_constraint.allowed_lever_classes.is_empty());
        Ok(())
    }

    #[test]
    fn current_policy_keeps_no_trigger_without_negative_slice_deltas(
    ) -> Result<(), PsionExecutorMixtureRollbackPolicyError> {
        let root = workspace_root();
        let packet = builtin_executor_mixture_rollback_policy_packet(root.as_path())?;
        assert!(!packet.same_budget_win_claim_present);
        assert!(!packet.rollback_triggered);
        assert_eq!(packet.rollback_decision, "hold_no_misleading_mixture_win");
        Ok(())
    }
}
