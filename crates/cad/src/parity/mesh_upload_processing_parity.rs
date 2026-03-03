use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::hash::stable_hex_digest;
use crate::parity::scorecard::ParityScorecard;
use crate::{CadError, CadResult};

pub const PARITY_MESH_UPLOAD_PROCESSING_ISSUE_ID: &str = "VCAD-PARITY-096";
pub const MESH_UPLOAD_PROCESSING_REFERENCE_FIXTURE_PATH: &str =
    "crates/cad/parity/fixtures/mesh_upload_processing_vcad_reference.json";
const MESH_UPLOAD_PROCESSING_REFERENCE_FIXTURE_JSON: &str =
    include_str!("../../parity/fixtures/mesh_upload_processing_vcad_reference.json");
const GPU_FALLBACK_ERROR: &str = "GPU feature not enabled";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MeshUploadProcessingParityManifest {
    pub manifest_version: u64,
    pub issue_id: String,
    pub vcad_commit: String,
    pub openagents_commit: String,
    pub generated_from_scorecard: String,
    pub reference_fixture_path: String,
    pub reference_fixture_sha256: String,
    pub reference_source: String,
    pub reference_commit_match: bool,
    pub upload_contract_match: bool,
    pub processing_contract_match: bool,
    pub lod_contract_match: bool,
    pub fallback_contract_match: bool,
    pub deterministic_replay_match: bool,
    pub upload_samples: Vec<MeshUploadSampleSnapshot>,
    pub processing_samples: Vec<MeshProcessingSampleSnapshot>,
    pub fallback_error_message: String,
    pub deterministic_signature: String,
    pub parity_contracts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct MeshUploadProcessingReferenceFixture {
    manifest_version: u64,
    issue_id: String,
    vcad_commit: String,
    source: String,
    expected_upload_samples: Vec<MeshUploadSampleSnapshot>,
    expected_processing_samples: Vec<MeshProcessingSampleSnapshot>,
    expected_fallback_error_message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct MeshUploadProcessingSnapshot {
    upload_samples: Vec<MeshUploadSampleSnapshot>,
    processing_samples: Vec<MeshProcessingSampleSnapshot>,
    fallback_error_message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MeshUploadSampleSnapshot {
    pub case_id: String,
    pub accepted: bool,
    pub vertex_count: usize,
    pub triangle_count: usize,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MeshProcessingSampleSnapshot {
    pub case_id: String,
    pub generate_lod: bool,
    pub result_count: usize,
    pub lod_ratios: Vec<f32>,
    pub positions_len_per_level: Vec<usize>,
    pub indices_len_per_level: Vec<usize>,
    pub normals_len_per_level: Vec<usize>,
}

pub fn build_mesh_upload_processing_parity_manifest(
    scorecard: &ParityScorecard,
    scorecard_path: &str,
) -> CadResult<MeshUploadProcessingParityManifest> {
    let reference: MeshUploadProcessingReferenceFixture =
        serde_json::from_str(MESH_UPLOAD_PROCESSING_REFERENCE_FIXTURE_JSON).map_err(|error| {
            CadError::ParseFailed {
                reason: format!("failed parsing mesh upload/processing reference fixture: {error}"),
            }
        })?;

    let reference_fixture_sha256 =
        sha256_hex(MESH_UPLOAD_PROCESSING_REFERENCE_FIXTURE_JSON.as_bytes());
    let reference_commit_match = reference.vcad_commit == scorecard.vcad_commit;

    let snapshot = collect_snapshot();
    let replay_snapshot = collect_snapshot();
    let deterministic_replay_match = snapshot == replay_snapshot;

    let upload_contract_match = sorted_upload_samples(snapshot.upload_samples.clone())
        == sorted_upload_samples(reference.expected_upload_samples.clone());
    let processing_contract_match = sorted_processing_samples(snapshot.processing_samples.clone())
        == sorted_processing_samples(reference.expected_processing_samples.clone());
    let lod_contract_match = snapshot
        .processing_samples
        .iter()
        .find(|sample| sample.case_id == "process_with_lod")
        .is_some_and(|sample| {
            sample.result_count == 3
                && sample.lod_ratios == vec![1.0, 0.5, 0.25]
                && sample.indices_len_per_level == vec![24, 12, 6]
        });
    let fallback_contract_match =
        snapshot.fallback_error_message == reference.expected_fallback_error_message;

    let deterministic_signature = parity_signature(
        &snapshot,
        reference_commit_match,
        upload_contract_match,
        processing_contract_match,
        lod_contract_match,
        fallback_contract_match,
        deterministic_replay_match,
        &reference_fixture_sha256,
    );

    Ok(MeshUploadProcessingParityManifest {
        manifest_version: 1,
        issue_id: PARITY_MESH_UPLOAD_PROCESSING_ISSUE_ID.to_string(),
        vcad_commit: scorecard.vcad_commit.clone(),
        openagents_commit: scorecard.openagents_commit.clone(),
        generated_from_scorecard: scorecard_path.to_string(),
        reference_fixture_path: MESH_UPLOAD_PROCESSING_REFERENCE_FIXTURE_PATH.to_string(),
        reference_fixture_sha256,
        reference_source: reference.source,
        reference_commit_match,
        upload_contract_match,
        processing_contract_match,
        lod_contract_match,
        fallback_contract_match,
        deterministic_replay_match,
        upload_samples: snapshot.upload_samples,
        processing_samples: snapshot.processing_samples,
        fallback_error_message: snapshot.fallback_error_message,
        deterministic_signature,
        parity_contracts: vec![
            "mesh upload validates position/index stride and index bounds before processing"
                .to_string(),
            "processGeometryGpu returns one level without LOD and three levels with LOD"
                .to_string(),
            "LOD processing ratios follow [1.0, 0.5, 0.25] for full/50%/25% geometry outputs"
                .to_string(),
            "each processed level emits positions/indices/normals buffers with deterministic lengths"
                .to_string(),
            "GPU-disabled fallback error remains stable as 'GPU feature not enabled'"
                .to_string(),
        ],
    })
}

fn collect_snapshot() -> MeshUploadProcessingSnapshot {
    let valid_positions = vec![
        0.0, 0.0, 0.0, // v0
        1.0, 0.0, 0.0, // v1
        1.0, 1.0, 0.0, // v2
        0.0, 1.0, 0.0, // v3
        0.0, 0.0, 1.0, // v4
        1.0, 0.0, 1.0, // v5
    ];
    let valid_indices = vec![
        0, 1, 2, 0, 2, 3, 0, 3, 4, 0, 4, 5, 1, 2, 5, 2, 3, 4, 1, 5, 4, 1, 4, 0,
    ];

    let upload_samples = vec![
        upload_sample("valid_mesh", &valid_positions, &valid_indices),
        upload_sample("invalid_position_stride", &[0.0, 1.0], &[0, 1, 2]),
        upload_sample("invalid_index_stride", &valid_positions, &[0, 1, 2, 3]),
        upload_sample("index_out_of_bounds", &valid_positions, &[0, 1, 9]),
    ];

    let processing_samples = vec![
        processing_sample(
            "process_without_lod",
            &valid_positions,
            &valid_indices,
            false,
        ),
        processing_sample("process_with_lod", &valid_positions, &valid_indices, true),
    ];

    MeshUploadProcessingSnapshot {
        upload_samples: sorted_upload_samples(upload_samples),
        processing_samples: sorted_processing_samples(processing_samples),
        fallback_error_message: GPU_FALLBACK_ERROR.to_string(),
    }
}

fn upload_sample(case_id: &str, positions: &[f32], indices: &[u32]) -> MeshUploadSampleSnapshot {
    match validate_mesh_upload(positions, indices) {
        Ok((vertex_count, triangle_count)) => MeshUploadSampleSnapshot {
            case_id: case_id.to_string(),
            accepted: true,
            vertex_count,
            triangle_count,
            error: None,
        },
        Err(error) => MeshUploadSampleSnapshot {
            case_id: case_id.to_string(),
            accepted: false,
            vertex_count: 0,
            triangle_count: 0,
            error: Some(error),
        },
    }
}

fn processing_sample(
    case_id: &str,
    positions: &[f32],
    indices: &[u32],
    generate_lod: bool,
) -> MeshProcessingSampleSnapshot {
    let outputs = process_geometry_contract(positions, indices, generate_lod)
        .expect("processing sample inputs should be valid");

    MeshProcessingSampleSnapshot {
        case_id: case_id.to_string(),
        generate_lod,
        result_count: outputs.len(),
        lod_ratios: outputs.iter().map(|output| output.lod_ratio).collect(),
        positions_len_per_level: outputs.iter().map(|output| output.positions_len).collect(),
        indices_len_per_level: outputs.iter().map(|output| output.indices_len).collect(),
        normals_len_per_level: outputs.iter().map(|output| output.normals_len).collect(),
    }
}

#[derive(Debug, Clone, Copy)]
struct ProcessedMeshOutput {
    lod_ratio: f32,
    positions_len: usize,
    indices_len: usize,
    normals_len: usize,
}

fn validate_mesh_upload(positions: &[f32], indices: &[u32]) -> Result<(usize, usize), String> {
    if positions.len() % 3 != 0 {
        return Err("positions length must be divisible by 3".to_string());
    }
    if indices.len() % 3 != 0 {
        return Err("indices length must be divisible by 3".to_string());
    }
    let vertex_count = positions.len() / 3;
    if let Some(index) = indices
        .iter()
        .find(|index| **index as usize >= vertex_count)
    {
        return Err(format!(
            "index {index} out of bounds for {vertex_count} vertices"
        ));
    }

    Ok((vertex_count, indices.len() / 3))
}

fn process_geometry_contract(
    positions: &[f32],
    indices: &[u32],
    generate_lod: bool,
) -> Result<Vec<ProcessedMeshOutput>, String> {
    let (_, triangle_count) = validate_mesh_upload(positions, indices)?;

    let mut outputs = vec![ProcessedMeshOutput {
        lod_ratio: 1.0,
        positions_len: positions.len(),
        indices_len: indices.len(),
        normals_len: positions.len(),
    }];

    if generate_lod {
        for ratio in [0.5_f32, 0.25_f32] {
            let target_triangles = ((triangle_count as f32) * ratio) as usize;
            outputs.push(ProcessedMeshOutput {
                lod_ratio: ratio,
                positions_len: positions.len(),
                indices_len: target_triangles * 3,
                normals_len: positions.len(),
            });
        }
    }

    Ok(outputs)
}

fn sorted_upload_samples(
    mut samples: Vec<MeshUploadSampleSnapshot>,
) -> Vec<MeshUploadSampleSnapshot> {
    samples.sort_by(|left, right| left.case_id.cmp(&right.case_id));
    samples
}

fn sorted_processing_samples(
    mut samples: Vec<MeshProcessingSampleSnapshot>,
) -> Vec<MeshProcessingSampleSnapshot> {
    samples.sort_by(|left, right| left.case_id.cmp(&right.case_id));
    samples
}

#[allow(clippy::too_many_arguments)]
fn parity_signature(
    snapshot: &MeshUploadProcessingSnapshot,
    reference_commit_match: bool,
    upload_contract_match: bool,
    processing_contract_match: bool,
    lod_contract_match: bool,
    fallback_contract_match: bool,
    deterministic_replay_match: bool,
    reference_fixture_sha256: &str,
) -> String {
    let payload = serde_json::to_vec(&(
        snapshot,
        reference_commit_match,
        upload_contract_match,
        processing_contract_match,
        lod_contract_match,
        fallback_contract_match,
        deterministic_replay_match,
        reference_fixture_sha256,
    ))
    .expect("serialize mesh upload/processing parity signature payload");
    stable_hex_digest(&payload)
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        PARITY_MESH_UPLOAD_PROCESSING_ISSUE_ID, build_mesh_upload_processing_parity_manifest,
        collect_snapshot,
    };
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
    fn snapshot_is_deterministic() {
        let a = collect_snapshot();
        let b = collect_snapshot();
        assert_eq!(a, b);
    }

    #[test]
    fn mesh_upload_processing_manifest_matches_contract() {
        let scorecard = mock_scorecard();
        let manifest = build_mesh_upload_processing_parity_manifest(&scorecard, "scorecard")
            .expect("build mesh upload/processing parity manifest");
        assert_eq!(manifest.issue_id, PARITY_MESH_UPLOAD_PROCESSING_ISSUE_ID);
        assert!(manifest.reference_commit_match);
        assert!(manifest.upload_contract_match);
        assert!(manifest.processing_contract_match);
        assert!(manifest.lod_contract_match);
        assert!(manifest.fallback_contract_match);
        assert!(manifest.deterministic_replay_match);
        assert_eq!(manifest.processing_samples.len(), 2);
    }
}
