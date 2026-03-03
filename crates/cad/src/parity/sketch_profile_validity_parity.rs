use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::parity::scorecard::ParityScorecard;
use crate::sketch::{CadSketchConstraint, CadSketchEntity, CadSketchModel, CadSketchPlane};
use crate::sketch_feature_ops::{
    SketchProfileFeatureKind, SketchProfileFeatureSpec, convert_sketch_profile_to_feature_node,
};
use crate::{CadError, CadResult};

pub const PARITY_SKETCH_PROFILE_VALIDITY_ISSUE_ID: &str = "VCAD-PARITY-051";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SketchProfileValidityParityManifest {
    pub manifest_version: u64,
    pub issue_id: String,
    pub vcad_commit: String,
    pub openagents_commit: String,
    pub generated_from_scorecard: String,
    pub vcad_reference_sources: Vec<String>,
    pub closed_case: SketchProfileValidityCaseSnapshot,
    pub open_case: SketchProfileValidityCaseSnapshot,
    pub duplicate_profile_entity_error: String,
    pub degenerate_line_error: String,
    pub unknown_entity_error: String,
    pub unsolved_constraint_error: String,
    pub deterministic_replay_match: bool,
    pub deterministic_signature: String,
    pub parity_contracts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SketchProfileValidityCaseSnapshot {
    pub profile_closed_loop: bool,
    pub warning_codes: Vec<String>,
    pub profile_bounds_mm: [f64; 4],
    pub profile_hash: String,
    pub node_hash: String,
}

pub fn build_sketch_profile_validity_parity_manifest(
    scorecard: &ParityScorecard,
    scorecard_path: &str,
) -> CadResult<SketchProfileValidityParityManifest> {
    let closed_model = closed_profile_model()?;
    let closed_spec = SketchProfileFeatureSpec {
        feature_id: "feature.sketch.profile.valid.closed".to_string(),
        profile_id: "profile.valid.closed".to_string(),
        plane_id: "plane.front".to_string(),
        profile_entity_ids: vec![
            "entity.line.b".to_string(),
            "entity.line.d".to_string(),
            "entity.line.a".to_string(),
            "entity.line.c".to_string(),
        ],
        kind: SketchProfileFeatureKind::Extrude,
        source_feature_id: None,
        depth_mm: Some(6.0),
        revolve_angle_deg: None,
        axis_anchor_ids: None,
        sweep_path_entity_ids: None,
        sweep_twist_deg: None,
        sweep_scale_start: None,
        sweep_scale_end: None,
        loft_profile_ids: None,
        loft_closed: None,
        tolerance_mm: Some(0.001),
    };
    let closed_conversion = convert_sketch_profile_to_feature_node(&closed_model, &closed_spec)?;
    let closed_case = case_snapshot(&closed_conversion)?;

    let open_model = open_profile_model()?;
    let open_spec = SketchProfileFeatureSpec {
        feature_id: "feature.sketch.profile.valid.open".to_string(),
        profile_id: "profile.valid.open".to_string(),
        plane_id: "plane.front".to_string(),
        profile_entity_ids: vec!["entity.line.open".to_string()],
        kind: SketchProfileFeatureKind::Extrude,
        source_feature_id: None,
        depth_mm: Some(6.0),
        revolve_angle_deg: None,
        axis_anchor_ids: None,
        sweep_path_entity_ids: None,
        sweep_twist_deg: None,
        sweep_scale_start: None,
        sweep_scale_end: None,
        loft_profile_ids: None,
        loft_closed: None,
        tolerance_mm: Some(0.001),
    };
    let open_conversion = convert_sketch_profile_to_feature_node(&open_model, &open_spec)?;
    let open_case = case_snapshot(&open_conversion)?;

    let duplicate_profile_entity_error = convert_sketch_profile_to_feature_node(
        &closed_model,
        &SketchProfileFeatureSpec {
            profile_entity_ids: vec!["entity.line.a".to_string(), "entity.line.a".to_string()],
            ..closed_spec.clone()
        },
    )
    .expect_err("duplicate profile entities must fail validation")
    .to_string();

    let degenerate_line_error = convert_sketch_profile_to_feature_node(
        &degenerate_profile_model()?,
        &SketchProfileFeatureSpec {
            feature_id: "feature.sketch.profile.valid.degenerate".to_string(),
            profile_id: "profile.valid.degenerate".to_string(),
            plane_id: "plane.front".to_string(),
            profile_entity_ids: vec!["entity.line.degenerate".to_string()],
            kind: SketchProfileFeatureKind::Extrude,
            source_feature_id: None,
            depth_mm: Some(6.0),
            revolve_angle_deg: None,
            axis_anchor_ids: None,
            sweep_path_entity_ids: None,
            sweep_twist_deg: None,
            sweep_scale_start: None,
            sweep_scale_end: None,
            loft_profile_ids: None,
            loft_closed: None,
            tolerance_mm: Some(0.001),
        },
    )
    .expect_err("degenerate line profiles must fail conversion")
    .to_string();

    let unknown_entity_error = convert_sketch_profile_to_feature_node(
        &closed_model,
        &SketchProfileFeatureSpec {
            profile_entity_ids: vec!["entity.line.missing".to_string()],
            ..closed_spec.clone()
        },
    )
    .expect_err("unknown profile entity must fail conversion")
    .to_string();

    let unsolved_constraint_error = convert_sketch_profile_to_feature_node(
        &unsolved_profile_model()?,
        &SketchProfileFeatureSpec {
            feature_id: "feature.sketch.profile.valid.unsolved".to_string(),
            profile_id: "profile.valid.unsolved".to_string(),
            plane_id: "plane.front".to_string(),
            profile_entity_ids: vec!["entity.line.unsolved".to_string()],
            kind: SketchProfileFeatureKind::Extrude,
            source_feature_id: None,
            depth_mm: Some(6.0),
            revolve_angle_deg: None,
            axis_anchor_ids: None,
            sweep_path_entity_ids: None,
            sweep_twist_deg: None,
            sweep_scale_start: None,
            sweep_scale_end: None,
            loft_profile_ids: None,
            loft_closed: None,
            tolerance_mm: Some(0.001),
        },
    )
    .expect_err("unsolved constraints should block conversion")
    .to_string();

    let replay_manifest = {
        let closed_model = closed_profile_model()?;
        let closed_conversion =
            convert_sketch_profile_to_feature_node(&closed_model, &closed_spec)?;
        let open_model = open_profile_model()?;
        let open_conversion = convert_sketch_profile_to_feature_node(&open_model, &open_spec)?;
        (
            case_snapshot(&closed_conversion)?,
            case_snapshot(&open_conversion)?,
            duplicate_profile_entity_error.clone(),
            degenerate_line_error.clone(),
            unknown_entity_error.clone(),
            unsolved_constraint_error.clone(),
        )
    };

    let deterministic_replay_match = closed_case == replay_manifest.0
        && open_case == replay_manifest.1
        && duplicate_profile_entity_error == replay_manifest.2
        && degenerate_line_error == replay_manifest.3
        && unknown_entity_error == replay_manifest.4
        && unsolved_constraint_error == replay_manifest.5;

    let deterministic_signature = parity_signature(
        &closed_case,
        &open_case,
        &duplicate_profile_entity_error,
        &degenerate_line_error,
        &unknown_entity_error,
        &unsolved_constraint_error,
        deterministic_replay_match,
    );

    Ok(SketchProfileValidityParityManifest {
        manifest_version: 1,
        issue_id: PARITY_SKETCH_PROFILE_VALIDITY_ISSUE_ID.to_string(),
        vcad_commit: scorecard.vcad_commit.clone(),
        openagents_commit: scorecard.openagents_commit.clone(),
        generated_from_scorecard: scorecard_path.to_string(),
        vcad_reference_sources: vec![
            "docs/features/sketch-operations.md (profile edge cases + validation)".to_string(),
            "crates/vcad-kernel-sketch/src/profile.rs (closed/degenerate profile guards)"
                .to_string(),
        ],
        closed_case,
        open_case,
        duplicate_profile_entity_error,
        degenerate_line_error,
        unknown_entity_error,
        unsolved_constraint_error,
        deterministic_replay_match,
        deterministic_signature,
        parity_contracts: vec![
            "profile conversion rejects duplicate entity ids and degenerate line segments deterministically".to_string(),
            "open profiles emit deterministic CAD-WARN-NON-MANIFOLD warnings".to_string(),
            "unknown-entity and unsolved-constraint inputs fail conversion deterministically".to_string(),
        ],
    })
}

fn case_snapshot(
    conversion: &crate::sketch_feature_ops::SketchProfileFeatureConversion,
) -> CadResult<SketchProfileValidityCaseSnapshot> {
    let profile_closed_loop = conversion
        .node
        .params
        .get("profile_closed_loop")
        .map(|value| value == "true")
        .unwrap_or(false);
    let warning_codes = conversion
        .warnings
        .iter()
        .map(|warning| warning.code.stable_code().to_string())
        .collect::<Vec<_>>();
    let node_hash = stable_hash_json(&conversion.node)?;

    Ok(SketchProfileValidityCaseSnapshot {
        profile_closed_loop,
        warning_codes,
        profile_bounds_mm: conversion.profile_bounds_mm,
        profile_hash: conversion.profile_hash.clone(),
        node_hash,
    })
}

fn closed_profile_model() -> CadResult<CadSketchModel> {
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
        end_mm: [40.0, 0.0],
        anchor_ids: ["anchor.a.start".to_string(), "anchor.a.end".to_string()],
        construction: false,
    })?;
    model.insert_entity(CadSketchEntity::Line {
        id: "entity.line.b".to_string(),
        plane_id: "plane.front".to_string(),
        start_mm: [40.0, 0.0],
        end_mm: [40.0, 20.0],
        anchor_ids: ["anchor.b.start".to_string(), "anchor.b.end".to_string()],
        construction: false,
    })?;
    model.insert_entity(CadSketchEntity::Line {
        id: "entity.line.c".to_string(),
        plane_id: "plane.front".to_string(),
        start_mm: [40.0, 20.0],
        end_mm: [0.0, 20.0],
        anchor_ids: ["anchor.c.start".to_string(), "anchor.c.end".to_string()],
        construction: false,
    })?;
    model.insert_entity(CadSketchEntity::Line {
        id: "entity.line.d".to_string(),
        plane_id: "plane.front".to_string(),
        start_mm: [0.0, 20.0],
        end_mm: [0.0, 0.0],
        anchor_ids: ["anchor.d.start".to_string(), "anchor.d.end".to_string()],
        construction: false,
    })?;
    model.insert_constraint(CadSketchConstraint::Coincident {
        id: "constraint.closed.01".to_string(),
        first_anchor_id: "anchor.a.end".to_string(),
        second_anchor_id: "anchor.b.start".to_string(),
        tolerance_mm: Some(0.001),
    })?;
    model.insert_constraint(CadSketchConstraint::Coincident {
        id: "constraint.closed.02".to_string(),
        first_anchor_id: "anchor.b.end".to_string(),
        second_anchor_id: "anchor.c.start".to_string(),
        tolerance_mm: Some(0.001),
    })?;
    model.insert_constraint(CadSketchConstraint::Coincident {
        id: "constraint.closed.03".to_string(),
        first_anchor_id: "anchor.c.end".to_string(),
        second_anchor_id: "anchor.d.start".to_string(),
        tolerance_mm: Some(0.001),
    })?;
    model.insert_constraint(CadSketchConstraint::Coincident {
        id: "constraint.closed.04".to_string(),
        first_anchor_id: "anchor.d.end".to_string(),
        second_anchor_id: "anchor.a.start".to_string(),
        tolerance_mm: Some(0.001),
    })?;
    Ok(model)
}

