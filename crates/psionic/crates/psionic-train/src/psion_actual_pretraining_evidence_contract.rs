use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    PSION_ACTUAL_PRETRAINING_LANE_ID, PSION_ACTUAL_PRETRAINING_RECIPE_ID,
    PSION_ACTUAL_PRETRAINING_TOPOLOGY_STORAGE_BUNDLE_ID, PsionActualPretrainingArtifactRef,
};

/// Stable schema version for the canonical actual-lane output and evidence contract.
pub const PSION_ACTUAL_PRETRAINING_EVIDENCE_CONTRACT_SCHEMA_VERSION: &str =
    "psion.actual_pretraining_evidence_contract.v1";

/// Stable contract identifier for the canonical actual-lane output and evidence contract.
pub const PSION_ACTUAL_PRETRAINING_EVIDENCE_CONTRACT_ID: &str =
    "psion_actual_pretraining_evidence_contract_v1";

/// Stable evidence family for the canonical actual pretraining lane.
pub const PSION_ACTUAL_PRETRAINING_EVIDENCE_FAMILY: &str = "psion.actual_pretraining.evidence.v1";

/// Stable run-root family for the canonical actual pretraining lane.
pub const PSION_ACTUAL_PRETRAINING_RUN_ROOT_FAMILY: &str = "psion_actual_pretraining_runs/<run_id>";

/// Stable artifact slot under the actual-lane retained output family.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionActualPretrainingArtifactSlot {
    /// Relative path under `psion_actual_pretraining_runs/<run_id>/`.
    pub relative_path: String,
    /// Artifact kind written at that path.
    pub artifact_kind: String,
    /// Retention class for that slot.
    pub retention_class: String,
    /// Short detail.
    pub detail: String,
}

/// Required provenance field for actual-lane retained artifacts.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionActualPretrainingProvenanceField {
    /// Field name that later launcher surfaces must emit.
    pub field_name: String,
    /// Retained artifact location where the field must appear.
    pub location: String,
    /// Whether the field is required.
    pub required: bool,
    /// Short detail.
    pub detail: String,
}

/// Retained-artifact redaction rule for the actual lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionActualPretrainingRedactionRule {
    /// Surface this rule applies to.
    pub retained_surface: String,
    /// Approved retained value classes.
    pub allowed_value_classes: Vec<String>,
    /// Forbidden retained value classes.
    pub forbidden_value_classes: Vec<String>,
    /// Short detail.
    pub detail: String,
}

/// Canonical output, evidence, provenance, and redaction contract for the actual lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionActualPretrainingEvidenceContract {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable contract identifier.
    pub contract_id: String,
    /// Stable actual-lane identifier.
    pub lane_id: String,
    /// Stable recipe identifier.
    pub recipe_id: String,
    /// Stable topology/storage bundle identifier.
    pub topology_storage_bundle_id: String,
    /// Canonical recipe bundle ref this contract depends on.
    pub recipe_bundle: PsionActualPretrainingArtifactRef,
    /// Canonical topology/storage bundle ref this contract depends on.
    pub topology_storage_bundle: PsionActualPretrainingArtifactRef,
    /// Stable retained evidence family.
    pub evidence_family: String,
    /// Stable run-root family.
    pub run_root_family: String,
    /// Example run id used by the retained output tree.
    pub example_run_id: String,
    /// Retained artifact slots under the run-root family.
    pub artifact_slots: Vec<PsionActualPretrainingArtifactSlot>,
    /// Required provenance fields.
    pub provenance_fields: Vec<PsionActualPretrainingProvenanceField>,
    /// Required redaction rules.
    pub redaction_rules: Vec<PsionActualPretrainingRedactionRule>,
    /// Short summary.
    pub summary: String,
}

