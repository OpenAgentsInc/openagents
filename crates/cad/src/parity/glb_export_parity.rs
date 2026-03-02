use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::glb::export_glb_from_mesh;
use crate::mesh::{
    CadMeshBounds, CadMeshMaterialSlot, CadMeshPayload, CadMeshTopology, CadMeshVertex,
};
use crate::parity::scorecard::ParityScorecard;
use crate::{CadError, CadResult};

pub const PARITY_GLB_EXPORT_ISSUE_ID: &str = "VCAD-PARITY-082";
pub const GLB_EXPORT_REFERENCE_CORPUS_PATH: &str =
    "crates/cad/parity/fixtures/glb_export_vcad_reference.json";
const GLB_EXPORT_REFERENCE_CORPUS_JSON: &str =
    include_str!("../../parity/fixtures/glb_export_vcad_reference.json");

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GlbExportParityManifest {
    pub manifest_version: u64,
    pub issue_id: String,
    pub vcad_commit: String,
    pub openagents_commit: String,
    pub generated_from_scorecard: String,
    pub reference_corpus_path: String,
    pub reference_corpus_sha256: String,
    pub reference_source: String,
    pub reference_commit_match: bool,
    pub case_snapshots: Vec<GlbExportCaseSnapshot>,
    pub glb_contract_match: bool,
    pub deterministic_replay_match: bool,
    pub deterministic_signature: String,
    pub parity_contracts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct GlbExportReferenceCorpus {
    manifest_version: u64,
    issue_id: String,
    vcad_commit: String,
    source: String,
    expected_case_expectations: Vec<GlbExportCaseExpectation>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct GlbExportCaseExpectation {
    case_id: String,
    export_succeeds: bool,
    error: Option<String>,
    header_magic: Option<String>,
    version: Option<u32>,
    json_chunk_type: Option<String>,
    bin_chunk_type: Option<String>,
    generator: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct GlbExportSnapshot {
    case_snapshots: Vec<GlbExportCaseSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GlbExportCaseSnapshot {
    pub case_id: String,
    pub export_succeeds: bool,
    pub error: Option<String>,
    pub byte_count: usize,
    pub deterministic_hash: Option<String>,
    pub vertex_count: Option<usize>,
    pub index_count: Option<usize>,
    pub header_magic: Option<String>,
    pub version: Option<u32>,
    pub json_chunk_type: Option<String>,
    pub bin_chunk_type: Option<String>,
    pub generator: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
struct GlbBinaryContract {
    header_magic: String,
    version: u32,
    json_chunk_type: String,
    bin_chunk_type: String,
    generator: Option<String>,
}

pub fn build_glb_export_parity_manifest(
    scorecard: &ParityScorecard,
    scorecard_path: &str,
) -> CadResult<GlbExportParityManifest> {
    let corpus: GlbExportReferenceCorpus = serde_json::from_str(GLB_EXPORT_REFERENCE_CORPUS_JSON)
        .map_err(|error| CadError::ParseFailed {
        reason: format!("failed to parse glb export reference corpus: {error}"),
    })?;

    let reference_corpus_sha256 = sha256_hex(GLB_EXPORT_REFERENCE_CORPUS_JSON.as_bytes());
    let reference_commit_match = corpus.vcad_commit == scorecard.vcad_commit;

    let snapshot = collect_glb_export_snapshot()?;
    let replay_snapshot = collect_glb_export_snapshot()?;
    let deterministic_replay_match = snapshot == replay_snapshot;

    let expected_contract = sorted_expectations(corpus.expected_case_expectations);
    let actual_contract = sorted_expectations(
        snapshot
            .case_snapshots
            .iter()
            .map(contract_snapshot)
            .collect(),
    );
    let glb_contract_match = actual_contract == expected_contract;

    let deterministic_signature = parity_signature(
        &snapshot.case_snapshots,
        reference_commit_match,
        glb_contract_match,
        deterministic_replay_match,
        &reference_corpus_sha256,
    );

    Ok(GlbExportParityManifest {
        manifest_version: 1,
        issue_id: PARITY_GLB_EXPORT_ISSUE_ID.to_string(),
        vcad_commit: scorecard.vcad_commit.clone(),
        openagents_commit: scorecard.openagents_commit.clone(),
        generated_from_scorecard: scorecard_path.to_string(),
        reference_corpus_path: GLB_EXPORT_REFERENCE_CORPUS_PATH.to_string(),
        reference_corpus_sha256,
        reference_source: corpus.source,
        reference_commit_match,
        case_snapshots: snapshot.case_snapshots,
        glb_contract_match,
        deterministic_replay_match,
        deterministic_signature,
        parity_contracts: vec![
            "GLB export writes valid GLB 2.0 header and JSON/BIN chunk contracts".to_string(),
            "GLB export emits vcad-aligned glTF asset generator metadata".to_string(),
            "GLB export enforces mesh contract validation and variant-id matching".to_string(),
            "GLB export parity fixtures replay deterministically".to_string(),
        ],
    })
}

fn collect_glb_export_snapshot() -> CadResult<GlbExportSnapshot> {
    let valid_mesh = sample_tetra_mesh();
    let invalid_mesh = mesh_without_triangles();

    let export_case = snapshot_export_case(&valid_mesh)?;
    let variant_mismatch_case = snapshot_variant_mismatch_case(&valid_mesh);
    let invalid_mesh_case = snapshot_invalid_mesh_case(&invalid_mesh);

    let case_snapshots = sorted_cases(vec![export_case, invalid_mesh_case, variant_mismatch_case]);
    Ok(GlbExportSnapshot { case_snapshots })
}

fn snapshot_export_case(mesh: &CadMeshPayload) -> CadResult<GlbExportCaseSnapshot> {
    let artifact = export_glb_from_mesh(
        "doc.parity.glb",
        mesh.document_revision,
        &mesh.variant_id,
        mesh,
    )?;
    let contract = inspect_glb_bytes(&artifact.bytes)?;

    Ok(GlbExportCaseSnapshot {
        case_id: "mesh_export".to_string(),
        export_succeeds: true,
        error: None,
        byte_count: artifact.bytes.len(),
        deterministic_hash: Some(artifact.receipt.deterministic_hash),
        vertex_count: Some(artifact.receipt.vertex_count),
        index_count: Some(artifact.receipt.index_count),
        header_magic: Some(contract.header_magic),
        version: Some(contract.version),
        json_chunk_type: Some(contract.json_chunk_type),
        bin_chunk_type: Some(contract.bin_chunk_type),
        generator: contract.generator,
    })
}

fn snapshot_variant_mismatch_case(mesh: &CadMeshPayload) -> GlbExportCaseSnapshot {
    let error = export_glb_from_mesh(
        "doc.parity.glb",
        mesh.document_revision,
        "variant.mismatch",
        mesh,
    )
    .expect_err("variant mismatch should fail");

    GlbExportCaseSnapshot {
        case_id: "variant_mismatch".to_string(),
        export_succeeds: false,
        error: Some(export_reason(&error)),
        byte_count: 0,
        deterministic_hash: None,
        vertex_count: None,
        index_count: None,
        header_magic: None,
        version: None,
        json_chunk_type: None,
        bin_chunk_type: None,
        generator: None,
    }
}

fn snapshot_invalid_mesh_case(mesh: &CadMeshPayload) -> GlbExportCaseSnapshot {
    let error = export_glb_from_mesh(
        "doc.parity.glb",
        mesh.document_revision,
        &mesh.variant_id,
        mesh,
    )
    .expect_err("invalid mesh contract should fail");

    GlbExportCaseSnapshot {
        case_id: "invalid_mesh_contract".to_string(),
        export_succeeds: false,
        error: Some(export_reason(&error)),
        byte_count: 0,
        deterministic_hash: None,
        vertex_count: None,
        index_count: None,
        header_magic: None,
        version: None,
        json_chunk_type: None,
        bin_chunk_type: None,
        generator: None,
    }
}

fn export_reason(error: &CadError) -> String {
    match error {
        CadError::ExportFailed { reason, .. } => reason.clone(),
        _ => error.to_string(),
    }
}

fn inspect_glb_bytes(bytes: &[u8]) -> CadResult<GlbBinaryContract> {
    if bytes.len() < 12 + 8 + 8 {
        return Err(CadError::ParseFailed {
            reason: format!(
                "invalid GLB: expected at least {} bytes, got {}",
                28,
                bytes.len()
            ),
        });
    }

    let header_magic = std::str::from_utf8(&bytes[0..4])
        .map_err(|error| CadError::ParseFailed {
            reason: format!("invalid GLB header magic: {error}"),
        })?
        .to_string();

    let version = read_u32(bytes, 4)?;
    let total_length = read_u32(bytes, 8)? as usize;
    if total_length != bytes.len() {
        return Err(CadError::ParseFailed {
            reason: format!(
                "invalid GLB header length: header={} actual={}",
                total_length,
                bytes.len()
            ),
        });
    }

    let json_chunk_length = read_u32(bytes, 12)? as usize;
    let json_chunk_type = read_u32(bytes, 16)?;
    let json_start = 20usize;
    let json_end =
        json_start
            .checked_add(json_chunk_length)
            .ok_or_else(|| CadError::ParseFailed {
                reason: "invalid GLB: JSON chunk length overflow".to_string(),
            })?;
    if json_end > bytes.len() {
        return Err(CadError::ParseFailed {
            reason: format!(
                "invalid GLB: JSON chunk out of bounds end={} len={}",
                json_end,
                bytes.len()
            ),
        });
    }

    let json_value: serde_json::Value = serde_json::from_slice(&bytes[json_start..json_end])
        .map_err(|error| CadError::ParseFailed {
            reason: format!("invalid GLB JSON chunk: {error}"),
        })?;
    let generator = json_value
        .get("asset")
        .and_then(|asset| asset.get("generator"))
        .and_then(|value| value.as_str())
        .map(str::to_string);

    let bin_header_offset = json_end;
    if bin_header_offset + 8 > bytes.len() {
        return Err(CadError::ParseFailed {
            reason: "invalid GLB: missing BIN chunk header".to_string(),
        });
    }

    let bin_chunk_length = read_u32(bytes, bin_header_offset)? as usize;
    let bin_chunk_type = read_u32(bytes, bin_header_offset + 4)?;
    let bin_data_start = bin_header_offset + 8;
    let bin_data_end = bin_data_start
        .checked_add(bin_chunk_length)
        .ok_or_else(|| CadError::ParseFailed {
            reason: "invalid GLB: BIN chunk length overflow".to_string(),
        })?;
    if bin_data_end != bytes.len() {
        return Err(CadError::ParseFailed {
            reason: format!(
                "invalid GLB: BIN chunk end={} does not equal payload len={}",
                bin_data_end,
                bytes.len()
            ),
        });
    }

    Ok(GlbBinaryContract {
        header_magic,
        version,
        json_chunk_type: chunk_type_label(json_chunk_type),
        bin_chunk_type: chunk_type_label(bin_chunk_type),
        generator,
    })
}

fn read_u32(bytes: &[u8], offset: usize) -> CadResult<u32> {
    let end = offset + 4;
    if end > bytes.len() {
        return Err(CadError::ParseFailed {
            reason: format!(
                "invalid GLB: expected 4 bytes at offset {} but payload is {} bytes",
                offset,
                bytes.len()
            ),
        });
    }

    Ok(u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ]))
}

fn chunk_type_label(chunk_type: u32) -> String {
    match chunk_type {
        0x4E4F534A => "JSON".to_string(),
        0x004E4942 => "BIN".to_string(),
        _ => format!("0x{chunk_type:08X}"),
    }
}

fn contract_snapshot(case: &GlbExportCaseSnapshot) -> GlbExportCaseExpectation {
    GlbExportCaseExpectation {
        case_id: case.case_id.clone(),
        export_succeeds: case.export_succeeds,
        error: case.error.clone(),
        header_magic: case.header_magic.clone(),
        version: case.version,
        json_chunk_type: case.json_chunk_type.clone(),
        bin_chunk_type: case.bin_chunk_type.clone(),
        generator: case.generator.clone(),
    }
}

fn sorted_cases(mut cases: Vec<GlbExportCaseSnapshot>) -> Vec<GlbExportCaseSnapshot> {
    cases.sort_by(|left, right| left.case_id.cmp(&right.case_id));
    cases
}

fn sorted_expectations(
    mut expectations: Vec<GlbExportCaseExpectation>,
) -> Vec<GlbExportCaseExpectation> {
    expectations.sort_by(|left, right| left.case_id.cmp(&right.case_id));
    expectations
}

fn parity_signature(
    case_snapshots: &[GlbExportCaseSnapshot],
    reference_commit_match: bool,
    glb_contract_match: bool,
    deterministic_replay_match: bool,
    reference_corpus_sha256: &str,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(format!(
        "ref_match={reference_commit_match};contract={glb_contract_match};replay={deterministic_replay_match};ref_sha={reference_corpus_sha256}"
    ));
    for case in case_snapshots {
        hasher
            .update(serde_json::to_vec(case).expect("glb export case snapshots should serialize"));
    }
    format!("{:x}", hasher.finalize())[..16].to_string()
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn sample_tetra_mesh() -> CadMeshPayload {
    CadMeshPayload {
        mesh_id: "mesh.parity.glb".to_string(),
        document_revision: 45,
        variant_id: "variant.parity.glb".to_string(),
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

fn mesh_without_triangles() -> CadMeshPayload {
    let mut mesh = sample_tetra_mesh();
    mesh.mesh_id = "mesh.parity.glb.invalid".to_string();
    mesh.variant_id = "variant.parity.glb.invalid".to_string();
    mesh.triangle_indices.clear();
    mesh
}

#[cfg(test)]
mod tests {
    use super::{PARITY_GLB_EXPORT_ISSUE_ID, build_glb_export_parity_manifest};
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
    fn glb_export_parity_manifest_matches_contract() {
        let scorecard = mock_scorecard();
        let manifest =
            build_glb_export_parity_manifest(&scorecard, "scorecard").expect("build manifest");
        assert_eq!(manifest.issue_id, PARITY_GLB_EXPORT_ISSUE_ID);
        assert!(manifest.reference_commit_match);
        assert!(manifest.glb_contract_match);
        assert!(manifest.deterministic_replay_match);
        assert_eq!(manifest.case_snapshots.len(), 3);
    }
}
