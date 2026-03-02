use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::cli::{CAD_CLI_APP_NAME, run_cli_tokens};
use crate::document::CadDocument;
use crate::export::export_step_from_mesh;
use crate::hash::stable_hex_digest;
use crate::mesh::{
    CadMeshBounds, CadMeshMaterialSlot, CadMeshPayload, CadMeshTopology, CadMeshVertex,
};
use crate::parity::scorecard::ParityScorecard;
use crate::stl::export_stl_from_mesh;
use crate::{CadError, CadResult};

pub const PARITY_CAD_CLI_COMMANDS_ISSUE_ID: &str = "VCAD-PARITY-084";
pub const CAD_CLI_COMMANDS_REFERENCE_CORPUS_PATH: &str =
    "crates/cad/parity/fixtures/cad_cli_commands_vcad_reference.json";
const CAD_CLI_COMMANDS_REFERENCE_CORPUS_JSON: &str =
    include_str!("../../parity/fixtures/cad_cli_commands_vcad_reference.json");

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CadCliCommandsParityManifest {
    pub manifest_version: u64,
    pub issue_id: String,
    pub vcad_commit: String,
    pub openagents_commit: String,
    pub generated_from_scorecard: String,
    pub reference_corpus_path: String,
    pub reference_corpus_sha256: String,
    pub reference_source: String,
    pub reference_commit_match: bool,
    pub case_snapshots: Vec<CadCliCommandsCaseSnapshot>,
    pub case_contract_snapshots: Vec<CadCliCommandsCaseContractSnapshot>,
    pub import_stl_counts_match: bool,
    pub import_step_feature_count_match: bool,
    pub command_parity_match: bool,
    pub deterministic_replay_match: bool,
    pub deterministic_signature: String,
    pub parity_contracts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct CadCliCommandsReferenceCorpus {
    manifest_version: u64,
    issue_id: String,
    vcad_commit: String,
    source: String,
    expected_cases: Vec<CadCliCommandsCaseExpectation>,
    expected_import_stl_vertex_count: usize,
    expected_import_stl_triangle_count: usize,
    expected_import_step_min_feature_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct CadCliCommandsCaseExpectation {
    case_id: String,
    command: String,
    exit_code: i32,
    stdout_marker: Option<String>,
    stderr_marker: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct CadCliCommandsSnapshot {
    case_snapshots: Vec<CadCliCommandsCaseSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CadCliCommandsCaseSnapshot {
    pub case_id: String,
    pub command: String,
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub stdout_hash: String,
    pub output_hash: Option<String>,
    pub output_byte_count: usize,
    pub imported_vertex_count: Option<usize>,
    pub imported_triangle_count: Option<usize>,
    pub imported_feature_count: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CadCliCommandsCaseContractSnapshot {
    pub case_id: String,
    pub command_match: bool,
    pub exit_code_match: bool,
    pub stdout_marker_match: bool,
    pub stderr_marker_match: bool,
}

pub fn build_cad_cli_commands_parity_manifest(
    scorecard: &ParityScorecard,
    scorecard_path: &str,
) -> CadResult<CadCliCommandsParityManifest> {
    let corpus: CadCliCommandsReferenceCorpus =
        serde_json::from_str(CAD_CLI_COMMANDS_REFERENCE_CORPUS_JSON).map_err(|error| {
            CadError::ParseFailed {
                reason: format!("failed to parse cad cli commands reference corpus: {error}"),
            }
        })?;

    let reference_corpus_sha256 = sha256_hex(CAD_CLI_COMMANDS_REFERENCE_CORPUS_JSON.as_bytes());
    let reference_commit_match = corpus.vcad_commit == scorecard.vcad_commit;

    let snapshot = collect_cad_cli_commands_snapshot()?;
    let replay_snapshot = collect_cad_cli_commands_snapshot()?;
    let deterministic_replay_match = snapshot == replay_snapshot;

    let expected_cases = corpus
        .expected_cases
        .iter()
        .map(|expectation| (expectation.case_id.clone(), expectation.clone()))
        .collect::<BTreeMap<_, _>>();

    let case_contract_snapshots = sorted_contract_snapshots(
        snapshot
            .case_snapshots
            .iter()
            .map(|case| contract_snapshot(case, expected_cases.get(&case.case_id)))
            .collect(),
    );

    let expected_contract_snapshots = sorted_contract_snapshots(
        corpus
            .expected_cases
            .iter()
            .map(|expectation| CadCliCommandsCaseContractSnapshot {
                case_id: expectation.case_id.clone(),
                command_match: true,
                exit_code_match: true,
                stdout_marker_match: true,
                stderr_marker_match: true,
            })
            .collect(),
    );

    let import_stl_case = snapshot
        .case_snapshots
        .iter()
        .find(|case| case.case_id == "import_stl");
    let import_stl_counts_match = import_stl_case.is_some_and(|case| {
        case.imported_vertex_count == Some(corpus.expected_import_stl_vertex_count)
            && case.imported_triangle_count == Some(corpus.expected_import_stl_triangle_count)
    });

    let import_step_case = snapshot
        .case_snapshots
        .iter()
        .find(|case| case.case_id == "import_step");
    let import_step_feature_count_match = import_step_case.is_some_and(|case| {
        case.imported_feature_count
            .is_some_and(|count| count >= corpus.expected_import_step_min_feature_count)
    });

    let command_parity_match = snapshot.case_snapshots.len() == corpus.expected_cases.len()
        && case_contract_snapshots == expected_contract_snapshots
        && import_stl_counts_match
        && import_step_feature_count_match;

    let deterministic_signature = parity_signature(
        &snapshot.case_snapshots,
        &case_contract_snapshots,
        reference_commit_match,
        import_stl_counts_match,
        import_step_feature_count_match,
        command_parity_match,
        deterministic_replay_match,
        &reference_corpus_sha256,
    );

    Ok(CadCliCommandsParityManifest {
        manifest_version: 1,
        issue_id: PARITY_CAD_CLI_COMMANDS_ISSUE_ID.to_string(),
        vcad_commit: scorecard.vcad_commit.clone(),
        openagents_commit: scorecard.openagents_commit.clone(),
        generated_from_scorecard: scorecard_path.to_string(),
        reference_corpus_path: CAD_CLI_COMMANDS_REFERENCE_CORPUS_PATH.to_string(),
        reference_corpus_sha256,
        reference_source: corpus.source,
        reference_commit_match,
        case_snapshots: snapshot.case_snapshots,
        case_contract_snapshots,
        import_stl_counts_match,
        import_step_feature_count_match,
        command_parity_match,
        deterministic_replay_match,
        deterministic_signature,
        parity_contracts: vec![
            "openagents-cad-cli export supports STL, GLB, and STEP outputs".to_string(),
            "openagents-cad-cli import supports STL mesh JSON and STEP document JSON".to_string(),
            "openagents-cad-cli info reports mesh/document/step summary contracts".to_string(),
            "cad cli command parity fixtures replay deterministically".to_string(),
        ],
    })
}

fn collect_cad_cli_commands_snapshot() -> CadResult<CadCliCommandsSnapshot> {
    let workspace = parity_workspace_dir();
    if workspace.exists() {
        fs::remove_dir_all(&workspace).map_err(|error| CadError::ParseFailed {
            reason: format!(
                "failed to reset cad cli commands parity workspace {}: {error}",
                workspace.display()
            ),
        })?;
    }
    fs::create_dir_all(&workspace).map_err(|error| CadError::ParseFailed {
        reason: format!(
            "failed to create cad cli commands parity workspace {}: {error}",
            workspace.display()
        ),
    })?;

    let mesh = sample_tetra_mesh();
    let mesh_path = workspace.join("mesh_input.json");
    write_pretty_json(&mesh_path, &mesh)?;

    let export_stl_case = snapshot_export_case(&workspace, &mesh_path, "stl")?;
    let export_glb_case = snapshot_export_case(&workspace, &mesh_path, "glb")?;
    let export_step_case = snapshot_export_case(&workspace, &mesh_path, "step")?;
    let import_stl_case = snapshot_import_stl_case(&workspace, &mesh)?;
    let import_step_case = snapshot_import_step_case(&workspace, &mesh)?;
    let info_mesh_case = snapshot_info_mesh_case(&mesh_path)?;

    let case_snapshots = sorted_cases(vec![
        export_glb_case,
        export_step_case,
        export_stl_case,
        import_step_case,
        import_stl_case,
        info_mesh_case,
    ]);

    Ok(CadCliCommandsSnapshot { case_snapshots })
}

fn snapshot_export_case(
    workspace: &Path,
    mesh_path: &Path,
    extension: &str,
) -> CadResult<CadCliCommandsCaseSnapshot> {
    let output_path = workspace.join(format!("export_output.{extension}"));
    let args = vec![
        CAD_CLI_APP_NAME.to_string(),
        "export".to_string(),
        mesh_path.to_string_lossy().to_string(),
        output_path.to_string_lossy().to_string(),
    ];
    let outcome = run_cli_owned(&args);

    let (output_hash, output_byte_count) = if outcome.exit_code == 0 {
        let bytes = fs::read(&output_path).map_err(|error| CadError::ParseFailed {
            reason: format!(
                "failed reading export output {}: {error}",
                output_path.display()
            ),
        })?;
        (Some(stable_hex_digest(&bytes)), bytes.len())
    } else {
        (None, 0usize)
    };

    Ok(CadCliCommandsCaseSnapshot {
        case_id: format!("export_{extension}"),
        command: "export".to_string(),
        exit_code: outcome.exit_code,
        stdout: normalize_output(&outcome.stdout),
        stderr: normalize_output(&outcome.stderr),
        stdout_hash: stable_hex_digest(outcome.stdout.as_bytes()),
        output_hash,
        output_byte_count,
        imported_vertex_count: None,
        imported_triangle_count: None,
        imported_feature_count: None,
    })
}

fn snapshot_import_stl_case(
    workspace: &Path,
    mesh: &CadMeshPayload,
) -> CadResult<CadCliCommandsCaseSnapshot> {
    let stl_input = workspace.join("import_input.stl");
    let stl_artifact = export_stl_from_mesh("cad-cli.parity.import", 11, &mesh.variant_id, mesh)
        .map_err(|error| CadError::ParseFailed {
            reason: format!("failed generating STL import fixture: {error}"),
        })?;
    fs::write(&stl_input, &stl_artifact.bytes).map_err(|error| CadError::ParseFailed {
        reason: format!(
            "failed writing STL import fixture {}: {error}",
            stl_input.display()
        ),
    })?;

    let output_path = workspace.join("import_stl_output.json");
    let args = vec![
        CAD_CLI_APP_NAME.to_string(),
        "import".to_string(),
        stl_input.to_string_lossy().to_string(),
        output_path.to_string_lossy().to_string(),
        "--name".to_string(),
        "imported_mesh".to_string(),
    ];
    let outcome = run_cli_owned(&args);

    let (output_hash, output_byte_count, imported_vertex_count, imported_triangle_count) =
        if outcome.exit_code == 0 {
            let bytes = fs::read(&output_path).map_err(|error| CadError::ParseFailed {
                reason: format!(
                    "failed reading STL import output {}: {error}",
                    output_path.display()
                ),
            })?;
            let mesh: CadMeshPayload =
                serde_json::from_slice(&bytes).map_err(|error| CadError::ParseFailed {
                    reason: format!(
                        "failed parsing STL import output {}: {error}",
                        output_path.display()
                    ),
                })?;
            (
                Some(stable_hex_digest(&bytes)),
                bytes.len(),
                Some(mesh.vertices.len()),
                Some(mesh.triangle_indices.len() / 3),
            )
        } else {
            (None, 0usize, None, None)
        };

    Ok(CadCliCommandsCaseSnapshot {
        case_id: "import_stl".to_string(),
        command: "import".to_string(),
        exit_code: outcome.exit_code,
        stdout: normalize_output(&outcome.stdout),
        stderr: normalize_output(&outcome.stderr),
        stdout_hash: stable_hex_digest(outcome.stdout.as_bytes()),
        output_hash,
        output_byte_count,
        imported_vertex_count,
        imported_triangle_count,
        imported_feature_count: None,
    })
}

fn snapshot_import_step_case(
    workspace: &Path,
    mesh: &CadMeshPayload,
) -> CadResult<CadCliCommandsCaseSnapshot> {
    let step_input = workspace.join("import_input.step");
    let step_artifact = export_step_from_mesh("cad-cli.parity.import", 11, &mesh.variant_id, mesh)
        .map_err(|error| CadError::ParseFailed {
            reason: format!("failed generating STEP import fixture: {error}"),
        })?;
    fs::write(&step_input, &step_artifact.bytes).map_err(|error| CadError::ParseFailed {
        reason: format!(
            "failed writing STEP import fixture {}: {error}",
            step_input.display()
        ),
    })?;

    let output_path = workspace.join("import_step_output.json");
    let args = vec![
        CAD_CLI_APP_NAME.to_string(),
        "import".to_string(),
        step_input.to_string_lossy().to_string(),
        output_path.to_string_lossy().to_string(),
        "--name".to_string(),
        "imported_step_doc".to_string(),
    ];
    let outcome = run_cli_owned(&args);

    let (output_hash, output_byte_count, imported_feature_count) = if outcome.exit_code == 0 {
        let bytes = fs::read(&output_path).map_err(|error| CadError::ParseFailed {
            reason: format!(
                "failed reading STEP import output {}: {error}",
                output_path.display()
            ),
        })?;
        let document: CadDocument =
            serde_json::from_slice(&bytes).map_err(|error| CadError::ParseFailed {
                reason: format!(
                    "failed parsing STEP import output {}: {error}",
                    output_path.display()
                ),
            })?;
        (
            Some(stable_hex_digest(&bytes)),
            bytes.len(),
            Some(document.feature_ids.len()),
        )
    } else {
        (None, 0usize, None)
    };

    Ok(CadCliCommandsCaseSnapshot {
        case_id: "import_step".to_string(),
        command: "import".to_string(),
        exit_code: outcome.exit_code,
        stdout: normalize_output(&outcome.stdout),
        stderr: normalize_output(&outcome.stderr),
        stdout_hash: stable_hex_digest(outcome.stdout.as_bytes()),
        output_hash,
        output_byte_count,
        imported_vertex_count: None,
        imported_triangle_count: None,
        imported_feature_count,
    })
}

fn snapshot_info_mesh_case(mesh_path: &Path) -> CadResult<CadCliCommandsCaseSnapshot> {
    let args = vec![
        CAD_CLI_APP_NAME.to_string(),
        "info".to_string(),
        mesh_path.to_string_lossy().to_string(),
    ];
    let outcome = run_cli_owned(&args);

    Ok(CadCliCommandsCaseSnapshot {
        case_id: "info_mesh".to_string(),
        command: "info".to_string(),
        exit_code: outcome.exit_code,
        stdout: normalize_output(&outcome.stdout),
        stderr: normalize_output(&outcome.stderr),
        stdout_hash: stable_hex_digest(outcome.stdout.as_bytes()),
        output_hash: None,
        output_byte_count: 0,
        imported_vertex_count: None,
        imported_triangle_count: None,
        imported_feature_count: None,
    })
}

fn run_cli_owned(args: &[String]) -> crate::cli::CadCliRunOutcome {
    let refs = args.iter().map(String::as_str).collect::<Vec<_>>();
    run_cli_tokens(&refs)
}

fn write_pretty_json<T: Serialize>(path: &Path, value: &T) -> CadResult<()> {
    let mut payload =
        serde_json::to_string_pretty(value).map_err(|error| CadError::ParseFailed {
            reason: format!("failed to serialize {}: {error}", path.display()),
        })?;
    payload.push('\n');
    fs::write(path, payload).map_err(|error| CadError::ParseFailed {
        reason: format!("failed to write {}: {error}", path.display()),
    })
}

fn parity_workspace_dir() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(Path::parent)
        .map(|path| path.join("target/cad-cli-commands-parity"))
        .unwrap_or_else(|| manifest_dir.join("target/cad-cli-commands-parity"))
}

fn normalize_output(raw: &str) -> String {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo_root = manifest_dir
        .parent()
        .and_then(Path::parent)
        .map(|path| path.to_string_lossy().to_string())
        .unwrap_or_default();
    if repo_root.is_empty() {
        return raw.to_string();
    }
    raw.replace(&repo_root, "$REPO_ROOT")
}

fn contract_snapshot(
    case: &CadCliCommandsCaseSnapshot,
    expected: Option<&CadCliCommandsCaseExpectation>,
) -> CadCliCommandsCaseContractSnapshot {
    if let Some(expected) = expected {
        let stdout_marker_match = expected
            .stdout_marker
            .as_ref()
            .is_none_or(|marker| case.stdout.contains(marker));
        let stderr_marker_match = expected
            .stderr_marker
            .as_ref()
            .is_none_or(|marker| case.stderr.contains(marker));

        CadCliCommandsCaseContractSnapshot {
            case_id: case.case_id.clone(),
            command_match: case.command == expected.command,
            exit_code_match: case.exit_code == expected.exit_code,
            stdout_marker_match,
            stderr_marker_match,
        }
    } else {
        CadCliCommandsCaseContractSnapshot {
            case_id: case.case_id.clone(),
            command_match: false,
            exit_code_match: false,
            stdout_marker_match: false,
            stderr_marker_match: false,
        }
    }
}

fn sorted_cases(mut cases: Vec<CadCliCommandsCaseSnapshot>) -> Vec<CadCliCommandsCaseSnapshot> {
    cases.sort_by(|left, right| left.case_id.cmp(&right.case_id));
    cases
}

fn sorted_contract_snapshots(
    mut snapshots: Vec<CadCliCommandsCaseContractSnapshot>,
) -> Vec<CadCliCommandsCaseContractSnapshot> {
    snapshots.sort_by(|left, right| left.case_id.cmp(&right.case_id));
    snapshots
}

#[allow(clippy::too_many_arguments)]
fn parity_signature(
    case_snapshots: &[CadCliCommandsCaseSnapshot],
    case_contract_snapshots: &[CadCliCommandsCaseContractSnapshot],
    reference_commit_match: bool,
    import_stl_counts_match: bool,
    import_step_feature_count_match: bool,
    command_parity_match: bool,
    deterministic_replay_match: bool,
    reference_corpus_sha256: &str,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(format!(
        "ref_match={reference_commit_match};import_stl={import_stl_counts_match};import_step={import_step_feature_count_match};command_parity={command_parity_match};replay={deterministic_replay_match};ref_sha={reference_corpus_sha256}"
    ));

    for case in case_snapshots {
        hasher.update(
            serde_json::to_vec(case).expect("cad cli commands case snapshot should serialize"),
        );
    }
    for contract in case_contract_snapshots {
        hasher.update(
            serde_json::to_vec(contract)
                .expect("cad cli commands case contract snapshot should serialize"),
        );
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
        mesh_id: "mesh.parity.cli.commands".to_string(),
        document_revision: 46,
        variant_id: "variant.parity.cli.commands".to_string(),
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

#[cfg(test)]
mod tests {
    use super::{PARITY_CAD_CLI_COMMANDS_ISSUE_ID, build_cad_cli_commands_parity_manifest};
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
    fn cad_cli_commands_parity_manifest_matches_contract() {
        let scorecard = mock_scorecard();
        let manifest = build_cad_cli_commands_parity_manifest(&scorecard, "scorecard")
            .expect("build cad cli commands parity manifest");
        assert_eq!(manifest.issue_id, PARITY_CAD_CLI_COMMANDS_ISSUE_ID);
        assert!(manifest.reference_commit_match);
        assert!(manifest.import_stl_counts_match);
        assert!(manifest.import_step_feature_count_match);
        assert!(manifest.command_parity_match);
        assert!(manifest.deterministic_replay_match);
        assert_eq!(manifest.case_snapshots.len(), 6);
    }
}
