use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::PsionicTrainRuntimeContractError;

/// Stable role labels for the first grouped-replica stage contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionicTrainGroupedReplicaStageRole {
    Ingress,
    Intermediate,
    Egress,
}

/// Machine-readable node-to-stage assignment for one grouped-replica worker.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionicTrainGroupedReplicaStageAssignment {
    /// Stable grouped-replica identifier shared across all stages in one replica.
    pub replica_id: String,
    /// Stable stage identifier for this worker.
    pub stage_id: String,
    /// Zero-based stage index within the grouped replica.
    pub stage_index: u32,
    /// Total number of stages in the grouped replica.
    pub stage_count: u32,
    /// Topological role of this stage.
    pub stage_role: PsionicTrainGroupedReplicaStageRole,
    /// Stable upstream stage identifier when this stage is not the ingress stage.
    pub upstream_stage_id: Option<String>,
    /// Stable downstream stage identifier when this stage is not the egress stage.
    pub downstream_stage_id: Option<String>,
    /// Stable digest over the grouped-stage assignment payload.
    pub assignment_digest: String,
}

impl PsionicTrainGroupedReplicaStageAssignment {
    /// Creates one grouped-replica stage assignment with the canonical digest populated.
    pub fn new(
        replica_id: impl Into<String>,
        stage_id: impl Into<String>,
        stage_index: u32,
        stage_count: u32,
        stage_role: PsionicTrainGroupedReplicaStageRole,
        upstream_stage_id: Option<String>,
        downstream_stage_id: Option<String>,
    ) -> Result<Self, PsionicTrainRuntimeContractError> {
        let mut assignment = Self {
            replica_id: replica_id.into(),
            stage_id: stage_id.into(),
            stage_index,
            stage_count,
            stage_role,
            upstream_stage_id,
            downstream_stage_id,
            assignment_digest: String::new(),
        };
        assignment.assignment_digest = assignment.stable_assignment_digest();
        assignment.validate("grouped_stage_assignment")?;
        Ok(assignment)
    }

    /// Computes the stable digest for the assignment payload.
    #[must_use]
    pub fn stable_assignment_digest(&self) -> String {
        let mut digest_basis = self.clone();
        digest_basis.assignment_digest.clear();
        let bytes = serde_json::to_vec(&digest_basis)
            .expect("grouped stage assignment should serialize for stable digest");
        let mut digest = Sha256::new();
        digest.update(b"psionic_train_grouped_replica_stage_assignment|");
        digest.update(&bytes);
        format!("{:x}", digest.finalize())
    }

    /// Validates the grouped stage assignment payload.
    pub fn validate(&self, field_prefix: &str) -> Result<(), PsionicTrainRuntimeContractError> {
        require_nonempty(
            self.replica_id.as_str(),
            format!("{field_prefix}.replica_id").as_str(),
        )?;
        require_nonempty(
            self.stage_id.as_str(),
            format!("{field_prefix}.stage_id").as_str(),
        )?;
        require_nonempty(
            self.assignment_digest.as_str(),
            format!("{field_prefix}.assignment_digest").as_str(),
        )?;
        if self.stage_count < 2 {
            return Err(PsionicTrainRuntimeContractError::InvalidValue {
                field: format!("{field_prefix}.stage_count"),
                detail: String::from(
                    "grouped-replica stage assignment requires at least two stages",
                ),
            });
        }
        if self.stage_index >= self.stage_count {
            return Err(PsionicTrainRuntimeContractError::InvalidValue {
                field: format!("{field_prefix}.stage_index"),
                detail: format!(
                    "stage_index {} must be smaller than stage_count {}",
                    self.stage_index, self.stage_count
                ),
            });
        }

        let expected_role = if self.stage_index == 0 {
            PsionicTrainGroupedReplicaStageRole::Ingress
        } else if self.stage_index + 1 == self.stage_count {
            PsionicTrainGroupedReplicaStageRole::Egress
        } else {
            PsionicTrainGroupedReplicaStageRole::Intermediate
        };
        if self.stage_role != expected_role {
            return Err(PsionicTrainRuntimeContractError::InvalidValue {
                field: format!("{field_prefix}.stage_role"),
                detail: format!(
                    "stage_role {:?} does not match stage_index {} within stage_count {}",
                    self.stage_role, self.stage_index, self.stage_count
                ),
            });
        }

        validate_optional_field(
            self.upstream_stage_id.as_deref(),
            field_prefix,
            "upstream_stage_id",
        )?;
        validate_optional_field(
            self.downstream_stage_id.as_deref(),
            field_prefix,
            "downstream_stage_id",
        )?;

        if self.stage_index == 0 {
            if self.upstream_stage_id.is_some() {
                return Err(PsionicTrainRuntimeContractError::InvalidValue {
                    field: format!("{field_prefix}.upstream_stage_id"),
                    detail: String::from("ingress stage must not declare an upstream_stage_id"),
                });
            }
        } else if self.upstream_stage_id.is_none() {
            return Err(PsionicTrainRuntimeContractError::MissingField {
                field: format!("{field_prefix}.upstream_stage_id"),
            });
        }

        if self.stage_index + 1 == self.stage_count {
            if self.downstream_stage_id.is_some() {
                return Err(PsionicTrainRuntimeContractError::InvalidValue {
                    field: format!("{field_prefix}.downstream_stage_id"),
                    detail: String::from("egress stage must not declare a downstream_stage_id"),
                });
            }
        } else if self.downstream_stage_id.is_none() {
            return Err(PsionicTrainRuntimeContractError::MissingField {
                field: format!("{field_prefix}.downstream_stage_id"),
            });
        }

        if self.upstream_stage_id.as_deref() == Some(self.stage_id.as_str()) {
            return Err(PsionicTrainRuntimeContractError::InvalidValue {
                field: format!("{field_prefix}.upstream_stage_id"),
                detail: String::from("stage_id must not equal upstream_stage_id"),
            });
        }
        if self.downstream_stage_id.as_deref() == Some(self.stage_id.as_str()) {
            return Err(PsionicTrainRuntimeContractError::InvalidValue {
                field: format!("{field_prefix}.downstream_stage_id"),
                detail: String::from("stage_id must not equal downstream_stage_id"),
            });
        }
        if self.upstream_stage_id.is_some() && self.upstream_stage_id == self.downstream_stage_id {
            return Err(PsionicTrainRuntimeContractError::InvalidValue {
                field: format!("{field_prefix}.downstream_stage_id"),
                detail: String::from(
                    "upstream_stage_id and downstream_stage_id must not collapse to the same stage",
                ),
            });
        }
        if self.assignment_digest != self.stable_assignment_digest() {
            return Err(PsionicTrainRuntimeContractError::FieldMismatch {
                field: format!("{field_prefix}.assignment_digest"),
                expected: self.stable_assignment_digest(),
                actual: self.assignment_digest.clone(),
            });
        }
        Ok(())
    }
}

