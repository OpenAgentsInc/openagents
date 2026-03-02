use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::drafting::ViewDirection;
use crate::drafting::hidden_line::{
    DraftingProjectionOptions, DraftingTriangleMesh, project_mesh_with_options,
};
use crate::kernel_math::Point3;
use crate::parity::scorecard::ParityScorecard;
use crate::{CadError, CadResult};

pub const PARITY_DRAFTING_HIDDEN_LINE_ISSUE_ID: &str = "VCAD-PARITY-069";
pub const DRAFTING_HIDDEN_LINE_REFERENCE_CORPUS_PATH: &str =
    "crates/cad/parity/fixtures/drafting_hidden_line_vcad_reference.json";
const DRAFTING_HIDDEN_LINE_REFERENCE_CORPUS_JSON: &str =
    include_str!("../../parity/fixtures/drafting_hidden_line_vcad_reference.json");

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DraftingHiddenLineParityManifest {
    pub manifest_version: u64,
    pub issue_id: String,
    pub vcad_commit: String,
    pub openagents_commit: String,
    pub generated_from_scorecard: String,
    pub reference_corpus_path: String,
    pub reference_corpus_sha256: String,
    pub reference_source: String,
    pub reference_commit_match: bool,
    pub view_reports: Vec<HiddenLineViewReport>,
    pub front_contract_match: bool,
    pub top_contract_match: bool,
    pub isometric_contract_match: bool,
    pub occlusion_probe_contract_match: bool,
    pub visibility_deterministic_match: bool,
    pub deterministic_replay_match: bool,
    pub deterministic_signature: String,
    pub parity_contracts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct DraftingHiddenLineReferenceCorpus {
    manifest_version: u64,
    issue_id: String,
    vcad_commit: String,
    source: String,
    projection_options: DraftingProjectionOptions,
    front_contract: HiddenLineViewContract,
    top_contract: HiddenLineViewContract,
    isometric_contract: IsometricHiddenLineContract,
    occlusion_probe_contract: HiddenLineViewContract,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HiddenLineViewReport {
    pub view_direction: String,
    pub total_edges: usize,
    pub visible_edges: usize,
    pub hidden_edges: usize,
    pub bounds_width: f64,
    pub bounds_height: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct HiddenLineViewContract {
    min_total_edges: usize,
    min_visible_edges: usize,
    min_hidden_edges: usize,
    bounds_width_min: f64,
    bounds_width_max: f64,
    bounds_height_min: f64,
    bounds_height_max: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct IsometricHiddenLineContract {
    total_edges: usize,
    require_hidden_edges: bool,
    min_visible_edges: usize,
}

pub fn build_drafting_hidden_line_parity_manifest(
    scorecard: &ParityScorecard,
    scorecard_path: &str,
) -> CadResult<DraftingHiddenLineParityManifest> {
    let corpus: DraftingHiddenLineReferenceCorpus =
        serde_json::from_str(DRAFTING_HIDDEN_LINE_REFERENCE_CORPUS_JSON).map_err(|error| {
            CadError::ParseFailed {
                reason: format!("failed to parse drafting hidden-line reference corpus: {error}"),
            }
        })?;

    let reference_corpus_sha256 = sha256_hex(DRAFTING_HIDDEN_LINE_REFERENCE_CORPUS_JSON.as_bytes());
    let reference_commit_match = corpus.vcad_commit == scorecard.vcad_commit;

    let mesh = build_cube_mesh(1.0);
    let snapshot = collect_view_reports(&mesh, corpus.projection_options);
    let replay_snapshot = collect_view_reports(&mesh, corpus.projection_options);
    let deterministic_replay_match = snapshot == replay_snapshot;

    let front_report = find_report(&snapshot, "front")?;
    let top_report = find_report(&snapshot, "top")?;
    let isometric_report = find_report(&snapshot, "isometric")?;

    let front_contract_match = matches_contract(front_report, &corpus.front_contract);
    let top_contract_match = matches_contract(top_report, &corpus.top_contract);
    let isometric_contract_match =
        isometric_matches_contract(isometric_report, &corpus.isometric_contract);
    let occlusion_probe_contract_match = matches_contract(
        find_report(&snapshot, "occlusion_probe")?,
        &corpus.occlusion_probe_contract,
    );

    let visibility_deterministic_match =
        project_mesh_with_options(&mesh, ViewDirection::Front, corpus.projection_options).edges
            == project_mesh_with_options(&mesh, ViewDirection::Front, corpus.projection_options)
                .edges;

    let deterministic_signature = parity_signature(
        &snapshot,
        reference_commit_match,
        front_contract_match,
        top_contract_match,
        isometric_contract_match,
        occlusion_probe_contract_match,
        visibility_deterministic_match,
        deterministic_replay_match,
        &reference_corpus_sha256,
    );

    Ok(DraftingHiddenLineParityManifest {
        manifest_version: 1,
        issue_id: PARITY_DRAFTING_HIDDEN_LINE_ISSUE_ID.to_string(),
        vcad_commit: scorecard.vcad_commit.clone(),
        openagents_commit: scorecard.openagents_commit.clone(),
        generated_from_scorecard: scorecard_path.to_string(),
        reference_corpus_path: DRAFTING_HIDDEN_LINE_REFERENCE_CORPUS_PATH.to_string(),
        reference_corpus_sha256,
        reference_source: corpus.source,
        reference_commit_match,
        view_reports: snapshot,
        front_contract_match,
        top_contract_match,
        isometric_contract_match,
        occlusion_probe_contract_match,
        visibility_deterministic_match,
        deterministic_replay_match,
        deterministic_signature,
        parity_contracts: vec![
            "front/top drafting views produce stable edge sets with unit-cube bounds".to_string(),
            "isometric drafting view retains all 12 cube edges".to_string(),
            "occlusion probe scene retains hidden edges for back-layer geometry".to_string(),
            "hidden-line occlusion classification is deterministic across repeated runs"
                .to_string(),
            "drafting hidden-line parity fixtures replay deterministically".to_string(),
        ],
    })
}

fn collect_view_reports(
    mesh: &DraftingTriangleMesh,
    options: DraftingProjectionOptions,
) -> Vec<HiddenLineViewReport> {
    let views = [
        ("front", ViewDirection::Front),
        ("top", ViewDirection::Top),
        ("isometric", ViewDirection::ISOMETRIC_STANDARD),
    ];

    let mut reports = Vec::with_capacity(views.len() + 1);
    for (view_tag, view_direction) in views {
        let projected = project_mesh_with_options(mesh, view_direction, options);
        reports.push(HiddenLineViewReport {
            view_direction: view_tag.to_string(),
            total_edges: projected.edges.len(),
            visible_edges: projected.num_visible(),
            hidden_edges: projected.num_hidden(),
            bounds_width: projected.bounds.width(),
            bounds_height: projected.bounds.height(),
        });
    }

    let probe_mesh = build_occlusion_probe_mesh();
    let probe_projected = project_mesh_with_options(&probe_mesh, ViewDirection::Front, options);
    reports.push(HiddenLineViewReport {
        view_direction: "occlusion_probe".to_string(),
        total_edges: probe_projected.edges.len(),
        visible_edges: probe_projected.num_visible(),
        hidden_edges: probe_projected.num_hidden(),
        bounds_width: probe_projected.bounds.width(),
        bounds_height: probe_projected.bounds.height(),
    });

    reports.sort_by(|left, right| left.view_direction.cmp(&right.view_direction));
    reports
}

fn find_report<'a>(
    reports: &'a [HiddenLineViewReport],
    view_direction: &str,
) -> CadResult<&'a HiddenLineViewReport> {
    reports
        .iter()
        .find(|report| report.view_direction == view_direction)
        .ok_or_else(|| CadError::ParseFailed {
            reason: format!("missing hidden-line report for view '{view_direction}'"),
        })
}

fn matches_contract(report: &HiddenLineViewReport, contract: &HiddenLineViewContract) -> bool {
    report.total_edges >= contract.min_total_edges
        && report.visible_edges >= contract.min_visible_edges
        && report.hidden_edges >= contract.min_hidden_edges
        && report.bounds_width >= contract.bounds_width_min
        && report.bounds_width <= contract.bounds_width_max
        && report.bounds_height >= contract.bounds_height_min
        && report.bounds_height <= contract.bounds_height_max
}

fn isometric_matches_contract(
    report: &HiddenLineViewReport,
    contract: &IsometricHiddenLineContract,
) -> bool {
    report.total_edges == contract.total_edges
        && report.visible_edges >= contract.min_visible_edges
        && (!contract.require_hidden_edges || report.hidden_edges > 0)
}

fn build_cube_mesh(size: f64) -> DraftingTriangleMesh {
    let s = size;
    let vertices = vec![
        Point3::new(0.0, 0.0, 0.0),
        Point3::new(s, 0.0, 0.0),
        Point3::new(s, s, 0.0),
        Point3::new(0.0, s, 0.0),
        Point3::new(0.0, 0.0, s),
        Point3::new(s, 0.0, s),
        Point3::new(s, s, s),
        Point3::new(0.0, s, s),
    ];

    let triangles = vec![
        [0, 2, 1],
        [0, 3, 2],
        [4, 5, 6],
        [4, 6, 7],
        [0, 1, 5],
        [0, 5, 4],
        [2, 3, 7],
        [2, 7, 6],
        [0, 4, 7],
        [0, 7, 3],
        [1, 2, 6],
        [1, 6, 5],
    ];

    DraftingTriangleMesh {
        vertices,
        triangles,
    }
}

fn build_occlusion_probe_mesh() -> DraftingTriangleMesh {
    let vertices = vec![
        Point3::new(0.0, 0.0, 0.0),
        Point3::new(1.0, 0.0, 0.0),
        Point3::new(1.0, 0.0, 1.0),
        Point3::new(0.0, 0.0, 1.0),
        Point3::new(0.0, 1.0, 0.0),
        Point3::new(1.0, 1.0, 0.0),
        Point3::new(1.0, 1.0, 1.0),
        Point3::new(0.0, 1.0, 1.0),
    ];

    // Two stacked quads with +Y winding so the front quad can occlude the back quad
    // for `ViewDirection::Front` (view vector +Y).
    let triangles = vec![[0, 3, 2], [0, 2, 1], [4, 7, 6], [4, 6, 5]];

    DraftingTriangleMesh {
        vertices,
        triangles,
    }
}

fn parity_signature(
    view_reports: &[HiddenLineViewReport],
    reference_commit_match: bool,
    front_contract_match: bool,
    top_contract_match: bool,
    isometric_contract_match: bool,
    occlusion_probe_contract_match: bool,
    visibility_deterministic_match: bool,
    deterministic_replay_match: bool,
    reference_corpus_sha256: &str,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(
        serde_json::to_vec(&(
            view_reports,
            reference_commit_match,
            front_contract_match,
            top_contract_match,
            isometric_contract_match,
            occlusion_probe_contract_match,
            visibility_deterministic_match,
            deterministic_replay_match,
            reference_corpus_sha256,
        ))
        .expect("serialize drafting hidden-line parity payload"),
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
    use super::HiddenLineViewReport;
    use super::parity_signature;

    #[test]
    fn parity_signature_is_stable_for_identical_inputs() {
        let reports = vec![HiddenLineViewReport {
            view_direction: "front".to_string(),
            total_edges: 12,
            visible_edges: 4,
            hidden_edges: 8,
            bounds_width: 1.0,
            bounds_height: 1.0,
        }];

        let first = parity_signature(&reports, true, true, true, true, true, true, true, "sha");
        let second = parity_signature(&reports, true, true, true, true, true, true, true, "sha");
        assert_eq!(first, second);
    }
}
