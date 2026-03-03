use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::hash::stable_hex_digest;
use crate::parity::scorecard::ParityScorecard;
use crate::{CadError, CadResult};

pub const PARITY_TRIMMED_SURFACE_RAY_HIT_ISSUE_ID: &str = "VCAD-PARITY-099";
pub const TRIMMED_SURFACE_RAY_HIT_REFERENCE_FIXTURE_PATH: &str =
    "crates/cad/parity/fixtures/trimmed_surface_ray_hit_vcad_reference.json";
const TRIMMED_SURFACE_RAY_HIT_REFERENCE_FIXTURE_JSON: &str =
    include_str!("../../parity/fixtures/trimmed_surface_ray_hit_vcad_reference.json");

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TrimmedSurfaceRayHitParityManifest {
    pub manifest_version: u64,
    pub issue_id: String,
    pub vcad_commit: String,
    pub openagents_commit: String,
    pub generated_from_scorecard: String,
    pub reference_fixture_path: String,
    pub reference_fixture_sha256: String,
    pub reference_source: String,
    pub reference_commit_match: bool,
    pub sample_set_match: bool,
    pub concave_behavior_match: bool,
    pub outer_inner_loop_filter_match: bool,
    pub hole_rejection_match: bool,
    pub closest_hit_filter_match: bool,
    pub winding_rule_match: bool,
    pub deterministic_replay_match: bool,
    pub samples: Vec<TrimmedFaceSample>,
    pub concave_checks: Vec<ConcaveCheckSample>,
    pub closest_hit_t: Option<f64>,
    pub winding_nonzero_rule_holds: bool,
    pub deterministic_signature: String,
    pub parity_contracts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct TrimmedSurfaceRayHitReferenceFixture {
    manifest_version: u64,
    issue_id: String,
    vcad_commit: String,
    source: String,
    expected_samples: Vec<TrimmedFaceSample>,
    expected_concave_checks: Vec<ConcaveCheckSample>,
    expected_closest_hit_t: f64,
    expected_winding_nonzero_rule: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct TrimmedSurfaceRayHitSnapshot {
    samples: Vec<TrimmedFaceSample>,
    concave_checks: Vec<ConcaveCheckSample>,
    closest_hit_t: Option<f64>,
    winding_nonzero_rule_holds: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TrimmedFaceSample {
    pub case_id: String,
    pub t: f64,
    pub uv: [f64; 2],
    pub inside_outer: bool,
    pub inside_hole: bool,
    pub accepted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConcaveCheckSample {
    pub case_id: String,
    pub point: [f64; 2],
    pub inside: bool,
}

pub fn build_trimmed_surface_ray_hit_parity_manifest(
    scorecard: &ParityScorecard,
    scorecard_path: &str,
) -> CadResult<TrimmedSurfaceRayHitParityManifest> {
    let reference: TrimmedSurfaceRayHitReferenceFixture =
        serde_json::from_str(TRIMMED_SURFACE_RAY_HIT_REFERENCE_FIXTURE_JSON).map_err(|error| {
            CadError::ParseFailed {
                reason: format!(
                    "failed parsing trimmed-surface ray hit reference fixture: {error}"
                ),
            }
        })?;

    let reference_fixture_sha256 =
        sha256_hex(TRIMMED_SURFACE_RAY_HIT_REFERENCE_FIXTURE_JSON.as_bytes());
    let reference_commit_match = reference.vcad_commit == scorecard.vcad_commit;

    let snapshot = collect_snapshot();
    let replay_snapshot = collect_snapshot();
    let deterministic_replay_match = snapshot == replay_snapshot;

    let sample_set_match = samples_match(
        snapshot.samples.clone(),
        reference.expected_samples.clone(),
        1e-9,
    );
    let concave_behavior_match = concave_samples_match(
        snapshot.concave_checks.clone(),
        reference.expected_concave_checks.clone(),
        1e-9,
    );

    let outer_inner_loop_filter_match = snapshot
        .samples
        .iter()
        .all(|sample| sample.accepted == (sample.inside_outer && !sample.inside_hole));
    let hole_rejection_match = snapshot
        .samples
        .iter()
        .filter(|sample| sample.inside_hole)
        .all(|sample| !sample.accepted);
    let closest_hit_filter_match = snapshot
        .closest_hit_t
        .is_some_and(|value| approx_eq(value, reference.expected_closest_hit_t, 1e-9));
    let winding_rule_match =
        snapshot.winding_nonzero_rule_holds == reference.expected_winding_nonzero_rule;

    let deterministic_signature = parity_signature(
        &snapshot,
        reference_commit_match,
        sample_set_match,
        concave_behavior_match,
        outer_inner_loop_filter_match,
        hole_rejection_match,
        closest_hit_filter_match,
        winding_rule_match,
        deterministic_replay_match,
        &reference_fixture_sha256,
    );

    Ok(TrimmedSurfaceRayHitParityManifest {
        manifest_version: 1,
        issue_id: PARITY_TRIMMED_SURFACE_RAY_HIT_ISSUE_ID.to_string(),
        vcad_commit: scorecard.vcad_commit.clone(),
        openagents_commit: scorecard.openagents_commit.clone(),
        generated_from_scorecard: scorecard_path.to_string(),
        reference_fixture_path: TRIMMED_SURFACE_RAY_HIT_REFERENCE_FIXTURE_PATH.to_string(),
        reference_fixture_sha256,
        reference_source: reference.source,
        reference_commit_match,
        sample_set_match,
        concave_behavior_match,
        outer_inner_loop_filter_match,
        hole_rejection_match,
        closest_hit_filter_match,
        winding_rule_match,
        deterministic_replay_match,
        samples: snapshot.samples,
        concave_checks: snapshot.concave_checks,
        closest_hit_t: snapshot.closest_hit_t,
        winding_nonzero_rule_holds: snapshot.winding_nonzero_rule_holds,
        deterministic_signature,
        parity_contracts: vec![
            "point_in_face accepts hits inside outer loop and rejects hits outside boundary"
                .to_string(),
            "inner trim loops behave as holes and reject hits that would otherwise pass outer loop"
                .to_string(),
            "point_in_polygon uses winding-number semantics and supports concave polygons"
                .to_string(),
            "ray hit filtering keeps only accepted trimmed hits before closest-hit selection"
                .to_string(),
            "closest trimmed hit is selected deterministically by minimum positive t among accepted hits"
                .to_string(),
        ],
    })
}

fn collect_snapshot() -> TrimmedSurfaceRayHitSnapshot {
    let outer_loop = vec![[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]];
    let inner_holes = vec![vec![[4.0, 4.0], [6.0, 4.0], [6.0, 6.0], [4.0, 6.0]]];

    let mut samples = vec![
        sample(
            "hit_inner_valid_low",
            4.0,
            [2.0, 2.0],
            &outer_loop,
            &inner_holes,
        ),
        sample(
            "hit_hole_rejected",
            6.0,
            [5.0, 5.0],
            &outer_loop,
            &inner_holes,
        ),
        sample(
            "hit_inner_valid_high",
            8.0,
            [8.0, 8.0],
            &outer_loop,
            &inner_holes,
        ),
        sample(
            "hit_outside_rejected",
            10.0,
            [12.0, 5.0],
            &outer_loop,
            &inner_holes,
        ),
    ];
    samples.sort_by(|left, right| left.case_id.cmp(&right.case_id));

    let l_shape = vec![
        [0.0, 0.0],
        [2.0, 0.0],
        [2.0, 1.0],
        [1.0, 1.0],
        [1.0, 2.0],
        [0.0, 2.0],
    ];
    let mut concave_checks = vec![
        concave_check("concave_inside_main", [0.5, 0.5], &l_shape),
        concave_check("concave_inside_leg", [0.5, 1.5], &l_shape),
        concave_check("concave_notch_rejected", [1.5, 1.5], &l_shape),
    ];
    concave_checks.sort_by(|left, right| left.case_id.cmp(&right.case_id));

    let closest_hit_t = samples
        .iter()
        .filter(|sample| sample.accepted)
        .map(|sample| sample.t)
        .min_by(|left, right| left.partial_cmp(right).unwrap_or(std::cmp::Ordering::Equal));

    let square_ccw = vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
    let square_cw = vec![[0.0, 0.0], [0.0, 1.0], [1.0, 1.0], [1.0, 0.0]];
    let winding_nonzero_rule_holds =
        point_in_polygon([0.5, 0.5], &square_ccw) && point_in_polygon([0.5, 0.5], &square_cw);

    TrimmedSurfaceRayHitSnapshot {
        samples,
        concave_checks,
        closest_hit_t,
        winding_nonzero_rule_holds,
    }
}

fn sample(
    case_id: &str,
    t: f64,
    uv: [f64; 2],
    outer_loop: &[[f64; 2]],
    inner_holes: &[Vec<[f64; 2]>],
) -> TrimmedFaceSample {
    let inside_outer = point_in_polygon(uv, outer_loop);
    let inside_hole = inner_holes
        .iter()
        .any(|hole| point_in_polygon(uv, hole.as_slice()));
    let accepted = inside_outer && !inside_hole;

    TrimmedFaceSample {
        case_id: case_id.to_string(),
        t: canonical_f64(t),
        uv: [canonical_f64(uv[0]), canonical_f64(uv[1])],
        inside_outer,
        inside_hole,
        accepted,
    }
}

fn concave_check(case_id: &str, point: [f64; 2], polygon: &[[f64; 2]]) -> ConcaveCheckSample {
    ConcaveCheckSample {
        case_id: case_id.to_string(),
        point: [canonical_f64(point[0]), canonical_f64(point[1])],
        inside: point_in_polygon(point, polygon),
    }
}

fn point_in_polygon(point: [f64; 2], polygon: &[[f64; 2]]) -> bool {
    if polygon.len() < 3 {
        return false;
    }

    let mut winding = 0i32;
    let n = polygon.len();

    for index in 0..n {
        let p1 = polygon[index];
        let p2 = polygon[(index + 1) % n];

        if p1[1] <= point[1] {
            if p2[1] > point[1] && is_left(p1, p2, point) > 0.0 {
                winding += 1;
            }
        } else if p2[1] <= point[1] && is_left(p1, p2, point) < 0.0 {
            winding -= 1;
        }
    }

    winding != 0
}

fn is_left(p0: [f64; 2], p1: [f64; 2], p2: [f64; 2]) -> f64 {
    (p1[0] - p0[0]) * (p2[1] - p0[1]) - (p2[0] - p0[0]) * (p1[1] - p0[1])
}

fn samples_match(
    mut actual: Vec<TrimmedFaceSample>,
    mut expected: Vec<TrimmedFaceSample>,
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
        .all(|(left, right)| sample_approx_eq(left, right, epsilon))
}

fn sample_approx_eq(left: &TrimmedFaceSample, right: &TrimmedFaceSample, epsilon: f64) -> bool {
    left.case_id == right.case_id
        && approx_eq(left.t, right.t, epsilon)
        && approx_eq(left.uv[0], right.uv[0], epsilon)
        && approx_eq(left.uv[1], right.uv[1], epsilon)
        && left.inside_outer == right.inside_outer
        && left.inside_hole == right.inside_hole
        && left.accepted == right.accepted
}

fn concave_samples_match(
    mut actual: Vec<ConcaveCheckSample>,
    mut expected: Vec<ConcaveCheckSample>,
    epsilon: f64,
) -> bool {
    actual.sort_by(|left, right| left.case_id.cmp(&right.case_id));
    expected.sort_by(|left, right| left.case_id.cmp(&right.case_id));

    if actual.len() != expected.len() {
        return false;
    }

    actual.iter().zip(expected.iter()).all(|(left, right)| {
        left.case_id == right.case_id
            && approx_eq(left.point[0], right.point[0], epsilon)
            && approx_eq(left.point[1], right.point[1], epsilon)
            && left.inside == right.inside
    })
}

fn canonical_f64(value: f64) -> f64 {
    let rounded = (value * 1_000_000_000.0).round() / 1_000_000_000.0;
    if rounded.abs() < 1e-12 { 0.0 } else { rounded }
}

fn approx_eq(left: f64, right: f64, epsilon: f64) -> bool {
    (left - right).abs() <= epsilon
}

#[allow(clippy::too_many_arguments)]
fn parity_signature(
    snapshot: &TrimmedSurfaceRayHitSnapshot,
    reference_commit_match: bool,
    sample_set_match: bool,
    concave_behavior_match: bool,
    outer_inner_loop_filter_match: bool,
    hole_rejection_match: bool,
    closest_hit_filter_match: bool,
    winding_rule_match: bool,
    deterministic_replay_match: bool,
    reference_fixture_sha256: &str,
) -> String {
    let payload = serde_json::to_vec(&(
        snapshot,
        reference_commit_match,
        sample_set_match,
        concave_behavior_match,
        outer_inner_loop_filter_match,
        hole_rejection_match,
        closest_hit_filter_match,
        winding_rule_match,
        deterministic_replay_match,
        reference_fixture_sha256,
    ))
    .expect("serialize trimmed-surface ray hit parity signature payload");
    stable_hex_digest(&payload)
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}
