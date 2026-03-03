use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::document::CadDocument;
use crate::format::ApcadDocumentEnvelope;
use crate::hash::stable_hex_digest;
use crate::semantic_refs::CadSemanticRefRegistry;
use crate::step_checker::{
    CadStepCheckerReport, check_step_text_structural, collect_step_entity_type_counts,
};
use crate::step_import_metadata::CadStepImportMetadata;
use crate::{CadError, CadResult};

/// STEP entity types currently treated as supported by vcad baseline import paths.
pub const VCAD_STEP_IMPORT_SUPPORTED_ENTITY_TYPES: [&str; 29] = [
    "ADVANCED_FACE",
    "AXIS1_PLACEMENT",
    "AXIS2_PLACEMENT_3D",
    "BOUNDED_SURFACE",
    "B_SPLINE_SURFACE",
    "B_SPLINE_SURFACE_WITH_KNOTS",
    "CARTESIAN_POINT",
    "CIRCLE",
    "CLOSED_SHELL",
    "CONICAL_SURFACE",
    "CYLINDRICAL_SURFACE",
    "DIRECTION",
    "EDGE_CURVE",
    "EDGE_LOOP",
    "FACE",
    "FACETED_BREP",
    "FACE_BOUND",
    "FACE_OUTER_BOUND",
    "LINE",
    "MANIFOLD_SOLID_BREP",
    "OPEN_SHELL",
    "ORIENTED_EDGE",
    "PLANE",
    "POLY_LOOP",
    "SPHERICAL_SURFACE",
    "TOROIDAL_SURFACE",
    "VECTOR",
    "VERTEX_LOOP",
    "VERTEX_POINT",
];

const VCAD_STEP_IMPORT_IGNORED_ENTITY_TYPES: [&str; 22] = [
    "ADVANCED_BREP_SHAPE_REPRESENTATION",
    "APPLICATION_CONTEXT",
    "APPLICATION_PROTOCOL_DEFINITION",
    "DESIGN_CONTEXT",
    "GEOMETRIC_REPRESENTATION_CONTEXT",
    "GLOBAL_UNCERTAINTY_ASSIGNED_CONTEXT",
    "GLOBAL_UNIT_ASSIGNED_CONTEXT",
    "LENGTH_MEASURE",
    "LENGTH_UNIT",
    "MECHANICAL_CONTEXT",
    "NAMED_UNIT",
    "PLANE_ANGLE_UNIT",
    "PRODUCT",
    "PRODUCT_DEFINITION",
    "PRODUCT_DEFINITION_FORMATION",
    "PRODUCT_DEFINITION_SHAPE",
    "PRODUCT_RELATED_PRODUCT_CATEGORY",
    "REPRESENTATION_CONTEXT",
    "SHAPE_DEFINITION_REPRESENTATION",
    "SI_UNIT",
    "SOLID_ANGLE_UNIT",
    "UNCERTAINTY_MEASURE_WITH_UNIT",
];

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CadStepImportEntityCoverage {
    pub total_entity_count: usize,
    pub entity_type_counts: BTreeMap<String, usize>,
    pub supported_entity_types_present: Vec<String>,
    pub unsupported_entity_types_present: Vec<String>,
}

