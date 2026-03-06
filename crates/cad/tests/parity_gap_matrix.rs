#![allow(
    clippy::all,
    clippy::expect_used,
    clippy::panic,
    clippy::pedantic,
    clippy::print_stderr,
    clippy::print_stdout,
    clippy::unwrap_used
)]

use std::fs;
use std::path::{Path, PathBuf};

use openagents_cad::parity::gap_matrix::{
    PARITY_GAP_MATRIX_ISSUE_ID, ParityGapMatrix, build_gap_matrix,
};
use openagents_cad::parity::openagents_crawler::OpenagentsCapabilityInventory;
use openagents_cad::parity::vcad_crawler::VcadCapabilityInventory;

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
fn parity_gap_matrix_fixture_is_well_formed() {
    let matrix_path = parity_dir().join("vcad_openagents_gap_matrix.json");
    let matrix: ParityGapMatrix = load_json(&matrix_path);
    assert_eq!(matrix.manifest_version, 1);
    assert_eq!(matrix.issue_id, PARITY_GAP_MATRIX_ISSUE_ID);
    assert!(matrix.docs.reference_count > 0);
    assert!(matrix.crates.reference_count > 0);
    assert!(matrix.commands.reference_count > 0);
    assert_eq!(
        matrix.summary.total_reference_count,
        matrix.docs.reference_count
            + matrix.crates.reference_count
            + matrix.commands.reference_count
    );
}

#[test]
fn parity_gap_matrix_fixture_matches_generation() {
    let dir = parity_dir();
    let vcad_path = dir.join("vcad_capabilities_inventory.json");
    let openagents_path = dir.join("openagents_capabilities_inventory.json");
    let matrix_path = dir.join("vcad_openagents_gap_matrix.json");

    let vcad: VcadCapabilityInventory = load_json(&vcad_path);
    let openagents: OpenagentsCapabilityInventory = load_json(&openagents_path);
    let generated = build_gap_matrix(
        &vcad,
        &openagents,
        &vcad_path.to_string_lossy(),
        &openagents_path.to_string_lossy(),
    );
    let generated_json = format!(
        "{}\n",
        serde_json::to_string_pretty(&generated).expect("serialize generated gap matrix")
    );
    let fixture_json = fs::read_to_string(matrix_path).expect("read gap matrix fixture");
    assert_eq!(generated_json, fixture_json);
}
