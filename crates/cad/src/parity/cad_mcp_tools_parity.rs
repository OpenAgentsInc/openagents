use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::hash::stable_hex_digest;
use crate::mcp_tools::{
    CadMcpCreateInput, CadMcpInspectInput, CadMcpInspectResult, CadMcpOperation, CadMcpPartInput,
    CadMcpPrimitive, CadMcpPrimitiveType, CadMcpToolResponse, CadMcpVec3, MCP_CAD_CREATE_TOOL,
    MCP_CAD_EXPORT_TOOL, MCP_CAD_INSPECT_TOOL, cad_document_from_text, create_cad_document,
    create_cad_document_schema, export_cad, export_cad_schema, inspect_cad, inspect_cad_schema,
    mcp_document_hash,
};
use crate::parity::scorecard::ParityScorecard;
use crate::{CadError, CadResult};

pub const PARITY_CAD_MCP_TOOLS_ISSUE_ID: &str = "VCAD-PARITY-085";
pub const CAD_MCP_TOOLS_REFERENCE_CORPUS_PATH: &str =
    "crates/cad/parity/fixtures/cad_mcp_tools_vcad_reference.json";
const CAD_MCP_TOOLS_REFERENCE_CORPUS_JSON: &str =
    include_str!("../../parity/fixtures/cad_mcp_tools_vcad_reference.json");

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CadMcpToolsParityManifest {
    pub manifest_version: u64,
    pub issue_id: String,
    pub vcad_commit: String,
    pub openagents_commit: String,
    pub generated_from_scorecard: String,
    pub reference_corpus_path: String,
    pub reference_corpus_sha256: String,
    pub reference_source: String,
    pub reference_commit_match: bool,
    pub tool_name_match: bool,
    pub schema_contract_match: bool,
    pub case_contract_match: bool,
    pub deterministic_replay_match: bool,
    pub create_snapshot: CadMcpCreateSnapshot,
    pub inspect_snapshot: CadMcpInspectSnapshot,
    pub export_snapshots: Vec<CadMcpExportSnapshot>,
    pub deterministic_signature: String,
    pub parity_contracts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct CadMcpToolsReferenceCorpus {
    manifest_version: u64,
    issue_id: String,
    vcad_commit: String,
    source: String,
    expected_tool_names: Vec<String>,
    expected_export_formats: Vec<CadMcpExportExpectation>,
    expected_create_min_nodes: usize,
    expected_create_root_count: usize,
    expected_inspect_parts: usize,
    expected_inspect_min_volume_mm3: f64,
    expected_inspect_max_volume_mm3: f64,
    expected_inspect_min_triangles: usize,
    expected_mass_present: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct CadMcpExportExpectation {
    format: String,
    min_bytes: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct CadMcpToolsSnapshot {
    create_snapshot: CadMcpCreateSnapshot,
    inspect_snapshot: CadMcpInspectSnapshot,
    export_snapshots: Vec<CadMcpExportSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CadMcpCreateSnapshot {
    pub document_hash: String,
    pub response_hash: String,
    pub node_count: usize,
    pub root_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CadMcpInspectSnapshot {
    pub response_hash: String,
    pub volume_mm3: f64,
    pub surface_area_mm2: f64,
    pub triangles: usize,
    pub parts: usize,
    pub mass_present: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CadMcpExportSnapshot {
    pub format: String,
    pub bytes: usize,
    pub response_hash: String,
    pub output_hash: String,
}

pub fn build_cad_mcp_tools_parity_manifest(
    scorecard: &ParityScorecard,
    scorecard_path: &str,
) -> CadResult<CadMcpToolsParityManifest> {
    let corpus: CadMcpToolsReferenceCorpus =
        serde_json::from_str(CAD_MCP_TOOLS_REFERENCE_CORPUS_JSON).map_err(|error| {
            CadError::ParseFailed {
                reason: format!("failed to parse cad mcp tools reference corpus: {error}"),
            }
        })?;

    let reference_corpus_sha256 = sha256_hex(CAD_MCP_TOOLS_REFERENCE_CORPUS_JSON.as_bytes());
    let reference_commit_match = corpus.vcad_commit == scorecard.vcad_commit;

    let snapshot = collect_cad_mcp_tools_snapshot()?;
    let replay_snapshot = collect_cad_mcp_tools_snapshot()?;
    let deterministic_replay_match = snapshot == replay_snapshot;

    let expected_tool_names = corpus
        .expected_tool_names
        .iter()
        .map(|name| name.to_ascii_lowercase())
        .collect::<Vec<_>>();
    let actual_tool_names = vec![
        MCP_CAD_CREATE_TOOL.to_ascii_lowercase(),
        MCP_CAD_EXPORT_TOOL.to_ascii_lowercase(),
        MCP_CAD_INSPECT_TOOL.to_ascii_lowercase(),
    ];
    let tool_name_match = actual_tool_names == expected_tool_names;

    let create_schema = create_cad_document_schema();
    let export_schema = export_cad_schema();
    let inspect_schema = inspect_cad_schema();
    let schema_contract_match = create_schema.is_object()
        && export_schema.is_object()
        && inspect_schema.is_object()
        && create_schema
            .pointer("/required")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|required| required.iter().any(|entry| entry == "parts"))
        && export_schema
            .pointer("/required")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|required| required.iter().any(|entry| entry == "filename"))
        && inspect_schema
            .pointer("/required")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|required| required.iter().any(|entry| entry == "ir"));

    let expected_export = corpus
        .expected_export_formats
        .iter()
        .map(|entry| (entry.format.clone(), entry.min_bytes))
        .collect::<BTreeMap<_, _>>();
    let actual_export = snapshot
        .export_snapshots
        .iter()
        .map(|entry| (entry.format.clone(), entry.bytes))
        .collect::<BTreeMap<_, _>>();

    let export_contract_match = expected_export.len() == actual_export.len()
        && expected_export.iter().all(|(format, min_bytes)| {
            actual_export
                .get(format)
                .is_some_and(|bytes| bytes >= min_bytes)
        });

    let case_contract_match = snapshot.create_snapshot.node_count
        >= corpus.expected_create_min_nodes
        && snapshot.create_snapshot.root_count == corpus.expected_create_root_count
        && snapshot.inspect_snapshot.parts == corpus.expected_inspect_parts
        && snapshot.inspect_snapshot.volume_mm3 >= corpus.expected_inspect_min_volume_mm3
        && snapshot.inspect_snapshot.volume_mm3 <= corpus.expected_inspect_max_volume_mm3
        && snapshot.inspect_snapshot.triangles >= corpus.expected_inspect_min_triangles
        && snapshot.inspect_snapshot.mass_present == corpus.expected_mass_present
        && export_contract_match;

    let deterministic_signature = parity_signature(
        &snapshot,
        reference_commit_match,
        tool_name_match,
        schema_contract_match,
        case_contract_match,
        deterministic_replay_match,
        &reference_corpus_sha256,
    );

    Ok(CadMcpToolsParityManifest {
        manifest_version: 1,
        issue_id: PARITY_CAD_MCP_TOOLS_ISSUE_ID.to_string(),
        vcad_commit: scorecard.vcad_commit.clone(),
        openagents_commit: scorecard.openagents_commit.clone(),
        generated_from_scorecard: scorecard_path.to_string(),
        reference_corpus_path: CAD_MCP_TOOLS_REFERENCE_CORPUS_PATH.to_string(),
        reference_corpus_sha256,
        reference_source: corpus.source,
        reference_commit_match,
        tool_name_match,
        schema_contract_match,
        case_contract_match,
        deterministic_replay_match,
        create_snapshot: snapshot.create_snapshot,
        inspect_snapshot: snapshot.inspect_snapshot,
        export_snapshots: snapshot.export_snapshots,
        deterministic_signature,
        parity_contracts: vec![
            "create_cad_document returns deterministic IR document text payload".to_string(),
            "inspect_cad returns deterministic geometry summary JSON with volume/area/bbox/com"
                .to_string(),
            "export_cad writes deterministic STL/GLB files and reports path/bytes/format/parts"
                .to_string(),
            "MCP CAD parity fixtures replay deterministically across repeated runs".to_string(),
        ],
    })
}

fn collect_cad_mcp_tools_snapshot() -> CadResult<CadMcpToolsSnapshot> {
    let create_input = sample_create_input();
    let created = create_cad_document(create_input)?;
    let created_text = response_text(&created, MCP_CAD_CREATE_TOOL)?;
    let document = cad_document_from_text(&created_text)?;

    let create_snapshot = CadMcpCreateSnapshot {
        document_hash: mcp_document_hash(&document)?,
        response_hash: stable_hex_digest(created_text.as_bytes()),
        node_count: document.nodes.len(),
        root_count: document.roots.len(),
    };

    let inspected = inspect_cad(CadMcpInspectInput {
        ir: document.clone(),
    })?;
    let inspect_text = response_text(&inspected, MCP_CAD_INSPECT_TOOL)?;
    let inspect_result =
        serde_json::from_str::<CadMcpInspectResult>(&inspect_text).map_err(|error| {
            CadError::ParseFailed {
                reason: format!("failed to parse inspect_cad response text: {error}"),
            }
        })?;
    let inspect_snapshot = CadMcpInspectSnapshot {
        response_hash: stable_hex_digest(inspect_text.as_bytes()),
        volume_mm3: inspect_result.volume_mm3,
        surface_area_mm2: inspect_result.surface_area_mm2,
        triangles: inspect_result.triangles,
        parts: inspect_result.parts,
        mass_present: inspect_result.mass_g.is_some(),
    };

    let workspace = parity_workspace_dir();
    if workspace.exists() {
        fs::remove_dir_all(&workspace).map_err(|error| CadError::ParseFailed {
            reason: format!(
                "failed to clear mcp parity workspace {}: {error}",
                workspace.display()
            ),
        })?;
    }
    fs::create_dir_all(&workspace).map_err(|error| CadError::ParseFailed {
        reason: format!(
            "failed to create mcp parity workspace {}: {error}",
            workspace.display()
        ),
    })?;

    let export_stl = snapshot_export(&workspace, &document, "stl")?;
    let export_glb = snapshot_export(&workspace, &document, "glb")?;

    Ok(CadMcpToolsSnapshot {
        create_snapshot,
        inspect_snapshot,
        export_snapshots: sorted_exports(vec![export_glb, export_stl]),
    })
}

fn snapshot_export(
    workspace: &Path,
    document: &crate::mcp_tools::CadMcpDocument,
    format: &str,
) -> CadResult<CadMcpExportSnapshot> {
    let output = workspace.join(format!("mcp_export.{format}"));
    let response = export_cad(crate::mcp_tools::CadMcpExportInput {
        ir: document.clone(),
        filename: output.to_string_lossy().to_string(),
    })?;
    let text = response_text(&response, MCP_CAD_EXPORT_TOOL)?;
    let output_json: serde_json::Value =
        serde_json::from_str(&text).map_err(|error| CadError::ParseFailed {
            reason: format!("failed to parse export_cad response text: {error}"),
        })?;

    let bytes = output_json
        .get("bytes")
        .and_then(serde_json::Value::as_u64)
        .map(|value| value as usize)
        .ok_or_else(|| CadError::ParseFailed {
            reason: "export_cad response missing bytes field".to_string(),
        })?;

    let output_bytes = fs::read(&output).map_err(|error| CadError::ParseFailed {
        reason: format!("failed reading export output {}: {error}", output.display()),
    })?;

    Ok(CadMcpExportSnapshot {
        format: format.to_string(),
        bytes,
        response_hash: stable_hex_digest(text.as_bytes()),
        output_hash: stable_hex_digest(&output_bytes),
    })
}

fn sample_create_input() -> CadMcpCreateInput {
    CadMcpCreateInput {
        parts: vec![CadMcpPartInput {
            name: "parity_cube".to_string(),
            primitive: CadMcpPrimitive {
                primitive_type: CadMcpPrimitiveType::Cube,
                size: Some(CadMcpVec3 {
                    x: 10.0,
                    y: 10.0,
                    z: 10.0,
                }),
                radius: None,
                height: None,
                segments: None,
                radius_bottom: None,
                radius_top: None,
            },
            operations: vec![CadMcpOperation::Translate {
                offset: CadMcpVec3 {
                    x: 5.0,
                    y: 0.0,
                    z: 0.0,
                },
            }],
            material: Some("aluminum".to_string()),
        }],
        format: None,
    }
}

fn response_text(response: &CadMcpToolResponse, tool: &str) -> CadResult<String> {
    response
        .content
        .first()
        .map(|entry| entry.text.clone())
        .ok_or_else(|| CadError::ParseFailed {
            reason: format!("{tool} returned no content entries"),
        })
}

fn sorted_exports(mut exports: Vec<CadMcpExportSnapshot>) -> Vec<CadMcpExportSnapshot> {
    exports.sort_by(|left, right| left.format.cmp(&right.format));
    exports
}

fn parity_workspace_dir() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let root = manifest_dir
        .parent()
        .and_then(Path::parent)
        .expect("crate should be in <repo>/crates/cad");
    root.join("target/cad-mcp-tools-parity")
}

#[allow(clippy::too_many_arguments)]
fn parity_signature(
    snapshot: &CadMcpToolsSnapshot,
    reference_commit_match: bool,
    tool_name_match: bool,
    schema_contract_match: bool,
    case_contract_match: bool,
    deterministic_replay_match: bool,
    reference_corpus_sha256: &str,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(format!(
        "ref={reference_commit_match};tools={tool_name_match};schema={schema_contract_match};case={case_contract_match};replay={deterministic_replay_match};ref_sha={reference_corpus_sha256}"
    ));
    hasher.update(
        serde_json::to_vec(snapshot).expect("cad mcp tools snapshot should serialize for hashing"),
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
    use super::{PARITY_CAD_MCP_TOOLS_ISSUE_ID, build_cad_mcp_tools_parity_manifest};
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
    fn cad_mcp_tools_manifest_matches_contract() {
        let scorecard = mock_scorecard();
        let manifest = build_cad_mcp_tools_parity_manifest(&scorecard, "scorecard")
            .expect("build cad mcp tools manifest");
        assert_eq!(manifest.issue_id, PARITY_CAD_MCP_TOOLS_ISSUE_ID);
        assert!(manifest.reference_commit_match);
        assert!(manifest.tool_name_match);
        assert!(manifest.schema_contract_match);
        assert!(manifest.case_contract_match);
        assert!(manifest.deterministic_replay_match);
        assert_eq!(manifest.export_snapshots.len(), 2);
    }
}
