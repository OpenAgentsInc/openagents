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

use openagents_cad::parity::bvh_build_traverse_parity::{
    BvhBuildTraverseParityManifest, PARITY_BVH_BUILD_TRAVERSE_ISSUE_ID,
    build_bvh_build_traverse_parity_manifest,
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
fn bvh_build_traverse_manifest_fixture_is_well_formed() {
    let path = parity_dir().join("bvh_build_traverse_parity_manifest.json");
    let manifest: BvhBuildTraverseParityManifest = load_json(&path);
    assert_eq!(manifest.manifest_version, 1);
    assert_eq!(manifest.issue_id, PARITY_BVH_BUILD_TRAVERSE_ISSUE_ID);
    assert!(manifest.reference_commit_match);
    assert!(manifest.sah_constants_match);
    assert!(manifest.leaf_partition_match);
    assert!(manifest.trace_ordering_match);
    assert!(manifest.closest_hit_match);
    assert!(manifest.flatten_contract_match);
    assert!(manifest.deterministic_replay_match);
    assert_eq!(manifest.primary_leaf_sizes, vec![4, 4]);
    assert_eq!(manifest.fallback_leaf_sizes, vec![3, 3]);
    assert_eq!(
        manifest.trace_t_values,
        vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]
    );
    assert_eq!(manifest.closest_t, Some(1.0));
    assert!(manifest.fallback_triggered);
}

#[test]
fn bvh_build_traverse_manifest_fixture_matches_generation() {
    let parity = parity_dir();
    let scorecard_path = parity.join("parity_scorecard.json");
    let fixture_path = parity.join("bvh_build_traverse_parity_manifest.json");
    let scorecard: ParityScorecard = load_json(&scorecard_path);
    let generated =
        build_bvh_build_traverse_parity_manifest(&scorecard, &scorecard_path.to_string_lossy())
            .expect("build bvh build/traverse parity manifest");
    let generated_json = format!(
        "{}\n",
        serde_json::to_string_pretty(&generated)
            .expect("serialize generated bvh build/traverse parity manifest")
    );
    let fixture_json = fs::read_to_string(fixture_path).expect("read bvh build/traverse fixture");
    assert_eq!(generated_json, fixture_json);
}
