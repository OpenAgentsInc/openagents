#![allow(clippy::all, clippy::expect_used, clippy::panic, clippy::pedantic, clippy::print_stderr, clippy::print_stdout, clippy::unwrap_used)]

use std::path::PathBuf;

use openagents_cad::export::export_step_from_mesh;
use openagents_cad::mesh::{
    CadMeshBounds, CadMeshMaterialSlot, CadMeshPayload, CadMeshTopology, CadMeshVertex,
};
use openagents_cad::step_checker::check_step_text_structural;
use openagents_cad::step_roundtrip::{
    CadStepRoundTripTolerance, assert_step_round_trip_tolerance, evaluate_step_round_trip_tolerance,
};

fn sample_tetra_mesh(variant_id: &str, scale: f32) -> CadMeshPayload {
    CadMeshPayload {
        mesh_id: format!("mesh.{}.roundtrip-fixture", variant_id.replace('.', "-")),
        document_revision: 11,
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
        std::env::temp_dir().join("openagents-cad-step-roundtrip")
    }
}

#[test]
fn step_roundtrip_tolerance_baseline_and_boundary_fixtures() {
    let artifact_dir = artifact_dir();
    std::fs::create_dir_all(&artifact_dir).expect("artifact dir should exist");
    let tolerance = CadStepRoundTripTolerance {
        bbox_axis_delta_mm: 0.05,
        volume_delta_mm3: 10.0,
    };

    for (variant_id, scale) in [
        ("variant.baseline", 1.0_f32),
        ("variant.lightweight", 0.87_f32),
    ] {
        let mesh = sample_tetra_mesh(variant_id, scale);
        let step = export_step_from_mesh(
            "cad.doc.roundtrip",
            mesh.document_revision,
            variant_id,
            &mesh,
        )
        .expect("step export should succeed");
        let checker = check_step_text_structural(
            step.text().expect("step payload should decode"),
            variant_id,
        );

        let passing = assert_step_round_trip_tolerance(&mesh, &checker, tolerance)
            .expect("identical roundtrip should pass tolerance");
        std::fs::write(
            artifact_dir.join(format!(
                "{}-roundtrip-pass.json",
                variant_id.replace('.', "-")
            )),
            passing
                .to_pretty_json()
                .expect("roundtrip pass report should serialize"),
        )
        .expect("roundtrip pass report should write");

        let mut near_pass_checker = checker.clone();
        if let Some(mut max) = near_pass_checker.bbox_max_mm {
            max[0] += 0.049;
            near_pass_checker.bbox_max_mm = Some(max);
        }
        let near_pass = evaluate_step_round_trip_tolerance(&mesh, &near_pass_checker, tolerance)
            .expect("near-threshold pass report should evaluate");
        assert!(
            near_pass.passed,
            "near-threshold fixture should pass (variant={variant_id})"
        );
        std::fs::write(
            artifact_dir.join(format!(
                "{}-roundtrip-near-pass.json",
                variant_id.replace('.', "-")
            )),
            near_pass
                .to_pretty_json()
                .expect("near pass report should serialize"),
        )
        .expect("near pass report should write");

        let mut near_fail_checker = checker.clone();
        if let Some(mut max) = near_fail_checker.bbox_max_mm {
            max[0] += 0.051;
            near_fail_checker.bbox_max_mm = Some(max);
        }
        let near_fail = evaluate_step_round_trip_tolerance(&mesh, &near_fail_checker, tolerance)
            .expect("near-threshold fail report should evaluate");
        assert!(
            !near_fail.passed,
            "near-threshold fixture should fail (variant={variant_id})"
        );
        assert!(
            near_fail
                .failure_messages
                .iter()
                .any(|message| message.contains("bbox_extent_x_mm"))
        );
        std::fs::write(
            artifact_dir.join(format!(
                "{}-roundtrip-near-fail.json",
                variant_id.replace('.', "-")
            )),
            near_fail
                .to_pretty_json()
                .expect("near fail report should serialize"),
        )
        .expect("near fail report should write");
    }
}
