use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::hash::stable_hex_digest;
use crate::mcp_tools::{
    CadMcpDocument, CadMcpMaterial, CadMcpNode, CadMcpNodeOp, CadMcpRoot, CadMcpVec3,
    cad_document_from_compact, cad_document_to_compact,
};
use crate::parity::scorecard::ParityScorecard;
use crate::{CadError, CadResult};

pub const PARITY_COMPACT_IR_ISSUE_ID: &str = "VCAD-PARITY-086";
pub const COMPACT_IR_REFERENCE_CORPUS_PATH: &str =
    "crates/cad/parity/fixtures/compact_ir_vcad_reference.json";
const COMPACT_IR_REFERENCE_CORPUS_JSON: &str =
    include_str!("../../parity/fixtures/compact_ir_vcad_reference.json");

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CompactIrParityManifest {
    pub manifest_version: u64,
    pub issue_id: String,
    pub vcad_commit: String,
    pub openagents_commit: String,
    pub generated_from_scorecard: String,
    pub reference_corpus_path: String,
    pub reference_corpus_sha256: String,
    pub reference_source: String,
    pub reference_commit_match: bool,
    pub opcode_coverage_match: bool,
    pub parse_error_contract_match: bool,
    pub roundtrip_contract_match: bool,
    pub deterministic_replay_match: bool,
    pub snapshot: CompactIrSnapshot,
    pub deterministic_signature: String,
    pub parity_contracts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct CompactIrReferenceCorpus {
    manifest_version: u64,
    issue_id: String,
    vcad_commit: String,
    source: String,
    expected_opcodes: Vec<String>,
    expected_min_non_comment_lines: usize,
    expected_node_count: usize,
    expected_root_count: usize,
    expected_root_material: String,
    expected_parse_error_marker: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CompactIrSnapshot {
    pub compact_hash: String,
    pub roundtrip_compact_hash: String,
    pub non_comment_line_count: usize,
    pub node_count: usize,
    pub root_count: usize,
    pub root_material: String,
    pub opcodes: Vec<String>,
    pub parse_error: String,
}

pub fn build_compact_ir_parity_manifest(
    scorecard: &ParityScorecard,
    scorecard_path: &str,
) -> CadResult<CompactIrParityManifest> {
    let corpus: CompactIrReferenceCorpus = serde_json::from_str(COMPACT_IR_REFERENCE_CORPUS_JSON)
        .map_err(|error| CadError::ParseFailed {
        reason: format!("failed to parse compact ir reference corpus: {error}"),
    })?;

    let reference_corpus_sha256 = sha256_hex(COMPACT_IR_REFERENCE_CORPUS_JSON.as_bytes());
    let reference_commit_match = corpus.vcad_commit == scorecard.vcad_commit;

    let snapshot = collect_snapshot()?;
    let replay_snapshot = collect_snapshot()?;
    let deterministic_replay_match = snapshot == replay_snapshot;

    let mut expected_opcodes = corpus.expected_opcodes.clone();
    expected_opcodes.sort();
    let opcode_coverage_match = snapshot.opcodes == expected_opcodes;

    let parse_error_contract_match = snapshot
        .parse_error
        .contains(&corpus.expected_parse_error_marker);

    let roundtrip_contract_match = snapshot.compact_hash == snapshot.roundtrip_compact_hash
        && snapshot.non_comment_line_count >= corpus.expected_min_non_comment_lines
        && snapshot.node_count == corpus.expected_node_count
        && snapshot.root_count == corpus.expected_root_count
        && snapshot.root_material == corpus.expected_root_material;

    let deterministic_signature = parity_signature(
        &snapshot,
        reference_commit_match,
        opcode_coverage_match,
        parse_error_contract_match,
        roundtrip_contract_match,
        deterministic_replay_match,
        &reference_corpus_sha256,
    );

    Ok(CompactIrParityManifest {
        manifest_version: 1,
        issue_id: PARITY_COMPACT_IR_ISSUE_ID.to_string(),
        vcad_commit: scorecard.vcad_commit.clone(),
        openagents_commit: scorecard.openagents_commit.clone(),
        generated_from_scorecard: scorecard_path.to_string(),
        reference_corpus_path: COMPACT_IR_REFERENCE_CORPUS_PATH.to_string(),
        reference_corpus_sha256,
        reference_source: corpus.source,
        reference_commit_match,
        opcode_coverage_match,
        parse_error_contract_match,
        roundtrip_contract_match,
        deterministic_replay_match,
        snapshot,
        deterministic_signature,
        parity_contracts: vec![
            "compact IR serializer emits vcad-style opcode stream with deterministic ordering"
                .to_string(),
            "compact IR parser accepts vcad-style materials/geometry/root records".to_string(),
            "compact IR parser rejects invalid node references with explicit diagnostics"
                .to_string(),
            "compact IR parse/serialize replay is deterministic across repeated runs".to_string(),
        ],
    })
}

fn collect_snapshot() -> CadResult<CompactIrSnapshot> {
    let document = sample_document();
    let compact = cad_document_to_compact(&document)?;
    let parsed = cad_document_from_compact(&compact)?;
    let roundtrip = cad_document_to_compact(&parsed)?;

    let parse_error = cad_document_from_compact("C 10 10 10\nD 0 4")
        .expect_err("invalid compact reference should fail")
        .to_string();

    Ok(CompactIrSnapshot {
        compact_hash: stable_hex_digest(compact.as_bytes()),
        roundtrip_compact_hash: stable_hex_digest(roundtrip.as_bytes()),
        non_comment_line_count: count_non_comment_lines(&compact),
        node_count: parsed.nodes.len(),
        root_count: parsed.roots.len(),
        root_material: parsed
            .roots
            .first()
            .map(|root| root.material.clone())
            .unwrap_or_default(),
        opcodes: collect_opcodes(&compact),
        parse_error,
    })
}

fn sample_document() -> CadMcpDocument {
    CadMcpDocument {
        version: "0.1".to_string(),
        nodes: BTreeMap::from([
            (
                "10".to_string(),
                CadMcpNode {
                    id: 10,
                    name: Some("base".to_string()),
                    op: CadMcpNodeOp::Cube {
                        size: CadMcpVec3 {
                            x: 50.0,
                            y: 30.0,
                            z: 5.0,
                        },
                    },
                },
            ),
            (
                "11".to_string(),
                CadMcpNode {
                    id: 11,
                    name: Some("cyl".to_string()),
                    op: CadMcpNodeOp::Cylinder {
                        radius: 5.0,
                        height: 10.0,
                        segments: 32,
                    },
                },
            ),
            (
                "12".to_string(),
                CadMcpNode {
                    id: 12,
                    name: Some("sphere".to_string()),
                    op: CadMcpNodeOp::Sphere {
                        radius: 4.0,
                        segments: 32,
                    },
                },
            ),
            (
                "13".to_string(),
                CadMcpNode {
                    id: 13,
                    name: Some("cone".to_string()),
                    op: CadMcpNodeOp::Cone {
                        radius_bottom: 6.0,
                        radius_top: 2.0,
                        height: 12.0,
                        segments: 32,
                    },
                },
            ),
            (
                "14".to_string(),
                CadMcpNode {
                    id: 14,
                    name: None,
                    op: CadMcpNodeOp::Translate {
                        child: 11,
                        offset: CadMcpVec3 {
                            x: 20.0,
                            y: 10.0,
                            z: 0.0,
                        },
                    },
                },
            ),
            (
                "15".to_string(),
                CadMcpNode {
                    id: 15,
                    name: None,
                    op: CadMcpNodeOp::Rotate {
                        child: 12,
                        angles: CadMcpVec3 {
                            x: 0.0,
                            y: 45.0,
                            z: 0.0,
                        },
                    },
                },
            ),
            (
                "16".to_string(),
                CadMcpNode {
                    id: 16,
                    name: None,
                    op: CadMcpNodeOp::Scale {
                        child: 13,
                        factor: CadMcpVec3 {
                            x: 1.2,
                            y: 1.2,
                            z: 1.0,
                        },
                    },
                },
            ),
            (
                "17".to_string(),
                CadMcpNode {
                    id: 17,
                    name: None,
                    op: CadMcpNodeOp::Union {
                        left: 10,
                        right: 14,
                    },
                },
            ),
            (
                "18".to_string(),
                CadMcpNode {
                    id: 18,
                    name: None,
                    op: CadMcpNodeOp::Difference {
                        left: 17,
                        right: 15,
                    },
                },
            ),
            (
                "19".to_string(),
                CadMcpNode {
                    id: 19,
                    name: Some("final".to_string()),
                    op: CadMcpNodeOp::Intersection {
                        left: 18,
                        right: 16,
                    },
                },
            ),
        ]),
        materials: BTreeMap::from([
            ("default".to_string(), CadMcpMaterial { density: None }),
            (
                "aluminum".to_string(),
                CadMcpMaterial {
                    density: Some(2700.0),
                },
            ),
        ]),
        roots: vec![CadMcpRoot {
            root: 19,
            material: "aluminum".to_string(),
        }],
        part_materials: BTreeMap::from([(String::from("final"), String::from("aluminum"))]),
    }
}

fn count_non_comment_lines(compact: &str) -> usize {
    compact
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            !trimmed.is_empty() && !trimmed.starts_with('#')
        })
        .count()
}

fn collect_opcodes(compact: &str) -> Vec<String> {
    let mut opcodes = compact
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                return None;
            }
            trimmed
                .split_whitespace()
                .next()
                .map(|opcode| opcode.to_string())
        })
        .collect::<Vec<_>>();
    opcodes.sort();
    opcodes.dedup();
    opcodes
}

fn parity_signature(
    snapshot: &CompactIrSnapshot,
    reference_commit_match: bool,
    opcode_coverage_match: bool,
    parse_error_contract_match: bool,
    roundtrip_contract_match: bool,
    deterministic_replay_match: bool,
    reference_sha256: &str,
) -> String {
    let payload = serde_json::to_vec(&(
        snapshot,
        reference_commit_match,
        opcode_coverage_match,
        parse_error_contract_match,
        roundtrip_contract_match,
        deterministic_replay_match,
        reference_sha256,
    ))
    .expect("serialize compact ir parity signature payload");
    stable_hex_digest(&payload)
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}
