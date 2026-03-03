use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::drafting::{
    BoundingBox2D, HatchRegion, Point2D, SectionOptions, SectionPlane, chain_segments,
    generate_hatch_lines, intersect_mesh_with_plane, project_to_section_plane, section_mesh,
};
use crate::kernel_math::{Point3, Vec3};
use crate::parity::scorecard::ParityScorecard;
use crate::{CadError, CadResult};

pub const PARITY_DRAFTING_SECTION_ISSUE_ID: &str = "VCAD-PARITY-072";
pub const DRAFTING_SECTION_REFERENCE_CORPUS_PATH: &str =
    "crates/cad/parity/fixtures/drafting_section_vcad_reference.json";
const DRAFTING_SECTION_REFERENCE_CORPUS_JSON: &str =
    include_str!("../../parity/fixtures/drafting_section_vcad_reference.json");

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DraftingSectionParityManifest {
    pub manifest_version: u64,
    pub issue_id: String,
    pub vcad_commit: String,
    pub openagents_commit: String,
    pub generated_from_scorecard: String,
    pub reference_corpus_path: String,
    pub reference_corpus_sha256: String,
    pub reference_source: String,
    pub reference_commit_match: bool,
    pub intersection_cases: Vec<SectionIntersectionCase>,
    pub chain_cases: Vec<SectionChainCase>,
    pub mesh_cases: Vec<SectionMeshCase>,
    pub projection_cases: Vec<SectionProjectionCase>,
    pub hatch_cases: Vec<SectionHatchCase>,
    pub intersection_contract_match: bool,
    pub chain_contract_match: bool,
    pub mesh_contract_match: bool,
    pub projection_contract_match: bool,
    pub hatch_contract_match: bool,
    pub deterministic_replay_match: bool,
    pub deterministic_signature: String,
    pub parity_contracts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct DraftingSectionReferenceCorpus {
    manifest_version: u64,
    issue_id: String,
    vcad_commit: String,
    source: String,
    scalar_tolerance: f64,
    expected_intersection_cases: Vec<SectionIntersectionCase>,
    expected_chain_cases: Vec<SectionChainCase>,
    expected_mesh_cases: Vec<SectionMeshCase>,
    expected_projection_cases: Vec<SectionProjectionCase>,
    expected_hatch_cases: Vec<SectionHatchCase>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SectionIntersectionCase {
    pub case_id: String,
    pub segment_count: usize,
    pub first_segment_length: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SectionChainCase {
    pub case_id: String,
    pub curve_count: usize,
    pub closed_curve_count: usize,
    pub total_points: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SectionMeshCase {
    pub case_id: String,
    pub curve_count: usize,
    pub hatch_line_count: usize,
    pub first_curve_point_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SectionProjectionCase {
    pub case_id: String,
    pub projected_x: f64,
    pub projected_y: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SectionHatchCase {
    pub case_id: String,
    pub line_count: usize,
    pub first_line_length: f64,
}

pub fn build_drafting_section_parity_manifest(
    scorecard: &ParityScorecard,
    scorecard_path: &str,
) -> CadResult<DraftingSectionParityManifest> {
    let corpus: DraftingSectionReferenceCorpus =
        serde_json::from_str(DRAFTING_SECTION_REFERENCE_CORPUS_JSON).map_err(|error| {
            CadError::ParseFailed {
                reason: format!("failed to parse drafting section reference corpus: {error}"),
            }
        })?;

    let reference_corpus_sha256 = sha256_hex(DRAFTING_SECTION_REFERENCE_CORPUS_JSON.as_bytes());
    let reference_commit_match = corpus.vcad_commit == scorecard.vcad_commit;

    let snapshot = collect_section_snapshot();
    let replay_snapshot = collect_section_snapshot();
    let deterministic_replay_match = snapshot == replay_snapshot;

    let expected_intersection_cases = sorted_intersection_cases(corpus.expected_intersection_cases);
    let expected_chain_cases = sorted_chain_cases(corpus.expected_chain_cases);
    let expected_mesh_cases = sorted_mesh_cases(corpus.expected_mesh_cases);
    let expected_projection_cases = sorted_projection_cases(corpus.expected_projection_cases);
    let expected_hatch_cases = sorted_hatch_cases(corpus.expected_hatch_cases);

    let intersection_contract_match = intersection_cases_match(
        &snapshot.intersection_cases,
        &expected_intersection_cases,
        corpus.scalar_tolerance,
    );
    let chain_contract_match = snapshot.chain_cases == expected_chain_cases;
    let mesh_contract_match = snapshot.mesh_cases == expected_mesh_cases;
    let projection_contract_match = projection_cases_match(
        &snapshot.projection_cases,
        &expected_projection_cases,
        corpus.scalar_tolerance,
    );
    let hatch_contract_match = hatch_cases_match(
        &snapshot.hatch_cases,
        &expected_hatch_cases,
        corpus.scalar_tolerance,
    );

    let deterministic_signature = parity_signature(
        &snapshot.intersection_cases,
        &snapshot.chain_cases,
        &snapshot.mesh_cases,
        &snapshot.projection_cases,
        &snapshot.hatch_cases,
        reference_commit_match,
        intersection_contract_match,
        chain_contract_match,
        mesh_contract_match,
        projection_contract_match,
        hatch_contract_match,
        deterministic_replay_match,
        &reference_corpus_sha256,
    );

    Ok(DraftingSectionParityManifest {
        manifest_version: 1,
        issue_id: PARITY_DRAFTING_SECTION_ISSUE_ID.to_string(),
        vcad_commit: scorecard.vcad_commit.clone(),
        openagents_commit: scorecard.openagents_commit.clone(),
        generated_from_scorecard: scorecard_path.to_string(),
        reference_corpus_path: DRAFTING_SECTION_REFERENCE_CORPUS_PATH.to_string(),
        reference_corpus_sha256,
        reference_source: corpus.source,
        reference_commit_match,
        intersection_cases: snapshot.intersection_cases,
        chain_cases: snapshot.chain_cases,
        mesh_cases: snapshot.mesh_cases,
        projection_cases: snapshot.projection_cases,
        hatch_cases: snapshot.hatch_cases,
        intersection_contract_match,
        chain_contract_match,
        mesh_contract_match,
        projection_contract_match,
        hatch_contract_match,
        deterministic_replay_match,
        deterministic_signature,
        parity_contracts: vec![
            "section plane/triangle intersection emits vcad-aligned deterministic segment counts"
                .to_string(),
            "section segment chaining preserves deterministic open/closed curve semantics"
                .to_string(),
            "section mesh projection + hatch generation remain deterministic across replay"
                .to_string(),
            "section projection basis produces stable 2d coordinates for reference points"
                .to_string(),
        ],
    })
}

#[derive(Debug, Clone, PartialEq)]
struct SectionSnapshot {
    intersection_cases: Vec<SectionIntersectionCase>,
    chain_cases: Vec<SectionChainCase>,
    mesh_cases: Vec<SectionMeshCase>,
    projection_cases: Vec<SectionProjectionCase>,
    hatch_cases: Vec<SectionHatchCase>,
}

fn collect_section_snapshot() -> SectionSnapshot {
    let z_plane = SectionPlane {
        origin: Point3::origin(),
        normal: Vec3::z(),
    };
    let tilted_plane = SectionPlane {
        origin: Point3::new(1.0, 2.0, 3.0),
        normal: Vec3::new(0.0, 1.0, 1.0),
    };

    let triangle_points = vec![
        Point3::new(-1.0, -1.0, -1.0),
        Point3::new(1.0, -1.0, 1.0),
        Point3::new(0.0, 1.0, 1.0),
    ];

    let intersection_triangle = intersect_mesh_with_plane(&triangle_points, &z_plane, 1e-6);
    let intersection_coplanar_polyline = intersect_mesh_with_plane(
        &[Point3::new(0.0, 0.0, 0.0), Point3::new(2.0, 0.0, 0.0)],
        &z_plane,
        1e-6,
    );

    let intersection_cases = sorted_intersection_cases(vec![
        intersection_case("coplanar_polyline", &intersection_coplanar_polyline),
        intersection_case("triangle_crossing", &intersection_triangle),
    ]);

    let closed_segments = vec![
        (Point3::new(0.0, 0.0, 0.0), Point3::new(1.0, 0.0, 0.0)),
        (Point3::new(1.0, 0.0, 0.0), Point3::new(1.0, 1.0, 0.0)),
        (Point3::new(1.0, 1.0, 0.0), Point3::new(0.0, 1.0, 0.0)),
        (Point3::new(0.0, 1.0, 0.0), Point3::new(0.0, 0.0, 0.0)),
    ];
    let open_segments = vec![
        (Point3::new(0.0, 0.0, 0.0), Point3::new(1.0, 0.0, 0.0)),
        (Point3::new(1.0, 0.0, 0.0), Point3::new(2.0, 0.0, 0.0)),
    ];

    let closed_curves = chain_segments(&closed_segments, &z_plane);
    let open_curves = chain_segments(&open_segments, &z_plane);
    let chain_cases = sorted_chain_cases(vec![
        chain_case("closed_square", &closed_curves),
        chain_case("open_polyline", &open_curves),
    ]);

    let mesh_with_hatch = section_mesh(&triangle_points, &z_plane, SectionOptions::default());
    let mesh_without_hatch =
        section_mesh(&triangle_points, &z_plane, SectionOptions { hatch: None });
    let mesh_cases = sorted_mesh_cases(vec![
        mesh_case("triangle_with_hatch", &mesh_with_hatch),
        mesh_case("triangle_without_hatch", &mesh_without_hatch),
    ]);

    let projection_cases = sorted_projection_cases(vec![
        projection_case(
            "tilted_plane_point",
            Point3::new(2.0, 4.0, 3.0),
            &tilted_plane,
        ),
        projection_case("z_plane_point", Point3::new(3.0, 2.0, 0.0), &z_plane),
    ]);

    let hatch_lines = generate_hatch_lines(
        &HatchRegion {
            bounds: BoundingBox2D {
                min_x: 0.0,
                min_y: 0.0,
                max_x: 4.0,
                max_y: 2.0,
            },
        },
        1.0,
        std::f64::consts::FRAC_PI_4,
    );
    let hatch_dense_lines = generate_hatch_lines(
        &HatchRegion {
            bounds: BoundingBox2D {
                min_x: -1.0,
                min_y: -1.0,
                max_x: 2.0,
                max_y: 2.0,
            },
        },
        0.5,
        0.0,
    );
    let hatch_cases = sorted_hatch_cases(vec![
        hatch_case("bounds_4x2_pi_over_4", &hatch_lines),
        hatch_case("bounds_3x3_horizontal_dense", &hatch_dense_lines),
    ]);

    SectionSnapshot {
        intersection_cases,
        chain_cases,
        mesh_cases,
        projection_cases,
        hatch_cases,
    }
}

fn intersection_case(case_id: &str, segments: &[(Point3, Point3)]) -> SectionIntersectionCase {
    let first_segment_length = segments
        .first()
        .map(|(start, end)| point3_distance(*start, *end))
        .unwrap_or(0.0);
    SectionIntersectionCase {
        case_id: case_id.to_string(),
        segment_count: segments.len(),
        first_segment_length,
    }
}

fn chain_case(case_id: &str, curves: &[crate::drafting::SectionCurve]) -> SectionChainCase {
    SectionChainCase {
        case_id: case_id.to_string(),
        curve_count: curves.len(),
        closed_curve_count: curves.iter().filter(|curve| curve.closed).count(),
        total_points: curves.iter().map(|curve| curve.points.len()).sum(),
    }
}

fn mesh_case(case_id: &str, view: &crate::drafting::SectionView) -> SectionMeshCase {
    SectionMeshCase {
        case_id: case_id.to_string(),
        curve_count: view.curves.len(),
        hatch_line_count: view.hatch_lines.len(),
        first_curve_point_count: view
            .curves
            .first()
            .map(|curve| curve.points.len())
            .unwrap_or(0),
    }
}

fn projection_case(case_id: &str, point: Point3, plane: &SectionPlane) -> SectionProjectionCase {
    let projected = project_to_section_plane(point, plane);
    SectionProjectionCase {
        case_id: case_id.to_string(),
        projected_x: projected.x,
        projected_y: projected.y,
    }
}

fn hatch_case(case_id: &str, lines: &[(Point2D, Point2D)]) -> SectionHatchCase {
    SectionHatchCase {
        case_id: case_id.to_string(),
        line_count: lines.len(),
        first_line_length: lines
            .first()
            .map(|(start, end)| start.distance(*end))
            .unwrap_or(0.0),
    }
}

fn sorted_intersection_cases(
    mut cases: Vec<SectionIntersectionCase>,
) -> Vec<SectionIntersectionCase> {
    cases.sort_by(|left, right| left.case_id.cmp(&right.case_id));
    cases
}

fn sorted_chain_cases(mut cases: Vec<SectionChainCase>) -> Vec<SectionChainCase> {
    cases.sort_by(|left, right| left.case_id.cmp(&right.case_id));
    cases
}

fn sorted_mesh_cases(mut cases: Vec<SectionMeshCase>) -> Vec<SectionMeshCase> {
    cases.sort_by(|left, right| left.case_id.cmp(&right.case_id));
    cases
}

fn sorted_projection_cases(mut cases: Vec<SectionProjectionCase>) -> Vec<SectionProjectionCase> {
    cases.sort_by(|left, right| left.case_id.cmp(&right.case_id));
    cases
}

fn sorted_hatch_cases(mut cases: Vec<SectionHatchCase>) -> Vec<SectionHatchCase> {
    cases.sort_by(|left, right| left.case_id.cmp(&right.case_id));
    cases
}

fn intersection_cases_match(
    actual: &[SectionIntersectionCase],
    expected: &[SectionIntersectionCase],
    tolerance: f64,
) -> bool {
    if actual.len() != expected.len() {
        return false;
    }

    actual.iter().zip(expected.iter()).all(|(left, right)| {
        left.case_id == right.case_id
            && left.segment_count == right.segment_count
            && approx_eq(
                left.first_segment_length,
                right.first_segment_length,
                tolerance,
            )
    })
}

fn projection_cases_match(
    actual: &[SectionProjectionCase],
    expected: &[SectionProjectionCase],
    tolerance: f64,
) -> bool {
    if actual.len() != expected.len() {
        return false;
    }

    actual.iter().zip(expected.iter()).all(|(left, right)| {
        left.case_id == right.case_id
            && approx_eq(left.projected_x, right.projected_x, tolerance)
            && approx_eq(left.projected_y, right.projected_y, tolerance)
    })
}

fn hatch_cases_match(
    actual: &[SectionHatchCase],
    expected: &[SectionHatchCase],
    tolerance: f64,
) -> bool {
    if actual.len() != expected.len() {
        return false;
    }

    actual.iter().zip(expected.iter()).all(|(left, right)| {
        left.case_id == right.case_id
            && left.line_count == right.line_count
            && approx_eq(left.first_line_length, right.first_line_length, tolerance)
    })
}

fn approx_eq(left: f64, right: f64, tolerance: f64) -> bool {
    (left - right).abs() <= tolerance
}

fn point3_distance(left: Point3, right: Point3) -> f64 {
    ((left.x - right.x).powi(2) + (left.y - right.y).powi(2) + (left.z - right.z).powi(2)).sqrt()
}

fn parity_signature(
    intersection_cases: &[SectionIntersectionCase],
    chain_cases: &[SectionChainCase],
    mesh_cases: &[SectionMeshCase],
    projection_cases: &[SectionProjectionCase],
    hatch_cases: &[SectionHatchCase],
    reference_commit_match: bool,
    intersection_contract_match: bool,
    chain_contract_match: bool,
    mesh_contract_match: bool,
    projection_contract_match: bool,
    hatch_contract_match: bool,
    deterministic_replay_match: bool,
    reference_corpus_sha256: &str,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(
        serde_json::to_vec(&(
            intersection_cases,
            chain_cases,
            mesh_cases,
            projection_cases,
            hatch_cases,
            reference_commit_match,
            intersection_contract_match,
            chain_contract_match,
            mesh_contract_match,
            projection_contract_match,
            hatch_contract_match,
            deterministic_replay_match,
            reference_corpus_sha256,
        ))
        .expect("serialize drafting section parity payload"),
    );
    format!("{:x}", hasher.finalize())[..16].to_string()
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::SectionChainCase;
    use super::SectionHatchCase;
    use super::SectionIntersectionCase;
    use super::SectionMeshCase;
    use super::SectionProjectionCase;
    use super::parity_signature;

    #[test]
    fn parity_signature_is_stable_for_identical_inputs() {
        let intersections = vec![SectionIntersectionCase {
            case_id: "triangle".to_string(),
            segment_count: 1,
            first_segment_length: 1.0,
        }];
        let chains = vec![SectionChainCase {
            case_id: "closed".to_string(),
            curve_count: 1,
            closed_curve_count: 1,
            total_points: 4,
        }];
        let meshes = vec![SectionMeshCase {
            case_id: "mesh".to_string(),
            curve_count: 1,
            hatch_line_count: 2,
            first_curve_point_count: 2,
        }];
        let projections = vec![SectionProjectionCase {
            case_id: "proj".to_string(),
            projected_x: 1.0,
            projected_y: 2.0,
        }];
        let hatches = vec![SectionHatchCase {
            case_id: "hatch".to_string(),
            line_count: 3,
            first_line_length: 4.0,
        }];

        let first = parity_signature(
            &intersections,
            &chains,
            &meshes,
            &projections,
            &hatches,
            true,
            true,
            true,
            true,
            true,
            true,
            true,
            "sha",
        );
        let second = parity_signature(
            &intersections,
            &chains,
            &meshes,
            &projections,
            &hatches,
            true,
            true,
            true,
            true,
            true,
            true,
            true,
            "sha",
        );
        assert_eq!(first, second);
    }
}
