#![allow(clippy::all, clippy::expect_used, clippy::panic, clippy::pedantic, clippy::print_stderr, clippy::print_stdout, clippy::unwrap_used)]

use openagents_cad::export::export_step_from_mesh;
use openagents_cad::format::ApcadDocumentEnvelope;
use openagents_cad::keys::import_metadata as import_keys;
use openagents_cad::mesh::{
    CadMeshBounds, CadMeshMaterialSlot, CadMeshPayload, CadMeshTopology, CadMeshVertex,
};
use openagents_cad::step_import::import_step_text_to_document;
use openagents_cad::step_import_metadata::CadStepImportMetadata;

fn sample_tetra_mesh() -> CadMeshPayload {
    CadMeshPayload {
        mesh_id: "mesh.import.pipeline".to_string(),
        document_revision: 64,
        variant_id: "variant.import.pipeline".to_string(),
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
                position_mm: [30.0, 0.0, 0.0],
                normal: [0.0, 0.0, 1.0],
                uv: [1.0, 0.0],
                material_slot: 0,
                flags: 0,
            },
            CadMeshVertex {
                position_mm: [0.0, 30.0, 0.0],
                normal: [0.0, 0.0, 1.0],
                uv: [0.0, 1.0],
                material_slot: 0,
                flags: 0,
            },
            CadMeshVertex {
                position_mm: [0.0, 0.0, 30.0],
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
            max_mm: [30.0, 30.0, 30.0],
        },
    }
}

#[test]
fn step_import_pipeline_maps_stable_ids_and_survives_apcad_reload() {
    let mesh = sample_tetra_mesh();
    let exported = export_step_from_mesh(
        "doc.import.pipeline.source",
        mesh.document_revision,
        &mesh.variant_id,
        &mesh,
    )
    .expect("step export should succeed");

    let imported = import_step_text_to_document(
        exported.text().expect("step payload should decode"),
        "doc.import.pipeline.target",
    )
    .expect("step import should succeed");

    assert_eq!(
        imported.imported_feature_ids,
        vec!["feature.imported.solid.001".to_string()]
    );
    assert_eq!(
        imported
            .stable_ids
            .get("imported_solid_001")
            .map(String::as_str),
        Some("solid.001")
    );
    assert_eq!(
        imported
            .document
            .metadata
            .get(import_keys::FORMAT.as_str())
            .map(String::as_str),
        Some("step")
    );
    assert_eq!(
        imported
            .document
            .metadata
            .get(import_keys::HASH.as_str())
            .map(String::as_str),
        Some(imported.import_hash.as_str())
    );
    let decoded =
        CadStepImportMetadata::decode_from(&imported.document.metadata).expect("typed metadata");
    assert_eq!(decoded, imported.import_metadata);

    let serialized = imported
        .envelope
        .to_pretty_json()
        .expect(".apcad serialization should succeed");
    let parsed =
        ApcadDocumentEnvelope::from_json(&serialized).expect(".apcad parse should succeed");
    assert_eq!(parsed.stable_ids, imported.stable_ids);
}

#[test]
fn step_import_pipeline_is_deterministic_for_identical_inputs() {
    let mesh = sample_tetra_mesh();
    let exported = export_step_from_mesh(
        "doc.import.pipeline.source",
        mesh.document_revision,
        &mesh.variant_id,
        &mesh,
    )
    .expect("step export should succeed");
    let step_text = exported.text().expect("step payload should decode");

    let first =
        import_step_text_to_document(step_text, "doc.import.pipeline.target").expect("first pass");
    let second =
        import_step_text_to_document(step_text, "doc.import.pipeline.target").expect("second pass");

    assert_eq!(first.import_hash, second.import_hash);
    assert_eq!(first.document, second.document);
    assert_eq!(first.stable_ids, second.stable_ids);
    assert_eq!(first.checker_report, second.checker_report);
}

#[test]
fn step_import_pipeline_rejects_structural_checker_errors() {
    let malformed = "ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION(('OpenAgents CAD STEP export'),'2;1');\nFILE_NAME('bad.step','1970-01-01T00:00:00',('OpenAgents'),('OpenAgents'),'openagents-cad','openagents-cad','deterministic');\nFILE_SCHEMA(('AUTOMOTIVE_DESIGN_CC2'));\nENDSEC;\nDATA;\n#1=FACETED_BREP('bad',#2);\nENDSEC;\nEND-ISO-10303-21;\n";
    let error = import_step_text_to_document(malformed, "doc.import.pipeline.bad")
        .expect_err("invalid shell topology should fail import");
    let message = error.to_string();
    assert!(
        message.contains("STEP_MISSING_SHELL"),
        "error should include checker diagnostic code: {message}"
    );
}
