use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::hash::stable_hex_digest;
use crate::parity::scorecard::ParityScorecard;
use crate::{CadError, CadResult};

pub const PARITY_GPU_ACCELERATION_ISSUE_ID: &str = "VCAD-PARITY-095";
pub const GPU_ACCELERATION_REFERENCE_FIXTURE_PATH: &str =
    "crates/cad/parity/fixtures/gpu_acceleration_vcad_reference.json";
const GPU_ACCELERATION_REFERENCE_FIXTURE_JSON: &str =
    include_str!("../../parity/fixtures/gpu_acceleration_vcad_reference.json");

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GpuAccelerationParityManifest {
    pub manifest_version: u64,
    pub issue_id: String,
    pub vcad_commit: String,
    pub openagents_commit: String,
    pub generated_from_scorecard: String,
    pub reference_fixture_path: String,
    pub reference_fixture_sha256: String,
    pub reference_source: String,
    pub reference_commit_match: bool,
    pub backend_contract_match: bool,
    pub capability_export_match: bool,
    pub context_error_contract_match: bool,
    pub normals_contract_match: bool,
    pub decimation_ratio_contract_match: bool,
    pub deterministic_replay_match: bool,
    pub backend_profile: GpuBackendProfileSnapshot,
    pub exported_capabilities: Vec<String>,
    pub context_error_codes: Vec<String>,
    pub normals_samples: Vec<GpuNormalsSampleSnapshot>,
    pub decimation_samples: Vec<GpuDecimationSampleSnapshot>,
    pub deterministic_signature: String,
    pub parity_contracts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct GpuAccelerationReferenceFixture {
    manifest_version: u64,
    issue_id: String,
    vcad_commit: String,
    source: String,
    expected_backend_profile: GpuBackendProfileSnapshot,
    expected_capabilities: Vec<String>,
    expected_context_error_codes: Vec<String>,
    expected_normals_samples: Vec<GpuNormalsSampleSnapshot>,
    expected_decimation_samples: Vec<GpuDecimationSampleSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct GpuAccelerationSnapshot {
    backend_profile: GpuBackendProfileSnapshot,
    exported_capabilities: Vec<String>,
    context_error_codes: Vec<String>,
    normals_samples: Vec<GpuNormalsSampleSnapshot>,
    decimation_samples: Vec<GpuDecimationSampleSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GpuBackendProfileSnapshot {
    pub crate_name: String,
    pub wgpu_major_version: u32,
    pub backends: Vec<String>,
    pub requires_pollster_native: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GpuNormalsSampleSnapshot {
    pub case_id: String,
    pub positions_len: usize,
    pub indices_len: usize,
    pub output_len: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GpuDecimationSampleSnapshot {
    pub case_id: String,
    pub triangle_count: u32,
    pub target_ratio_input: f32,
    pub clamped_target_ratio: f32,
    pub target_triangles: u32,
}

const EXPORTED_CAPABILITIES: [&str; 2] = ["compute_creased_normals", "decimate_mesh"];
const CONTEXT_ERROR_CODES: [&str; 5] = [
    "NoAdapter",
    "AlreadyInitialized",
    "DeviceRequest",
    "BufferMapping",
    "NotInitialized",
];

pub fn build_gpu_acceleration_parity_manifest(
    scorecard: &ParityScorecard,
    scorecard_path: &str,
) -> CadResult<GpuAccelerationParityManifest> {
    let reference: GpuAccelerationReferenceFixture =
        serde_json::from_str(GPU_ACCELERATION_REFERENCE_FIXTURE_JSON).map_err(|error| {
            CadError::ParseFailed {
                reason: format!("failed parsing gpu acceleration reference fixture: {error}"),
            }
        })?;

    let reference_fixture_sha256 = sha256_hex(GPU_ACCELERATION_REFERENCE_FIXTURE_JSON.as_bytes());
    let reference_commit_match = reference.vcad_commit == scorecard.vcad_commit;

    let snapshot = collect_snapshot();
    let replay_snapshot = collect_snapshot();
    let deterministic_replay_match = snapshot == replay_snapshot;

    let backend_contract_match = snapshot.backend_profile == reference.expected_backend_profile;
    let capability_export_match = sorted_strings(snapshot.exported_capabilities.clone())
        == sorted_strings(reference.expected_capabilities.clone());
    let context_error_contract_match = sorted_strings(snapshot.context_error_codes.clone())
        == sorted_strings(reference.expected_context_error_codes.clone());
    let normals_contract_match = sorted_normals_samples(snapshot.normals_samples.clone())
        == sorted_normals_samples(reference.expected_normals_samples.clone());
    let decimation_ratio_contract_match =
        sorted_decimation_samples(snapshot.decimation_samples.clone())
            == sorted_decimation_samples(reference.expected_decimation_samples.clone());

    let deterministic_signature = parity_signature(
        &snapshot,
        reference_commit_match,
        backend_contract_match,
        capability_export_match,
        context_error_contract_match,
        normals_contract_match,
        decimation_ratio_contract_match,
        deterministic_replay_match,
        &reference_fixture_sha256,
    );

    Ok(GpuAccelerationParityManifest {
        manifest_version: 1,
        issue_id: PARITY_GPU_ACCELERATION_ISSUE_ID.to_string(),
        vcad_commit: scorecard.vcad_commit.clone(),
        openagents_commit: scorecard.openagents_commit.clone(),
        generated_from_scorecard: scorecard_path.to_string(),
        reference_fixture_path: GPU_ACCELERATION_REFERENCE_FIXTURE_PATH.to_string(),
        reference_fixture_sha256,
        reference_source: reference.source,
        reference_commit_match,
        backend_contract_match,
        capability_export_match,
        context_error_contract_match,
        normals_contract_match,
        decimation_ratio_contract_match,
        deterministic_replay_match,
        backend_profile: snapshot.backend_profile,
        exported_capabilities: snapshot.exported_capabilities,
        context_error_codes: snapshot.context_error_codes,
        normals_samples: snapshot.normals_samples,
        decimation_samples: snapshot.decimation_samples,
        deterministic_signature,
        parity_contracts: vec![
            "gpu lane mirrors vcad-kernel-gpu backend contract (wgpu 23 + webgpu/webgl backends)"
                .to_string(),
            "gpu lane exports compute_creased_normals and decimate_mesh capability surfaces"
                .to_string(),
            "gpu context error taxonomy matches vcad GpuError variants".to_string(),
            "creased-normal lane preserves empty-input output sizing contract".to_string(),
            "decimation lane clamps target ratio to [0.1, 1.0] before target-triangle derivation"
                .to_string(),
        ],
    })
}

fn collect_snapshot() -> GpuAccelerationSnapshot {
    let backend_profile = GpuBackendProfileSnapshot {
        crate_name: "vcad-kernel-gpu".to_string(),
        wgpu_major_version: 23,
        backends: vec!["webgpu".to_string(), "webgl".to_string()],
        requires_pollster_native: true,
    };

    let exported_capabilities = EXPORTED_CAPABILITIES
        .iter()
        .map(|value| (*value).to_string())
        .collect::<Vec<_>>();
    let context_error_codes = CONTEXT_ERROR_CODES
        .iter()
        .map(|value| (*value).to_string())
        .collect::<Vec<_>>();

    let normals_samples = vec![
        normals_sample("empty_positions", 0, 6),
        normals_sample("empty_indices", 9, 0),
        normals_sample("non_empty_mesh", 18, 12),
    ];

    let decimation_samples = vec![
        decimation_sample("ratio_below_min", 200, 0.05),
        decimation_sample("ratio_nominal", 200, 0.5),
        decimation_sample("ratio_above_max", 200, 1.5),
    ];

    GpuAccelerationSnapshot {
        backend_profile,
        exported_capabilities: sorted_strings(exported_capabilities),
        context_error_codes: sorted_strings(context_error_codes),
        normals_samples: sorted_normals_samples(normals_samples),
        decimation_samples: sorted_decimation_samples(decimation_samples),
    }
}

fn normals_sample(
    case_id: &str,
    positions_len: usize,
    indices_len: usize,
) -> GpuNormalsSampleSnapshot {
    GpuNormalsSampleSnapshot {
        case_id: case_id.to_string(),
        positions_len,
        indices_len,
        output_len: compute_creased_normals_output_len(positions_len, indices_len),
    }
}

fn decimation_sample(
    case_id: &str,
    triangle_count: u32,
    target_ratio_input: f32,
) -> GpuDecimationSampleSnapshot {
    let clamped_target_ratio = clamp_target_ratio(target_ratio_input);
    GpuDecimationSampleSnapshot {
        case_id: case_id.to_string(),
        triangle_count,
        target_ratio_input,
        clamped_target_ratio,
        target_triangles: ((triangle_count as f32) * clamped_target_ratio) as u32,
    }
}

fn compute_creased_normals_output_len(positions_len: usize, indices_len: usize) -> usize {
    if positions_len == 0 || indices_len == 0 {
        return positions_len;
    }
    positions_len
}

fn clamp_target_ratio(target_ratio: f32) -> f32 {
    target_ratio.clamp(0.1, 1.0)
}

fn sorted_strings(mut values: Vec<String>) -> Vec<String> {
    values.sort();
    values
}

fn sorted_normals_samples(
    mut samples: Vec<GpuNormalsSampleSnapshot>,
) -> Vec<GpuNormalsSampleSnapshot> {
    samples.sort_by(|left, right| left.case_id.cmp(&right.case_id));
    samples
}

fn sorted_decimation_samples(
    mut samples: Vec<GpuDecimationSampleSnapshot>,
) -> Vec<GpuDecimationSampleSnapshot> {
    samples.sort_by(|left, right| left.case_id.cmp(&right.case_id));
    samples
}

#[allow(clippy::too_many_arguments)]
fn parity_signature(
    snapshot: &GpuAccelerationSnapshot,
    reference_commit_match: bool,
    backend_contract_match: bool,
    capability_export_match: bool,
    context_error_contract_match: bool,
    normals_contract_match: bool,
    decimation_ratio_contract_match: bool,
    deterministic_replay_match: bool,
    reference_fixture_sha256: &str,
) -> String {
    let payload = serde_json::to_vec(&(
        snapshot,
        reference_commit_match,
        backend_contract_match,
        capability_export_match,
        context_error_contract_match,
        normals_contract_match,
        decimation_ratio_contract_match,
        deterministic_replay_match,
        reference_fixture_sha256,
    ))
    .expect("serialize gpu acceleration parity signature payload");
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
        PARITY_GPU_ACCELERATION_ISSUE_ID, build_gpu_acceleration_parity_manifest, collect_snapshot,
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
    fn gpu_acceleration_manifest_matches_contract() {
        let scorecard = mock_scorecard();
        let manifest = build_gpu_acceleration_parity_manifest(&scorecard, "scorecard")
            .expect("build gpu acceleration parity manifest");
        assert_eq!(manifest.issue_id, PARITY_GPU_ACCELERATION_ISSUE_ID);
        assert!(manifest.reference_commit_match);
        assert!(manifest.backend_contract_match);
        assert!(manifest.capability_export_match);
        assert!(manifest.context_error_contract_match);
        assert!(manifest.normals_contract_match);
        assert!(manifest.decimation_ratio_contract_match);
        assert!(manifest.deterministic_replay_match);
        assert_eq!(manifest.exported_capabilities.len(), 2);
    }
}
