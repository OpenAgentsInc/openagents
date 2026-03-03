use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::parity::scorecard::ParityScorecard;
use crate::sketch::{CadSketchConstraint, CadSketchEntity, CadSketchModel, CadSketchPlane};
use crate::sketch_feature_ops::{
    SketchProfileFeatureKind, SketchProfileFeatureSpec, convert_sketch_profile_to_feature_node,
};
use crate::{CadError, CadResult};

pub const PARITY_SKETCH_LOFT_ISSUE_ID: &str = "VCAD-PARITY-050";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SketchLoftParityManifest {
    pub manifest_version: u64,
    pub issue_id: String,
    pub vcad_commit: String,
    pub openagents_commit: String,
    pub generated_from_scorecard: String,
    pub vcad_reference_sources: Vec<String>,
    pub standard_case: SketchLoftCaseSnapshot,
    pub closed_ring_case: SketchLoftCaseSnapshot,
    pub open_case: SketchLoftCaseSnapshot,
    pub profile_hash_order_stable: bool,
    pub empty_loft_profiles_error: String,
    pub duplicate_loft_profile_error: String,
    pub unsolved_constraint_error: String,
    pub deterministic_replay_match: bool,
    pub deterministic_signature: String,
    pub parity_contracts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SketchLoftCaseSnapshot {
    pub loft_profile_ids: Vec<String>,
    pub loft_closed: bool,
    pub profile_closed_loop: bool,
    pub warning_codes: Vec<String>,
    pub profile_bounds_mm: [f64; 4],
    pub profile_hash: String,
    pub node_hash: String,
}

pub fn build_sketch_loft_parity_manifest(
    scorecard: &ParityScorecard,
    scorecard_path: &str,
) -> CadResult<SketchLoftParityManifest> {
    let closed_model = closed_profile_model()?;
    let standard_spec = SketchProfileFeatureSpec {
        feature_id: "feature.sketch.loft.standard".to_string(),
        profile_id: "profile.loft.standard".to_string(),
        plane_id: "plane.front".to_string(),
        profile_entity_ids: vec![
            "entity.line.b".to_string(),
            "entity.line.d".to_string(),
            "entity.line.a".to_string(),
            "entity.line.c".to_string(),
        ],
        kind: SketchProfileFeatureKind::Loft,
        source_feature_id: None,
        depth_mm: None,
        revolve_angle_deg: None,
        axis_anchor_ids: None,
        sweep_path_entity_ids: None,
        sweep_twist_deg: None,
        sweep_scale_start: None,
        sweep_scale_end: None,
        loft_profile_ids: Some(vec!["profile.section.2".to_string()]),
        loft_closed: Some(false),
        tolerance_mm: Some(0.001),
    };
    let closed_ring_spec = SketchProfileFeatureSpec {
        feature_id: "feature.sketch.loft.closed".to_string(),
        profile_id: "profile.loft.closed".to_string(),
        plane_id: "plane.front".to_string(),
        profile_entity_ids: standard_spec.profile_entity_ids.clone(),
        kind: SketchProfileFeatureKind::Loft,
        source_feature_id: None,
        depth_mm: None,
        revolve_angle_deg: None,
        axis_anchor_ids: None,
        sweep_path_entity_ids: None,
        sweep_twist_deg: None,
        sweep_scale_start: None,
        sweep_scale_end: None,
        loft_profile_ids: Some(vec![
            "profile.section.2".to_string(),
            "profile.section.3".to_string(),
        ]),
        loft_closed: Some(true),
        tolerance_mm: Some(0.001),
    };

    let standard_conversion =
        convert_sketch_profile_to_feature_node(&closed_model, &standard_spec)?;
    let closed_ring_conversion =
        convert_sketch_profile_to_feature_node(&closed_model, &closed_ring_spec)?;
    let standard_case = case_snapshot(&standard_conversion)?;
    let closed_ring_case = case_snapshot(&closed_ring_conversion)?;

    let reversed_standard_spec = SketchProfileFeatureSpec {
        profile_entity_ids: standard_spec
            .profile_entity_ids
            .iter()
            .cloned()
            .rev()
            .collect(),
        ..standard_spec.clone()
    };
    let reversed_standard_conversion =
        convert_sketch_profile_to_feature_node(&closed_model, &reversed_standard_spec)?;
    let profile_hash_order_stable =
        standard_conversion.profile_hash == reversed_standard_conversion.profile_hash;

    let open_model = open_profile_model()?;
    let open_spec = SketchProfileFeatureSpec {
        feature_id: "feature.sketch.loft.open".to_string(),
        profile_id: "profile.loft.open".to_string(),
        plane_id: "plane.front".to_string(),
        profile_entity_ids: vec!["entity.line.open".to_string()],
        kind: SketchProfileFeatureKind::Loft,
        source_feature_id: None,
        depth_mm: None,
        revolve_angle_deg: None,
        axis_anchor_ids: None,
        sweep_path_entity_ids: None,
        sweep_twist_deg: None,
        sweep_scale_start: None,
        sweep_scale_end: None,
        loft_profile_ids: Some(vec!["profile.section.2".to_string()]),
        loft_closed: Some(false),
        tolerance_mm: Some(0.001),
    };
    let open_conversion = convert_sketch_profile_to_feature_node(&open_model, &open_spec)?;
    let open_case = case_snapshot(&open_conversion)?;

    let empty_loft_profiles_error = convert_sketch_profile_to_feature_node(
        &closed_model,
        &SketchProfileFeatureSpec {
            loft_profile_ids: Some(Vec::new()),
            ..standard_spec.clone()
        },
    )
    .expect_err("empty loft profile list must fail validation")
    .to_string();

    let duplicate_loft_profile_error = convert_sketch_profile_to_feature_node(
        &closed_model,
        &SketchProfileFeatureSpec {
            loft_profile_ids: Some(vec!["profile.loft.standard".to_string()]),
            ..standard_spec.clone()
        },
    )
    .expect_err("loft profile list containing primary profile must fail validation")
    .to_string();

    let unsolved_constraint_error = convert_sketch_profile_to_feature_node(
        &unsolved_profile_model()?,
        &SketchProfileFeatureSpec {
            feature_id: "feature.sketch.loft.unsolved".to_string(),
            profile_id: "profile.loft.unsolved".to_string(),
            plane_id: "plane.front".to_string(),
            profile_entity_ids: vec!["entity.line.unsolved".to_string()],
            kind: SketchProfileFeatureKind::Loft,
            source_feature_id: None,
            depth_mm: None,
            revolve_angle_deg: None,
            axis_anchor_ids: None,
            sweep_path_entity_ids: None,
            sweep_twist_deg: None,
            sweep_scale_start: None,
            sweep_scale_end: None,
            loft_profile_ids: Some(vec!["profile.section.2".to_string()]),
            loft_closed: Some(false),
            tolerance_mm: Some(0.001),
        },
    )
    .expect_err("unsolved constraints should block loft conversion")
    .to_string();

    let replay_manifest = {
        let closed_model = closed_profile_model()?;
        let standard_conversion =
            convert_sketch_profile_to_feature_node(&closed_model, &standard_spec)?;
        let closed_ring_conversion =
            convert_sketch_profile_to_feature_node(&closed_model, &closed_ring_spec)?;
        let open_model = open_profile_model()?;
        let open_conversion = convert_sketch_profile_to_feature_node(&open_model, &open_spec)?;
        (
            case_snapshot(&standard_conversion)?,
            case_snapshot(&closed_ring_conversion)?,
            case_snapshot(&open_conversion)?,
            profile_hash_order_stable,
            empty_loft_profiles_error.clone(),
            duplicate_loft_profile_error.clone(),
            unsolved_constraint_error.clone(),
        )
    };

    let deterministic_replay_match = standard_case == replay_manifest.0
        && closed_ring_case == replay_manifest.1
        && open_case == replay_manifest.2
        && profile_hash_order_stable == replay_manifest.3
        && empty_loft_profiles_error == replay_manifest.4
        && duplicate_loft_profile_error == replay_manifest.5
        && unsolved_constraint_error == replay_manifest.6;

    let deterministic_signature = parity_signature(
        &standard_case,
        &closed_ring_case,
        &open_case,
        profile_hash_order_stable,
        &empty_loft_profiles_error,
        &duplicate_loft_profile_error,
        &unsolved_constraint_error,
        deterministic_replay_match,
    );

    Ok(SketchLoftParityManifest {
        manifest_version: 1,
        issue_id: PARITY_SKETCH_LOFT_ISSUE_ID.to_string(),
        vcad_commit: scorecard.vcad_commit.clone(),
        openagents_commit: scorecard.openagents_commit.clone(),
        generated_from_scorecard: scorecard_path.to_string(),
        vcad_reference_sources: vec![
            "docs/features/sketch-operations.md (Loft operation + profile requirements)"
                .to_string(),
            "crates/vcad-kernel-sweep/src/loft.rs (multi-profile + closed option semantics)"
                .to_string(),
        ],
        standard_case,
        closed_ring_case,
        open_case,
        profile_hash_order_stable,
        empty_loft_profiles_error,
        duplicate_loft_profile_error,
        unsolved_constraint_error,
        deterministic_replay_match,
        deterministic_signature,
        parity_contracts: vec![
            "loft from sketch emits deterministic sketch.loft.v1 feature nodes with explicit secondary profile ids".to_string(),
            "closed loft flag is serialized deterministically and replay-stable".to_string(),
            "open profiles warn deterministically; invalid/unsolved loft inputs fail deterministically".to_string(),
        ],
    })
}

fn case_snapshot(
    conversion: &crate::sketch_feature_ops::SketchProfileFeatureConversion,
) -> CadResult<SketchLoftCaseSnapshot> {
    let loft_profile_ids = conversion
        .node
        .params
        .get("loft_profile_ids")
        .map(|value| {
            value
                .split(',')
                .filter(|segment| !segment.is_empty())
                .map(|segment| segment.to_string())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let loft_closed = conversion
        .node
        .params
        .get("loft_closed")
        .map(|value| value == "true")
        .unwrap_or(false);
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

    Ok(SketchLoftCaseSnapshot {
        loft_profile_ids,
        loft_closed,
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
        reason: format!("failed to serialize sketch loft parity payload: {error}"),
    })?;
    Ok(short_sha256(&bytes))
}

fn short_sha256(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())[..16].to_string()
}

fn parity_signature(
    standard_case: &SketchLoftCaseSnapshot,
    closed_ring_case: &SketchLoftCaseSnapshot,
    open_case: &SketchLoftCaseSnapshot,
    profile_hash_order_stable: bool,
    empty_loft_profiles_error: &str,
    duplicate_loft_profile_error: &str,
    unsolved_constraint_error: &str,
    deterministic_replay_match: bool,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(
        serde_json::to_vec(&(
            standard_case,
            closed_ring_case,
            open_case,
            profile_hash_order_stable,
            empty_loft_profiles_error,
            duplicate_loft_profile_error,
            unsolved_constraint_error,
            deterministic_replay_match,
        ))
        .expect("serialize sketch loft parity payload"),
    );
    format!("{:x}", hasher.finalize())[..16].to_string()
}

#[cfg(test)]
mod tests {
    use super::{PARITY_SKETCH_LOFT_ISSUE_ID, build_sketch_loft_parity_manifest};
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
    fn sketch_loft_manifest_reports_standard_closed_open_and_invalid_behaviors() {
        let manifest = build_sketch_loft_parity_manifest(&mock_scorecard(), "scorecard.json")
            .expect("build sketch loft parity manifest");
        assert_eq!(manifest.issue_id, PARITY_SKETCH_LOFT_ISSUE_ID);
        assert!(manifest.standard_case.profile_closed_loop);
        assert!(!manifest.standard_case.loft_closed);
        assert!(manifest.closed_ring_case.loft_closed);
        assert!(!manifest.open_case.profile_closed_loop);
        assert!(
            manifest
                .open_case
                .warning_codes
                .iter()
                .any(|code| code == "CAD-WARN-NON-MANIFOLD")
        );
        assert!(manifest.profile_hash_order_stable);
        assert!(
            manifest
                .empty_loft_profiles_error
                .contains("loft_profile_ids")
        );
        assert!(
            manifest
                .duplicate_loft_profile_error
                .contains("primary profile_id")
        );
        assert!(
            manifest
                .unsolved_constraint_error
                .contains("unsolved constraints")
        );
        assert!(manifest.deterministic_replay_match);
    }
}