fn open_profile_model() -> CadResult<CadSketchModel> {
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
        id: "entity.line.open".to_string(),
        plane_id: "plane.front".to_string(),
        start_mm: [0.0, 0.0],
        end_mm: [20.0, 0.0],
        anchor_ids: [
            "anchor.open.start".to_string(),
            "anchor.open.end".to_string(),
        ],
        construction: false,
    })?;
    Ok(model)
}

fn degenerate_profile_model() -> CadResult<CadSketchModel> {
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
        id: "entity.line.degenerate".to_string(),
        plane_id: "plane.front".to_string(),
        start_mm: [2.0, 3.0],
        end_mm: [2.5, 3.0],
        anchor_ids: [
            "anchor.degenerate.start".to_string(),
            "anchor.degenerate.end".to_string(),
        ],
        construction: false,
    })?;
    if let CadSketchEntity::Line {
        start_mm, end_mm, ..
    } = model
        .entities
        .get_mut("entity.line.degenerate")
        .ok_or_else(|| CadError::ParseFailed {
            reason: "missing degenerate line entity".to_string(),
        })?
    {
        *end_mm = *start_mm;
    } else {
        return Err(CadError::ParseFailed {
            reason: "expected line entity for degenerate profile fixture".to_string(),
        });
    }
    Ok(model)
}

