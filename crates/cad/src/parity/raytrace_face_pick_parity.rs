use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::hash::stable_hex_digest;
use crate::parity::scorecard::ParityScorecard;
use crate::{CadError, CadResult};

pub const PARITY_RAYTRACE_FACE_PICK_ISSUE_ID: &str = "VCAD-PARITY-102";
pub const RAYTRACE_FACE_PICK_REFERENCE_FIXTURE_PATH: &str =
    "crates/cad/parity/fixtures/raytrace_face_pick_vcad_reference.json";
const RAYTRACE_FACE_PICK_REFERENCE_FIXTURE_JSON: &str =
    include_str!("../../parity/fixtures/raytrace_face_pick_vcad_reference.json");

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RaytraceFacePickParityManifest {
    pub manifest_version: u64,
    pub issue_id: String,
    pub vcad_commit: String,
    pub openagents_commit: String,
    pub generated_from_scorecard: String,
    pub reference_fixture_path: String,
    pub reference_fixture_sha256: String,
    pub reference_source: String,
    pub reference_commit_match: bool,
    pub input_validation_match: bool,
    pub scene_guard_match: bool,
    pub sample_set_match: bool,
    pub stub_background_match: bool,
    pub ndc_mapping_match: bool,
    pub ray_direction_normalized_match: bool,
    pub deterministic_replay_match: bool,
    pub error_samples: Vec<RaytraceFacePickErrorSample>,
    pub pick_samples: Vec<RaytraceFacePickSample>,
    pub deterministic_signature: String,
    pub parity_contracts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct RaytraceFacePickReferenceFixture {
    manifest_version: u64,
    issue_id: String,
    vcad_commit: String,
    source: String,
    expected_error_samples: Vec<RaytraceFacePickErrorSample>,
    expected_pick_samples: Vec<RaytraceFacePickSample>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct RaytraceFacePickSnapshot {
    error_samples: Vec<RaytraceFacePickErrorSample>,
    pick_samples: Vec<RaytraceFacePickSample>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RaytraceFacePickErrorSample {
    pub case_id: String,
    pub error: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RaytraceFacePickSample {
    pub case_id: String,
    pub camera: [f64; 3],
    pub target: [f64; 3],
    pub up: [f64; 3],
    pub width: u32,
    pub height: u32,
    pub fov_radians: f64,
    pub pixel: [u32; 2],
    pub ndc: [f64; 2],
    pub ray_direction: [f64; 3],
    pub face_index: i32,
}

pub fn build_raytrace_face_pick_parity_manifest(
    scorecard: &ParityScorecard,
    scorecard_path: &str,
) -> CadResult<RaytraceFacePickParityManifest> {
    let reference: RaytraceFacePickReferenceFixture =
        serde_json::from_str(RAYTRACE_FACE_PICK_REFERENCE_FIXTURE_JSON).map_err(|error| {
            CadError::ParseFailed {
                reason: format!("failed parsing raytrace face-pick reference fixture: {error}"),
            }
        })?;

    let reference_fixture_sha256 = sha256_hex(RAYTRACE_FACE_PICK_REFERENCE_FIXTURE_JSON.as_bytes());
    let reference_commit_match = reference.vcad_commit == scorecard.vcad_commit;

    let snapshot = collect_snapshot();
    let replay_snapshot = collect_snapshot();
    let deterministic_replay_match = snapshot == replay_snapshot;

    let input_validation_match =
        validation_error_matches(&snapshot.error_samples, &reference.expected_error_samples);
    let scene_guard_match = snapshot
        .error_samples
        .iter()
        .any(|sample| sample.error == "No solid uploaded. Call uploadSolid() first.");
    let sample_set_match = pick_samples_match(
        snapshot.pick_samples.clone(),
        reference.expected_pick_samples.clone(),
        1e-9,
    );
    let stub_background_match = snapshot
        .pick_samples
        .iter()
        .all(|sample| sample.face_index == -1);
    let ndc_mapping_match = pick_samples_match(
        snapshot.pick_samples.clone(),
        reference.expected_pick_samples.clone(),
        1e-9,
    );
    let ray_direction_normalized_match = snapshot.pick_samples.iter().all(|sample| {
        let len = length3(sample.ray_direction);
        approx_eq(len, 1.0, 1e-9)
    });

    let deterministic_signature = parity_signature(
        &snapshot,
        reference_commit_match,
        input_validation_match,
        scene_guard_match,
        sample_set_match,
        stub_background_match,
        ndc_mapping_match,
        ray_direction_normalized_match,
        deterministic_replay_match,
        &reference_fixture_sha256,
    );

    Ok(RaytraceFacePickParityManifest {
        manifest_version: 1,
        issue_id: PARITY_RAYTRACE_FACE_PICK_ISSUE_ID.to_string(),
        vcad_commit: scorecard.vcad_commit.clone(),
        openagents_commit: scorecard.openagents_commit.clone(),
        generated_from_scorecard: scorecard_path.to_string(),
        reference_fixture_path: RAYTRACE_FACE_PICK_REFERENCE_FIXTURE_PATH.to_string(),
        reference_fixture_sha256,
        reference_source: reference.source,
        reference_commit_match,
        input_validation_match,
        scene_guard_match,
        sample_set_match,
        stub_background_match,
        ndc_mapping_match,
        ray_direction_normalized_match,
        deterministic_replay_match,
        error_samples: snapshot.error_samples,
        pick_samples: snapshot.pick_samples,
        deterministic_signature,
        parity_contracts: vec![
            "pick validates camera/target/up vectors are each length 3 and returns explicit error on mismatch"
                .to_string(),
            "pick returns explicit guard error when no solid is uploaded".to_string(),
            "pixel-to-NDC mapping uses pixel-center offset (+0.5) and top-left origin"
                .to_string(),
            "ray direction uses forward/right/up basis and remains normalized".to_string(),
            "current vcad wasm baseline returns -1 (background) for all picks while CPU face-pick path is TODO"
                .to_string(),
        ],
    })
}

fn collect_snapshot() -> RaytraceFacePickSnapshot {
    let mut error_samples = vec![
        error_sample(
            "invalid_camera_len",
            simulate_pick(PickInput {
                scene_uploaded: true,
                camera: vec![0.0, 0.0],
                target: vec![0.0, 0.0, -1.0],
                up: vec![0.0, 1.0, 0.0],
                width: 800,
                height: 600,
                fov_radians: std::f64::consts::FRAC_PI_3,
                pixel_x: 400,
                pixel_y: 300,
            }),
        ),
        error_sample(
            "invalid_target_len",
            simulate_pick(PickInput {
                scene_uploaded: true,
                camera: vec![0.0, 0.0, 5.0],
                target: vec![0.0, 0.0],
                up: vec![0.0, 1.0, 0.0],
                width: 800,
                height: 600,
                fov_radians: std::f64::consts::FRAC_PI_3,
                pixel_x: 400,
                pixel_y: 300,
            }),
        ),
        error_sample(
            "invalid_up_len",
            simulate_pick(PickInput {
                scene_uploaded: true,
                camera: vec![0.0, 0.0, 5.0],
                target: vec![0.0, 0.0, 0.0],
                up: vec![0.0, 1.0],
                width: 800,
                height: 600,
                fov_radians: std::f64::consts::FRAC_PI_3,
                pixel_x: 400,
                pixel_y: 300,
            }),
        ),
        error_sample(
            "scene_not_uploaded",
            simulate_pick(PickInput {
                scene_uploaded: false,
                camera: vec![0.0, 0.0, 5.0],
                target: vec![0.0, 0.0, 0.0],
                up: vec![0.0, 1.0, 0.0],
                width: 800,
                height: 600,
                fov_radians: std::f64::consts::FRAC_PI_3,
                pixel_x: 400,
                pixel_y: 300,
            }),
        ),
    ];
    error_samples.sort_by(|left, right| left.case_id.cmp(&right.case_id));

    let mut pick_samples = vec![
        pick_sample(
            "center_pixel",
            PickInput {
                scene_uploaded: true,
                camera: vec![0.0, 0.0, 5.0],
                target: vec![0.0, 0.0, 0.0],
                up: vec![0.0, 1.0, 0.0],
                width: 800,
                height: 600,
                fov_radians: std::f64::consts::FRAC_PI_3,
                pixel_x: 399,
                pixel_y: 299,
            },
        ),
        pick_sample(
            "top_left_pixel",
            PickInput {
                scene_uploaded: true,
                camera: vec![10.0, 8.0, 6.0],
                target: vec![0.0, 0.0, 0.0],
                up: vec![0.0, 0.0, 1.0],
                width: 1920,
                height: 1080,
                fov_radians: 0.785398163,
                pixel_x: 0,
                pixel_y: 0,
            },
        ),
        pick_sample(
            "bottom_right_pixel",
            PickInput {
                scene_uploaded: true,
                camera: vec![10.0, 8.0, 6.0],
                target: vec![0.0, 0.0, 0.0],
                up: vec![0.0, 0.0, 1.0],
                width: 1920,
                height: 1080,
                fov_radians: 0.785398163,
                pixel_x: 1919,
                pixel_y: 1079,
            },
        ),
    ];
    pick_samples.sort_by(|left, right| left.case_id.cmp(&right.case_id));

    RaytraceFacePickSnapshot {
        error_samples,
        pick_samples,
    }
}

#[derive(Debug, Clone)]
struct PickInput {
    scene_uploaded: bool,
    camera: Vec<f64>,
    target: Vec<f64>,
    up: Vec<f64>,
    width: u32,
    height: u32,
    fov_radians: f64,
    pixel_x: u32,
    pixel_y: u32,
}

#[derive(Debug, Clone)]
struct PickSuccess {
    ndc: [f64; 2],
    ray_direction: [f64; 3],
    face_index: i32,
}

fn simulate_pick(input: PickInput) -> Result<PickSuccess, String> {
    if input.camera.len() != 3 || input.target.len() != 3 || input.up.len() != 3 {
        return Err("camera, target, and up must each have 3 components".to_string());
    }

    if !input.scene_uploaded {
        return Err("No solid uploaded. Call uploadSolid() first.".to_string());
    }

    let camera = [input.camera[0], input.camera[1], input.camera[2]];
    let target = [input.target[0], input.target[1], input.target[2]];
    let up = [input.up[0], input.up[1], input.up[2]];

    let forward = normalize3(sub3(target, camera));
    let right = normalize3(cross3(forward, up));
    let up_normalized = cross3(right, forward);

    let aspect = input.width as f64 / input.height as f64;
    let fov_tan = (input.fov_radians * 0.5).tan();

    let ndc_x = (input.pixel_x as f64 + 0.5) / input.width as f64 * 2.0 - 1.0;
    let ndc_y = 1.0 - (input.pixel_y as f64 + 0.5) / input.height as f64 * 2.0;

    let ray_direction = normalize3(add3(
        forward,
        add3(
            mul3(right, ndc_x * fov_tan * aspect),
            mul3(up_normalized, ndc_y * fov_tan),
        ),
    ));

    Ok(PickSuccess {
        ndc: [canonical_f64(ndc_x), canonical_f64(ndc_y)],
        ray_direction: [
            canonical_f64(ray_direction[0]),
            canonical_f64(ray_direction[1]),
            canonical_f64(ray_direction[2]),
        ],
        // Mirrors current vcad wasm behavior: TODO CPU pick path returns background sentinel.
        face_index: -1,
    })
}

fn error_sample(
    case_id: &str,
    outcome: Result<PickSuccess, String>,
) -> RaytraceFacePickErrorSample {
    let error = outcome.unwrap_err();
    RaytraceFacePickErrorSample {
        case_id: case_id.to_string(),
        error,
    }
}

fn pick_sample(case_id: &str, input: PickInput) -> RaytraceFacePickSample {
    let camera = [input.camera[0], input.camera[1], input.camera[2]];
    let target = [input.target[0], input.target[1], input.target[2]];
    let up = [input.up[0], input.up[1], input.up[2]];
    let width = input.width;
    let height = input.height;
    let fov_radians = canonical_f64(input.fov_radians);
    let pixel = [input.pixel_x, input.pixel_y];

    let result = simulate_pick(input).expect("pick sample should succeed");

    RaytraceFacePickSample {
        case_id: case_id.to_string(),
        camera: [
            canonical_f64(camera[0]),
            canonical_f64(camera[1]),
            canonical_f64(camera[2]),
        ],
        target: [
            canonical_f64(target[0]),
            canonical_f64(target[1]),
            canonical_f64(target[2]),
        ],
        up: [
            canonical_f64(up[0]),
            canonical_f64(up[1]),
            canonical_f64(up[2]),
        ],
        width,
        height,
        fov_radians,
        pixel,
        ndc: result.ndc,
        ray_direction: result.ray_direction,
        face_index: result.face_index,
    }
}

fn validation_error_matches(
    actual: &[RaytraceFacePickErrorSample],
    expected: &[RaytraceFacePickErrorSample],
) -> bool {
    let mut actual_sorted = actual.to_vec();
    let mut expected_sorted = expected.to_vec();
    actual_sorted.sort_by(|left, right| left.case_id.cmp(&right.case_id));
    expected_sorted.sort_by(|left, right| left.case_id.cmp(&right.case_id));
    actual_sorted == expected_sorted
}

fn pick_samples_match(
    mut actual: Vec<RaytraceFacePickSample>,
    mut expected: Vec<RaytraceFacePickSample>,
    epsilon: f64,
) -> bool {
    actual.sort_by(|left, right| left.case_id.cmp(&right.case_id));
    expected.sort_by(|left, right| left.case_id.cmp(&right.case_id));

    if actual.len() != expected.len() {
        return false;
    }

    actual
        .iter()
        .zip(expected.iter())
        .all(|(left, right)| pick_sample_approx_eq(left, right, epsilon))
}

fn pick_sample_approx_eq(
    left: &RaytraceFacePickSample,
    right: &RaytraceFacePickSample,
    epsilon: f64,
) -> bool {
    left.case_id == right.case_id
        && approx_vec3(left.camera, right.camera, epsilon)
        && approx_vec3(left.target, right.target, epsilon)
        && approx_vec3(left.up, right.up, epsilon)
        && left.width == right.width
        && left.height == right.height
        && approx_eq(left.fov_radians, right.fov_radians, epsilon)
        && left.pixel == right.pixel
        && approx_eq(left.ndc[0], right.ndc[0], epsilon)
        && approx_eq(left.ndc[1], right.ndc[1], epsilon)
        && approx_vec3(left.ray_direction, right.ray_direction, epsilon)
        && left.face_index == right.face_index
}

fn approx_vec3(left: [f64; 3], right: [f64; 3], epsilon: f64) -> bool {
    approx_eq(left[0], right[0], epsilon)
        && approx_eq(left[1], right[1], epsilon)
        && approx_eq(left[2], right[2], epsilon)
}

fn sub3(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn add3(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn mul3(a: [f64; 3], scalar: f64) -> [f64; 3] {
    [a[0] * scalar, a[1] * scalar, a[2] * scalar]
}

fn cross3(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn length3(v: [f64; 3]) -> f64 {
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
}

fn normalize3(v: [f64; 3]) -> [f64; 3] {
    let len = length3(v);
    if len <= 1e-12 {
        [0.0, 0.0, 0.0]
    } else {
        [v[0] / len, v[1] / len, v[2] / len]
    }
}

fn approx_eq(left: f64, right: f64, epsilon: f64) -> bool {
    (left - right).abs() <= epsilon
}

fn canonical_f64(value: f64) -> f64 {
    let rounded = (value * 1_000_000_000.0).round() / 1_000_000_000.0;
    if rounded.abs() < 1e-12 { 0.0 } else { rounded }
}

#[allow(clippy::too_many_arguments)]
fn parity_signature(
    snapshot: &RaytraceFacePickSnapshot,
    reference_commit_match: bool,
    input_validation_match: bool,
    scene_guard_match: bool,
    sample_set_match: bool,
    stub_background_match: bool,
    ndc_mapping_match: bool,
    ray_direction_normalized_match: bool,
    deterministic_replay_match: bool,
    reference_fixture_sha256: &str,
) -> String {
    let payload = serde_json::to_vec(&(
        snapshot,
        reference_commit_match,
        input_validation_match,
        scene_guard_match,
        sample_set_match,
        stub_background_match,
        ndc_mapping_match,
        ray_direction_normalized_match,
        deterministic_replay_match,
        reference_fixture_sha256,
    ))
    .expect("serialize raytrace face-pick parity signature payload");
    stable_hex_digest(&payload)
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{PickInput, simulate_pick};

    #[test]
    fn simulate_pick_errors_for_invalid_vector_lengths() {
        let result = simulate_pick(PickInput {
            scene_uploaded: true,
            camera: vec![0.0, 0.0],
            target: vec![0.0, 0.0, 0.0],
            up: vec![0.0, 1.0, 0.0],
            width: 800,
            height: 600,
            fov_radians: std::f64::consts::FRAC_PI_3,
            pixel_x: 0,
            pixel_y: 0,
        });
        assert!(result.is_err());
    }
}
