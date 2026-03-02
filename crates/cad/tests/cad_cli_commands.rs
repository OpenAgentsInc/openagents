use std::fs;
use std::path::{Path, PathBuf};

use openagents_cad::cli::{CAD_CLI_APP_NAME, run_cli_tokens};
use openagents_cad::document::CadDocument;
use openagents_cad::export::export_step_from_mesh;
use openagents_cad::mesh::{
    CadMeshBounds, CadMeshMaterialSlot, CadMeshPayload, CadMeshTopology, CadMeshVertex,
};

fn parity_workspace_dir() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let root = manifest_dir
        .parent()
        .and_then(Path::parent)
        .expect("crate should be in <repo>/crates/cad");
    root.join("target/cad-cli-command-tests")
}

fn reset_workspace(test_id: &str) -> PathBuf {
    let workspace = parity_workspace_dir().join(test_id);
    if workspace.exists() {
        fs::remove_dir_all(&workspace)
            .unwrap_or_else(|error| panic!("failed to clear {}: {error}", workspace.display()));
    }
    fs::create_dir_all(&workspace)
        .unwrap_or_else(|error| panic!("failed to create {}: {error}", workspace.display()));
    workspace
}

fn run_cli(args: &[String]) -> openagents_cad::cli::CadCliRunOutcome {
    let refs = args.iter().map(String::as_str).collect::<Vec<_>>();
    run_cli_tokens(&refs)
}

fn write_mesh(path: &Path, mesh: &CadMeshPayload) {
    let mut payload = serde_json::to_string_pretty(mesh).expect("serialize mesh");
    payload.push('\n');
    fs::write(path, payload)
        .unwrap_or_else(|error| panic!("failed to write {}: {error}", path.display()));
}

fn sample_tetra_mesh() -> CadMeshPayload {
    CadMeshPayload {
        mesh_id: "mesh.cli.command.tests".to_string(),
        document_revision: 47,
        variant_id: "variant.cli.command.tests".to_string(),
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

#[test]
fn export_import_info_stl_round_trip_succeeds() {
    let workspace = reset_workspace("stl_roundtrip");
    let mesh = sample_tetra_mesh();
    let input = workspace.join("mesh_input.json");
    let stl = workspace.join("mesh_output.stl");
    let imported = workspace.join("mesh_imported.json");
    write_mesh(&input, &mesh);

    let export = run_cli(&[
        CAD_CLI_APP_NAME.to_string(),
        "export".to_string(),
        input.to_string_lossy().to_string(),
        stl.to_string_lossy().to_string(),
    ]);
    assert_eq!(export.exit_code, 0);
    assert!(stl.exists());

    let import = run_cli(&[
        CAD_CLI_APP_NAME.to_string(),
        "import".to_string(),
        stl.to_string_lossy().to_string(),
        imported.to_string_lossy().to_string(),
    ]);
    assert_eq!(import.exit_code, 0);

    let imported_mesh: CadMeshPayload = serde_json::from_str(
        &fs::read_to_string(&imported)
            .unwrap_or_else(|error| panic!("failed reading {}: {error}", imported.display())),
    )
    .expect("parse imported mesh");
    assert_eq!(imported_mesh.vertices.len(), 4);
    assert_eq!(imported_mesh.triangle_indices.len() / 3, 4);

    let info = run_cli(&[
        CAD_CLI_APP_NAME.to_string(),
        "info".to_string(),
        imported.to_string_lossy().to_string(),
    ]);
    assert_eq!(info.exit_code, 0);
    assert!(info.stdout.contains("openagents cad mesh:"));
    assert!(info.stdout.contains("vertices: 4"));
}

#[test]
fn export_glb_and_step_succeed() {
    let workspace = reset_workspace("export_glb_step");
    let mesh = sample_tetra_mesh();
    let input = workspace.join("mesh_input.json");
    let glb = workspace.join("mesh_output.glb");
    let step = workspace.join("mesh_output.step");
    write_mesh(&input, &mesh);

    let export_glb = run_cli(&[
        CAD_CLI_APP_NAME.to_string(),
        "export".to_string(),
        input.to_string_lossy().to_string(),
        glb.to_string_lossy().to_string(),
    ]);
    assert_eq!(export_glb.exit_code, 0);
    assert!(glb.exists());

    let export_step = run_cli(&[
        CAD_CLI_APP_NAME.to_string(),
        "export".to_string(),
        input.to_string_lossy().to_string(),
        step.to_string_lossy().to_string(),
    ]);
    assert_eq!(export_step.exit_code, 0);
    assert!(step.exists());
}

#[test]
fn import_step_and_info_step_succeed() {
    let workspace = reset_workspace("import_step_info");
    let mesh = sample_tetra_mesh();
    let step_input = workspace.join("input.step");
    let imported = workspace.join("imported_document.json");

    let step = export_step_from_mesh("cli.tests", 11, &mesh.variant_id, &mesh)
        .expect("export step fixture");
    fs::write(&step_input, step.bytes)
        .unwrap_or_else(|error| panic!("failed writing {}: {error}", step_input.display()));

    let import = run_cli(&[
        CAD_CLI_APP_NAME.to_string(),
        "import".to_string(),
        step_input.to_string_lossy().to_string(),
        imported.to_string_lossy().to_string(),
        "--name".to_string(),
        "cli_imported_doc".to_string(),
    ]);
    assert_eq!(import.exit_code, 0);

    let document: CadDocument = serde_json::from_str(
        &fs::read_to_string(&imported)
            .unwrap_or_else(|error| panic!("failed reading {}: {error}", imported.display())),
    )
    .expect("parse imported document");
    assert!(!document.feature_ids.is_empty());

    let info = run_cli(&[
        CAD_CLI_APP_NAME.to_string(),
        "info".to_string(),
        step_input.to_string_lossy().to_string(),
    ]);
    assert_eq!(info.exit_code, 0);
    assert!(info.stdout.contains("openagents cad step:"));
    assert!(info.stdout.contains("solids:"));
}
