use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::mesh::{
    CadMeshBounds, CadMeshMaterialSlot, CadMeshPayload, CadMeshTopology, CadMeshVertex,
};
use crate::parity::scorecard::ParityScorecard;
use crate::stl::{STL_BINARY_HEADER_LABEL, export_stl_from_mesh, import_stl_to_mesh};
use crate::{CadError, CadResult};

pub const PARITY_STL_IMPORT_EXPORT_ISSUE_ID: &str = "VCAD-PARITY-081";
pub const STL_IMPORT_EXPORT_REFERENCE_CORPUS_PATH: &str =
    "crates/cad/parity/fixtures/stl_import_export_vcad_reference.json";
const STL_IMPORT_EXPORT_REFERENCE_CORPUS_JSON: &str =
    include_str!("../../parity/fixtures/stl_import_export_vcad_reference.json");

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StlImportExportParityManifest {
    pub manifest_version: u64,
    pub issue_id: String,
    pub vcad_commit: String,
    pub openagents_commit: String,
    pub generated_from_scorecard: String,
    pub reference_corpus_path: String,
    pub reference_corpus_sha256: String,
    pub reference_source: String,
    pub reference_commit_match: bool,
    pub header_label_match: bool,
    pub case_snapshots: Vec<StlImportExportCaseSnapshot>,
    pub stl_contract_match: bool,
    pub deterministic_replay_match: bool,
    pub deterministic_signature: String,
    pub parity_contracts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct StlImportExportReferenceCorpus {
    manifest_version: u64,
    issue_id: String,
    vcad_commit: String,
    source: String,
    expected_header_label: String,
    expected_case_expectations: Vec<StlImportExportCaseExpectation>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct StlImportExportCaseExpectation {
    case_id: String,
    source_format: Option<String>,
    triangle_count: usize,
    unique_vertex_count: usize,
    export_succeeds: bool,
    import_succeeds: bool,
    error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct StlImportExportSnapshot {
    case_snapshots: Vec<StlImportExportCaseSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StlImportExportCaseSnapshot {
    pub case_id: String,
    pub source_format: Option<String>,
    pub triangle_count: usize,
    pub unique_vertex_count: usize,
    pub export_succeeds: bool,
    pub import_succeeds: bool,
    pub error: Option<String>,
    pub export_hash: Option<String>,
    pub import_hash: Option<String>,
    pub header_prefix: Option<String>,
}

pub fn build_stl_import_export_parity_manifest(
    scorecard: &ParityScorecard,
    scorecard_path: &str,
) -> CadResult<StlImportExportParityManifest> {
    let corpus: StlImportExportReferenceCorpus =
        serde_json::from_str(STL_IMPORT_EXPORT_REFERENCE_CORPUS_JSON).map_err(|error| {
            CadError::ParseFailed {
                reason: format!("failed to parse stl import/export reference corpus: {error}"),
            }
        })?;

    let reference_corpus_sha256 = sha256_hex(STL_IMPORT_EXPORT_REFERENCE_CORPUS_JSON.as_bytes());
    let reference_commit_match = corpus.vcad_commit == scorecard.vcad_commit;
    let header_label_match = corpus.expected_header_label == STL_BINARY_HEADER_LABEL;

    let snapshot = collect_stl_import_export_snapshot()?;
    let replay_snapshot = collect_stl_import_export_snapshot()?;
    let deterministic_replay_match = snapshot == replay_snapshot;

    let expected_contract = sorted_expectations(corpus.expected_case_expectations);
    let actual_contract = sorted_expectations(
        snapshot
            .case_snapshots
            .iter()
            .map(contract_snapshot)
            .collect(),
    );
    let stl_contract_match = header_label_match && actual_contract == expected_contract;

    let deterministic_signature = parity_signature(
        &snapshot.case_snapshots,
        reference_commit_match,
        header_label_match,
        stl_contract_match,
        deterministic_replay_match,
        &reference_corpus_sha256,
    );

    Ok(StlImportExportParityManifest {
        manifest_version: 1,
        issue_id: PARITY_STL_IMPORT_EXPORT_ISSUE_ID.to_string(),
        vcad_commit: scorecard.vcad_commit.clone(),
        openagents_commit: scorecard.openagents_commit.clone(),
        generated_from_scorecard: scorecard_path.to_string(),
        reference_corpus_path: STL_IMPORT_EXPORT_REFERENCE_CORPUS_PATH.to_string(),
        reference_corpus_sha256,
        reference_source: corpus.source,
        reference_commit_match,
        header_label_match,
        case_snapshots: snapshot.case_snapshots,
        stl_contract_match,
        deterministic_replay_match,
        deterministic_signature,
        parity_contracts: vec![
            "binary STL export uses vcad header label and deterministic triangle byte ordering"
                .to_string(),
            "STL import autodetects binary and ASCII formats with vcad-style parser rules"
                .to_string(),
            "STL import deduplicates repeated per-face vertices deterministically".to_string(),
            "STL import/export parity fixtures replay deterministically".to_string(),
        ],
    })
}

fn collect_stl_import_export_snapshot() -> CadResult<StlImportExportSnapshot> {
    let mesh = sample_tetra_mesh();
    let binary_roundtrip = snapshot_binary_roundtrip_case(&mesh)?;
    let ascii_import = snapshot_ascii_import_case()?;
    let truncated_binary = snapshot_truncated_binary_case();

    let case_snapshots = sorted_cases(vec![binary_roundtrip, ascii_import, truncated_binary]);
    Ok(StlImportExportSnapshot { case_snapshots })
}

fn snapshot_binary_roundtrip_case(mesh: &CadMeshPayload) -> CadResult<StlImportExportCaseSnapshot> {
    let exported = export_stl_from_mesh(
        "doc.parity.stl",
        mesh.document_revision,
        &mesh.variant_id,
        mesh,
    )?;
    let imported = import_stl_to_mesh(mesh.document_revision, &mesh.variant_id, &exported.bytes)?;

    let header_prefix = String::from_utf8_lossy(&exported.bytes[..80])
        .trim_end_matches('\0')
        .to_string();

    Ok(StlImportExportCaseSnapshot {
        case_id: "binary_export_roundtrip".to_string(),
        source_format: Some(imported.source_format.as_str().to_string()),
        triangle_count: imported.triangle_count,
        unique_vertex_count: imported.unique_vertex_count,
        export_succeeds: true,
        import_succeeds: true,
        error: None,
        export_hash: Some(exported.receipt.deterministic_hash),
        import_hash: Some(imported.import_hash),
        header_prefix: Some(header_prefix),
    })
}

fn snapshot_ascii_import_case() -> CadResult<StlImportExportCaseSnapshot> {
    let ascii = "solid fixture\n\
  facet normal 0 0 1\n\
    outer loop\n\
      vertex 0 0 0\n\
      vertex 10 0 0\n\
      vertex 0 10 0\n\
    endloop\n\
  endfacet\n\
  facet normal 0 0 1\n\
    outer loop\n\
      vertex 10 0 0\n\
      vertex 10 10 0\n\
      vertex 0 10 0\n\
    endloop\n\
  endfacet\n\
endsolid fixture\n";
    let imported = import_stl_to_mesh(44, "variant.parity.stl.ascii", ascii.as_bytes())?;

    Ok(StlImportExportCaseSnapshot {
        case_id: "ascii_import".to_string(),
        source_format: Some(imported.source_format.as_str().to_string()),
        triangle_count: imported.triangle_count,
        unique_vertex_count: imported.unique_vertex_count,
        export_succeeds: false,
        import_succeeds: true,
        error: None,
        export_hash: None,
        import_hash: Some(imported.import_hash),
        header_prefix: None,
    })
}

fn snapshot_truncated_binary_case() -> StlImportExportCaseSnapshot {
    let error = import_stl_to_mesh(44, "variant.parity.stl.bad", &[0u8; 12])
        .expect_err("truncated binary stl should fail");

    StlImportExportCaseSnapshot {
        case_id: "truncated_binary_import".to_string(),
        source_format: None,
        triangle_count: 0,
        unique_vertex_count: 0,
        export_succeeds: false,
        import_succeeds: false,
        error: Some(error_reason(&error)),
        export_hash: None,
        import_hash: None,
        header_prefix: None,
    }
}

fn error_reason(error: &CadError) -> String {
    match error {
        CadError::ParseFailed { reason } => reason.clone(),
        _ => error.to_string(),
    }
}

fn sample_tetra_mesh() -> CadMeshPayload {
    CadMeshPayload {
        mesh_id: "mesh.parity.stl".to_string(),
        document_revision: 44,
        variant_id: "variant.parity.stl".to_string(),
        topology: CadMeshTopology::Triangles,
        vertices: vec![
            CadMeshVertex {
                position_mm: [0.0, 0.0, 0.0],
                normal: [0.0, 0.0, 1.0],
                uv: [0.0, 0.0],
                material_slot: 0,
                flags: 0,
            },
            CadMeshVertex {
                position_mm: [20.0, 0.0, 0.0],
                normal: [0.0, 0.0, 1.0],
                uv: [1.0, 0.0],
                material_slot: 0,
                flags: 0,
            },
            CadMeshVertex {
                position_mm: [0.0, 20.0, 0.0],
                normal: [0.0, 0.0, 1.0],
                uv: [0.0, 1.0],
                material_slot: 0,
                flags: 0,
            },
            CadMeshVertex {
                position_mm: [0.0, 0.0, 20.0],
                normal: [0.0, 1.0, 0.0],
                uv: [0.5, 0.5],
                material_slot: 0,
                flags: 0,
            },
        ],
        triangle_indices: vec![0, 1, 2, 0, 1, 3, 1, 2, 3, 0, 2, 3],
        edges: Vec::new(),
        material_slots: vec![CadMeshMaterialSlot::default()],
        bounds: CadMeshBounds {
            min_mm: [0.0, 0.0, 0.0],
            max_mm: [20.0, 20.0, 20.0],
        },
    }
}

fn contract_snapshot(case: &StlImportExportCaseSnapshot) -> StlImportExportCaseExpectation {
    StlImportExportCaseExpectation {
        case_id: case.case_id.clone(),
        source_format: case.source_format.clone(),
        triangle_count: case.triangle_count,
        unique_vertex_count: case.unique_vertex_count,
        export_succeeds: case.export_succeeds,
        import_succeeds: case.import_succeeds,
        error: case.error.clone(),
    }
}

fn sorted_cases(mut cases: Vec<StlImportExportCaseSnapshot>) -> Vec<StlImportExportCaseSnapshot> {
    cases.sort_by(|left, right| left.case_id.cmp(&right.case_id));
    cases
}

fn sorted_expectations(
    mut expectations: Vec<StlImportExportCaseExpectation>,
) -> Vec<StlImportExportCaseExpectation> {
    expectations.sort_by(|left, right| left.case_id.cmp(&right.case_id));
    expectations
}

fn parity_signature(
    case_snapshots: &[StlImportExportCaseSnapshot],
    reference_commit_match: bool,
    header_label_match: bool,
    stl_contract_match: bool,
    deterministic_replay_match: bool,
    reference_corpus_sha256: &str,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(format!(
        "ref_match={reference_commit_match};header={header_label_match};contract={stl_contract_match};replay={deterministic_replay_match};ref_sha={reference_corpus_sha256}"
    ));
    for case in case_snapshots {
        hasher.update(
            serde_json::to_vec(case).expect("stl import/export case snapshots should serialize"),
        );
    }
    format!("{:x}", hasher.finalize())[..16].to_string()
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{PARITY_STL_IMPORT_EXPORT_ISSUE_ID, build_stl_import_export_parity_manifest};
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
    fn build_manifest_tracks_stl_import_export_contracts() {
        let manifest = build_stl_import_export_parity_manifest(&mock_scorecard(), "scorecard.json")
            .expect("manifest");
        assert_eq!(manifest.issue_id, PARITY_STL_IMPORT_EXPORT_ISSUE_ID);
        assert!(manifest.reference_commit_match);
        assert!(manifest.header_label_match);
        assert!(manifest.stl_contract_match);
        assert!(manifest.deterministic_replay_match);
        assert_eq!(manifest.case_snapshots.len(), 3);
    }
}
