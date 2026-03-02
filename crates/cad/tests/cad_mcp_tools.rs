use std::fs;
use std::path::{Path, PathBuf};

use openagents_cad::mcp_tools::{
    CadMcpCreateInput, CadMcpDocument, CadMcpExportInput, CadMcpInspectInput, CadMcpOperation,
    CadMcpPartInput, CadMcpPrimitive, CadMcpPrimitiveType, CadMcpVec3, create_cad_document,
    export_cad, inspect_cad,
};

fn workspace_dir() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let root = manifest_dir
        .parent()
        .and_then(Path::parent)
        .expect("crate should be in <repo>/crates/cad");
    root.join("target/cad-mcp-tools-tests")
}

fn reset_workspace(test_id: &str) -> PathBuf {
    let workspace = workspace_dir().join(test_id);
    if workspace.exists() {
        fs::remove_dir_all(&workspace)
            .unwrap_or_else(|error| panic!("failed to clear {}: {error}", workspace.display()));
    }
    fs::create_dir_all(&workspace)
        .unwrap_or_else(|error| panic!("failed to create {}: {error}", workspace.display()));
    workspace
}

fn sample_create_input() -> CadMcpCreateInput {
    CadMcpCreateInput {
        parts: vec![CadMcpPartInput {
            name: "test_cube".to_string(),
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

fn create_document() -> CadMcpDocument {
    let created = create_cad_document(sample_create_input()).expect("create document");
    serde_json::from_str(&created.content[0].text).expect("parse mcp document")
}

#[test]
fn create_cad_document_produces_valid_roots_and_nodes() {
    let document = create_document();
    assert_eq!(document.version, "0.1");
    assert_eq!(document.roots.len(), 1);
    assert!(document.nodes.len() >= 2, "primitive + translate expected");
    assert_eq!(
        document.part_materials.get("test_cube"),
        Some(&"aluminum".to_string())
    );
}

#[test]
fn inspect_cad_reports_cube_properties() {
    let document = create_document();
    let inspected = inspect_cad(CadMcpInspectInput { ir: document }).expect("inspect cad");
    let payload: serde_json::Value =
        serde_json::from_str(&inspected.content[0].text).expect("parse inspect payload");

    assert_eq!(
        payload.get("parts").and_then(serde_json::Value::as_u64),
        Some(1)
    );
    assert!(
        payload
            .get("volume_mm3")
            .and_then(serde_json::Value::as_f64)
            .unwrap_or_default()
            > 900.0
    );
    assert!(
        payload
            .get("surface_area_mm2")
            .and_then(serde_json::Value::as_f64)
            .unwrap_or_default()
            > 500.0
    );
    assert!(
        payload
            .get("triangles")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or_default()
            > 0
    );
}

#[test]
fn inspect_cad_includes_mass_for_density_material() {
    let document = create_document();
    let inspected = inspect_cad(CadMcpInspectInput { ir: document }).expect("inspect cad");
    let payload: serde_json::Value =
        serde_json::from_str(&inspected.content[0].text).expect("parse inspect payload");

    assert!(payload.get("mass_g").is_some());
    assert!(payload.get("part_masses").is_some());
}

#[test]
fn export_cad_writes_stl_and_glb() {
    let workspace = reset_workspace("export_stl_glb");
    let document = create_document();
    let stl_name = workspace.join("mcp_export.stl");
    let glb_name = workspace.join("mcp_export.glb");

    let stl = export_cad(CadMcpExportInput {
        ir: document.clone(),
        filename: stl_name.to_string_lossy().to_string(),
    })
    .expect("export stl");
    let stl_json: serde_json::Value =
        serde_json::from_str(&stl.content[0].text).expect("parse stl output json");
    assert_eq!(
        stl_json.get("format").and_then(serde_json::Value::as_str),
        Some("stl")
    );
    assert!(stl_name.exists());

    let glb = export_cad(CadMcpExportInput {
        ir: document,
        filename: glb_name.to_string_lossy().to_string(),
    })
    .expect("export glb");
    let glb_json: serde_json::Value =
        serde_json::from_str(&glb.content[0].text).expect("parse glb output json");
    assert_eq!(
        glb_json.get("format").and_then(serde_json::Value::as_str),
        Some("glb")
    );
    assert!(glb_name.exists());
}

#[test]
fn export_cad_rejects_unknown_format() {
    let workspace = reset_workspace("export_unknown_format");
    let document = create_document();
    let err = export_cad(CadMcpExportInput {
        ir: document,
        filename: workspace
            .join("mcp_export.obj")
            .to_string_lossy()
            .to_string(),
    })
    .expect_err("unsupported format should fail");

    assert!(matches!(err, openagents_cad::CadError::ExportFailed { .. }));
    assert!(err.to_string().contains("Unsupported format"));
}
