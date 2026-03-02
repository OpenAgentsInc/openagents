use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::assembly::{
    CadAssemblyJoint, CadJointKind, CadPartDef, CadPartInstance, CadTransform3D,
};
use crate::document::CadDocument;
use crate::kernel_math::Vec3;
use crate::parity::scorecard::ParityScorecard;
use crate::{CadError, CadResult};

pub const PARITY_ASSEMBLY_SCHEMA_ISSUE_ID: &str = "VCAD-PARITY-056";
pub const ASSEMBLY_SCHEMA_REFERENCE_CORPUS_PATH: &str =
    "crates/cad/parity/fixtures/assembly_schema_vcad_reference.json";
const ASSEMBLY_SCHEMA_REFERENCE_CORPUS_JSON: &str =
    include_str!("../../parity/fixtures/assembly_schema_vcad_reference.json");

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AssemblySchemaParityManifest {
    pub manifest_version: u64,
    pub issue_id: String,
    pub vcad_commit: String,
    pub openagents_commit: String,
    pub generated_from_scorecard: String,
    pub reference_corpus_path: String,
    pub reference_corpus_sha256: String,
    pub reference_source: String,
    pub reference_commit_match: bool,
    pub document_field_names: Vec<String>,
    pub part_def_field_names: Vec<String>,
    pub instance_field_names: Vec<String>,
    pub joint_field_names: Vec<String>,
    pub joint_kind_tags: Vec<String>,
    pub joint_kinds_with_limits: Vec<String>,
    pub part_def_count: usize,
    pub instance_count: usize,
    pub joint_count: usize,
    pub ground_instance_id: String,
    pub schema_field_match: bool,
    pub deterministic_replay_match: bool,
    pub deterministic_signature: String,
    pub parity_contracts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct AssemblySchemaReferenceCorpus {
    manifest_version: u64,
    issue_id: String,
    vcad_commit: String,
    source: String,
    document_field_names: Vec<String>,
    part_def_field_names: Vec<String>,
    instance_field_names: Vec<String>,
    joint_field_names: Vec<String>,
    joint_kind_tags: Vec<String>,
    joint_kinds_with_limits: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct SchemaSnapshot {
    document_field_names: Vec<String>,
    part_def_field_names: Vec<String>,
    instance_field_names: Vec<String>,
    joint_field_names: Vec<String>,
    joint_kind_tags: Vec<String>,
    joint_kinds_with_limits: Vec<String>,
    part_def_count: usize,
    instance_count: usize,
    joint_count: usize,
    ground_instance_id: String,
}

pub fn build_assembly_schema_parity_manifest(
    scorecard: &ParityScorecard,
    scorecard_path: &str,
) -> CadResult<AssemblySchemaParityManifest> {
    let corpus: AssemblySchemaReferenceCorpus =
        serde_json::from_str(ASSEMBLY_SCHEMA_REFERENCE_CORPUS_JSON).map_err(|error| {
            CadError::ParseFailed {
                reason: format!("failed to parse assembly schema reference corpus: {error}"),
            }
        })?;

    let reference_corpus_sha256 = sha256_hex(ASSEMBLY_SCHEMA_REFERENCE_CORPUS_JSON.as_bytes());
    let reference_commit_match = corpus.vcad_commit == scorecard.vcad_commit;

    let snapshot = collect_schema_snapshot()?;
    let replay_snapshot = collect_schema_snapshot()?;
    let deterministic_replay_match = snapshot == replay_snapshot;

    let expected_document_field_names = sorted(corpus.document_field_names.clone());
    let expected_part_def_field_names = sorted(corpus.part_def_field_names.clone());
    let expected_instance_field_names = sorted(corpus.instance_field_names.clone());
    let expected_joint_field_names = sorted(corpus.joint_field_names.clone());
    let expected_joint_kind_tags = sorted(corpus.joint_kind_tags.clone());
    let expected_joint_kinds_with_limits = sorted(corpus.joint_kinds_with_limits.clone());

    let schema_field_match = snapshot.document_field_names == expected_document_field_names
        && snapshot.part_def_field_names == expected_part_def_field_names
        && snapshot.instance_field_names == expected_instance_field_names
        && snapshot.joint_field_names == expected_joint_field_names
        && snapshot.joint_kind_tags == expected_joint_kind_tags
        && snapshot.joint_kinds_with_limits == expected_joint_kinds_with_limits;

    let deterministic_signature = parity_signature(
        &snapshot,
        reference_commit_match,
        schema_field_match,
        deterministic_replay_match,
        &reference_corpus_sha256,
    );

    Ok(AssemblySchemaParityManifest {
        manifest_version: 1,
        issue_id: PARITY_ASSEMBLY_SCHEMA_ISSUE_ID.to_string(),
        vcad_commit: scorecard.vcad_commit.clone(),
        openagents_commit: scorecard.openagents_commit.clone(),
        generated_from_scorecard: scorecard_path.to_string(),
        reference_corpus_path: ASSEMBLY_SCHEMA_REFERENCE_CORPUS_PATH.to_string(),
        reference_corpus_sha256,
        reference_source: corpus.source,
        reference_commit_match,
        document_field_names: snapshot.document_field_names,
        part_def_field_names: snapshot.part_def_field_names,
        instance_field_names: snapshot.instance_field_names,
        joint_field_names: snapshot.joint_field_names,
        joint_kind_tags: snapshot.joint_kind_tags,
        joint_kinds_with_limits: snapshot.joint_kinds_with_limits,
        part_def_count: snapshot.part_def_count,
        instance_count: snapshot.instance_count,
        joint_count: snapshot.joint_count,
        ground_instance_id: snapshot.ground_instance_id,
        schema_field_match,
        deterministic_replay_match,
        deterministic_signature,
        parity_contracts: vec![
            "cad document schema includes partDefs/instances/joints/groundInstanceId assembly fields".to_string(),
            "part definition, instance, and joint payloads serialize with vcad-compatible camelCase keys".to_string(),
            "joint kind tags include Fixed/Revolute/Slider/Cylindrical/Ball and limits only on Revolute/Slider".to_string(),
            "assembly schema parity fixture replays deterministically".to_string(),
        ],
    })
}

fn collect_schema_snapshot() -> CadResult<SchemaSnapshot> {
    let document = sample_document();
    let sample_part_def = sample_part_def();
    let sample_instance = sample_instance();
    let sample_joint = sample_joint();

    let document_value =
        serde_json::to_value(&document).map_err(|error| CadError::Serialization {
            reason: format!("failed to serialize assembly sample document to json value: {error}"),
        })?;
    let part_def_value =
        serde_json::to_value(&sample_part_def).map_err(|error| CadError::Serialization {
            reason: format!("failed to serialize sample part def to json value: {error}"),
        })?;
    let instance_value =
        serde_json::to_value(&sample_instance).map_err(|error| CadError::Serialization {
            reason: format!("failed to serialize sample instance to json value: {error}"),
        })?;
    let joint_value =
        serde_json::to_value(&sample_joint).map_err(|error| CadError::Serialization {
            reason: format!("failed to serialize sample joint to json value: {error}"),
        })?;

    let document_field_names = filtered_document_fields(&document_value)?;
    let part_def_field_names = object_field_names(&part_def_value)?;
    let instance_field_names = object_field_names(&instance_value)?;
    let joint_field_names = object_field_names(&joint_value)?;

    let joint_kind_samples = [
        CadJointKind::Fixed,
        CadJointKind::Revolute {
            axis: Vec3::new(0.0, 0.0, 1.0),
            limits: Some((-90.0, 90.0)),
        },
        CadJointKind::Slider {
            axis: Vec3::new(1.0, 0.0, 0.0),
            limits: Some((0.0, 100.0)),
        },
        CadJointKind::Cylindrical {
            axis: Vec3::new(0.0, 1.0, 0.0),
        },
        CadJointKind::Ball,
    ];

    let mut joint_kind_tags = Vec::with_capacity(joint_kind_samples.len());
    let mut joint_kinds_with_limits = Vec::new();
    for kind in joint_kind_samples {
        let value = serde_json::to_value(&kind).map_err(|error| CadError::Serialization {
            reason: format!("failed to serialize sample joint kind to json value: {error}"),
        })?;
        let object = value.as_object().ok_or_else(|| CadError::ParseFailed {
            reason: "joint kind sample should serialize as object".to_string(),
        })?;
        let tag = object
            .get("type")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| CadError::ParseFailed {
                reason: "joint kind sample missing type tag".to_string(),
            })?
            .to_string();
        if object.contains_key("limits") {
            joint_kinds_with_limits.push(tag.clone());
        }
        joint_kind_tags.push(tag);
    }

    Ok(SchemaSnapshot {
        document_field_names,
        part_def_field_names,
        instance_field_names,
        joint_field_names,
        joint_kind_tags: sorted(joint_kind_tags),
        joint_kinds_with_limits: sorted(joint_kinds_with_limits),
        part_def_count: document.part_defs.as_ref().map_or(0, BTreeMap::len),
        instance_count: document.instances.as_ref().map_or(0, Vec::len),
        joint_count: document.joints.as_ref().map_or(0, Vec::len),
        ground_instance_id: document.ground_instance_id.unwrap_or_default(),
    })
}

fn sample_document() -> CadDocument {
    let mut document = CadDocument::new_empty("doc.assembly.parity");
    document.part_defs = Some(BTreeMap::from([
        ("base".to_string(), sample_part_def()),
        (
            "arm".to_string(),
            CadPartDef {
                id: "arm".to_string(),
                name: Some("Arm".to_string()),
                root: 2,
                default_material: None,
            },
        ),
    ]));
    document.instances = Some(vec![
        CadPartInstance {
            id: "base_inst".to_string(),
            part_def_id: "base".to_string(),
            name: Some("Base Instance".to_string()),
            transform: None,
            material: None,
        },
        sample_instance(),
    ]);
    document.joints = Some(vec![sample_joint()]);
    document.ground_instance_id = Some("base_inst".to_string());
    document
}

fn sample_part_def() -> CadPartDef {
    CadPartDef {
        id: "base".to_string(),
        name: Some("Base Plate".to_string()),
        root: 1,
        default_material: Some("aluminum".to_string()),
    }
}

fn sample_instance() -> CadPartInstance {
    CadPartInstance {
        id: "arm_inst".to_string(),
        part_def_id: "arm".to_string(),
        name: Some("Arm Instance".to_string()),
        transform: Some(CadTransform3D {
            translation: Vec3::new(0.0, 0.0, 10.0),
            rotation: Vec3::new(0.0, 0.0, 0.0),
            scale: Vec3::new(1.0, 1.0, 1.0),
        }),
        material: Some("steel".to_string()),
    }
}

fn sample_joint() -> CadAssemblyJoint {
    CadAssemblyJoint {
        id: "joint.revolute.001".to_string(),
        name: Some("Base-Arm".to_string()),
        parent_instance_id: Some("base_inst".to_string()),
        child_instance_id: "arm_inst".to_string(),
        parent_anchor: Vec3::new(0.0, 0.0, 10.0),
        child_anchor: Vec3::new(0.0, 0.0, 0.0),
        kind: CadJointKind::Revolute {
            axis: Vec3::new(0.0, 0.0, 1.0),
            limits: Some((-90.0, 90.0)),
        },
        state: 0.0,
    }
}

fn filtered_document_fields(document_value: &serde_json::Value) -> CadResult<Vec<String>> {
    let object = document_value
        .as_object()
        .ok_or_else(|| CadError::ParseFailed {
            reason: "assembly sample document should serialize as object".to_string(),
        })?;
    let mut fields = Vec::new();
    for key in ["partDefs", "instances", "joints", "groundInstanceId"] {
        if object.contains_key(key) {
            fields.push(key.to_string());
        }
    }
    Ok(sorted(fields))
}

fn object_field_names(value: &serde_json::Value) -> CadResult<Vec<String>> {
    let object = value.as_object().ok_or_else(|| CadError::ParseFailed {
        reason: "assembly sample payload should serialize as object".to_string(),
    })?;
    Ok(sorted(object.keys().cloned().collect()))
}

fn sorted(mut values: Vec<String>) -> Vec<String> {
    values.sort();
    values.dedup();
    values
}

fn parity_signature(
    snapshot: &SchemaSnapshot,
    reference_commit_match: bool,
    schema_field_match: bool,
    deterministic_replay_match: bool,
    reference_corpus_sha256: &str,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(
        serde_json::to_vec(&(
            snapshot,
            reference_commit_match,
            schema_field_match,
            deterministic_replay_match,
            reference_corpus_sha256,
        ))
        .expect("serialize assembly schema parity payload"),
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
    use super::SchemaSnapshot;
    use super::parity_signature;

    #[test]
    fn parity_signature_is_stable_for_identical_inputs() {
        let snapshot = SchemaSnapshot {
            document_field_names: vec!["partDefs".to_string()],
            part_def_field_names: vec!["id".to_string()],
            instance_field_names: vec!["id".to_string()],
            joint_field_names: vec!["id".to_string()],
            joint_kind_tags: vec!["Fixed".to_string()],
            joint_kinds_with_limits: vec![],
            part_def_count: 1,
            instance_count: 1,
            joint_count: 1,
            ground_instance_id: "base_inst".to_string(),
        };

        let first = parity_signature(&snapshot, true, true, true, "abc");
        let second = parity_signature(&snapshot, true, true, true, "abc");
        assert_eq!(first, second);
    }
}
