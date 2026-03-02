use std::fs;
use std::path::{Path, PathBuf};

use openagents_cad::parity::fixture_corpus::{
    PARITY_FIXTURE_CORPUS_ISSUE_ID, ParityFixtureCorpus, build_fixture_corpus,
};
use openagents_cad::parity::gap_matrix::ParityGapMatrix;

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
fn parity_fixture_corpus_fixture_is_well_formed() {
    let path = parity_dir()
        .join("fixtures")
        .join("parity_fixture_corpus.json");
    let corpus: ParityFixtureCorpus = load_json(&path);
    assert_eq!(corpus.manifest_version, 1);
    assert_eq!(corpus.issue_id, PARITY_FIXTURE_CORPUS_ISSUE_ID);
    assert!(!corpus.fixtures.is_empty(), "expected seeded fixtures");
    assert_eq!(corpus.seed_policy.matched_per_surface, 3);
    assert_eq!(corpus.seed_policy.missing_per_surface, 8);
    assert!(corpus.summary.total_seed_count >= 9);
}

#[test]
fn parity_fixture_corpus_fixture_matches_generation() {
    let dir = parity_dir();
    let matrix_path = dir.join("vcad_openagents_gap_matrix.json");
    let corpus_path = dir.join("fixtures").join("parity_fixture_corpus.json");
    let matrix: ParityGapMatrix = load_json(&matrix_path);
    let generated = build_fixture_corpus(&matrix, &matrix_path.to_string_lossy());
    let generated_json = format!(
        "{}\n",
        serde_json::to_string_pretty(&generated).expect("serialize generated fixture corpus")
    );
    let fixture_json = fs::read_to_string(corpus_path).expect("read fixture corpus");
    assert_eq!(generated_json, fixture_json);
}
