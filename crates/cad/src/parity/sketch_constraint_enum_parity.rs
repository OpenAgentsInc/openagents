use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::parity::scorecard::ParityScorecard;
use crate::sketch::{
    CadDimensionConstraintKind, CadSketchConstraint, CadSketchEntity, CadSketchModel,
    CadSketchPlane,
};
use crate::{CadError, CadResult};

pub const PARITY_SKETCH_CONSTRAINT_ENUM_ISSUE_ID: &str = "VCAD-PARITY-043";

const GEOMETRIC_KINDS: [&str; 15] = [
    "coincident",
    "point_on_line",
    "parallel",
    "perpendicular",
    "horizontal",
    "vertical",
    "tangent",
    "equal_length",
    "equal_radius",
    "concentric",
    "fixed",
    "point_on_circle",
    "line_through_center",
    "midpoint",
    "symmetric",
];

const DIMENSIONAL_KINDS: [&str; 8] = [
    "distance",
    "point_line_distance",
    "angle",
    "radius",
    "length",
    "horizontal_distance",
    "vertical_distance",
    "diameter",
];

const LEGACY_KINDS: [&str; 1] = ["dimension"];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SketchConstraintEnumParityManifest {
    pub manifest_version: u64,
    pub issue_id: String,
    pub vcad_commit: String,
    pub openagents_commit: String,
    pub generated_from_scorecard: String,
    pub vcad_reference_sources: Vec<String>,
    pub supported_geometric_kinds: Vec<String>,
    pub supported_dimensional_kinds: Vec<String>,
    pub legacy_kinds: Vec<String>,
    pub constraint_kind_summaries: Vec<ConstraintKindSummary>,
    pub solver_summary: ConstraintSolverCoverageSummary,
    pub sample_model_hash: String,
    pub solve_report_hash: String,
    pub deterministic_replay_match: bool,
    pub deterministic_signature: String,
    pub parity_contracts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConstraintKindSummary {
    pub kind: String,
    pub category: String,
    pub schema_supported: bool,
    pub solver_supported: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConstraintSolverCoverageSummary {
    pub solved_constraints: usize,
    pub unsolved_constraints: usize,
    pub unsupported_kind_warning_count: usize,
    pub warning_codes: Vec<String>,
}

pub fn build_sketch_constraint_enum_parity_manifest(
    scorecard: &ParityScorecard,
    scorecard_path: &str,
) -> CadResult<SketchConstraintEnumParityManifest> {
    let model = constraint_enum_model()?;
    let constraint_kind_summaries = build_kind_summaries(&model)?;
    let solver_summary = solve_summary(&model)?;
    let sample_model_hash = model_hash(&model)?;
    let solve_report_hash = solve_hash(&model)?;

    let replay_model = constraint_enum_model()?;
    let replay_summaries = build_kind_summaries(&replay_model)?;
    let replay_solver_summary = solve_summary(&replay_model)?;
    let replay_model_hash = model_hash(&replay_model)?;
    let replay_solve_hash = solve_hash(&replay_model)?;

    let deterministic_replay_match = constraint_kind_summaries == replay_summaries
        && solver_summary == replay_solver_summary
        && sample_model_hash == replay_model_hash
        && solve_report_hash == replay_solve_hash;

    let deterministic_signature = parity_signature(
        &constraint_kind_summaries,
        &solver_summary,
        &sample_model_hash,
        &solve_report_hash,
        deterministic_replay_match,
    );

    Ok(SketchConstraintEnumParityManifest {
        manifest_version: 1,
        issue_id: PARITY_SKETCH_CONSTRAINT_ENUM_ISSUE_ID.to_string(),
        vcad_commit: scorecard.vcad_commit.clone(),
        openagents_commit: scorecard.openagents_commit.clone(),
        generated_from_scorecard: scorecard_path.to_string(),
        vcad_reference_sources: vec![
            "docs/features/sketch-mode.md (Constraint Types table)".to_string(),
            "crates/vcad-kernel-constraints/src/constraint.rs (Constraint enum)".to_string(),
        ],
        supported_geometric_kinds: GEOMETRIC_KINDS
            .iter()
            .map(|kind| (*kind).to_string())
            .collect(),
        supported_dimensional_kinds: DIMENSIONAL_KINDS
            .iter()
            .map(|kind| (*kind).to_string())
            .collect(),
        legacy_kinds: LEGACY_KINDS
            .iter()
            .map(|kind| (*kind).to_string())
            .collect(),
        constraint_kind_summaries,
        solver_summary,
        sample_model_hash,
        solve_report_hash,
        deterministic_replay_match,
        deterministic_signature,
        parity_contracts: vec![
            "sketch constraint schema exposes full geometric + dimensional enum parity surface"
                .to_string(),
            "model-level validation enforces anchor/entity reference integrity for all constraint kinds"
                .to_string(),
            "deterministic solver reports unsupported kinds with stable warning diagnostics"
                .to_string(),
            "constraint enum sample corpus replays deterministically".to_string(),
        ],
    })
}

fn constraint_enum_model() -> CadResult<CadSketchModel> {
    let mut model = CadSketchModel::default();
    model.insert_plane(CadSketchPlane {
        id: "plane.front".to_string(),
        name: "Front".to_string(),
        origin_mm: [0.0, 0.0, 0.0],
        normal: [0.0, 0.0, 1.0],
        x_axis: [1.0, 0.0, 0.0],
        y_axis: [0.0, 1.0, 0.0],
    })?;

    model.insert_entity(CadSketchEntity::Line {
        id: "entity.line.a".to_string(),
        plane_id: "plane.front".to_string(),
        start_mm: [0.0, 0.0],
        end_mm: [20.0, 0.0],
        anchor_ids: [
            "anchor.line.a.start".to_string(),
            "anchor.line.a.end".to_string(),
        ],
        construction: false,
    })?;
    model.insert_entity(CadSketchEntity::Line {
        id: "entity.line.b".to_string(),
        plane_id: "plane.front".to_string(),
        start_mm: [0.0, 10.0],
        end_mm: [20.0, 10.0],
        anchor_ids: [
            "anchor.line.b.start".to_string(),
            "anchor.line.b.end".to_string(),
        ],
        construction: false,
    })?;
    model.insert_entity(CadSketchEntity::Line {
        id: "entity.line.c".to_string(),
        plane_id: "plane.front".to_string(),
        start_mm: [10.0, -10.0],
        end_mm: [10.0, 20.0],
        anchor_ids: [
            "anchor.line.c.start".to_string(),
            "anchor.line.c.end".to_string(),
        ],
        construction: false,
    })?;
    model.insert_entity(CadSketchEntity::Arc {
        id: "entity.arc.a".to_string(),
        plane_id: "plane.front".to_string(),
        center_mm: [40.0, 0.0],
        radius_mm: 5.0,
        start_deg: 0.0,
        end_deg: 180.0,
        anchor_ids: [
            "anchor.arc.a.center".to_string(),
            "anchor.arc.a.start".to_string(),
            "anchor.arc.a.end".to_string(),
        ],
        construction: false,
    })?;
    model.insert_entity(CadSketchEntity::Circle {
        id: "entity.circle.a".to_string(),
        plane_id: "plane.front".to_string(),
        center_mm: [40.0, 12.0],
        radius_mm: 5.0,
        anchor_ids: [
            "anchor.circle.a.center".to_string(),
            "anchor.circle.a.radius".to_string(),
        ],
        construction: false,
    })?;
    model.insert_entity(CadSketchEntity::Point {
        id: "entity.point.a".to_string(),
        plane_id: "plane.front".to_string(),
        position_mm: [5.0, 5.0],
        anchor_id: "anchor.point.a".to_string(),
        construction: false,
    })?;
    model.insert_entity(CadSketchEntity::Point {
        id: "entity.point.b".to_string(),
        plane_id: "plane.front".to_string(),
        position_mm: [15.0, 5.0],
        anchor_id: "anchor.point.b".to_string(),
        construction: false,
    })?;

    for constraint in constraint_enum_constraints() {
        model.insert_constraint(constraint)?;
    }

    Ok(model)
}

fn constraint_enum_constraints() -> Vec<CadSketchConstraint> {
    vec![
        CadSketchConstraint::Coincident {
            id: "constraint.enum.coincident".to_string(),
            first_anchor_id: "anchor.point.a".to_string(),
            second_anchor_id: "anchor.line.a.start".to_string(),
            tolerance_mm: Some(0.01),
        },
        CadSketchConstraint::PointOnLine {
            id: "constraint.enum.point_on_line".to_string(),
            point_anchor_id: "anchor.point.b".to_string(),
            line_entity_id: "entity.line.a".to_string(),
            tolerance_mm: Some(0.01),
        },
        CadSketchConstraint::Parallel {
            id: "constraint.enum.parallel".to_string(),
            first_line_entity_id: "entity.line.a".to_string(),
            second_line_entity_id: "entity.line.b".to_string(),
            tolerance_mm: Some(0.01),
        },
        CadSketchConstraint::Perpendicular {
            id: "constraint.enum.perpendicular".to_string(),
            first_line_entity_id: "entity.line.a".to_string(),
            second_line_entity_id: "entity.line.c".to_string(),
            tolerance_mm: Some(0.01),
        },
        CadSketchConstraint::Horizontal {
            id: "constraint.enum.horizontal".to_string(),
            line_entity_id: "entity.line.a".to_string(),
        },
        CadSketchConstraint::Vertical {
            id: "constraint.enum.vertical".to_string(),
            line_entity_id: "entity.line.c".to_string(),
        },
        CadSketchConstraint::Tangent {
            id: "constraint.enum.tangent".to_string(),
            line_entity_id: "entity.line.b".to_string(),
            arc_entity_id: "entity.arc.a".to_string(),
            at_anchor_id: Some("anchor.arc.a.start".to_string()),
            tolerance_mm: Some(0.01),
        },
        CadSketchConstraint::EqualLength {
            id: "constraint.enum.equal_length".to_string(),
            first_line_entity_id: "entity.line.a".to_string(),
            second_line_entity_id: "entity.line.b".to_string(),
            tolerance_mm: Some(0.01),
        },
        CadSketchConstraint::EqualRadius {
            id: "constraint.enum.equal_radius".to_string(),
            first_curve_entity_id: "entity.arc.a".to_string(),
            second_curve_entity_id: "entity.circle.a".to_string(),
            tolerance_mm: Some(0.01),
        },
        CadSketchConstraint::Concentric {
            id: "constraint.enum.concentric".to_string(),
            first_curve_entity_id: "entity.arc.a".to_string(),
            second_curve_entity_id: "entity.circle.a".to_string(),
            tolerance_mm: Some(0.01),
        },
        CadSketchConstraint::Fixed {
            id: "constraint.enum.fixed".to_string(),
            point_anchor_id: "anchor.point.a".to_string(),
            target_mm: [5.0, 5.0],
            tolerance_mm: Some(0.01),
        },
        CadSketchConstraint::PointOnCircle {
            id: "constraint.enum.point_on_circle".to_string(),
            point_anchor_id: "anchor.point.b".to_string(),
            circle_entity_id: "entity.circle.a".to_string(),
            tolerance_mm: Some(0.01),
        },
        CadSketchConstraint::LineThroughCenter {
            id: "constraint.enum.line_through_center".to_string(),
            line_entity_id: "entity.line.c".to_string(),
            circle_entity_id: "entity.circle.a".to_string(),
            tolerance_mm: Some(0.01),
        },
        CadSketchConstraint::Midpoint {
            id: "constraint.enum.midpoint".to_string(),
            midpoint_anchor_id: "anchor.point.a".to_string(),
            line_entity_id: "entity.line.b".to_string(),
            tolerance_mm: Some(0.01),
        },
        CadSketchConstraint::Symmetric {
            id: "constraint.enum.symmetric".to_string(),
            first_anchor_id: "anchor.point.a".to_string(),
            second_anchor_id: "anchor.point.b".to_string(),
            axis_line_entity_id: "entity.line.c".to_string(),
            tolerance_mm: Some(0.01),
        },
        CadSketchConstraint::Distance {
            id: "constraint.enum.distance".to_string(),
            first_anchor_id: "anchor.point.a".to_string(),
            second_anchor_id: "anchor.point.b".to_string(),
            target_mm: 10.0,
            tolerance_mm: Some(0.01),
        },
        CadSketchConstraint::PointLineDistance {
            id: "constraint.enum.point_line_distance".to_string(),
            point_anchor_id: "anchor.point.b".to_string(),
            line_entity_id: "entity.line.c".to_string(),
            target_mm: 5.0,
            tolerance_mm: Some(0.01),
        },
        CadSketchConstraint::Angle {
            id: "constraint.enum.angle".to_string(),
            first_line_entity_id: "entity.line.a".to_string(),
            second_line_entity_id: "entity.line.c".to_string(),
            target_deg: 90.0,
            tolerance_deg: Some(0.01),
        },
        CadSketchConstraint::Radius {
            id: "constraint.enum.radius".to_string(),
            curve_entity_id: "entity.circle.a".to_string(),
            target_mm: 5.0,
            tolerance_mm: Some(0.01),
        },
        CadSketchConstraint::Length {
            id: "constraint.enum.length".to_string(),
            line_entity_id: "entity.line.a".to_string(),
            target_mm: 20.0,
            tolerance_mm: Some(0.01),
        },
        CadSketchConstraint::HorizontalDistance {
            id: "constraint.enum.horizontal_distance".to_string(),
            point_anchor_id: "anchor.point.a".to_string(),
            target_mm: 5.0,
            tolerance_mm: Some(0.01),
        },
        CadSketchConstraint::VerticalDistance {
            id: "constraint.enum.vertical_distance".to_string(),
            point_anchor_id: "anchor.point.a".to_string(),
            target_mm: 5.0,
            tolerance_mm: Some(0.01),
        },
        CadSketchConstraint::Diameter {
            id: "constraint.enum.diameter".to_string(),
            circle_entity_id: "entity.circle.a".to_string(),
            target_mm: 10.0,
            tolerance_mm: Some(0.01),
        },
        CadSketchConstraint::Dimension {
            id: "constraint.enum.dimension.legacy".to_string(),
            entity_id: "entity.line.b".to_string(),
            dimension_kind: CadDimensionConstraintKind::Length,
            target_mm: 20.0,
            tolerance_mm: Some(0.01),
        },
    ]
}

fn build_kind_summaries(model: &CadSketchModel) -> CadResult<Vec<ConstraintKindSummary>> {
    model.validate()?;

    let mut seen = BTreeSet::<String>::new();
    for constraint in model.constraints.values() {
        seen.insert(constraint.kind_key().to_string());
    }

    let mut summaries = Vec::with_capacity(seen.len());
    for kind in seen {
        summaries.push(ConstraintKindSummary {
            category: kind_category(&kind).to_string(),
            schema_supported: true,
            solver_supported: solver_supported(&kind),
            kind,
        });
    }
    summaries.sort_by(|left, right| left.kind.cmp(&right.kind));
    Ok(summaries)
}

fn solve_summary(model: &CadSketchModel) -> CadResult<ConstraintSolverCoverageSummary> {
    let mut solve_model = model.clone();
    let report = solve_model.solve_constraints_deterministic()?;
    let unsupported_kind_warning_count = report
        .diagnostics
        .iter()
        .filter(|entry| entry.code == "SKETCH_CONSTRAINT_KIND_NOT_IMPLEMENTED")
        .count();
    let mut warning_codes = report
        .diagnostics
        .iter()
        .map(|entry| entry.code.clone())
        .collect::<Vec<_>>();
    warning_codes.sort();
    warning_codes.dedup();

    Ok(ConstraintSolverCoverageSummary {
        solved_constraints: report.solved_constraints,
        unsolved_constraints: report.unsolved_constraints,
        unsupported_kind_warning_count,
        warning_codes,
    })
}

fn model_hash(model: &CadSketchModel) -> CadResult<String> {
    let bytes = serde_json::to_vec(model).map_err(|error| CadError::ParseFailed {
        reason: format!("failed to serialize constraint enum model for parity hash: {error}"),
    })?;
    Ok(short_sha256(&bytes))
}

fn solve_hash(model: &CadSketchModel) -> CadResult<String> {
    let mut solve_model = model.clone();
    let report = solve_model.solve_constraints_deterministic()?;
    let bytes = serde_json::to_vec(&report).map_err(|error| CadError::ParseFailed {
        reason: format!("failed to serialize constraint solve report for parity hash: {error}"),
    })?;
    Ok(short_sha256(&bytes))
}

fn short_sha256(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())[..16].to_string()
}

fn kind_category(kind: &str) -> &'static str {
    if GEOMETRIC_KINDS.contains(&kind) {
        return "geometric";
    }
    if DIMENSIONAL_KINDS.contains(&kind) {
        return "dimensional";
    }
    "legacy"
}

