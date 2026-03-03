use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::assembly::{CadAssemblySchema, CadPartInstance};
use crate::parity::scorecard::ParityScorecard;
use crate::{CadError, CadResult};

pub const PARITY_ASSEMBLY_PART_INSTANCE_ISSUE_ID: &str = "VCAD-PARITY-057";
pub const ASSEMBLY_PART_INSTANCE_REFERENCE_CORPUS_PATH: &str =
    "crates/cad/parity/fixtures/assembly_part_instance_vcad_reference.json";
const ASSEMBLY_PART_INSTANCE_REFERENCE_CORPUS_JSON: &str =
    include_str!("../../parity/fixtures/assembly_part_instance_vcad_reference.json");

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AssemblyPartInstanceParityManifest {
    pub manifest_version: u64,
    pub issue_id: String,
    pub vcad_commit: String,
    pub openagents_commit: String,
    pub generated_from_scorecard: String,
    pub reference_corpus_path: String,
    pub reference_corpus_sha256: String,
    pub reference_source: String,
    pub reference_commit_match: bool,
    pub operation_trace: Vec<String>,
    pub created_instance_ids: Vec<String>,
    pub resolved_instance_ids: Vec<String>,
    pub unresolved_instance_ids: Vec<String>,
    pub material_fallback_order: Vec<String>,
    pub material_sources: BTreeMap<String, String>,
    pub effective_materials: BTreeMap<String, String>,
    pub id_pattern_match: bool,
    pub behavior_match: bool,
    pub deterministic_replay_match: bool,
    pub deterministic_signature: String,
    pub parity_contracts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct AssemblyPartInstanceReferenceCorpus {
    manifest_version: u64,
    issue_id: String,
    vcad_commit: String,
    source: String,
    expected_instance_id_pattern: String,
    expected_material_fallback_order: Vec<String>,
    expected_resolved_instance_ids: Vec<String>,
    expected_material_sources: BTreeMap<String, String>,
    expected_effective_materials: BTreeMap<String, String>,
    expected_unresolved_instance_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct PartInstanceSnapshot {
    operation_trace: Vec<String>,
    created_instance_ids: Vec<String>,
    resolved_instance_ids: Vec<String>,
    unresolved_instance_ids: Vec<String>,
    material_fallback_order: Vec<String>,
    material_sources: BTreeMap<String, String>,
    effective_materials: BTreeMap<String, String>,
    id_pattern_match: bool,
}

pub fn build_assembly_part_instance_parity_manifest(
    scorecard: &ParityScorecard,
    scorecard_path: &str,
) -> CadResult<AssemblyPartInstanceParityManifest> {
    let corpus: AssemblyPartInstanceReferenceCorpus =
        serde_json::from_str(ASSEMBLY_PART_INSTANCE_REFERENCE_CORPUS_JSON).map_err(|error| {
            CadError::ParseFailed {
                reason: format!("failed to parse assembly part/instance reference corpus: {error}"),
            }
        })?;

    let reference_corpus_sha256 =
        sha256_hex(ASSEMBLY_PART_INSTANCE_REFERENCE_CORPUS_JSON.as_bytes());
    let reference_commit_match = corpus.vcad_commit == scorecard.vcad_commit;

    let snapshot = run_part_instance_scenario()?;
    let replay_snapshot = run_part_instance_scenario()?;
    let deterministic_replay_match = snapshot == replay_snapshot;

    let expected_resolved_instance_ids = sorted(corpus.expected_resolved_instance_ids);
    let expected_unresolved_instance_ids = sorted(corpus.expected_unresolved_instance_ids);
    let expected_material_fallback_order = corpus.expected_material_fallback_order;

    let behavior_match = snapshot.id_pattern_match
        && snapshot.resolved_instance_ids == expected_resolved_instance_ids
        && snapshot.unresolved_instance_ids == expected_unresolved_instance_ids
        && snapshot.material_fallback_order == expected_material_fallback_order
        && snapshot.material_sources == corpus.expected_material_sources
        && snapshot.effective_materials == corpus.expected_effective_materials;

    let deterministic_signature = parity_signature(
        &snapshot,
        reference_commit_match,
        behavior_match,
        deterministic_replay_match,
        &reference_corpus_sha256,
    );

    Ok(AssemblyPartInstanceParityManifest {
        manifest_version: 1,
        issue_id: PARITY_ASSEMBLY_PART_INSTANCE_ISSUE_ID.to_string(),
        vcad_commit: scorecard.vcad_commit.clone(),
        openagents_commit: scorecard.openagents_commit.clone(),
        generated_from_scorecard: scorecard_path.to_string(),
        reference_corpus_path: ASSEMBLY_PART_INSTANCE_REFERENCE_CORPUS_PATH.to_string(),
        reference_corpus_sha256,
        reference_source: corpus.source,
        reference_commit_match,
        operation_trace: snapshot.operation_trace,
        created_instance_ids: snapshot.created_instance_ids,
        resolved_instance_ids: snapshot.resolved_instance_ids,
        unresolved_instance_ids: snapshot.unresolved_instance_ids,
        material_fallback_order: snapshot.material_fallback_order,
        material_sources: snapshot.material_sources,
        effective_materials: snapshot.effective_materials,
        id_pattern_match: snapshot.id_pattern_match,
        behavior_match,
        deterministic_replay_match,
        deterministic_signature,
        parity_contracts: vec![
            "instance creation requires existing part definitions and uses deterministic ids (`<partDefId>-<n>`)".to_string(),
            "resolved instance materials follow vcad order: instance override -> part default -> fallback default".to_string(),
            "instances with missing part definitions are surfaced as unresolved and skipped from resolved bindings".to_string(),
            "part definition/instance parity scenario replays deterministically".to_string(),
        ],
    })
}

fn run_part_instance_scenario() -> CadResult<PartInstanceSnapshot> {
    let mut schema = CadAssemblySchema::default();
    let mut operation_trace = Vec::new();

    let base = schema.create_part_def(
        "base",
        1,
        Some("Base".to_string()),
        Some("powder_coat".to_string()),
    )?;
    operation_trace.push(format!("create_part_def:{base}"));

    let arm =
        schema.create_part_def("arm", 2, Some("Arm".to_string()), Some("steel".to_string()))?;
    operation_trace.push(format!("create_part_def:{arm}"));

    let pin = schema.create_part_def("pin", 3, Some("Pin".to_string()), None)?;
    operation_trace.push(format!("create_part_def:{pin}"));

    let arm_one = schema.create_instance("arm", Some("Arm One".to_string()), None)?;
    let arm_two = schema.create_instance("arm", Some("Arm Two".to_string()), None)?;
    let base_one = schema.create_instance("base", Some("Base One".to_string()), None)?;
    let pin_one = schema.create_instance("pin", Some("Pin One".to_string()), None)?;
    operation_trace.push(format!(
        "create_instance:{},{},{},{}",
        arm_one, arm_two, base_one, pin_one
    ));

    schema.set_instance_material(&arm_two, Some("anodized".to_string()))?;
    operation_trace.push(format!("set_instance_material:{arm_two}:anodized"));

    schema.instances.push(CadPartInstance {
        id: "orphan-1".to_string(),
        part_def_id: "missing".to_string(),
        name: Some("Orphan".to_string()),
        transform: None,
        material: None,
    });
    operation_trace.push("inject_unresolved:orphan-1".to_string());

    let resolved = schema.resolve_part_instances();

    let mut created_instance_ids = vec![arm_one, arm_two, base_one, pin_one];
    let mut resolved_instance_ids = resolved
        .resolved_instances
        .iter()
        .map(|instance| instance.instance_id.clone())
        .collect::<Vec<_>>();
    let unresolved_instance_ids = resolved.unresolved_instance_ids;
    created_instance_ids = sorted(created_instance_ids);
    resolved_instance_ids = sorted(resolved_instance_ids);

    let material_sources = resolved
        .resolved_instances
        .iter()
        .map(|instance| {
            (
                instance.instance_id.clone(),
                instance.material_source.clone(),
            )
        })
        .collect::<BTreeMap<_, _>>();

    let effective_materials = resolved
        .resolved_instances
        .iter()
        .map(|instance| {
            (
                instance.instance_id.clone(),
                instance.effective_material.clone(),
            )
        })
        .collect::<BTreeMap<_, _>>();

    let material_fallback_order = vec![
        material_sources
            .get("arm-2")
            .cloned()
            .unwrap_or_else(|| "missing".to_string()),
        material_sources
            .get("arm-1")
            .cloned()
            .unwrap_or_else(|| "missing".to_string()),
        material_sources
            .get("pin-1")
            .cloned()
            .unwrap_or_else(|| "missing".to_string()),
    ];

    let id_pattern_match = created_instance_ids
        .iter()
        .all(|instance_id| matches_instance_id_pattern(instance_id));

    Ok(PartInstanceSnapshot {
        operation_trace,
        created_instance_ids,
        resolved_instance_ids,
        unresolved_instance_ids,
        material_fallback_order,
        material_sources,
        effective_materials,
        id_pattern_match,
    })
}

fn matches_instance_id_pattern(instance_id: &str) -> bool {
    let Some((prefix, suffix)) = instance_id.rsplit_once('-') else {
        return false;
    };
    !prefix.is_empty() && suffix.parse::<u64>().is_ok()
}

fn sorted(mut values: Vec<String>) -> Vec<String> {
    values.sort();
    values.dedup();
    values
}

fn parity_signature(
    snapshot: &PartInstanceSnapshot,
    reference_commit_match: bool,
    behavior_match: bool,
    deterministic_replay_match: bool,
    reference_corpus_sha256: &str,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(
        serde_json::to_vec(&(
            snapshot,
            reference_commit_match,
            behavior_match,
            deterministic_replay_match,
            reference_corpus_sha256,
        ))
        .expect("serialize assembly part-instance parity payload"),
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
    use super::PartInstanceSnapshot;
    use super::parity_signature;

    #[test]
    fn parity_signature_is_stable_for_identical_inputs() {
        let snapshot = PartInstanceSnapshot {
            operation_trace: vec!["create_part_def:base".to_string()],
            created_instance_ids: vec!["base-1".to_string()],
            resolved_instance_ids: vec!["base-1".to_string()],
            unresolved_instance_ids: vec![],
            material_fallback_order: vec!["part_default".to_string()],
            material_sources: std::collections::BTreeMap::from([(
                "base-1".to_string(),
                "part_default".to_string(),
            )]),
            effective_materials: std::collections::BTreeMap::from([(
                "base-1".to_string(),
                "powder_coat".to_string(),
            )]),
            id_pattern_match: true,
        };

        let first = parity_signature(&snapshot, true, true, true, "abc");
        let second = parity_signature(&snapshot, true, true, true, "abc");
        assert_eq!(first, second);
    }
}
