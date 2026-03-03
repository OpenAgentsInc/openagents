use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::drafting::{EdgeType, Point2D, ProjectedEdge, ProjectedView, ViewDirection, Visibility};
use crate::export::export_projected_view_to_pdf;
use crate::parity::scorecard::ParityScorecard;
use crate::{CadError, CadResult};

pub const PARITY_DRAFTING_PDF_EXPORT_ISSUE_ID: &str = "VCAD-PARITY-077";
pub const DRAFTING_PDF_EXPORT_REFERENCE_CORPUS_PATH: &str =
    "crates/cad/parity/fixtures/drafting_pdf_export_vcad_reference.json";
const DRAFTING_PDF_EXPORT_REFERENCE_CORPUS_JSON: &str =
    include_str!("../../parity/fixtures/drafting_pdf_export_vcad_reference.json");

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DraftingPdfExportParityManifest {
    pub manifest_version: u64,
    pub issue_id: String,
    pub vcad_commit: String,
    pub openagents_commit: String,
    pub generated_from_scorecard: String,
    pub reference_corpus_path: String,
    pub reference_corpus_sha256: String,
    pub reference_source: String,
    pub reference_commit_match: bool,
    pub case_snapshots: Vec<DraftingPdfExportCaseSnapshot>,
    pub unsupported_contract_match: bool,
    pub deterministic_replay_match: bool,
    pub deterministic_signature: String,
    pub parity_contracts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct DraftingPdfExportReferenceCorpus {
    manifest_version: u64,
    issue_id: String,
    vcad_commit: String,
    source: String,
    expected_case_snapshots: Vec<DraftingPdfExportCaseSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct DraftingPdfExportSnapshot {
    case_snapshots: Vec<DraftingPdfExportCaseSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DraftingPdfExportCaseSnapshot {
    pub case_id: String,
    pub edge_count: usize,
    pub error_format: String,
    pub error_reason: String,
    pub error_message: String,
}

pub fn build_drafting_pdf_export_parity_manifest(
    scorecard: &ParityScorecard,
    scorecard_path: &str,
) -> CadResult<DraftingPdfExportParityManifest> {
    let corpus: DraftingPdfExportReferenceCorpus =
        serde_json::from_str(DRAFTING_PDF_EXPORT_REFERENCE_CORPUS_JSON).map_err(|error| {
            CadError::ParseFailed {
                reason: format!("failed to parse drafting pdf export reference corpus: {error}"),
            }
        })?;

    let reference_corpus_sha256 = sha256_hex(DRAFTING_PDF_EXPORT_REFERENCE_CORPUS_JSON.as_bytes());
    let reference_commit_match = corpus.vcad_commit == scorecard.vcad_commit;

    let snapshot = collect_drafting_pdf_export_snapshot()?;
    let replay_snapshot = collect_drafting_pdf_export_snapshot()?;
    let deterministic_replay_match = snapshot == replay_snapshot;

    let expected_case_snapshots = sorted_cases(corpus.expected_case_snapshots);
    let unsupported_contract_match = snapshot.case_snapshots == expected_case_snapshots;

    let deterministic_signature = parity_signature(
        &snapshot.case_snapshots,
        reference_commit_match,
        unsupported_contract_match,
        deterministic_replay_match,
        &reference_corpus_sha256,
    );

    Ok(DraftingPdfExportParityManifest {
        manifest_version: 1,
        issue_id: PARITY_DRAFTING_PDF_EXPORT_ISSUE_ID.to_string(),
        vcad_commit: scorecard.vcad_commit.clone(),
        openagents_commit: scorecard.openagents_commit.clone(),
        generated_from_scorecard: scorecard_path.to_string(),
        reference_corpus_path: DRAFTING_PDF_EXPORT_REFERENCE_CORPUS_PATH.to_string(),
        reference_corpus_sha256,
        reference_source: corpus.source,
        reference_commit_match,
        case_snapshots: snapshot.case_snapshots,
        unsupported_contract_match,
        deterministic_replay_match,
        deterministic_signature,
        parity_contracts: vec![
            "vcad baseline exposes no native CAD-core drawing PDF exporter".to_string(),
            "drawing PDF export API returns deterministic ExportFailed(format=pdf) contract"
                .to_string(),
            "error reason directs callers to desktop/browser print pipeline".to_string(),
            "pdf export unsupported-contract fixtures replay deterministically".to_string(),
        ],
    })
}

fn collect_drafting_pdf_export_snapshot() -> CadResult<DraftingPdfExportSnapshot> {
    let mixed_visibility = snapshot_for_case("mixed_visibility", &sample_view_mixed_visibility())?;
    let empty_view = snapshot_for_case("empty_view", &sample_view_empty())?;
    let case_snapshots = sorted_cases(vec![mixed_visibility, empty_view]);
    Ok(DraftingPdfExportSnapshot { case_snapshots })
}

fn snapshot_for_case(
    case_id: &str,
    view: &ProjectedView,
) -> CadResult<DraftingPdfExportCaseSnapshot> {
    let error = export_projected_view_to_pdf(view).expect_err("pdf export should be unsupported");
    let (error_format, error_reason) = match &error {
        CadError::ExportFailed { format, reason } => (format.clone(), reason.clone()),
        _ => {
            return Err(CadError::ParseFailed {
                reason: format!(
                    "expected ExportFailed(format=pdf) for {case_id}, received {}",
                    error.code() as u8
                ),
            });
        }
    };
    Ok(DraftingPdfExportCaseSnapshot {
        case_id: case_id.to_string(),
        edge_count: view.edges.len(),
        error_format,
        error_reason,
        error_message: error.to_string(),
    })
}

fn sample_view_mixed_visibility() -> ProjectedView {
    let mut view = ProjectedView::new(ViewDirection::Front);
    view.add_edge(ProjectedEdge::new(
        Point2D::new(0.0, 0.0),
        Point2D::new(20.0, 0.0),
        Visibility::Visible,
        EdgeType::Sharp,
        0.0,
    ));
    view.add_edge(ProjectedEdge::new(
        Point2D::new(20.0, 0.0),
        Point2D::new(20.0, 10.0),
        Visibility::Hidden,
        EdgeType::Boundary,
        0.0,
    ));
    view
}

fn sample_view_empty() -> ProjectedView {
    ProjectedView::new(ViewDirection::Top)
}

fn sorted_cases(
    mut cases: Vec<DraftingPdfExportCaseSnapshot>,
) -> Vec<DraftingPdfExportCaseSnapshot> {
    cases.sort_by(|left, right| left.case_id.cmp(&right.case_id));
    cases
}

fn parity_signature(
    case_snapshots: &[DraftingPdfExportCaseSnapshot],
    reference_commit_match: bool,
    unsupported_contract_match: bool,
    deterministic_replay_match: bool,
    reference_corpus_sha256: &str,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(
        serde_json::to_vec(&(
            case_snapshots,
            reference_commit_match,
            unsupported_contract_match,
            deterministic_replay_match,
            reference_corpus_sha256,
        ))
        .expect("serialize drafting pdf export parity payload"),
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
    use super::{DraftingPdfExportCaseSnapshot, parity_signature};

    #[test]
    fn parity_signature_is_stable_for_identical_inputs() {
        let cases = vec![DraftingPdfExportCaseSnapshot {
            case_id: "mixed_visibility".to_string(),
            edge_count: 2,
            error_format: "pdf".to_string(),
            error_reason: "not implemented".to_string(),
            error_message: "export failed (pdf): not implemented".to_string(),
        }];

        let first = parity_signature(&cases, true, true, true, "sha");
        let second = parity_signature(&cases, true, true, true, "sha");
        assert_eq!(first, second);
    }
}