fn solver_supported(kind: &str) -> bool {
    matches!(
        kind,
        "coincident" | "horizontal" | "vertical" | "tangent" | "length" | "radius" | "dimension"
    )
}

fn parity_signature(
    constraint_kind_summaries: &[ConstraintKindSummary],
    solver_summary: &ConstraintSolverCoverageSummary,
    sample_model_hash: &str,
    solve_report_hash: &str,
    deterministic_replay_match: bool,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(
        serde_json::to_vec(&(
            constraint_kind_summaries,
            solver_summary,
            sample_model_hash,
            solve_report_hash,
            deterministic_replay_match,
        ))
        .expect("serialize sketch constraint enum parity payload"),
    );
    format!("{:x}", hasher.finalize())[..16].to_string()
}

#[cfg(test)]
mod tests {
    use super::{
        PARITY_SKETCH_CONSTRAINT_ENUM_ISSUE_ID, build_sketch_constraint_enum_parity_manifest,
    };
    use crate::parity::scorecard::{
        ParityScorecard, ScorecardCurrent, ScorecardEvaluation, ScorecardThresholdProfile,
    };

    fn mock_scorecard() -> ParityScorecard {
        ParityScorecard {
            manifest_version: 1,
            issue_id: "VCAD-PARITY-005".to_string(),
            vcad_commit: "vcad".to_string(),
            openagents_commit: "openagents".to_string(),
            generated_from_gap_matrix: "gap".to_string(),
            current: ScorecardCurrent {
                docs_match_rate: 0.0,
                crates_match_rate: 0.0,
                commands_match_rate: 0.0,
                overall_match_rate: 0.0,
                docs_reference_count: 0,
                crates_reference_count: 0,
                commands_reference_count: 0,
            },
            threshold_profiles: vec![ScorecardThresholdProfile {
                profile_id: "phase_a_baseline_v1".to_string(),
                docs_match_rate_min: 0.0,
                crates_match_rate_min: 0.0,
                commands_match_rate_min: 0.0,
                overall_match_rate_min: 0.0,
            }],
            evaluations: vec![ScorecardEvaluation {
                profile_id: "phase_a_baseline_v1".to_string(),
                docs_pass: true,
                crates_pass: true,
                commands_pass: true,
                overall_pass: true,
                pass: true,
            }],
        }
    }

    #[test]
    fn build_manifest_tracks_constraint_enum_coverage() {
        let manifest =
            build_sketch_constraint_enum_parity_manifest(&mock_scorecard(), "scorecard.json")
                .expect("build sketch constraint enum parity manifest");
        assert_eq!(manifest.issue_id, PARITY_SKETCH_CONSTRAINT_ENUM_ISSUE_ID);
        assert_eq!(manifest.supported_geometric_kinds.len(), 15);
        assert_eq!(manifest.supported_dimensional_kinds.len(), 8);
        assert_eq!(manifest.legacy_kinds, vec!["dimension"]);
        assert_eq!(manifest.constraint_kind_summaries.len(), 24);
        assert!(
            manifest
                .solver_summary
                .warning_codes
                .iter()
                .any(|code| code == "SKETCH_CONSTRAINT_KIND_NOT_IMPLEMENTED")
        );
        assert!(manifest.deterministic_replay_match);
    }
}
