use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(Path::parent)
        .expect("crate should be in <repo>/crates/cad")
        .to_path_buf()
}

#[test]
fn parity_check_script_list_mode_includes_required_lanes() {
    let script = repo_root().join("scripts/cad/parity_check.sh");
    let output = Command::new("bash")
        .arg(script.as_os_str())
        .arg("--list")
        .output()
        .expect("parity_check --list should run");
    assert!(
        output.status.success(),
        "parity_check --list failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    let lines: Vec<&str> = stdout.lines().collect();
    assert!(lines.contains(&"baseline-manifests"));
    assert!(lines.contains(&"fixture-corpus-pipeline"));
    assert!(lines.contains(&"kernel-adapter-v2"));
    assert!(lines.contains(&"kernel-math"));
    assert!(lines.contains(&"kernel-topology"));
    assert!(lines.contains(&"kernel-geom"));
    assert!(lines.contains(&"kernel-primitives"));
    assert!(lines.contains(&"kernel-tessellate"));
    assert!(lines.contains(&"kernel-precision"));
    assert!(lines.contains(&"assembly-schema"));
    assert!(lines.contains(&"assembly-part-instance"));
    assert!(lines.contains(&"assembly-joint-frs"));
    assert!(lines.contains(&"assembly-joint-cb"));
    assert!(lines.contains(&"assembly-joint-limits-state"));
    assert!(lines.contains(&"assembly-fk"));
    assert!(lines.contains(&"assembly-ground-delete"));
    assert!(lines.contains(&"assembly-ui-selection-edit"));
    assert!(lines.contains(&"assembly-serialization-replay"));
    assert!(lines.contains(&"assembly-acceptance-scenes"));
    assert!(lines.contains(&"drafting-drawing-mode-ui"));
    assert!(lines.contains(&"drafting-persistence"));
    assert!(lines.contains(&"drafting-dxf-export"));
    assert!(lines.contains(&"drafting-pdf-export"));
    assert!(lines.contains(&"drafting-checkpoint"));
    assert!(lines.contains(&"step-import-entity"));
    assert!(lines.contains(&"step-export-post-boolean"));
    assert!(lines.contains(&"stl-import-export"));
    assert!(lines.contains(&"cad-mcp-tools"));
    assert!(lines.contains(&"compact-ir"));
    assert!(lines.contains(&"intent-modeling"));
    assert!(lines.contains(&"text-to-cad"));
    assert!(lines.contains(&"text-to-cad-dataset"));
    assert!(lines.contains(&"text-to-cad-training-eval"));
    assert!(lines.contains(&"headless-script-harness"));
    assert!(lines.contains(&"io-headless-ai-checkpoint"));
    assert!(lines.contains(&"viewport-camera-gizmo"));
    assert!(lines.contains(&"render-mode"));
    assert!(lines.contains(&"gpu-acceleration"));
    assert!(lines.contains(&"mesh-upload-processing"));
    assert!(lines.contains(&"direct-brep-raytrace-scaffolding"));
    assert!(lines.contains(&"analytic-ray-intersections"));
    assert!(lines.contains(&"trimmed-surface-ray-hit"));
    assert!(lines.contains(&"bvh-build-traverse"));
    assert!(lines.contains(&"raytrace-quality-mode"));
    assert!(lines.contains(&"raytrace-face-pick"));
    assert!(lines.contains(&"raytrace-ui-toggle-fallback"));
    assert!(lines.contains(&"rendering-raytrace-checkpoint"));
    assert!(lines.contains(&"physics-crate-integration"));
    assert!(lines.contains(&"collision-shape-generation"));
    assert!(lines.contains(&"convex-decomposition"));
    assert!(lines.contains(&"joint-physics-mapping"));
    assert!(lines.contains(&"simulation-step-reset-api"));
    assert!(lines.contains(&"simulation-ui-controls"));
    assert!(lines.contains(&"gym-style-api"));
    assert!(lines.contains(&"mcp-simulation-tools"));
    assert!(lines.contains(&"urdf-import"));
    assert!(lines.contains(&"urdf-export-fixtures"));
    assert!(lines.contains(&"crdt-lane-architecture"));
    assert!(lines.contains(&"collaborative-document-sync"));
    assert!(lines.contains(&"presence-cursor-selection"));
    assert!(lines.contains(&"branch-fork-workflow"));
    assert!(lines.contains(&"ecad-symbols-lane"));
    assert!(lines.contains(&"ecad-schematic-lane"));
    assert!(lines.contains(&"ecad-pcb-lane"));
    assert!(lines.contains(&"ecad-export-lane"));
    assert!(lines.contains(&"ecad-simulation-lane"));
    assert!(lines.contains(&"slicer-core-lane"));
    assert!(lines.contains(&"slicer-gcode-lane"));
    assert!(lines.contains(&"slicer-bambu-lane"));
    assert!(lines.contains(&"slicer-wasm-lane"));
    assert!(lines.contains(&"embroidery-core-lane"));
    assert!(lines.contains(&"embroidery-dst-pes-lane"));
    assert!(lines.contains(&"cam-stocksim-lane"));
    assert!(lines.contains(&"license-compliance-audit"));
    assert!(lines.contains(&"security-posture-review"));
    assert!(lines.contains(&"deterministic-replay-all-fixtures"));
    assert!(lines.contains(&"ci-artifact-manifest"));
    assert!(lines.contains(&"risk-register-workflow"));
    assert!(lines.contains(&"baseline-dashboard"));
    assert!(lines.contains(&"parity-fixture-tests"));
    assert!(lines.contains(&"rustfmt-check"));
}