fn unsolved_profile_model() -> CadResult<CadSketchModel> {
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
        id: "entity.line.unsolved".to_string(),
        plane_id: "plane.front".to_string(),
        start_mm: [0.0, 0.0],
        end_mm: [20.0, 0.0],
        anchor_ids: [
            "anchor.unsolved.start".to_string(),
            "anchor.unsolved.end".to_string(),
        ],
        construction: false,
    })?;
    model.insert_entity(CadSketchEntity::Point {
        id: "entity.point.unsolved".to_string(),
        plane_id: "plane.front".to_string(),
        position_mm: [8.0, 5.0],
        anchor_id: "anchor.point.unsolved".to_string(),
        construction: false,
    })?;
    model.insert_constraint(CadSketchConstraint::PointOnLine {
        id: "constraint.unsolved.point_on_line".to_string(),
        point_anchor_id: "anchor.point.unsolved".to_string(),
        line_entity_id: "entity.line.unsolved".to_string(),
        tolerance_mm: Some(0.001),
    })?;
    Ok(model)
}

fn stable_hash_json<T: Serialize>(value: &T) -> CadResult<String> {
    let bytes = serde_json::to_vec(value).map_err(|error| CadError::ParseFailed {
        reason: format!("failed to serialize sketch profile validity parity payload: {error}"),
    })?;
    Ok(short_sha256(&bytes))
}

