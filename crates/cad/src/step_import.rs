use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::document::CadDocument;
use crate::format::ApcadDocumentEnvelope;
use crate::hash::stable_hex_digest;
use crate::semantic_refs::CadSemanticRefRegistry;
use crate::step_checker::{CadStepCheckerReport, check_step_text_structural};
use crate::step_import_metadata::CadStepImportMetadata;
use crate::{CadError, CadResult};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CadStepImportResult {
    pub document: CadDocument,
    pub envelope: ApcadDocumentEnvelope,
    pub checker_report: CadStepCheckerReport,
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
        import_metadata,
        import_hash,
        imported_feature_ids,
        stable_ids,
    })
}

#[cfg(test)]
mod tests {
    use super::import_step_text_to_document;
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
    fn step_import_rejects_payload_without_solids() {
        let payload = "ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION(('x'),'2;1');\nENDSEC;\nDATA;\nENDSEC;\nEND-ISO-10303-21;\n";
        let result = import_step_text_to_document(payload, "doc-empty");
        assert!(result.is_err(), "payload without solids should fail import");
    }
}
