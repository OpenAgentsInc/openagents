#![allow(clippy::all, clippy::expect_used, clippy::panic, clippy::pedantic, clippy::print_stderr, clippy::print_stdout, clippy::unwrap_used)]

use std::fs;
use std::path::{Path, PathBuf};

use openagents_cad::parity::kernel_primitives_parity::{
    KernelPrimitivesParityManifest, PARITY_KERNEL_PRIMITIVES_ISSUE_ID,
    build_kernel_primitives_parity_manifest,
};
use openagents_cad::parity::scorecard::ParityScorecard;

fn repo_root() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(Path::parent)
        .expect("crate should be in <repo>/crates/cad")
        .to_path_buf()
}

fn parity_dir() -> PathBuf {
    repo_root().join("crates/cad/parity")
}

fn load_json<T: serde::de::DeserializeOwned>(path: &Path) -> T {
    let raw = fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("failed reading {}: {error}", path.display()));
    serde_json::from_str(&raw)
        .unwrap_or_else(|error| panic!("failed parsing {}: {error}", path.display()))
}

#[test]
fn kernel_primitives_manifest_fixture_is_well_formed() {
    let path = parity_dir().join("kernel_primitives_parity_manifest.json");
    let manifest: KernelPrimitivesParityManifest = load_json(&path);
    assert_eq!(manifest.manifest_version, 1);
    assert_eq!(manifest.issue_id, PARITY_KERNEL_PRIMITIVES_ISSUE_ID);
    assert_eq!(manifest.sample_counts.cube.topology.face_count, 6);
    assert_eq!(manifest.sample_counts.cylinder.topology.edge_count, 3);
    assert_eq!(manifest.sample_counts.sphere.topology.face_count, 1);
    assert!(manifest.cone_equal_radii_routes_to_cylinder);
}

#[test]
fn kernel_primitives_manifest_fixture_matches_generation() {
    let parity = parity_dir();
    let scorecard_path = parity.join("parity_scorecard.json");
    let fixture_path = parity.join("kernel_primitives_parity_manifest.json");
    let scorecard: ParityScorecard = load_json(&scorecard_path);
    let generated =
        build_kernel_primitives_parity_manifest(&scorecard, &scorecard_path.to_string_lossy());
    let generated_json = format!(
        "{}\n",
        serde_json::to_string_pretty(&generated).expect("serialize generated manifest")
    );
    let fixture_json = fs::read_to_string(fixture_path).expect("read kernel primitives fixture");
    assert_eq!(generated_json, fixture_json);
}