fn require_nonempty(value: &str, field: &str) -> Result<(), PsionicTrainRuntimeContractError> {
    if value.trim().is_empty() {
        return Err(PsionicTrainRuntimeContractError::MissingField {
            field: field.to_string(),
        });
    }
    Ok(())
}

fn validate_optional_field(
    value: Option<&str>,
    field_prefix: &str,
    field_name: &str,
) -> Result<(), PsionicTrainRuntimeContractError> {
    if let Some(value) = value {
        require_nonempty(value, format!("{field_prefix}.{field_name}").as_str())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{PsionicTrainGroupedReplicaStageAssignment, PsionicTrainGroupedReplicaStageRole};
    use crate::PsionicTrainRuntimeContractError;

    #[test]
    fn grouped_stage_assignment_round_trips_with_digest() {
        let assignment = PsionicTrainGroupedReplicaStageAssignment::new(
            "replica-01",
            "stage-01",
            0,
            3,
            PsionicTrainGroupedReplicaStageRole::Ingress,
            None,
            Some(String::from("stage-02")),
        )
        .expect("assignment should build");
        assignment
            .validate("grouped_stage_assignment")
            .expect("assignment should validate");
    }

    #[test]
    fn grouped_stage_assignment_requires_ingress_without_upstream() {
        let error = PsionicTrainGroupedReplicaStageAssignment::new(
            "replica-01",
            "stage-01",
            0,
            2,
            PsionicTrainGroupedReplicaStageRole::Ingress,
            Some(String::from("stage-00")),
            Some(String::from("stage-02")),
        )
        .expect_err("ingress stage must not admit upstream stage id");
        assert_eq!(
            error,
            PsionicTrainRuntimeContractError::InvalidValue {
                field: String::from("grouped_stage_assignment.upstream_stage_id"),
                detail: String::from("ingress stage must not declare an upstream_stage_id"),
            }
        );
    }

    #[test]
    fn grouped_stage_assignment_requires_intermediate_role_consistency() {
        let error = PsionicTrainGroupedReplicaStageAssignment::new(
            "replica-01",
            "stage-02",
            1,
            3,
            PsionicTrainGroupedReplicaStageRole::Egress,
            Some(String::from("stage-01")),
            Some(String::from("stage-03")),
        )
        .expect_err("middle stage role must match topology");
        assert_eq!(
            error,
            PsionicTrainRuntimeContractError::InvalidValue {
                field: String::from("grouped_stage_assignment.stage_role"),
                detail: String::from(
                    "stage_role Egress does not match stage_index 1 within stage_count 3",
                ),
            }
        );
    }
}
