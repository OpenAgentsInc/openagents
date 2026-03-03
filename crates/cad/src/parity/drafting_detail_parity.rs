use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::drafting::{
    DetailViewParams, EdgeType, Point2D, ProjectedEdge, ProjectedView, ViewDirection, Visibility,
    create_detail_view,
};
use crate::parity::scorecard::ParityScorecard;
use crate::{CadError, CadResult};

pub const PARITY_DRAFTING_DETAIL_ISSUE_ID: &str = "VCAD-PARITY-073";
pub const DRAFTING_DETAIL_REFERENCE_CORPUS_PATH: &str =
    "crates/cad/parity/fixtures/drafting_detail_vcad_reference.json";
const DRAFTING_DETAIL_REFERENCE_CORPUS_JSON: &str =
    include_str!("../../parity/fixtures/drafting_detail_vcad_reference.json");

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DraftingDetailParityManifest {
    pub manifest_version: u64,
    pub issue_id: String,
    pub vcad_commit: String,
    pub openagents_commit: String,
    pub generated_from_scorecard: String,
    pub reference_corpus_path: String,
    pub reference_corpus_sha256: String,
    pub reference_source: String,
    pub reference_commit_match: bool,
    pub detail_cases: Vec<DetailCase>,
    pub detail_contract_match: bool,
    pub deterministic_replay_match: bool,
    pub deterministic_signature: String,
    pub parity_contracts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct DraftingDetailReferenceCorpus {
    manifest_version: u64,
    issue_id: String,
    vcad_commit: String,
    source: String,
    scalar_tolerance: f64,
    expected_detail_cases: Vec<DetailCase>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DetailCase {
    pub case_id: String,
    pub edge_count: usize,
    pub hidden_edge_count: usize,
    pub bounds_width: f64,
    pub bounds_height: f64,
    pub first_edge_length: f64,
    pub scale: f64,
    pub label: String,
}

pub fn build_drafting_detail_parity_manifest(
    scorecard: &ParityScorecard,
    scorecard_path: &str,
) -> CadResult<DraftingDetailParityManifest> {
    let corpus: DraftingDetailReferenceCorpus =
        serde_json::from_str(DRAFTING_DETAIL_REFERENCE_CORPUS_JSON).map_err(|error| {
            CadError::ParseFailed {
                reason: format!("failed to parse drafting detail reference corpus: {error}"),
            }
        })?;

    let reference_corpus_sha256 = sha256_hex(DRAFTING_DETAIL_REFERENCE_CORPUS_JSON.as_bytes());
    let reference_commit_match = corpus.vcad_commit == scorecard.vcad_commit;

    let snapshot = collect_detail_snapshot();
    let replay_snapshot = collect_detail_snapshot();
    let deterministic_replay_match = snapshot == replay_snapshot;

    let expected_detail_cases = sorted_cases(corpus.expected_detail_cases);
    let detail_contract_match = detail_cases_match(
        &snapshot.detail_cases,
        &expected_detail_cases,
        corpus.scalar_tolerance,
    );

    let deterministic_signature = parity_signature(
        &snapshot.detail_cases,
        reference_commit_match,
        detail_contract_match,
        deterministic_replay_match,
        &reference_corpus_sha256,
    );

    Ok(DraftingDetailParityManifest {
        manifest_version: 1,
        issue_id: PARITY_DRAFTING_DETAIL_ISSUE_ID.to_string(),
        vcad_commit: scorecard.vcad_commit.clone(),
        openagents_commit: scorecard.openagents_commit.clone(),
        generated_from_scorecard: scorecard_path.to_string(),
        reference_corpus_path: DRAFTING_DETAIL_REFERENCE_CORPUS_PATH.to_string(),
        reference_corpus_sha256,
        reference_source: corpus.source,
        reference_commit_match,
        detail_cases: snapshot.detail_cases,
        detail_contract_match,
        deterministic_replay_match,
        deterministic_signature,
        parity_contracts: vec![
            "detail views clip parent projected edges to the selected region".to_string(),
            "detail view transform recenters to origin and scales geometry deterministically"
                .to_string(),
            "detail output preserves visibility classifications for clipped edges".to_string(),
            "detail-view parity fixtures replay deterministically".to_string(),
        ],
    })
}

#[derive(Debug, Clone, PartialEq)]
struct DetailSnapshot {
    detail_cases: Vec<DetailCase>,
}

fn collect_detail_snapshot() -> DetailSnapshot {
    let mut parent = ProjectedView::new(ViewDirection::Front);
    parent.add_edge(ProjectedEdge::new(
        Point2D::new(0.0, 0.0),
        Point2D::new(100.0, 0.0),
        Visibility::Visible,
        EdgeType::Sharp,
        0.0,
    ));
    parent.add_edge(ProjectedEdge::new(
        Point2D::new(100.0, 0.0),
        Point2D::new(100.0, 100.0),
        Visibility::Visible,
        EdgeType::Sharp,
        0.0,
    ));
    parent.add_edge(ProjectedEdge::new(
        Point2D::new(100.0, 100.0),
        Point2D::new(0.0, 100.0),
        Visibility::Visible,
        EdgeType::Sharp,
        0.0,
    ));
    parent.add_edge(ProjectedEdge::new(
        Point2D::new(0.0, 100.0),
        Point2D::new(0.0, 0.0),
        Visibility::Visible,
        EdgeType::Sharp,
        0.0,
    ));
    parent.add_edge(ProjectedEdge::new(
        Point2D::new(0.0, 0.0),
        Point2D::new(100.0, 100.0),
        Visibility::Hidden,
        EdgeType::Sharp,
        0.0,
    ));

    let corner_zoom = create_detail_view(
        &parent,
        &DetailViewParams {
            center: Point2D::new(25.0, 25.0),
            width: 50.0,
            height: 50.0,
            scale: 2.0,
            label: "A".to_string(),
        },
    );
    let outside_region = create_detail_view(
        &parent,
        &DetailViewParams {
            center: Point2D::new(200.0, 200.0),
            width: 20.0,
            height: 20.0,
            scale: 2.0,
            label: "B".to_string(),
        },
    );
    let center_zoom_scale3 = create_detail_view(
        &parent,
        &DetailViewParams {
            center: Point2D::new(50.0, 50.0),
            width: 40.0,
            height: 40.0,
            scale: 3.0,
            label: "C".to_string(),
        },
    );

    let detail_cases = sorted_cases(vec![
        detail_case("center_zoom_scale3", &center_zoom_scale3),
        detail_case("corner_zoom", &corner_zoom),
        detail_case("outside_region", &outside_region),
    ]);

    DetailSnapshot { detail_cases }
}

fn detail_case(case_id: &str, detail: &crate::drafting::DetailView) -> DetailCase {
    DetailCase {
        case_id: case_id.to_string(),
        edge_count: detail.edges.len(),
        hidden_edge_count: detail
            .edges
            .iter()
            .filter(|edge| edge.visibility == Visibility::Hidden)
            .count(),
        bounds_width: detail.bounds.width(),
        bounds_height: detail.bounds.height(),
        first_edge_length: detail
            .edges
            .first()
            .map(|edge| edge.length())
            .unwrap_or(0.0),
        scale: detail.scale,
        label: detail.label.clone(),
    }
}

fn sorted_cases(mut cases: Vec<DetailCase>) -> Vec<DetailCase> {
    cases.sort_by(|left, right| left.case_id.cmp(&right.case_id));
    cases
}

fn detail_cases_match(actual: &[DetailCase], expected: &[DetailCase], tolerance: f64) -> bool {
    if actual.len() != expected.len() {
        return false;
    }

    actual.iter().zip(expected.iter()).all(|(left, right)| {
        left.case_id == right.case_id
            && left.edge_count == right.edge_count
            && left.hidden_edge_count == right.hidden_edge_count
            && left.label == right.label
            && approx_eq(left.bounds_width, right.bounds_width, tolerance)
            && approx_eq(left.bounds_height, right.bounds_height, tolerance)
            && approx_eq(left.first_edge_length, right.first_edge_length, tolerance)
            && approx_eq(left.scale, right.scale, tolerance)
    })
}

fn approx_eq(left: f64, right: f64, tolerance: f64) -> bool {
    (left - right).abs() <= tolerance
}

fn parity_signature(
    detail_cases: &[DetailCase],
    reference_commit_match: bool,
    detail_contract_match: bool,
    deterministic_replay_match: bool,
    reference_corpus_sha256: &str,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(
        serde_json::to_vec(&(
            detail_cases,
            reference_commit_match,
            detail_contract_match,
            deterministic_replay_match,
            reference_corpus_sha256,
        ))
        .expect("serialize drafting detail parity payload"),
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
    use super::DetailCase;
    use super::parity_signature;

    #[test]
    fn parity_signature_is_stable_for_identical_inputs() {
        let cases = vec![DetailCase {
            case_id: "corner_zoom".to_string(),
            edge_count: 3,
            hidden_edge_count: 1,
            bounds_width: 100.0,
            bounds_height: 100.0,
            first_edge_length: 100.0,
            scale: 2.0,
            label: "A".to_string(),
        }];

        let first = parity_signature(&cases, true, true, true, "sha");
        let second = parity_signature(&cases, true, true, true, "sha");
        assert_eq!(first, second);
    }
}