impl PsionActualPretrainingEvidenceContract {
    /// Validates the contract.
    pub fn validate(&self) -> Result<(), PsionActualPretrainingEvidenceContractError> {
        ensure_nonempty(self.schema_version.as_str(), "contract.schema_version")?;
        if self.schema_version != PSION_ACTUAL_PRETRAINING_EVIDENCE_CONTRACT_SCHEMA_VERSION {
            return Err(PsionActualPretrainingEvidenceContractError::FieldMismatch {
                field: String::from("schema_version"),
                expected: String::from(PSION_ACTUAL_PRETRAINING_EVIDENCE_CONTRACT_SCHEMA_VERSION),
                actual: self.schema_version.clone(),
            });
        }
        if self.contract_id != PSION_ACTUAL_PRETRAINING_EVIDENCE_CONTRACT_ID {
            return Err(PsionActualPretrainingEvidenceContractError::FieldMismatch {
                field: String::from("contract_id"),
                expected: String::from(PSION_ACTUAL_PRETRAINING_EVIDENCE_CONTRACT_ID),
                actual: self.contract_id.clone(),
            });
        }
        if self.lane_id != PSION_ACTUAL_PRETRAINING_LANE_ID {
            return Err(PsionActualPretrainingEvidenceContractError::FieldMismatch {
                field: String::from("lane_id"),
                expected: String::from(PSION_ACTUAL_PRETRAINING_LANE_ID),
                actual: self.lane_id.clone(),
            });
        }
        if self.recipe_id != PSION_ACTUAL_PRETRAINING_RECIPE_ID {
            return Err(PsionActualPretrainingEvidenceContractError::FieldMismatch {
                field: String::from("recipe_id"),
                expected: String::from(PSION_ACTUAL_PRETRAINING_RECIPE_ID),
                actual: self.recipe_id.clone(),
            });
        }
        if self.topology_storage_bundle_id != PSION_ACTUAL_PRETRAINING_TOPOLOGY_STORAGE_BUNDLE_ID {
            return Err(PsionActualPretrainingEvidenceContractError::FieldMismatch {
                field: String::from("topology_storage_bundle_id"),
                expected: String::from(PSION_ACTUAL_PRETRAINING_TOPOLOGY_STORAGE_BUNDLE_ID),
                actual: self.topology_storage_bundle_id.clone(),
            });
        }
        ensure_artifact_ref(&self.recipe_bundle, "contract.recipe_bundle")?;
        ensure_artifact_ref(
            &self.topology_storage_bundle,
            "contract.topology_storage_bundle",
        )?;
        if self.evidence_family != PSION_ACTUAL_PRETRAINING_EVIDENCE_FAMILY {
            return Err(PsionActualPretrainingEvidenceContractError::FieldMismatch {
                field: String::from("evidence_family"),
                expected: String::from(PSION_ACTUAL_PRETRAINING_EVIDENCE_FAMILY),
                actual: self.evidence_family.clone(),
            });
        }
        if self.run_root_family != PSION_ACTUAL_PRETRAINING_RUN_ROOT_FAMILY {
            return Err(PsionActualPretrainingEvidenceContractError::FieldMismatch {
                field: String::from("run_root_family"),
                expected: String::from(PSION_ACTUAL_PRETRAINING_RUN_ROOT_FAMILY),
                actual: self.run_root_family.clone(),
            });
        }
        ensure_nonempty(self.example_run_id.as_str(), "contract.example_run_id")?;
        if self.artifact_slots.is_empty() {
            return Err(PsionActualPretrainingEvidenceContractError::MissingArtifactSlots);
        }
        for slot in &self.artifact_slots {
            ensure_nonempty(slot.relative_path.as_str(), "artifact_slot.relative_path")?;
            ensure_nonempty(slot.artifact_kind.as_str(), "artifact_slot.artifact_kind")?;
            ensure_nonempty(
                slot.retention_class.as_str(),
                "artifact_slot.retention_class",
            )?;
            ensure_nonempty(slot.detail.as_str(), "artifact_slot.detail")?;
        }
        for required_path in [
            "manifests/launch_manifest.json",
            "manifests/resume_manifest.json",
            "status/current_run_status.json",
            "status/retained_summary.json",
            "checkpoints/latest_accepted_checkpoint_pointer.json",
            "checkpoints/latest_accepted_checkpoint_backup_receipt.json",
            "checkpoints/auto_resume_receipt.json",
            "evals/latest_checkpoint_eval_decision.json",
            "evals/latest_checkpoint_eval_failure.json",
            "decisions/latest_checkpoint_comparison.json",
            "decisions/latest_continue_restart_decision.json",
            "dashboard/current_dashboard.json",
            "preflight/hardware_qualification.json",
            "preflight/run_shape_qualification.json",
            "alerts/latest_redacted_alert.json",
            "alerts/active_alerts.json",
            "closeout/closeout_bundle.json",
        ] {
            if !self
                .artifact_slots
                .iter()
                .any(|slot| slot.relative_path == required_path)
            {
                return Err(
                    PsionActualPretrainingEvidenceContractError::MissingArtifactPath {
                        relative_path: String::from(required_path),
                    },
                );
            }
        }
        if self.provenance_fields.is_empty() {
            return Err(PsionActualPretrainingEvidenceContractError::MissingProvenanceFields);
        }
        for field in &self.provenance_fields {
            ensure_nonempty(field.field_name.as_str(), "provenance.field_name")?;
            ensure_nonempty(field.location.as_str(), "provenance.location")?;
            ensure_nonempty(field.detail.as_str(), "provenance.detail")?;
        }
        for required_field in ["git_commit_sha", "selected_git_ref", "dirty_tree_admission"] {
            if !self
                .provenance_fields
                .iter()
                .any(|field| field.field_name == required_field && field.required)
            {
                return Err(
                    PsionActualPretrainingEvidenceContractError::MissingProvenanceRequirement {
                        field_name: String::from(required_field),
                    },
                );
            }
        }
        if self.redaction_rules.is_empty() {
            return Err(PsionActualPretrainingEvidenceContractError::MissingRedactionRules);
        }
        for rule in &self.redaction_rules {
            ensure_nonempty(rule.retained_surface.as_str(), "redaction.retained_surface")?;
            ensure_nonempty(rule.detail.as_str(), "redaction.detail")?;
            if rule.allowed_value_classes.is_empty() || rule.forbidden_value_classes.is_empty() {
                return Err(
                    PsionActualPretrainingEvidenceContractError::IncompleteRedactionRule {
                        retained_surface: rule.retained_surface.clone(),
                    },
                );
            }
        }
        ensure_nonempty(self.summary.as_str(), "contract.summary")?;
        Ok(())
    }
}

