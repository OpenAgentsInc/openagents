use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::contracts::{CadWarning, CadWarningCode, CadWarningSeverity};
use crate::hash::stable_hex_digest;
use crate::{CadError, CadResult};

/// Operation family that produced candidate geometry prior to repair.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TopologyRepairOperation {
    Boolean,
    Finishing,
}

/// Defect counters used by deterministic repair policy.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct TopologyDefectCounts {
    pub non_manifold_edges: u32,
    pub self_intersections: u32,
    pub sliver_faces: u32,
}

/// Deterministic repair actions selected by the repair policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TopologyRepairAction {
    WeldVertices,
    RebuildFaceLoops,
    MergeSliverFaces,
    ReorientShells,
}

/// Repair status for post-op topology healing.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TopologyRepairStatus {
    NoRepairNeeded,
    Repaired,
    FallbackKeptSource,
}

/// Input contract for deterministic topology repair.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TopologyRepairRequest {
    pub feature_id: String,
    pub operation: TopologyRepairOperation,
    pub source_geometry_hash: String,
    pub candidate_geometry_hash: String,
    pub defects_before: TopologyDefectCounts,
    pub allow_fallback: bool,
}

/// Repair receipt used by parity fixtures and downstream eval receipts.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TopologyRepairResult {
    pub feature_id: String,
    pub operation: TopologyRepairOperation,
    pub status: TopologyRepairStatus,
    pub geometry_hash: String,
    pub defects_before: TopologyDefectCounts,
    pub defects_after: TopologyDefectCounts,
    pub actions: Vec<TopologyRepairAction>,
    pub warnings: Vec<CadWarning>,
    pub repair_signature: String,
}

impl TopologyRepairRequest {
    pub fn validate(&self) -> CadResult<()> {
        if self.feature_id.trim().is_empty() {
            return Err(CadError::InvalidPrimitive {
                reason: "topology repair feature id must not be empty".to_string(),
            });
        }
        if self.source_geometry_hash.trim().is_empty()
            || self.candidate_geometry_hash.trim().is_empty()
        {
            return Err(CadError::InvalidPrimitive {
                reason: "topology repair geometry hashes must not be empty".to_string(),
            });
        }
        Ok(())
    }
}

pub fn repair_topology_after_operation(
    request: &TopologyRepairRequest,
) -> CadResult<TopologyRepairResult> {
    request.validate()?;

    if request.defects_before == TopologyDefectCounts::default() {
        return Ok(TopologyRepairResult {
            feature_id: request.feature_id.clone(),
            operation: request.operation,
            status: TopologyRepairStatus::NoRepairNeeded,
            geometry_hash: request.candidate_geometry_hash.clone(),
            defects_before: request.defects_before,
            defects_after: request.defects_before,
            actions: Vec::new(),
            warnings: Vec::new(),
            repair_signature: stable_hex_digest(
                format!(
                    "repair|feature={}|op={:?}|status=no_repair|hash={}",
                    request.feature_id, request.operation, request.candidate_geometry_hash
                )
                .as_bytes(),
            ),
        });
    }

    let budget = deterministic_budget(request);
    let mut defects_after = request.defects_before;
    let mut actions = Vec::new();

    if defects_after.non_manifold_edges > 0 {
        actions.push(TopologyRepairAction::WeldVertices);
        defects_after.non_manifold_edges = defects_after.non_manifold_edges.saturating_sub(budget);
    }
    if defects_after.self_intersections > 0 {
        actions.push(TopologyRepairAction::RebuildFaceLoops);
        defects_after.self_intersections = defects_after.self_intersections.saturating_sub(budget);
    }
    if defects_after.sliver_faces > 0 {
        actions.push(TopologyRepairAction::MergeSliverFaces);
        defects_after.sliver_faces = defects_after.sliver_faces.saturating_sub(budget);
    }
    if request.operation == TopologyRepairOperation::Finishing
        && (request.defects_before.non_manifold_edges > 0
            || request.defects_before.self_intersections > 0)
    {
        actions.push(TopologyRepairAction::ReorientShells);
    }

    let unrepaired_critical =
        defects_after.non_manifold_edges > 0 || defects_after.self_intersections > 0;
    if unrepaired_critical && request.allow_fallback {
        let warning = fallback_warning(request, defects_after);
        return Ok(TopologyRepairResult {
            feature_id: request.feature_id.clone(),
            operation: request.operation,
            status: TopologyRepairStatus::FallbackKeptSource,
            geometry_hash: request.source_geometry_hash.clone(),
            defects_before: request.defects_before,
            defects_after,
            actions,
            warnings: vec![warning],
            repair_signature: stable_hex_digest(
                format!(
                    "repair|feature={}|op={:?}|status=fallback|src={}|candidate={}|budget={}",
                    request.feature_id,
                    request.operation,
                    request.source_geometry_hash,
                    request.candidate_geometry_hash,
                    budget
                )
                .as_bytes(),
            ),
        });
    }

    let repaired_hash = stable_hex_digest(
        format!(
            "repair|feature={}|op={:?}|candidate={}|before={:?}|after={:?}|actions={:?}|budget={}",
            request.feature_id,
            request.operation,
            request.candidate_geometry_hash,
            request.defects_before,
            defects_after,
            actions,
            budget
        )
        .as_bytes(),
    )[..16]
        .to_string();

    let mut warnings = Vec::new();
    if defects_after.non_manifold_edges > 0 {
        warnings.push(fallback_warning(request, defects_after));
    }

    let repair_signature = stable_hex_digest(
        format!(
            "repair|feature={}|op={:?}|status=repaired|hash={}",
            request.feature_id, request.operation, repaired_hash
        )
        .as_bytes(),
    );

    Ok(TopologyRepairResult {
        feature_id: request.feature_id.clone(),
        operation: request.operation,
        status: TopologyRepairStatus::Repaired,
        geometry_hash: repaired_hash,
        defects_before: request.defects_before,
        defects_after,
        actions,
        warnings,
        repair_signature,
    })
}

