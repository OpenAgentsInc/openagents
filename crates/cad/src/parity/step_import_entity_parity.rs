use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::export::export_step_from_mesh;
use crate::mesh::{
    CadMeshBounds, CadMeshMaterialSlot, CadMeshPayload, CadMeshTopology, CadMeshVertex,
};
use crate::parity::scorecard::ParityScorecard;
use crate::step_import::{VCAD_STEP_IMPORT_SUPPORTED_ENTITY_TYPES, import_step_text_to_document};
use crate::{CadError, CadResult};

pub const PARITY_STEP_IMPORT_ENTITY_ISSUE_ID: &str = "VCAD-PARITY-079";
pub const STEP_IMPORT_ENTITY_REFERENCE_CORPUS_PATH: &str =
    "crates/cad/parity/fixtures/step_import_entity_vcad_reference.json";
const STEP_IMPORT_ENTITY_REFERENCE_CORPUS_JSON: &str =
    include_str!("../../parity/fixtures/step_import_entity_vcad_reference.json");

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StepImportEntityParityManifest {
    pub manifest_version: u64,
    pub issue_id: String,
    pub vcad_commit: String,
    pub openagents_commit: String,
    pub generated_from_scorecard: String,
    pub reference_corpus_path: String,
    pub reference_corpus_sha256: String,
    pub reference_source: String,
    pub reference_commit_match: bool,
    pub supported_entity_types: Vec<String>,
    pub supported_entity_contract_match: bool,
    pub case_snapshots: Vec<StepImportEntityCaseSnapshot>,
    pub step_import_contract_match: bool,
    pub deterministic_replay_match: bool,
    pub deterministic_signature: String,
    pub parity_contracts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct StepImportEntityReferenceCorpus {
    manifest_version: u64,
    issue_id: String,
    vcad_commit: String,
    source: String,
    expected_supported_entity_types: Vec<String>,
    expected_case_snapshots: Vec<StepImportEntityCaseSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct StepImportEntitySnapshot {
    case_snapshots: Vec<StepImportEntityCaseSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StepImportEntityCaseSnapshot {
    pub case_id: String,
    pub checker_passed: bool,
    pub solid_count: usize,
    pub shell_count: usize,
    pub face_count: usize,
    pub imported_feature_count: usize,
    pub supported_entity_type_count: usize,
    pub unsupported_entity_type_count: usize,
    pub supported_entity_types_present: Vec<String>,
    pub unsupported_entity_types_present: Vec<String>,
    pub entity_type_count: usize,
    pub import_hash: String,
}

pub fn build_step_import_entity_parity_manifest(
    scorecard: &ParityScorecard,
    scorecard_path: &str,
) -> CadResult<StepImportEntityParityManifest> {
    let corpus: StepImportEntityReferenceCorpus =
        serde_json::from_str(STEP_IMPORT_ENTITY_REFERENCE_CORPUS_JSON).map_err(|error| {
            CadError::ParseFailed {
                reason: format!("failed to parse step import entity reference corpus: {error}"),
            }
        })?;

    let reference_corpus_sha256 = sha256_hex(STEP_IMPORT_ENTITY_REFERENCE_CORPUS_JSON.as_bytes());
    let reference_commit_match = corpus.vcad_commit == scorecard.vcad_commit;
    let supported_entity_types = sorted_strings(
        VCAD_STEP_IMPORT_SUPPORTED_ENTITY_TYPES
            .iter()
            .map(|entry| entry.to_string())
            .collect::<Vec<_>>(),
    );
    let expected_supported_entity_types = sorted_strings(corpus.expected_supported_entity_types);
    let supported_entity_contract_match = supported_entity_types == expected_supported_entity_types;

    let snapshot = collect_step_import_entity_snapshot()?;
    let replay_snapshot = collect_step_import_entity_snapshot()?;
    let deterministic_replay_match = snapshot == replay_snapshot;

    let expected_case_snapshots = sorted_cases(corpus.expected_case_snapshots);
    let step_import_contract_match =
        supported_entity_contract_match && snapshot.case_snapshots == expected_case_snapshots;

    let deterministic_signature = parity_signature(
        &snapshot.case_snapshots,
        &supported_entity_types,
        reference_commit_match,
        supported_entity_contract_match,
        step_import_contract_match,
        deterministic_replay_match,
        &reference_corpus_sha256,
    );

    Ok(StepImportEntityParityManifest {
        manifest_version: 1,
        issue_id: PARITY_STEP_IMPORT_ENTITY_ISSUE_ID.to_string(),
        vcad_commit: scorecard.vcad_commit.clone(),
        openagents_commit: scorecard.openagents_commit.clone(),
        generated_from_scorecard: scorecard_path.to_string(),
        reference_corpus_path: STEP_IMPORT_ENTITY_REFERENCE_CORPUS_PATH.to_string(),
        reference_corpus_sha256,
        reference_source: corpus.source,
        reference_commit_match,
        supported_entity_types,
        supported_entity_contract_match,
        case_snapshots: snapshot.case_snapshots,
        step_import_contract_match,
        deterministic_replay_match,
        deterministic_signature,
        parity_contracts: vec![
            "STEP import checker accepts vcad-style MANIFOLD_SOLID_BREP + OPEN/CLOSED_SHELL payloads".to_string(),
            "STEP import checker counts both FACE and ADVANCED_FACE entities".to_string(),
            "entity coverage surfaces supported vs unsupported STEP entity types deterministically".to_string(),
            "step import entity parity fixtures replay deterministically".to_string(),
        ],
    })
}

fn collect_step_import_entity_snapshot() -> CadResult<StepImportEntitySnapshot> {
    let faceted_payload = sample_openagents_faceted_payload()?;
    let openagents_faceted = snapshot_for_case("openagents_faceted_export", &faceted_payload)?;
    let vcad_advanced = snapshot_for_case(
        "vcad_advanced_face_open_shell",
        sample_vcad_advanced_face_open_shell_payload(),
    )?;
    let vcad_advanced_with_unsupported = snapshot_for_case(
        "vcad_advanced_face_with_unsupported_entity",
        &format!(
            "{}#9000=SWEPT_SURFACE('',#11,1.0);\nENDSEC;\nEND-ISO-10303-21;\n",
            sample_vcad_advanced_face_open_shell_payload()
                .replace("ENDSEC;\nEND-ISO-10303-21;\n", "")
        ),
    )?;

    let case_snapshots = sorted_cases(vec![
        openagents_faceted,
        vcad_advanced,
        vcad_advanced_with_unsupported,
    ]);
    Ok(StepImportEntitySnapshot { case_snapshots })
}

fn snapshot_for_case(case_id: &str, step_text: &str) -> CadResult<StepImportEntityCaseSnapshot> {
    let document_id = format!("doc.parity.step.import.entity.{case_id}");
    let result = import_step_text_to_document(step_text, &document_id)?;
    let supported_entity_types_present = sorted_strings(
        result
            .entity_coverage
            .supported_entity_types_present
            .clone(),
    );
    let unsupported_entity_types_present = sorted_strings(
        result
            .entity_coverage
            .unsupported_entity_types_present
            .clone(),
    );

    Ok(StepImportEntityCaseSnapshot {
        case_id: case_id.to_string(),
        checker_passed: result.checker_report.passed,
        solid_count: result.checker_report.solid_count,
        shell_count: result.checker_report.shell_count,
        face_count: result.checker_report.face_count,
        imported_feature_count: result.imported_feature_ids.len(),
        supported_entity_type_count: supported_entity_types_present.len(),
        unsupported_entity_type_count: unsupported_entity_types_present.len(),
        supported_entity_types_present,
        unsupported_entity_types_present,
        entity_type_count: result.entity_coverage.entity_type_counts.len(),
        import_hash: result.import_hash,
    })
}

fn sample_openagents_faceted_payload() -> CadResult<String> {
    let mesh = sample_tetra_mesh();
    let artifact = export_step_from_mesh(
        "doc.parity.step.import.entity",
        mesh.document_revision,
        &mesh.variant_id,
        &mesh,
    )?;
    artifact.text().map(|text| text.to_string())
}

fn sample_tetra_mesh() -> CadMeshPayload {
    CadMeshPayload {
        mesh_id: "mesh.parity.step.import.entity".to_string(),
        document_revision: 97,
        variant_id: "variant.parity.step.import.entity".to_string(),
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
        triangle_indices: vec![
            0, 1, 2, //
            0, 1, 3, //
            1, 2, 3, //
            0, 2, 3, //
        ],
        edges: Vec::new(),
        material_slots: vec![CadMeshMaterialSlot::default()],
        bounds: CadMeshBounds {
            min_mm: [0.0, 0.0, 0.0],
            max_mm: [20.0, 20.0, 20.0],
        },
    }
}

fn sample_vcad_advanced_face_open_shell_payload() -> &'static str {
    "ISO-10303-21;\n\
HEADER;\n\
FILE_DESCRIPTION(('vcad parity fixture'),'2;1');\n\
FILE_NAME('vcad.step','1970-01-01T00:00:00',('vcad'),('vcad'),'vcad-kernel-step','vcad','');\n\
FILE_SCHEMA(('AUTOMOTIVE_DESIGN'));\n\
ENDSEC;\n\
DATA;\n\
#1=CARTESIAN_POINT('',(0.0,0.0,0.0));\n\
#2=CARTESIAN_POINT('',(10.0,0.0,0.0));\n\
#3=CARTESIAN_POINT('',(10.0,10.0,0.0));\n\
#4=CARTESIAN_POINT('',(0.0,10.0,0.0));\n\
#5=VERTEX_POINT('',#1);\n\
#6=VERTEX_POINT('',#2);\n\
#7=VERTEX_POINT('',#3);\n\
#8=VERTEX_POINT('',#4);\n\
#9=DIRECTION('',(0.0,0.0,1.0));\n\
#10=DIRECTION('',(1.0,0.0,0.0));\n\
#11=AXIS2_PLACEMENT_3D('',#1,#9,#10);\n\
#12=PLANE('',#11);\n\
#13=DIRECTION('',(1.0,0.0,0.0));\n\
#14=DIRECTION('',(0.0,1.0,0.0));\n\
#15=DIRECTION('',(-1.0,0.0,0.0));\n\
#16=DIRECTION('',(0.0,-1.0,0.0));\n\
#17=VECTOR('',#13,10.0);\n\
#18=VECTOR('',#14,10.0);\n\
#19=VECTOR('',#15,10.0);\n\
#20=VECTOR('',#16,10.0);\n\
#21=LINE('',#1,#17);\n\
#22=LINE('',#2,#18);\n\
#23=LINE('',#3,#19);\n\
#24=LINE('',#4,#20);\n\
#25=EDGE_CURVE('',#5,#6,#21,.T.);\n\
#26=EDGE_CURVE('',#6,#7,#22,.T.);\n\
#27=EDGE_CURVE('',#7,#8,#23,.T.);\n\
#28=EDGE_CURVE('',#8,#5,#24,.T.);\n\
#29=ORIENTED_EDGE('',*,*,#25,.T.);\n\
#30=ORIENTED_EDGE('',*,*,#26,.T.);\n\
#31=ORIENTED_EDGE('',*,*,#27,.T.);\n\
#32=ORIENTED_EDGE('',*,*,#28,.T.);\n\
#33=EDGE_LOOP('',(#29,#30,#31,#32));\n\
#34=FACE_OUTER_BOUND('',#33,.T.);\n\
#35=ADVANCED_FACE('',(#34),#12,.T.);\n\
#36=OPEN_SHELL('',(#35));\n\
#37=MANIFOLD_SOLID_BREP('SheetLike',#36);\n\
ENDSEC;\n\
END-ISO-10303-21;\n"
}

fn sorted_cases(mut cases: Vec<StepImportEntityCaseSnapshot>) -> Vec<StepImportEntityCaseSnapshot> {
    cases.sort_by(|left, right| left.case_id.cmp(&right.case_id));
    cases
}

fn sorted_strings(mut values: Vec<String>) -> Vec<String> {
    values.sort();
    values.dedup();
    values
}

fn parity_signature(
    case_snapshots: &[StepImportEntityCaseSnapshot],
    supported_entity_types: &[String],
    reference_commit_match: bool,
    supported_entity_contract_match: bool,
    step_import_contract_match: bool,
    deterministic_replay_match: bool,
    reference_corpus_sha256: &str,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(
        serde_json::to_vec(&(
            case_snapshots,
            supported_entity_types,
            reference_commit_match,
            supported_entity_contract_match,
            step_import_contract_match,
            deterministic_replay_match,
            reference_corpus_sha256,
        ))
        .expect("serialize step import entity parity payload"),
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
    use super::{StepImportEntityCaseSnapshot, parity_signature};

    #[test]
    fn parity_signature_is_stable_for_identical_inputs() {
        let cases = vec![StepImportEntityCaseSnapshot {
            case_id: "fixture".to_string(),
            checker_passed: true,
            solid_count: 1,
            shell_count: 1,
            face_count: 1,
            imported_feature_count: 1,
            supported_entity_type_count: 4,
            unsupported_entity_type_count: 0,
            supported_entity_types_present: vec!["ADVANCED_FACE".to_string()],
            unsupported_entity_types_present: Vec::new(),
            entity_type_count: 8,
            import_hash: "abcdef0123456789".to_string(),
        }];
        let supported = vec!["ADVANCED_FACE".to_string(), "OPEN_SHELL".to_string()];
        let first = parity_signature(&cases, &supported, true, true, true, true, "sha");
        let second = parity_signature(&cases, &supported, true, true, true, true, "sha");
        assert_eq!(first, second);
    }
}