/// Validation errors for the actual-pretraining evidence contract.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum PsionActualPretrainingEvidenceContractError {
    #[error("psion actual-pretraining evidence field `{field}` must not be empty")]
    MissingField { field: String },
    #[error(
        "psion actual-pretraining evidence field `{field}` mismatch: expected `{expected}`, got `{actual}`"
    )]
    FieldMismatch {
        field: String,
        expected: String,
        actual: String,
    },
    #[error("psion actual-pretraining evidence contract is missing artifact slots")]
    MissingArtifactSlots,
    #[error(
        "psion actual-pretraining evidence contract is missing required artifact path `{relative_path}`"
    )]
    MissingArtifactPath { relative_path: String },
    #[error("psion actual-pretraining evidence contract is missing provenance fields")]
    MissingProvenanceFields,
    #[error(
        "psion actual-pretraining evidence contract is missing required provenance field `{field_name}`"
    )]
    MissingProvenanceRequirement { field_name: String },
    #[error("psion actual-pretraining evidence contract is missing redaction rules")]
    MissingRedactionRules,
    #[error(
        "psion actual-pretraining evidence contract has an incomplete redaction rule for `{retained_surface}`"
    )]
    IncompleteRedactionRule { retained_surface: String },
}

fn ensure_artifact_ref(
    artifact: &PsionActualPretrainingArtifactRef,
    field_prefix: &str,
) -> Result<(), PsionActualPretrainingEvidenceContractError> {
    ensure_nonempty(artifact.path.as_str(), &format!("{field_prefix}.path"))?;
    ensure_nonempty(artifact.sha256.as_str(), &format!("{field_prefix}.sha256"))?;
    Ok(())
}

fn ensure_nonempty(
    value: &str,
    field: &str,
) -> Result<(), PsionActualPretrainingEvidenceContractError> {
    if value.trim().is_empty() {
        return Err(PsionActualPretrainingEvidenceContractError::MissingField {
            field: String::from(field),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::PsionActualPretrainingEvidenceContract;

    fn evidence_contract() -> PsionActualPretrainingEvidenceContract {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/pretrain/psion_actual_pretraining_evidence_contract_v1.json"
        ))
        .expect("actual pretraining evidence contract fixture should parse")
    }

    #[test]
    fn actual_pretraining_evidence_contract_fixture_validates() {
        evidence_contract()
            .validate()
            .expect("actual pretraining evidence contract fixture should validate");
    }

    #[test]
    fn actual_pretraining_evidence_contract_rejects_missing_git_sha_requirement() {
        let mut contract = evidence_contract();
        contract
            .provenance_fields
            .retain(|field| field.field_name != "git_commit_sha");
        let error = contract
            .validate()
            .expect_err("missing git commit SHA requirement should be rejected");
        assert_eq!(
            error,
            super::PsionActualPretrainingEvidenceContractError::MissingProvenanceRequirement {
                field_name: String::from("git_commit_sha"),
            }
        );
    }
}