fn deterministic_budget(request: &TopologyRepairRequest) -> u32 {
    let digest = stable_hex_digest(
        format!(
            "repair-budget|{}|{:?}|{}|{}",
            request.feature_id,
            request.operation,
            request.source_geometry_hash,
            request.candidate_geometry_hash
        )
        .as_bytes(),
    );
    let prefix = &digest[..8];
    let value = u32::from_str_radix(prefix, 16).unwrap_or(0);
    value % 3 + 1
}

fn fallback_warning(
    request: &TopologyRepairRequest,
    defects_after: TopologyDefectCounts,
) -> CadWarning {
    CadWarning {
        code: CadWarningCode::NonManifoldBody,
        severity: CadWarningSeverity::Warning,
        message: format!(
            "topology repair fallback kept source geometry for {} due to unresolved defects",
            request.feature_id
        ),
        remediation_hint:
            "Reduce operation complexity or simplify upstream boolean/finishing selections."
                .to_string(),
        semantic_refs: vec![request.feature_id.clone()],
        metadata: BTreeMap::from([
            ("operation".to_string(), format!("{:?}", request.operation)),
            (
                "non_manifold_edges_after".to_string(),
                defects_after.non_manifold_edges.to_string(),
            ),
            (
                "self_intersections_after".to_string(),
                defects_after.self_intersections.to_string(),
            ),
            (
                "sliver_faces_after".to_string(),
                defects_after.sliver_faces.to_string(),
            ),
        ]),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        TopologyDefectCounts, TopologyRepairAction, TopologyRepairOperation, TopologyRepairRequest,
        TopologyRepairStatus, repair_topology_after_operation,
    };
    use crate::contracts::CadWarningCode;
    use crate::{CadError, CadResult};

    fn request(defects_before: TopologyDefectCounts) -> TopologyRepairRequest {
        TopologyRepairRequest {
            feature_id: "feature.repair".to_string(),
            operation: TopologyRepairOperation::Boolean,
            source_geometry_hash: "hash.source".to_string(),
            candidate_geometry_hash: "hash.candidate".to_string(),
            defects_before,
            allow_fallback: true,
        }
    }

    #[test]
    fn no_defects_returns_no_repair_needed() -> CadResult<()> {
        let result = repair_topology_after_operation(&request(TopologyDefectCounts::default()))?;
        assert_eq!(result.status, TopologyRepairStatus::NoRepairNeeded);
        assert_eq!(result.geometry_hash, "hash.candidate");
        assert!(result.actions.is_empty());
        assert!(result.warnings.is_empty());
        Ok(())
    }

    #[test]
    fn boolean_repair_emits_deterministic_actions() -> CadResult<()> {
        let defects_before = TopologyDefectCounts {
            non_manifold_edges: 2,
            self_intersections: 1,
            sliver_faces: 3,
        };
        let result = repair_topology_after_operation(&request(defects_before))?;
        assert_eq!(result.status, TopologyRepairStatus::Repaired);
        assert!(result.actions.contains(&TopologyRepairAction::WeldVertices));
        assert!(
            result
                .actions
                .contains(&TopologyRepairAction::RebuildFaceLoops)
        );
        assert!(
            result
                .actions
                .contains(&TopologyRepairAction::MergeSliverFaces)
        );
        assert_eq!(result.feature_id, "feature.repair");
        let replay = repair_topology_after_operation(&request(defects_before))?;
        assert_eq!(result, replay);
        Ok(())
    }

    #[test]
    fn unresolved_critical_defects_fallback_to_source_when_allowed() -> CadResult<()> {
        let result = repair_topology_after_operation(&TopologyRepairRequest {
            feature_id: "feature.repair.fallback".to_string(),
            operation: TopologyRepairOperation::Finishing,
            source_geometry_hash: "hash.source".to_string(),
            candidate_geometry_hash: "hash.candidate".to_string(),
            defects_before: TopologyDefectCounts {
                non_manifold_edges: 10,
                self_intersections: 10,
                sliver_faces: 1,
            },
            allow_fallback: true,
        })?;
        assert_eq!(result.status, TopologyRepairStatus::FallbackKeptSource);
        assert_eq!(result.geometry_hash, "hash.source");
        assert_eq!(result.warnings.len(), 1);
        assert_eq!(
            result.warnings[0].code,
            CadWarningCode::NonManifoldBody,
            "fallback should emit CAD-WARN-NON-MANIFOLD"
        );
        Ok(())
    }

    #[test]
    fn invalid_request_rejects_empty_feature_id() {
        let error = repair_topology_after_operation(&TopologyRepairRequest {
            feature_id: String::new(),
            operation: TopologyRepairOperation::Boolean,
            source_geometry_hash: "hash.source".to_string(),
            candidate_geometry_hash: "hash.candidate".to_string(),
            defects_before: TopologyDefectCounts::default(),
            allow_fallback: true,
        })
        .expect_err("empty id should fail");
        assert!(matches!(error, CadError::InvalidPrimitive { .. }));
    }
}