impl CadStepImportEntityCoverage {
    fn from_entity_type_counts(entity_type_counts: BTreeMap<String, usize>) -> Self {
        let supported = VCAD_STEP_IMPORT_SUPPORTED_ENTITY_TYPES
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        let ignored = VCAD_STEP_IMPORT_IGNORED_ENTITY_TYPES
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        let total_entity_count = entity_type_counts.values().copied().sum::<usize>();

        let supported_entity_types_present = entity_type_counts
            .keys()
            .filter(|entity_type| supported.contains(entity_type.as_str()))
            .cloned()
            .collect::<Vec<_>>();
        let unsupported_entity_types_present = entity_type_counts
            .keys()
            .filter(|entity_type| {
                !supported.contains(entity_type.as_str()) && !ignored.contains(entity_type.as_str())
            })
            .cloned()
            .collect::<Vec<_>>();

        Self {
            total_entity_count,
            entity_type_counts,
            supported_entity_types_present,
            unsupported_entity_types_present,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CadStepImportResult {
    pub document: CadDocument,
    pub envelope: ApcadDocumentEnvelope,
    pub checker_report: CadStepCheckerReport,
    pub entity_coverage: CadStepImportEntityCoverage,
    pub import_metadata: CadStepImportMetadata,
    pub import_hash: String,
    pub imported_feature_ids: Vec<String>,
    pub stable_ids: BTreeMap<String, String>,
}

pub fn import_step_text_to_document(
    step_text: &str,
    document_id: &str,
) -> CadResult<CadStepImportResult> {
    if document_id.trim().is_empty() {
        return Err(CadError::ParseFailed {
            reason: "step import requires non-empty document_id".to_string(),
        });
    }
    if step_text.trim().is_empty() {
        return Err(CadError::ParseFailed {
            reason: "step import payload must not be empty".to_string(),
        });
    }

    let entity_coverage = CadStepImportEntityCoverage::from_entity_type_counts(
        collect_step_entity_type_counts(step_text),
    );
    let checker_report = check_step_text_structural(step_text, "step_import");
    if checker_report.solid_count == 0 {
        return Err(CadError::ParseFailed {
            reason: "step import failed: no solids detected in payload".to_string(),
        });
    }
    if !checker_report.passed {
        let codes = checker_report
            .diagnostics
            .iter()
            .map(|entry| entry.code.clone())
            .collect::<Vec<_>>()
            .join(",");
        return Err(CadError::ParseFailed {
            reason: format!(
                "step import failed checker validation: diagnostics={codes} source={}",
                checker_report.source
            ),
        });
    }

    let import_hash = stable_hex_digest(step_text.as_bytes());
    let import_metadata = CadStepImportMetadata::new(
        import_hash.clone(),
        checker_report.solid_count,
        checker_report.shell_count,
        checker_report.face_count,
    )?;
    let mut document = CadDocument::new_empty(document_id);
    document.revision = 1;
    import_metadata.encode_into(&mut document.metadata);

    let mut registry = CadSemanticRefRegistry::default();
    let mut imported_feature_ids = Vec::<String>::new();
    for index in 0..checker_report.solid_count {
        let ordinal = index + 1;
        let feature_id = format!("feature.imported.solid.{ordinal:03}");
        let semantic_ref = format!("imported_solid_{ordinal:03}");
        let entity_id = format!("solid.{ordinal:03}");
        registry.register(&semantic_ref, &entity_id, &feature_id)?;
        document.feature_ids.push(feature_id.clone());
        imported_feature_ids.push(feature_id);
    }

    let mut envelope = ApcadDocumentEnvelope::new(document_id);
    envelope.metadata = document.metadata.clone();
    envelope.set_semantic_ref_registry(&registry);
    let stable_ids = envelope.stable_ids.clone();

    Ok(CadStepImportResult {
        document,
        envelope,
        checker_report,
        entity_coverage,
        import_metadata,
        import_hash,
        imported_feature_ids,
        stable_ids,
    })
}

#[cfg(test)]
mod tests {
    use super::{VCAD_STEP_IMPORT_SUPPORTED_ENTITY_TYPES, import_step_text_to_document};
    use crate::export::export_step_from_mesh;
    use crate::keys::import_metadata as import_keys;
    use crate::mesh::{
        CadMeshBounds, CadMeshMaterialSlot, CadMeshPayload, CadMeshTopology, CadMeshVertex,
    };
    use crate::step_import_metadata::CadStepImportMetadata;

    fn sample_tetra_mesh() -> CadMeshPayload {
        CadMeshPayload {
            mesh_id: "mesh.import.fixture".to_string(),
            document_revision: 42,
            variant_id: "variant.import".to_string(),
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

    fn sample_vcad_advanced_face_step() -> &'static str {
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

    #[test]
    fn step_import_maps_solids_to_stable_ids_and_features() {
        let mesh = sample_tetra_mesh();
        let artifact = export_step_from_mesh(
            "doc-import-source",
            mesh.document_revision,
            &mesh.variant_id,
            &mesh,
        )
        .expect("step export should succeed");
        let result = import_step_text_to_document(
            artifact.text().expect("step payload utf8"),
            "doc-imported",
        )
        .expect("step import should succeed");

        assert_eq!(result.document.document_id, "doc-imported");
        assert_eq!(result.document.revision, 1);
        assert_eq!(result.imported_feature_ids.len(), 1);
        assert_eq!(
            result.imported_feature_ids[0],
            "feature.imported.solid.001".to_string()
        );
        assert_eq!(
            result
                .stable_ids
                .get("imported_solid_001")
                .map(String::as_str),
            Some("solid.001")
        );
        assert!(
            result
                .entity_coverage
                .supported_entity_types_present
                .contains(&"FACETED_BREP".to_string())
        );
    }

    #[test]
    fn imported_stable_ids_survive_apcad_save_reload() {
        let mesh = sample_tetra_mesh();
        let artifact = export_step_from_mesh(
            "doc-import-source",
            mesh.document_revision,
            &mesh.variant_id,
            &mesh,
        )
        .expect("step export should succeed");
        let result =
            import_step_text_to_document(artifact.text().expect("step payload utf8"), "doc-reload")
                .expect("step import should succeed");

        let payload = result
            .envelope
            .to_pretty_json()
            .expect(".apcad serialization should succeed");
        let parsed = crate::format::ApcadDocumentEnvelope::from_json(&payload)
            .expect(".apcad reload should succeed");
        assert_eq!(parsed.stable_ids, result.stable_ids);
        let registry = parsed
            .semantic_ref_registry()
            .expect("semantic refs should recover from stable ids");
        assert_eq!(registry.to_stable_ids(), result.stable_ids);
    }

    #[test]
    fn step_import_is_deterministic_for_same_input() {
        let mesh = sample_tetra_mesh();
        let artifact = export_step_from_mesh(
            "doc-import-source",
            mesh.document_revision,
            &mesh.variant_id,
            &mesh,
        )
        .expect("step export should succeed");
        let step_text = artifact.text().expect("step payload utf8");
        let first =
            import_step_text_to_document(step_text, "doc-deterministic").expect("import first");
        let second =
            import_step_text_to_document(step_text, "doc-deterministic").expect("import second");
        assert_eq!(first.import_hash, second.import_hash);
        assert_eq!(first.document, second.document);
        assert_eq!(first.stable_ids, second.stable_ids);
    }

    #[test]
    fn step_import_result_exposes_typed_metadata_decoded_from_document_map() {
        let mesh = sample_tetra_mesh();
        let artifact = export_step_from_mesh(
            "doc-import-source",
            mesh.document_revision,
            &mesh.variant_id,
            &mesh,
        )
        .expect("step export should succeed");
        let result = import_step_text_to_document(
            artifact.text().expect("step payload utf8"),
            "doc-typed-metadata",
        )
        .expect("step import should succeed");

        let decoded = CadStepImportMetadata::decode_from(&result.document.metadata)
            .expect("typed metadata should decode");
        assert_eq!(decoded, result.import_metadata);
        assert_eq!(
            result
                .document
                .metadata
                .get(import_keys::HASH.as_str())
                .map(String::as_str),
            Some(result.import_hash.as_str())
        );
    }

    #[test]
    fn step_import_accepts_vcad_advanced_face_open_shell_payload() {
        let result =
            import_step_text_to_document(sample_vcad_advanced_face_step(), "doc-vcad-advanced")
                .expect("vcad-style advanced face payload should import");
        assert!(result.checker_report.passed);
        assert_eq!(result.checker_report.solid_count, 1);
        assert_eq!(result.checker_report.shell_count, 1);
        assert_eq!(result.checker_report.face_count, 1);
        assert_eq!(result.imported_feature_ids.len(), 1);
        assert!(
            result
                .entity_coverage
                .supported_entity_types_present
                .contains(&"ADVANCED_FACE".to_string())
        );
        assert!(
            result
                .entity_coverage
                .supported_entity_types_present
                .contains(&"OPEN_SHELL".to_string())
        );
        assert_eq!(
            result.entity_coverage.unsupported_entity_types_present,
            Vec::<String>::new()
        );
    }

    #[test]
    fn step_import_surfaces_unsupported_entity_types_deterministically() {
        let payload = format!(
            "{}#9000=SWEPT_SURFACE('',#11,1.0);\nENDSEC;\nEND-ISO-10303-21;\n",
            sample_vcad_advanced_face_step().replace("ENDSEC;\nEND-ISO-10303-21;\n", "")
        );
        let result = import_step_text_to_document(&payload, "doc-vcad-unsupported")
            .expect("unsupported entities should be reported, not fatal");
        assert_eq!(
            result.entity_coverage.unsupported_entity_types_present,
            vec!["SWEPT_SURFACE".to_string()]
        );
    }

    #[test]
    fn supported_entity_contract_list_is_sorted_and_unique() {
        let mut sorted = VCAD_STEP_IMPORT_SUPPORTED_ENTITY_TYPES
            .iter()
            .map(|value| value.to_string())
            .collect::<Vec<_>>();
        sorted.sort();
        sorted.dedup();
        assert_eq!(
            sorted,
            VCAD_STEP_IMPORT_SUPPORTED_ENTITY_TYPES
                .iter()
                .map(|value| value.to_string())
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn step_import_rejects_payload_without_solids() {
        let payload = "ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION(('x'),'2;1');\nENDSEC;\nDATA;\nENDSEC;\nEND-ISO-10303-21;\n";
        let result = import_step_text_to_document(payload, "doc-empty");
        assert!(result.is_err(), "payload without solids should fail import");
    }
}
