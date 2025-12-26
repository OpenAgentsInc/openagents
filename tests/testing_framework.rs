use std::fs;
use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn test_testing_docs_include_snapshot_and_coverage_commands() {
    let testing_doc = repo_root().join("docs/development/testing.md");
    let contents = fs::read_to_string(&testing_doc).expect("read testing doc");

    assert!(contents.contains("cargo insta review"));
    assert!(contents.contains("cargo llvm-cov"));
    assert!(contents.contains("snapshot diffs"));
}

#[test]
fn test_coverage_config_present() {
    let config_path = repo_root().join(".cargo/llvm-cov.toml");
    assert!(config_path.exists());
}
