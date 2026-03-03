use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::hash::stable_hex_digest;
use crate::parity::scorecard::ParityScorecard;
use crate::{CadError, CadResult};

pub const PARITY_VIEWPORT_CAMERA_GIZMO_ISSUE_ID: &str = "VCAD-PARITY-093";
pub const VIEWPORT_CAMERA_GIZMO_REFERENCE_FIXTURE_PATH: &str =
    "crates/cad/parity/fixtures/viewport_camera_gizmo_vcad_reference.json";
const VIEWPORT_CAMERA_GIZMO_REFERENCE_FIXTURE_JSON: &str =
    include_str!("../../parity/fixtures/viewport_camera_gizmo_vcad_reference.json");

const CANONICAL_GRID_SNAP_INCREMENT_MM: f64 = 5.0;
const CANONICAL_GIZMO_MODES: [&str; 3] = ["translate", "rotate", "scale"];
const CANONICAL_SNAP_VIEWS: [(&str, [f64; 3]); 8] = [
    ("front", [0.0, 0.0, 80.0]),
    ("back", [0.0, 0.0, -80.0]),
    ("right", [80.0, 0.0, 0.0]),
    ("left", [-80.0, 0.0, 0.0]),
    ("top", [0.0, 80.0, 0.0]),
    ("bottom", [0.0, -80.0, 0.0]),
    ("iso", [50.0, 50.0, 50.0]),
    ("hero", [60.0, 45.0, 60.0]),
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ViewportCameraGizmoParityManifest {
    pub manifest_version: u64,
    pub issue_id: String,
    pub vcad_commit: String,
    pub openagents_commit: String,
    pub generated_from_scorecard: String,
    pub reference_fixture_path: String,
    pub reference_fixture_sha256: String,
    pub reference_source: String,
    pub reference_commit_match: bool,
    pub default_camera_match: bool,
    pub orbit_pan_zoom_contract_match: bool,
    pub pitch_clamp_contract_match: bool,
    pub snap_views_match: bool,
    pub gizmo_modes_match: bool,
    pub grid_snap_increment_match: bool,
    pub deterministic_replay_match: bool,
    pub default_camera: ViewportCameraSnapshot,
    pub interaction_snapshot: ViewportCameraInteractionSnapshot,
    pub snap_views: Vec<ViewportSnapViewSnapshot>,
    pub gizmo_modes: Vec<String>,
    pub grid_snap_increment_mm: f64,
    pub deterministic_signature: String,
    pub parity_contracts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct ViewportCameraGizmoReferenceFixture {
    manifest_version: u64,
    issue_id: String,
    vcad_commit: String,
    source: String,
    expected_default_camera: ViewportCameraExpectation,
    expected_pitch_clamp_deg: [f64; 2],
    expected_zoom_factors: ViewportZoomFactors,
    expected_snap_views: Vec<ViewportSnapViewSnapshot>,
    expected_gizmo_modes: Vec<String>,
    expected_grid_snap_increment_mm: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct ViewportCameraExpectation {
    azimuth_deg: f64,
    elevation_deg: f64,
    distance: f64,
    target_mm: [f64; 3],
    fov_deg: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct ViewportZoomFactors {
    zoom_in: f64,
    zoom_out: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct ViewportCameraGizmoSnapshot {
    default_camera: ViewportCameraSnapshot,
    interaction_snapshot: ViewportCameraInteractionSnapshot,
    snap_views: Vec<ViewportSnapViewSnapshot>,
    gizmo_modes: Vec<String>,
    grid_snap_increment_mm: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ViewportCameraSnapshot {
    pub azimuth_deg: f64,
    pub elevation_deg: f64,
    pub distance: f64,
    pub target_mm: [f64; 3],
    pub position_mm: [f64; 3],
    pub fov_deg: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ViewportCameraInteractionSnapshot {
    pub orbit_camera: ViewportCameraSnapshot,
    pub pan_target_mm: [f64; 3],
    pub distance_after_zoom_cycle: f64,
    pub pitch_after_max_up_deg: f64,
    pub pitch_after_max_down_deg: f64,
    pub reset_camera: ViewportCameraSnapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ViewportSnapViewSnapshot {
    pub view_id: String,
    pub position_mm: [f64; 3],
}

#[derive(Debug, Clone)]
struct CanonicalViewportCamera {
    azimuth_deg: f64,
    elevation_deg: f64,
    distance: f64,
    target_mm: [f64; 3],
    fov_deg: f64,
}

impl Default for CanonicalViewportCamera {
    fn default() -> Self {
        Self {
            azimuth_deg: 45.0,
            elevation_deg: 30.0,
            distance: 100.0,
            target_mm: [0.0, 0.0, 0.0],
            fov_deg: 60.0,
        }
    }
}

impl CanonicalViewportCamera {
    fn rotate_horizontal(&mut self, degrees: f64) {
        self.azimuth_deg += degrees;
    }

    fn rotate_vertical(&mut self, degrees: f64) {
        self.elevation_deg = (self.elevation_deg + degrees).clamp(-89.0, 89.0);
    }

    fn pan(&mut self, dx: f64, dy: f64) {
        let scale = self.distance * 0.01;
        let az = self.azimuth_deg.to_radians();
        let right_x = az.cos();
        let right_z = -az.sin();
        self.target_mm[0] += right_x * dx * scale;
        self.target_mm[2] += right_z * dx * scale;
        self.target_mm[1] -= dy * scale;
    }

    fn zoom(&mut self, factor: f64) {
        self.distance = (self.distance * factor).clamp(10.0, 1000.0);
    }

    fn reset(&mut self) {
        *self = Self::default();
    }

    fn position_mm(&self) -> [f64; 3] {
        let az = self.azimuth_deg.to_radians();
        let el = self.elevation_deg.to_radians();
        [
            self.target_mm[0] + self.distance * el.cos() * az.sin(),
            self.target_mm[1] + self.distance * el.sin(),
            self.target_mm[2] + self.distance * el.cos() * az.cos(),
        ]
    }

    fn snapshot(&self) -> ViewportCameraSnapshot {
        ViewportCameraSnapshot {
            azimuth_deg: self.azimuth_deg,
            elevation_deg: self.elevation_deg,
            distance: self.distance,
            target_mm: self.target_mm,
            position_mm: self.position_mm(),
            fov_deg: self.fov_deg,
        }
    }
}

pub fn build_viewport_camera_gizmo_parity_manifest(
    scorecard: &ParityScorecard,
    scorecard_path: &str,
) -> CadResult<ViewportCameraGizmoParityManifest> {
    let reference: ViewportCameraGizmoReferenceFixture =
        serde_json::from_str(VIEWPORT_CAMERA_GIZMO_REFERENCE_FIXTURE_JSON).map_err(|error| {
            CadError::ParseFailed {
                reason: format!("failed parsing viewport/camera/gizmo reference fixture: {error}"),
            }
        })?;

    let reference_fixture_sha256 =
        sha256_hex(VIEWPORT_CAMERA_GIZMO_REFERENCE_FIXTURE_JSON.as_bytes());
    let reference_commit_match = reference.vcad_commit == scorecard.vcad_commit;

    let snapshot = collect_snapshot(&reference.expected_zoom_factors);
    let replay_snapshot = collect_snapshot(&reference.expected_zoom_factors);
    let deterministic_replay_match = snapshot == replay_snapshot;

    let default_camera_match = default_camera_matches_reference(
        &snapshot.default_camera,
        &reference.expected_default_camera,
    );
    let orbit_pan_zoom_contract_match = orbit_pan_zoom_contract_holds(
        &snapshot,
        &reference.expected_default_camera,
        &reference.expected_zoom_factors,
    );
    let pitch_clamp_contract_match = pitch_clamp_matches_reference(
        &snapshot.interaction_snapshot,
        reference.expected_pitch_clamp_deg,
    );
    let snap_views_match = sorted_snap_views(snapshot.snap_views.clone())
        == sorted_snap_views(reference.expected_snap_views.clone());
    let gizmo_modes_match = sorted_strings(snapshot.gizmo_modes.clone())
        == sorted_strings(reference.expected_gizmo_modes.clone());
    let grid_snap_increment_match = approx_eq(
        snapshot.grid_snap_increment_mm,
        reference.expected_grid_snap_increment_mm,
        1e-9,
    );

    let deterministic_signature = parity_signature(
        &snapshot,
        reference_commit_match,
        default_camera_match,
        orbit_pan_zoom_contract_match,
        pitch_clamp_contract_match,
        snap_views_match,
        gizmo_modes_match,
        grid_snap_increment_match,
        deterministic_replay_match,
        &reference_fixture_sha256,
    );

    Ok(ViewportCameraGizmoParityManifest {
        manifest_version: 1,
        issue_id: PARITY_VIEWPORT_CAMERA_GIZMO_ISSUE_ID.to_string(),
        vcad_commit: scorecard.vcad_commit.clone(),
        openagents_commit: scorecard.openagents_commit.clone(),
        generated_from_scorecard: scorecard_path.to_string(),
        reference_fixture_path: VIEWPORT_CAMERA_GIZMO_REFERENCE_FIXTURE_PATH.to_string(),
        reference_fixture_sha256,
        reference_source: reference.source,
        reference_commit_match,
        default_camera_match,
        orbit_pan_zoom_contract_match,
        pitch_clamp_contract_match,
        snap_views_match,
        gizmo_modes_match,
        grid_snap_increment_match,
        deterministic_replay_match,
        default_camera: snapshot.default_camera,
        interaction_snapshot: snapshot.interaction_snapshot,
        snap_views: snapshot.snap_views,
        gizmo_modes: snapshot.gizmo_modes,
        grid_snap_increment_mm: snapshot.grid_snap_increment_mm,
        deterministic_signature,
        parity_contracts: vec![
            "default viewport camera baseline matches vcad orbit defaults (azimuth/elevation/distance/fov)"
                .to_string(),
            "camera orbit, pan, zoom and reset semantics are deterministic and replay-stable"
                .to_string(),
            "vertical orbit clamp matches vcad guard rails at [-89deg, 89deg]".to_string(),
            "orientation gizmo exposes vcad snap views (front/back/right/left/top/bottom/iso/hero)"
                .to_string(),
            "transform gizmo mode set and 5mm grid snap increment remain parity-locked".to_string(),
        ],
    })
}

fn collect_snapshot(zoom_factors: &ViewportZoomFactors) -> ViewportCameraGizmoSnapshot {
    let default_camera = CanonicalViewportCamera::default().snapshot();

    let mut camera = CanonicalViewportCamera::default();
    camera.rotate_horizontal(18.0);
    camera.rotate_vertical(-12.0);
    let orbit_camera = camera.snapshot();

    camera.pan(9.0, -4.0);
    let pan_target_mm = camera.target_mm;

    camera.zoom(zoom_factors.zoom_in);
    camera.zoom(zoom_factors.zoom_out);
    let distance_after_zoom_cycle = camera.distance;

    let mut clamp_camera = CanonicalViewportCamera::default();
    clamp_camera.rotate_vertical(1000.0);
    let pitch_after_max_up_deg = clamp_camera.elevation_deg;
    clamp_camera.rotate_vertical(-2000.0);
    let pitch_after_max_down_deg = clamp_camera.elevation_deg;

    camera.reset();
    let reset_camera = camera.snapshot();

    ViewportCameraGizmoSnapshot {
        default_camera,
        interaction_snapshot: ViewportCameraInteractionSnapshot {
            orbit_camera,
            pan_target_mm,
            distance_after_zoom_cycle,
            pitch_after_max_up_deg,
            pitch_after_max_down_deg,
            reset_camera,
        },
        snap_views: canonical_snap_views(),
        gizmo_modes: CANONICAL_GIZMO_MODES
            .iter()
            .map(|mode| (*mode).to_string())
            .collect(),
        grid_snap_increment_mm: CANONICAL_GRID_SNAP_INCREMENT_MM,
    }
}

fn canonical_snap_views() -> Vec<ViewportSnapViewSnapshot> {
    let mut views = CANONICAL_SNAP_VIEWS
        .iter()
        .map(|(view_id, position_mm)| ViewportSnapViewSnapshot {
            view_id: (*view_id).to_string(),
            position_mm: *position_mm,
        })
        .collect::<Vec<_>>();
    views.sort_by(|left, right| left.view_id.cmp(&right.view_id));
    views
}

fn default_camera_matches_reference(
    default_camera: &ViewportCameraSnapshot,
    expected: &ViewportCameraExpectation,
) -> bool {
    approx_eq(default_camera.azimuth_deg, expected.azimuth_deg, 1e-9)
        && approx_eq(default_camera.elevation_deg, expected.elevation_deg, 1e-9)
        && approx_eq(default_camera.distance, expected.distance, 1e-9)
        && approx_eq(default_camera.fov_deg, expected.fov_deg, 1e-9)
        && approx_vec3(default_camera.target_mm, expected.target_mm, 1e-9)
}

fn orbit_pan_zoom_contract_holds(
    snapshot: &ViewportCameraGizmoSnapshot,
    default_camera: &ViewportCameraExpectation,
    zoom_factors: &ViewportZoomFactors,
) -> bool {
    let interaction = &snapshot.interaction_snapshot;
    let zoom_round_trip_expected =
        default_camera.distance * zoom_factors.zoom_in * zoom_factors.zoom_out;
    let orbit_changed = !approx_eq(
        interaction.orbit_camera.azimuth_deg,
        default_camera.azimuth_deg,
        1e-9,
    ) && !approx_eq(
        interaction.orbit_camera.elevation_deg,
        default_camera.elevation_deg,
        1e-9,
    );
    let pan_changed = !approx_vec3(interaction.pan_target_mm, default_camera.target_mm, 1e-9);
    orbit_changed
        && pan_changed
        && approx_eq(
            interaction.distance_after_zoom_cycle,
            zoom_round_trip_expected,
            1e-9,
        )
        && default_camera_matches_reference(&interaction.reset_camera, default_camera)
}

fn pitch_clamp_matches_reference(
    interaction: &ViewportCameraInteractionSnapshot,
    expected_pitch_clamp_deg: [f64; 2],
) -> bool {
    approx_eq(
        interaction.pitch_after_max_down_deg,
        expected_pitch_clamp_deg[0],
        1e-9,
    ) && approx_eq(
        interaction.pitch_after_max_up_deg,
        expected_pitch_clamp_deg[1],
        1e-9,
    )
}

fn sorted_snap_views(mut views: Vec<ViewportSnapViewSnapshot>) -> Vec<ViewportSnapViewSnapshot> {
    views.sort_by(|left, right| left.view_id.cmp(&right.view_id));
    views
}

fn sorted_strings(mut values: Vec<String>) -> Vec<String> {
    values.sort();
    values
}

#[allow(clippy::too_many_arguments)]
fn parity_signature(
    snapshot: &ViewportCameraGizmoSnapshot,
    reference_commit_match: bool,
    default_camera_match: bool,
    orbit_pan_zoom_contract_match: bool,
    pitch_clamp_contract_match: bool,
    snap_views_match: bool,
    gizmo_modes_match: bool,
    grid_snap_increment_match: bool,
    deterministic_replay_match: bool,
    reference_fixture_sha256: &str,
) -> String {
    let payload = serde_json::to_vec(&(
        snapshot,
        reference_commit_match,
        default_camera_match,
        orbit_pan_zoom_contract_match,
        pitch_clamp_contract_match,
        snap_views_match,
        gizmo_modes_match,
        grid_snap_increment_match,
        deterministic_replay_match,
        reference_fixture_sha256,
    ))
    .expect("serialize viewport/camera/gizmo parity signature payload");
    stable_hex_digest(&payload)
}

fn approx_eq(left: f64, right: f64, epsilon: f64) -> bool {
    (left - right).abs() <= epsilon
}

fn approx_vec3(left: [f64; 3], right: [f64; 3], epsilon: f64) -> bool {
    approx_eq(left[0], right[0], epsilon)
        && approx_eq(left[1], right[1], epsilon)
        && approx_eq(left[2], right[2], epsilon)
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        PARITY_VIEWPORT_CAMERA_GIZMO_ISSUE_ID, ViewportZoomFactors,
        build_viewport_camera_gizmo_parity_manifest, collect_snapshot,
    };
    use crate::parity::scorecard::{
        ParityScorecard, ScorecardCurrent, ScorecardEvaluation, ScorecardThresholdProfile,
    };

    fn mock_scorecard() -> ParityScorecard {
        ParityScorecard {
            manifest_version: 1,
            issue_id: "VCAD-PARITY-005".to_string(),
            vcad_commit: "1b59e7948efcdb848d8dba6848785d57aa310e81".to_string(),
            openagents_commit: "openagents".to_string(),
            generated_from_gap_matrix: "gap".to_string(),
            current: ScorecardCurrent {
                docs_match_rate: 0.0,
                crates_match_rate: 0.0,
                commands_match_rate: 0.0,
                overall_match_rate: 0.0,
                docs_reference_count: 1,
                crates_reference_count: 1,
                commands_reference_count: 1,
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
    fn snapshot_is_deterministic_for_fixed_inputs() {
        let zoom_factors = ViewportZoomFactors {
            zoom_in: 0.8,
            zoom_out: 1.25,
        };
        let a = collect_snapshot(&zoom_factors);
        let b = collect_snapshot(&zoom_factors);
        assert_eq!(a, b);
    }

    #[test]
    fn viewport_camera_gizmo_manifest_matches_contract() {
        let scorecard = mock_scorecard();
        let manifest = build_viewport_camera_gizmo_parity_manifest(&scorecard, "scorecard")
            .expect("build viewport camera/gizmo parity manifest");
        assert_eq!(manifest.issue_id, PARITY_VIEWPORT_CAMERA_GIZMO_ISSUE_ID);
        assert!(manifest.reference_commit_match);
        assert!(manifest.default_camera_match);
        assert!(manifest.orbit_pan_zoom_contract_match);
        assert!(manifest.pitch_clamp_contract_match);
        assert!(manifest.snap_views_match);
        assert!(manifest.gizmo_modes_match);
        assert!(manifest.grid_snap_increment_match);
        assert!(manifest.deterministic_replay_match);
        assert_eq!(manifest.snap_views.len(), 8);
    }
}
