#![allow(clippy::all, clippy::expect_used, clippy::panic, clippy::pedantic, clippy::print_stderr, clippy::print_stdout, clippy::unwrap_used)]

use std::fs;
use std::path::{Path, PathBuf};

use openagents_cad::parity::kernel_precision_parity::{
    KernelPrecisionParityManifest, PARITY_KERNEL_PRECISION_ISSUE_ID,
    build_kernel_precision_parity_manifest,
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
fn kernel_precision_manifest_fixture_is_well_formed() {
    let path = parity_dir().join("kernel_precision_parity_manifest.json");
    let manifest: KernelPrecisionParityManifest = load_json(&path);
    assert_eq!(manifest.manifest_version, 1);
    assert_eq!(manifest.issue_id, PARITY_KERNEL_PRECISION_ISSUE_ID);
    assert_eq!(manifest.tolerance_policy.linear_tolerance_mm, 1e-6);
    assert_eq!(manifest.tolerance_policy.angular_tolerance_rad, 1e-9);
    assert_eq!(
        manifest.predicate_samples.orient2d_near_collinear,
        "Positive"
    );
    assert_eq!(
        manifest.predicate_samples.orient3d_near_coplanar,
        "Negative"
    );
    assert!(manifest.predicate_samples.point_on_segment);
    assert!(manifest.predicate_samples.point_on_plane);
}

#[test]
fn kernel_precision_manifest_fixture_matches_generation() {
    let parity = parity_dir();
    let scorecard_path = parity.join("parity_scorecard.json");
    let fixture_path = parity.join("kernel_precision_parity_manifest.json");
    let scorecard: ParityScorecard = load_json(&scorecard_path);
    let generated =
        build_kernel_precision_parity_manifest(&scorecard, &scorecard_path.to_string_lossy());
    let generated_json = format!(
        "{}\n",
        serde_json::to_string_pretty(&generated).expect("serialize generated manifest")
    );
    let fixture_json = fs::read_to_string(fixture_path).expect("read kernel precision fixture");
    assert_eq!(generated_json, fixture_json);
}
