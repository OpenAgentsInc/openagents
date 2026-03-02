use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::CadResult;
use crate::kernel_primitives::{make_cube, make_cylinder};
use crate::kernel_topology::{FaceId, Orientation};
use crate::parity::scorecard::ParityScorecard;
use crate::sketch::{CadSketchPlane, CadSketchPlanePreset};

pub const PARITY_SKETCH_PLANE_ISSUE_ID: &str = "VCAD-PARITY-042";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SketchPlaneParityManifest {
    pub manifest_version: u64,
    pub issue_id: String,
    pub vcad_commit: String,
    pub openagents_commit: String,
    pub generated_from_scorecard: String,
    pub vcad_reference_sources: Vec<String>,
    pub supported_standard_planes: Vec<String>,
    pub preset_planes: Vec<SketchPlanePresetSummary>,
    pub planar_face_selection_cases: Vec<PlanarFaceSelectionCase>,
    pub non_planar_face_rejection: String,
    pub deterministic_replay_match: bool,
    pub deterministic_signature: String,
    pub parity_contracts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SketchPlanePresetSummary {
    pub preset: String,
    pub plane_id: String,
    pub name: String,
    pub origin_mm: [f64; 3],
    pub normal: [f64; 3],
    pub x_axis: [f64; 3],
    pub y_axis: [f64; 3],
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlanarFaceSelectionCase {
    pub case_id: String,
    pub face_ref: String,
    pub orientation: String,
    pub plane_id: String,
    pub origin_mm: [f64; 3],
    pub normal: [f64; 3],
    pub x_axis: [f64; 3],
    pub y_axis: [f64; 3],
}

pub fn build_sketch_plane_parity_manifest(
    scorecard: &ParityScorecard,
    scorecard_path: &str,
) -> CadResult<SketchPlaneParityManifest> {
    let supported_standard_planes = vec!["xy".to_string(), "xz".to_string(), "yz".to_string()];
    let preset_planes = build_preset_planes();
    let (planar_face_selection_cases, non_planar_face_rejection) =
        build_planar_face_selection_cases()?;

    let replay_presets = build_preset_planes();
    let (replay_cases, replay_non_planar_rejection) = build_planar_face_selection_cases()?;

    let deterministic_replay_match = preset_planes == replay_presets
        && planar_face_selection_cases == replay_cases
        && non_planar_face_rejection == replay_non_planar_rejection;

    let deterministic_signature = parity_signature(
        &supported_standard_planes,
        &preset_planes,
        &planar_face_selection_cases,
        &non_planar_face_rejection,
        deterministic_replay_match,
    );

    Ok(SketchPlaneParityManifest {
        manifest_version: 1,
        issue_id: PARITY_SKETCH_PLANE_ISSUE_ID.to_string(),
        vcad_commit: scorecard.vcad_commit.clone(),
        openagents_commit: scorecard.openagents_commit.clone(),
        generated_from_scorecard: scorecard_path.to_string(),
        vcad_reference_sources: vec![
            "docs/features/sketch-mode.md (Sketch Planes)".to_string(),
            "crates/vcad-cli/src/tui/sketch_mode.rs (plane axis presets + custom plane basis)"
                .to_string(),
        ],
        supported_standard_planes,
        preset_planes,
        planar_face_selection_cases,
        non_planar_face_rejection,
        deterministic_replay_match,
        deterministic_signature,
        parity_contracts: vec![
            "sketch plane presets support XY, XZ, and YZ with origin-centered vcad-compatible bases"
                .to_string(),
            "planar face selection resolves plane basis from BRep face surface records"
                .to_string(),
            "reversed planar face orientation flips sketch plane normal to match face orientation"
                .to_string(),
            "non-planar faces are rejected with deterministic parse diagnostics".to_string(),
        ],
    })
}

fn build_preset_planes() -> Vec<SketchPlanePresetSummary> {
    let presets = [
        CadSketchPlanePreset::Xy,
        CadSketchPlanePreset::Xz,
        CadSketchPlanePreset::Yz,
    ];
    let mut summaries = Vec::with_capacity(presets.len());
    for preset in presets {
        let plane = CadSketchPlane::from_preset(preset);
        summaries.push(SketchPlanePresetSummary {
            preset: preset.key().to_string(),
            plane_id: plane.id,
            name: plane.name,
            origin_mm: plane.origin_mm,
            normal: plane.normal,
            x_axis: plane.x_axis,
            y_axis: plane.y_axis,
        });
    }
    summaries
}

fn build_planar_face_selection_cases() -> CadResult<(Vec<PlanarFaceSelectionCase>, String)> {
    let cube = make_cube(40.0, 20.0, 10.0)?;

    let cube_face_one = plane_case_from_face_ref(&cube, "cube.face.1", "face.1")?;
    let cube_face_two = plane_case_from_face_ref(&cube, "cube.face.2", "face.2")?;

    let mut reversed_cube = cube.clone();
    reversed_cube
        .topology
        .faces
        .get_mut(&FaceId(1))
        .expect("cube should contain face.1")
        .orientation = Orientation::Reversed;
    let reversed_face_one =
        plane_case_from_face_ref(&reversed_cube, "cube.face.1.reversed", "face.1")?;

    let mut cases = vec![cube_face_one, cube_face_two, reversed_face_one];
    cases.sort_by(|left, right| left.case_id.cmp(&right.case_id));

    let cylinder = make_cylinder(8.0, 12.0, 24)?;
    let non_planar_face_rejection = match CadSketchPlane::from_planar_face(&cylinder, "face.1") {
        Ok(_) => "expected non-planar face rejection but selection succeeded".to_string(),
        Err(error) => error.to_string(),
    };

    Ok((cases, non_planar_face_rejection))
}

fn plane_case_from_face_ref(
    solid: &crate::kernel_primitives::BRepSolid,
    case_id: &str,
    face_ref: &str,
) -> CadResult<PlanarFaceSelectionCase> {
    let plane = CadSketchPlane::from_planar_face(solid, face_ref)?;
    let face_id = parse_face_ref(face_ref);
    let face = solid
        .topology
        .faces
        .get(&face_id)
        .expect("selected face should exist");

    Ok(PlanarFaceSelectionCase {
        case_id: case_id.to_string(),
        face_ref: face_ref.to_string(),
        orientation: orientation_label(face.orientation).to_string(),
        plane_id: plane.id,
        origin_mm: plane.origin_mm,
        normal: plane.normal,
        x_axis: plane.x_axis,
        y_axis: plane.y_axis,
    })
}

fn parse_face_ref(face_ref: &str) -> FaceId {
    let raw = face_ref
        .strip_prefix("face.")
        .expect("face ref should use face.<id> format");
    let parsed = raw.parse::<u64>().expect("face id should parse as u64");
    FaceId(parsed)
}

fn orientation_label(orientation: Orientation) -> &'static str {
    match orientation {
        Orientation::Forward => "forward",
        Orientation::Reversed => "reversed",
    }
}

fn parity_signature(
    supported_standard_planes: &[String],
    preset_planes: &[SketchPlanePresetSummary],
    planar_face_selection_cases: &[PlanarFaceSelectionCase],
    non_planar_face_rejection: &str,
    deterministic_replay_match: bool,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(
        serde_json::to_vec(&(
            supported_standard_planes,
            preset_planes,
            planar_face_selection_cases,
            non_planar_face_rejection,
            deterministic_replay_match,
        ))
        .expect("serialize sketch plane parity payload"),
    );
    format!("{:x}", hasher.finalize())[..16].to_string()
}

#[cfg(test)]
mod tests {
    use super::{
        PARITY_SKETCH_PLANE_ISSUE_ID, build_preset_planes, build_sketch_plane_parity_manifest,
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
    fn sketch_plane_preset_summary_tracks_standard_planes() {
        let presets = build_preset_planes();
        let labels = presets
            .iter()
            .map(|summary| summary.preset.as_str())
            .collect::<Vec<_>>();
        assert_eq!(labels, vec!["xy", "xz", "yz"]);
    }

    #[test]
    fn build_manifest_tracks_planar_face_selection_parity() {
        let manifest = build_sketch_plane_parity_manifest(&mock_scorecard(), "scorecard.json")
            .expect("build sketch plane parity manifest");
        assert_eq!(manifest.issue_id, PARITY_SKETCH_PLANE_ISSUE_ID);
        assert_eq!(manifest.supported_standard_planes, vec!["xy", "xz", "yz"]);
        assert_eq!(manifest.preset_planes.len(), 3);
        assert_eq!(manifest.planar_face_selection_cases.len(), 3);
        assert!(
            manifest
                .non_planar_face_rejection
                .contains("must reference a planar face")
        );
        assert!(manifest.deterministic_replay_match);
    }
}
