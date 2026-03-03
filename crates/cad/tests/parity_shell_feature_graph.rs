use std::fs;
use std::path::{Path, PathBuf};

use openagents_cad::parity::scorecard::ParityScorecard;
use openagents_cad::parity::shell_feature_graph_parity::{
    PARITY_SHELL_FEATURE_GRAPH_ISSUE_ID, ShellFeatureGraphParityManifest,
    build_shell_feature_graph_parity_manifest,
};

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
fn shell_feature_graph_manifest_fixture_is_well_formed() {
    let path = parity_dir().join("shell_feature_graph_parity_manifest.json");
    let manifest: ShellFeatureGraphParityManifest = load_json(&path);
    assert_eq!(manifest.manifest_version, 1);
    assert_eq!(manifest.issue_id, PARITY_SHELL_FEATURE_GRAPH_ISSUE_ID);
    assert_eq!(manifest.shell_node_snapshot.operation_key, "shell.v1");
    assert_eq!(manifest.graph_topo_order[0], "feature.base");
    assert!(manifest.empty_remove_faces_supported);
    assert!(manifest.remove_face_selection_affects_hash);
}

#[test]
fn shell_feature_graph_manifest_fixture_matches_generation() {
    let parity = parity_dir();
    let scorecard_path = parity.join("parity_scorecard.json");
    let fixture_path = parity.join("shell_feature_graph_parity_manifest.json");
    let scorecard: ParityScorecard = load_json(&scorecard_path);
    let generated =
        build_shell_feature_graph_parity_manifest(&scorecard, &scorecard_path.to_string_lossy());
    let generated_json = format!(
        "{}\n",
        serde_json::to_string_pretty(&generated).expect("serialize generated manifest")
    );
    let fixture_json = fs::read_to_string(fixture_path).expect("read shell feature graph fixture");
    assert_eq!(generated_json, fixture_json);
}
