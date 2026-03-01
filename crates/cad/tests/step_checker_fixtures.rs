use std::path::{Path, PathBuf};

use openagents_cad::export::export_step_from_mesh;
use openagents_cad::mesh::{
    CadMeshBounds, CadMeshMaterialSlot, CadMeshPayload, CadMeshTopology, CadMeshVertex,
};
use openagents_cad::step_checker::{
    CadStepCheckerBackend, CadStepCheckerReport, check_step_file_with_backend,
};

fn sample_tetra_mesh(variant_id: &str, scale: f32) -> CadMeshPayload {
    CadMeshPayload {
        mesh_id: format!("mesh.{}.fixture", variant_id.replace('.', "-")),
        document_revision: 9,
        variant_id: variant_id.to_string(),
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
                position_mm: [40.0 * scale, 0.0, 0.0],
                normal: [0.0, 0.0, 1.0],
                uv: [1.0, 0.0],
                material_slot: 0,
                flags: 0,
            },
            CadMeshVertex {
                position_mm: [0.0, 40.0 * scale, 0.0],
                normal: [0.0, 0.0, 1.0],
                uv: [0.0, 1.0],
                material_slot: 0,
                flags: 0,
            },
            CadMeshVertex {
                position_mm: [0.0, 0.0, 40.0 * scale],
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
            max_mm: [40.0 * scale, 40.0 * scale, 40.0 * scale],
        },
    }
}

fn artifact_dir() -> PathBuf {
    if let Ok(path) = std::env::var("CAD_STEP_CHECKER_ARTIFACT_DIR") {
        PathBuf::from(path)
    } else {
        std::env::temp_dir().join("openagents-cad-step-checker")
    }
}

fn checker_backend() -> CadStepCheckerBackend {
    let mode =
        std::env::var("CAD_STEP_CHECKER_BACKEND").unwrap_or_else(|_| "structural".to_string());
    if mode.eq_ignore_ascii_case("opencascade") {
        let program = std::env::var("CAD_STEP_CHECKER_OCCT_PROGRAM")
            .unwrap_or_else(|_| "python3".to_string());
        let default_script = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../scripts/cad/opencascade_step_checker.py")
            .display()
            .to_string();
        let script = std::env::var("CAD_STEP_CHECKER_OCCT_SCRIPT").unwrap_or(default_script);
        CadStepCheckerBackend::OpenCascadeCommand {
            program,
            args: vec![script],
        }
    } else {
        CadStepCheckerBackend::Structural
    }
}

fn write_report(path: &Path, report: &CadStepCheckerReport) {
    let payload = report
        .to_pretty_json()
        .expect("checker report should serialize");
    std::fs::write(path, payload).expect("checker report artifact should write");
}

#[test]
fn step_checker_exports_baseline_and_variant_fixtures() {
    let artifact_dir = artifact_dir();
    std::fs::create_dir_all(&artifact_dir).expect("checker artifact dir should exist");
    let fixture_dir = artifact_dir.join("fixtures");
    std::fs::create_dir_all(&fixture_dir).expect("fixture dir should exist");
    let backend = checker_backend();

    let cases = [
        (
            "variant.baseline",
            sample_tetra_mesh("variant.baseline", 1.0_f32),
        ),
        (
            "variant.lightweight",
            sample_tetra_mesh("variant.lightweight", 0.86_f32),
        ),
    ];

    let mut summary = Vec::<(String, bool, usize, usize, usize)>::new();
    for (variant_id, mesh) in cases {
        let artifact = export_step_from_mesh("cad.doc.step-check", 9, variant_id, &mesh)
            .expect("step export should succeed");
        let step_path = fixture_dir.join(format!("{}-fixture.step", variant_id.replace('.', "-")));
        std::fs::write(&step_path, artifact.bytes).expect("step fixture should write");
        let report = check_step_file_with_backend(&step_path, backend.clone())
            .expect("checker backend should run");
        let report_path =
            artifact_dir.join(format!("{}-report.json", variant_id.replace('.', "-")));
        write_report(&report_path, &report);
        summary.push((
            variant_id.to_string(),
            report.passed,
            report.solid_count,
            report.shell_count,
            report.non_manifold_edge_count,
        ));
        assert!(
            report.passed,
            "checker should pass for {variant_id}; diagnostics at {}",
            report_path.display()
        );
    }

    let summary_json = serde_json::to_string_pretty(&summary).expect("summary should serialize");
    std::fs::write(artifact_dir.join("summary.json"), summary_json)
        .expect("summary artifact should write");
}