fn short_sha256(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())[..16].to_string()
}

fn parity_signature(
    closed_case: &SketchProfileValidityCaseSnapshot,
    open_case: &SketchProfileValidityCaseSnapshot,
    duplicate_profile_entity_error: &str,
    degenerate_line_error: &str,
    unknown_entity_error: &str,
    unsolved_constraint_error: &str,
    deterministic_replay_match: bool,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(
        serde_json::to_vec(&(
            closed_case,
            open_case,
            duplicate_profile_entity_error,
            degenerate_line_error,
            unknown_entity_error,
            unsolved_constraint_error,
            deterministic_replay_match,
        ))
        .expect("serialize sketch profile validity parity payload"),
    );
    format!("{:x}", hasher.finalize())[..16].to_string()
}

#[cfg(test)]
mod tests {
    use super::{
        PARITY_SKETCH_PROFILE_VALIDITY_ISSUE_ID, build_sketch_profile_validity_parity_manifest,
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
    fn sketch_profile_validity_manifest_reports_valid_and_invalid_cases() {
        let manifest =
            build_sketch_profile_validity_parity_manifest(&mock_scorecard(), "scorecard.json")
                .expect("build sketch profile validity parity manifest");
        assert_eq!(manifest.issue_id, PARITY_SKETCH_PROFILE_VALIDITY_ISSUE_ID);
        assert!(manifest.closed_case.profile_closed_loop);
        assert!(manifest.closed_case.warning_codes.is_empty());
        assert!(!manifest.open_case.profile_closed_loop);
        assert!(
            manifest
                .open_case
                .warning_codes
                .iter()
                .any(|code| code == "CAD-WARN-NON-MANIFOLD")
        );
        assert!(
            manifest
                .duplicate_profile_entity_error
                .contains("must not contain duplicates")
        );
        assert!(manifest.degenerate_line_error.contains("degenerate"));
        assert!(
            manifest
                .unknown_entity_error
                .contains("unknown sketch entity")
        );
        assert!(
            manifest
                .unsolved_constraint_error
                .contains("unsolved constraints")
        );
        assert!(manifest.deterministic_replay_match);
    }
}
