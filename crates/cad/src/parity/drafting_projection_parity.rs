use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::drafting::{ViewDirection, ViewMatrix, project_point_with_depth};
use crate::kernel_math::Point3;
use crate::parity::scorecard::ParityScorecard;
use crate::{CadError, CadResult};

pub const PARITY_DRAFTING_PROJECTION_ISSUE_ID: &str = "VCAD-PARITY-068";
pub const DRAFTING_PROJECTION_REFERENCE_CORPUS_PATH: &str =
    "crates/cad/parity/fixtures/drafting_projection_vcad_reference.json";
const DRAFTING_PROJECTION_REFERENCE_CORPUS_JSON: &str =
    include_str!("../../parity/fixtures/drafting_projection_vcad_reference.json");

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DraftingProjectionParityManifest {
    pub manifest_version: u64,
    pub issue_id: String,
    pub vcad_commit: String,
    pub openagents_commit: String,
    pub generated_from_scorecard: String,
    pub reference_corpus_path: String,
    pub reference_corpus_sha256: String,
    pub reference_source: String,
    pub reference_commit_match: bool,
    pub projection_cases: Vec<ProjectionCase>,
    pub orthonormal_views: Vec<OrthonormalViewCheck>,
    pub projection_case_match: bool,
    pub orthonormal_views_match: bool,
    pub isometric_up_positive_y: bool,
    pub deterministic_replay_match: bool,
    pub deterministic_signature: String,
    pub parity_contracts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct DraftingProjectionReferenceCorpus {
    manifest_version: u64,
    issue_id: String,
    vcad_commit: String,
    source: String,
    projection_tolerance: f64,
    orthonormal_tolerance: f64,
    isometric_up_min_y: f64,
    sample_point: ReferencePoint3,
    up_check_point: ReferencePoint3,
    expected_projection_cases: Vec<ProjectionCase>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
struct ReferencePoint3 {
    x: f64,
    y: f64,
    z: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProjectionCase {
    pub view_direction: String,
    pub projected_x: f64,
    pub projected_y: f64,
    pub depth: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OrthonormalViewCheck {
    pub view_direction: String,
    pub right_norm: f64,
    pub up_norm: f64,
    pub forward_norm: f64,
    pub dot_right_up: f64,
    pub dot_right_forward: f64,
    pub dot_up_forward: f64,
}

pub fn build_drafting_projection_parity_manifest(
    scorecard: &ParityScorecard,
    scorecard_path: &str,
) -> CadResult<DraftingProjectionParityManifest> {
    let corpus: DraftingProjectionReferenceCorpus =
        serde_json::from_str(DRAFTING_PROJECTION_REFERENCE_CORPUS_JSON).map_err(|error| {
            CadError::ParseFailed {
                reason: format!("failed to parse drafting projection reference corpus: {error}"),
            }
        })?;

    let reference_corpus_sha256 = sha256_hex(DRAFTING_PROJECTION_REFERENCE_CORPUS_JSON.as_bytes());
    let reference_commit_match = corpus.vcad_commit == scorecard.vcad_commit;

    let snapshot = collect_projection_snapshot(corpus.sample_point, corpus.up_check_point);
    let replay_snapshot = collect_projection_snapshot(corpus.sample_point, corpus.up_check_point);
    let deterministic_replay_match = snapshot == replay_snapshot;

    let projection_case_match = projection_cases_match(
        &snapshot.projection_cases,
        &corpus.expected_projection_cases,
        corpus.projection_tolerance,
    );

    let orthonormal_views_match = snapshot.orthonormal_views.iter().all(|check| {
        approx_eq(check.right_norm, 1.0, corpus.orthonormal_tolerance)
            && approx_eq(check.up_norm, 1.0, corpus.orthonormal_tolerance)
            && approx_eq(check.forward_norm, 1.0, corpus.orthonormal_tolerance)
            && approx_eq(check.dot_right_up, 0.0, corpus.orthonormal_tolerance)
            && approx_eq(check.dot_right_forward, 0.0, corpus.orthonormal_tolerance)
            && approx_eq(check.dot_up_forward, 0.0, corpus.orthonormal_tolerance)
    });

    let isometric_up_positive_y = snapshot.isometric_up_projection_y > corpus.isometric_up_min_y;

    let deterministic_signature = parity_signature(
        &snapshot.projection_cases,
        &snapshot.orthonormal_views,
        snapshot.isometric_up_projection_y,
        reference_commit_match,
        projection_case_match,
        orthonormal_views_match,
        isometric_up_positive_y,
        deterministic_replay_match,
        &reference_corpus_sha256,
    );

    Ok(DraftingProjectionParityManifest {
        manifest_version: 1,
        issue_id: PARITY_DRAFTING_PROJECTION_ISSUE_ID.to_string(),
        vcad_commit: scorecard.vcad_commit.clone(),
        openagents_commit: scorecard.openagents_commit.clone(),
        generated_from_scorecard: scorecard_path.to_string(),
        reference_corpus_path: DRAFTING_PROJECTION_REFERENCE_CORPUS_PATH.to_string(),
        reference_corpus_sha256,
        reference_source: corpus.source,
        reference_commit_match,
        projection_cases: snapshot.projection_cases,
        orthonormal_views: snapshot.orthonormal_views,
        projection_case_match,
        orthonormal_views_match,
        isometric_up_positive_y,
        deterministic_replay_match,
        deterministic_signature,
        parity_contracts: vec![
            "orthographic projection parity covers front/back/top/bottom/right/left views"
                .to_string(),
            "isometric projection parity preserves vcad standard azimuth/elevation semantics"
                .to_string(),
            "view matrices remain orthonormal across all drafting view directions".to_string(),
            "projection outputs are deterministic across replay".to_string(),
        ],
    })
}

#[derive(Debug, Clone, PartialEq)]
struct ProjectionSnapshot {
    projection_cases: Vec<ProjectionCase>,
    orthonormal_views: Vec<OrthonormalViewCheck>,
    isometric_up_projection_y: f64,
}

fn collect_projection_snapshot(
    sample_point: ReferencePoint3,
    up_check_point: ReferencePoint3,
) -> ProjectionSnapshot {
    let sample = Point3::new(sample_point.x, sample_point.y, sample_point.z);
    let up_check = Point3::new(up_check_point.x, up_check_point.y, up_check_point.z);

    let view_cases = [
        ("front", ViewDirection::Front),
        ("back", ViewDirection::Back),
        ("top", ViewDirection::Top),
        ("bottom", ViewDirection::Bottom),
        ("right", ViewDirection::Right),
        ("left", ViewDirection::Left),
        ("isometric", ViewDirection::ISOMETRIC_STANDARD),
    ];

    let mut projection_cases = Vec::with_capacity(view_cases.len());
    let mut orthonormal_views = Vec::with_capacity(view_cases.len());

    for (view_tag, view_direction) in view_cases {
        let (projected, depth) = project_point_with_depth(sample, view_direction);
        projection_cases.push(ProjectionCase {
            view_direction: view_tag.to_string(),
            projected_x: projected.x,
            projected_y: projected.y,
            depth,
        });

        let matrix = ViewMatrix::from_view_direction(view_direction);
        orthonormal_views.push(OrthonormalViewCheck {
            view_direction: view_tag.to_string(),
            right_norm: matrix.right.norm(),
            up_norm: matrix.up.norm(),
            forward_norm: matrix.forward.norm(),
            dot_right_up: matrix.right.dot(matrix.up),
            dot_right_forward: matrix.right.dot(matrix.forward),
            dot_up_forward: matrix.up.dot(matrix.forward),
        });
    }

    projection_cases.sort_by(|left, right| left.view_direction.cmp(&right.view_direction));
    orthonormal_views.sort_by(|left, right| left.view_direction.cmp(&right.view_direction));

    let isometric_up_projection_y =
        project_point_with_depth(up_check, ViewDirection::ISOMETRIC_STANDARD)
            .0
            .y;

    ProjectionSnapshot {
        projection_cases,
        orthonormal_views,
        isometric_up_projection_y,
    }
}

fn projection_cases_match(
    actual: &[ProjectionCase],
    expected: &[ProjectionCase],
    tolerance: f64,
) -> bool {
    if actual.len() != expected.len() {
        return false;
    }

    actual.iter().zip(expected.iter()).all(|(left, right)| {
        left.view_direction == right.view_direction
            && approx_eq(left.projected_x, right.projected_x, tolerance)
            && approx_eq(left.projected_y, right.projected_y, tolerance)
            && approx_eq(left.depth, right.depth, tolerance)
    })
}

fn approx_eq(left: f64, right: f64, tolerance: f64) -> bool {
    (left - right).abs() <= tolerance
}

fn parity_signature(
    projection_cases: &[ProjectionCase],
    orthonormal_views: &[OrthonormalViewCheck],
    isometric_up_projection_y: f64,
    reference_commit_match: bool,
    projection_case_match: bool,
    orthonormal_views_match: bool,
    isometric_up_positive_y: bool,
    deterministic_replay_match: bool,
    reference_corpus_sha256: &str,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(
        serde_json::to_vec(&(
            projection_cases,
            orthonormal_views,
            isometric_up_projection_y,
            reference_commit_match,
            projection_case_match,
            orthonormal_views_match,
            isometric_up_positive_y,
            deterministic_replay_match,
            reference_corpus_sha256,
        ))
        .expect("serialize drafting projection parity payload"),
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
    use super::OrthonormalViewCheck;
    use super::ProjectionCase;
    use super::parity_signature;

    #[test]
    fn parity_signature_is_stable_for_identical_inputs() {
        let projection_cases = vec![ProjectionCase {
            view_direction: "front".to_string(),
            projected_x: 1.0,
            projected_y: 2.0,
            depth: 3.0,
        }];
        let orthonormal_views = vec![OrthonormalViewCheck {
            view_direction: "front".to_string(),
            right_norm: 1.0,
            up_norm: 1.0,
            forward_norm: 1.0,
            dot_right_up: 0.0,
            dot_right_forward: 0.0,
            dot_up_forward: 0.0,
        }];

        let first = parity_signature(
            &projection_cases,
            &orthonormal_views,
            1.0,
            true,
            true,
            true,
            true,
            true,
            "sha",
        );
        let second = parity_signature(
            &projection_cases,
            &orthonormal_views,
            1.0,
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
