use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::drafting::{EdgeType, Point2D, ProjectedEdge, ProjectedView, ViewDirection, Visibility};
use crate::export::export_projected_view_to_dxf;
use crate::parity::reference_table_parity::canonicalize_scorecard_path;
use crate::parity::scorecard::ParityScorecard;
use crate::{CadError, CadResult};

pub const PARITY_DRAFTING_DXF_EXPORT_ISSUE_ID: &str = "VCAD-PARITY-076";
pub const DRAFTING_DXF_EXPORT_REFERENCE_CORPUS_PATH: &str =
    "crates/cad/parity/fixtures/drafting_dxf_export_vcad_reference.json";
const DRAFTING_DXF_EXPORT_REFERENCE_CORPUS_JSON: &str =
    include_str!("../../parity/fixtures/drafting_dxf_export_vcad_reference.json");

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DraftingDxfExportParityManifest {
    pub manifest_version: u64,
    pub issue_id: String,
    pub vcad_commit: String,
    pub openagents_commit: String,
    pub generated_from_scorecard: String,
    pub reference_corpus_path: String,
    pub reference_corpus_sha256: String,
    pub reference_source: String,
    pub reference_commit_match: bool,
    pub case_snapshots: Vec<DraftingDxfCaseSnapshot>,
    pub dxf_contract_match: bool,
    pub deterministic_replay_match: bool,
    pub deterministic_signature: String,
    pub parity_contracts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct DraftingDxfExportReferenceCorpus {
    manifest_version: u64,
    issue_id: String,
    vcad_commit: String,
    source: String,
    expected_case_snapshots: Vec<DraftingDxfCaseSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct DraftingDxfSnapshot {
    case_snapshots: Vec<DraftingDxfCaseSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DraftingDxfCaseSnapshot {
    pub case_id: String,
    pub edge_count: usize,
    pub visible_edge_count: usize,
    pub hidden_edge_count: usize,
    pub line_entity_count: usize,
    pub entity_visible_layer_count: usize,
    pub entity_hidden_layer_count: usize,
    pub byte_count: usize,
    pub deterministic_hash: String,
    pub ends_with_eof: bool,
}

pub fn build_drafting_dxf_export_parity_manifest(
    scorecard: &ParityScorecard,
    scorecard_path: &str,
) -> CadResult<DraftingDxfExportParityManifest> {
    let corpus: DraftingDxfExportReferenceCorpus =
        serde_json::from_str(DRAFTING_DXF_EXPORT_REFERENCE_CORPUS_JSON).map_err(|error| {
            CadError::ParseFailed {
                reason: format!("failed to parse drafting dxf export reference corpus: {error}"),
            }
        })?;

    let reference_corpus_sha256 = sha256_hex(DRAFTING_DXF_EXPORT_REFERENCE_CORPUS_JSON.as_bytes());
    let reference_commit_match = corpus.vcad_commit == scorecard.vcad_commit;

    let snapshot = collect_drafting_dxf_snapshot()?;
    let replay_snapshot = collect_drafting_dxf_snapshot()?;
    let deterministic_replay_match = snapshot == replay_snapshot;

    let expected_case_snapshots = sorted_cases(corpus.expected_case_snapshots);
    let dxf_contract_match = snapshot.case_snapshots == expected_case_snapshots;

    let deterministic_signature = parity_signature(
        &snapshot.case_snapshots,
        reference_commit_match,
        dxf_contract_match,
        deterministic_replay_match,
        &reference_corpus_sha256,
    );

    Ok(DraftingDxfExportParityManifest {
        manifest_version: 1,
        issue_id: PARITY_DRAFTING_DXF_EXPORT_ISSUE_ID.to_string(),
        vcad_commit: scorecard.vcad_commit.clone(),
        openagents_commit: scorecard.openagents_commit.clone(),
        generated_from_scorecard: canonicalize_scorecard_path(scorecard_path),
        reference_corpus_path: DRAFTING_DXF_EXPORT_REFERENCE_CORPUS_PATH.to_string(),
        reference_corpus_sha256,
        reference_source: corpus.source,
        reference_commit_match,
        case_snapshots: snapshot.case_snapshots,
        dxf_contract_match,
        deterministic_replay_match,
        deterministic_signature,
        parity_contracts: vec![
            "DXF drafting exports emit AC1009 + INSUNITS millimeter header".to_string(),
            "visible edges map to VISIBLE/CONTINUOUS and hidden edges map to HIDDEN/HIDDEN"
                .to_string(),
            "LINE entities preserve projected edge ordering and six-decimal coordinates"
                .to_string(),
            "drafting dxf export fixtures replay deterministically".to_string(),
        ],
    })
}

fn collect_drafting_dxf_snapshot() -> CadResult<DraftingDxfSnapshot> {
    let mixed_visibility = snapshot_for_case("mixed_visibility", &sample_view_mixed_visibility())?;
    let visible_only = snapshot_for_case("visible_only", &sample_view_visible_only())?;
    let empty = snapshot_for_case("empty_view", &sample_view_empty())?;

    let case_snapshots = sorted_cases(vec![mixed_visibility, visible_only, empty]);
    Ok(DraftingDxfSnapshot { case_snapshots })
}

fn snapshot_for_case(case_id: &str, view: &ProjectedView) -> CadResult<DraftingDxfCaseSnapshot> {
    let artifact = export_projected_view_to_dxf(view)?;
    let edge_count = artifact.receipt.edge_count;
    let visible_edge_count = artifact.receipt.visible_edge_count;
    let hidden_edge_count = artifact.receipt.hidden_edge_count;
    let byte_count = artifact.receipt.byte_count;
    let deterministic_hash = artifact.receipt.deterministic_hash.clone();
    let text = artifact.text()?.to_string();
    Ok(DraftingDxfCaseSnapshot {
        case_id: case_id.to_string(),
        edge_count,
        visible_edge_count,
        hidden_edge_count,
        line_entity_count: text.match_indices("\nLINE\n").count(),
        entity_visible_layer_count: text.match_indices("\n8\nVISIBLE\n6\nCONTINUOUS\n").count(),
        entity_hidden_layer_count: text.match_indices("\n8\nHIDDEN\n6\nHIDDEN\n").count(),
        byte_count,
        deterministic_hash,
        ends_with_eof: text.ends_with("0\nEOF\n"),
    })
}

fn sample_view_mixed_visibility() -> ProjectedView {
    let mut view = ProjectedView::new(ViewDirection::Front);
    view.add_edge(ProjectedEdge::new(
        Point2D::new(0.0, 0.0),
        Point2D::new(40.0, 0.0),
        Visibility::Visible,
        EdgeType::Sharp,
        0.0,
    ));
    view.add_edge(ProjectedEdge::new(
        Point2D::new(40.0, 0.0),
        Point2D::new(40.0, 25.0),
        Visibility::Hidden,
        EdgeType::Boundary,
        0.0,
    ));
    view.add_edge(ProjectedEdge::new(
        Point2D::new(40.0, 25.0),
        Point2D::new(0.0, 25.0),
        Visibility::Visible,
        EdgeType::Silhouette,
        0.0,
    ));
    view
}

fn sample_view_visible_only() -> ProjectedView {
    let mut view = ProjectedView::new(ViewDirection::Top);
    view.add_edge(ProjectedEdge::new(
        Point2D::new(-10.5, 2.25),
        Point2D::new(3.75, 8.0),
        Visibility::Visible,
        EdgeType::Sharp,
        0.0,
    ));
    view
}

fn sample_view_empty() -> ProjectedView {
    ProjectedView::new(ViewDirection::Isometric {
        azimuth: std::f64::consts::FRAC_PI_6,
        elevation: std::f64::consts::FRAC_PI_6,
    })
}

fn sorted_cases(mut cases: Vec<DraftingDxfCaseSnapshot>) -> Vec<DraftingDxfCaseSnapshot> {
    cases.sort_by(|left, right| left.case_id.cmp(&right.case_id));
    cases
}

fn parity_signature(
    case_snapshots: &[DraftingDxfCaseSnapshot],
    reference_commit_match: bool,
    dxf_contract_match: bool,
    deterministic_replay_match: bool,
    reference_corpus_sha256: &str,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(
        serde_json::to_vec(&(
            case_snapshots,
            reference_commit_match,
            dxf_contract_match,
            deterministic_replay_match,
            reference_corpus_sha256,
        ))
        .expect("serialize drafting dxf export parity payload"),
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
    use super::{DraftingDxfCaseSnapshot, parity_signature};
    use crate::parity::reference_table_parity::canonicalize_scorecard_path;

    #[test]
    fn parity_signature_is_stable_for_identical_inputs() {
        let cases = vec![DraftingDxfCaseSnapshot {
            case_id: "mixed_visibility".to_string(),
            edge_count: 3,
            visible_edge_count: 2,
            hidden_edge_count: 1,
            line_entity_count: 3,
            entity_visible_layer_count: 2,
            entity_hidden_layer_count: 1,
            byte_count: 1234,
            deterministic_hash: "abcdef0123456789".to_string(),
            ends_with_eof: true,
        }];

        let first = parity_signature(&cases, true, true, true, "sha");
        let second = parity_signature(&cases, true, true, true, "sha");
        assert_eq!(first, second);
    }

    #[test]
    fn normalize_scorecard_path_strips_machine_specific_prefix() {
        assert_eq!(
            canonicalize_scorecard_path(
                "/Users/christopherdavid/code/openagents/crates/cad/parity/parity_scorecard.json"
            ),
            "/home/christopherdavid/code/openagents/crates/cad/parity/parity_scorecard.json"
        );
        assert_eq!(
            canonicalize_scorecard_path(
                "/home/christopherdavid/code/openagents/crates/cad/parity/parity_scorecard.json"
            ),
            "/home/christopherdavid/code/openagents/crates/cad/parity/parity_scorecard.json"
        );
    }
}
