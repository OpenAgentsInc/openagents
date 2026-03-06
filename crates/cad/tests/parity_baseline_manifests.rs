#![allow(clippy::all, clippy::expect_used, clippy::panic, clippy::pedantic, clippy::print_stderr, clippy::print_stdout, clippy::unwrap_used)]

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Deserialize;
use sha2::{Digest, Sha256};

const ISSUE_ID: &str = "VCAD-PARITY-001";
const VCAD_COMMIT: &str = "1b59e7948efcdb848d8dba6848785d57aa310e81";
const OPENAGENTS_COMMIT: &str = "04faa5227f077c419f1c5c52ddebbb7552838fd4";

const VCAD_SOURCE_PATHS: &[&str] = &[
    "README.md",
    "docs/features/index.md",
    "docs/features/ROADMAP.md",
    "docs/features/sketch-mode.md",
    "docs/features/sketch-operations.md",
    "docs/features/boolean-operations.md",
    "docs/features/import-export.md",
    "docs/features/assembly-joints.md",
    "docs/features/drafting-2d.md",
    "docs/features/headless-api.md",
    "docs/features/ray-tracing.md",
    "docs/features/physics-simulation.md",
];

const OPENAGENTS_SOURCE_PATHS: &[&str] = &[
    "crates/cad/docs/PLAN.md",
    "crates/cad/docs/decisions/0001-kernel-strategy.md",
    "crates/cad/docs/CAD_FEATURE_OPS.md",
    "crates/cad/docs/CAD_SKETCH_CONSTRAINTS.md",
    "crates/cad/docs/CAD_SKETCH_FEATURE_OPS.md",
    "crates/cad/docs/CAD_STEP_IMPORT.md",
    "crates/cad/docs/CAD_STEP_EXPORT.md",
];

#[derive(Debug, Deserialize)]
struct ParityBaselineManifest {
    manifest_version: u64,
    issue_id: String,
    baseline_kind: String,
    repository: String,
    repository_commit: String,
    source_documents: Vec<ParitySourceDocument>,
}

#[derive(Debug, Deserialize)]
struct ParitySourceDocument {
    path: String,
    sha256: String,
    bytes: usize,
}

fn repo_root() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(Path::parent)
        .expect("crate should be in <repo>/crates/cad")
        .to_path_buf()
}

fn load_manifest(name: &str) -> ParityBaselineManifest {
    let path = repo_root().join("crates/cad/parity").join(name);
    let raw = fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed reading manifest {}: {error}", path.display()));
    serde_json::from_str(&raw)
        .unwrap_or_else(|error| panic!("failed parsing manifest {}: {error}", path.display()))
}

fn git_show_blob(repo: &Path, commit: &str, path: &str) -> Vec<u8> {
    let selector = format!("{commit}:{path}");
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .arg("show")
        .arg(&selector)
        .output()
        .unwrap_or_else(|error| {
            panic!(
                "failed running git show for {} (repo {}): {error}",
                selector,
                repo.display()
            )
        });
    if !output.status.success() {
        panic!(
            "git show failed for {} (repo {}): {}",
            selector,
            repo.display(),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    output.stdout
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

fn assert_manifest_shape(
    manifest: &ParityBaselineManifest,
    baseline_kind: &str,
    repository: &str,
    commit: &str,
    expected_paths: &[&str],
) {
    assert_eq!(manifest.manifest_version, 1);
    assert_eq!(manifest.issue_id, ISSUE_ID);
    assert_eq!(manifest.baseline_kind, baseline_kind);
    assert_eq!(manifest.repository, repository);
    assert_eq!(manifest.repository_commit, commit);
    assert_eq!(manifest.source_documents.len(), expected_paths.len());

    for (entry, expected_path) in manifest.source_documents.iter().zip(expected_paths) {
        assert_eq!(entry.path, *expected_path);
        assert!(
            entry.bytes > 0,
            "bytes should be non-zero for {}",
            entry.path
        );
        assert_eq!(entry.sha256.len(), 64, "sha256 length should be 64 chars");
        assert!(
            entry.sha256.chars().all(|char| char.is_ascii_hexdigit()),
            "sha256 should be hex for {}",
            entry.path
        );
        assert_eq!(
            entry.sha256,
            entry.sha256.to_ascii_lowercase(),
            "sha256 should be lowercase for {}",
            entry.path
        );
    }
}

#[test]
fn parity_baseline_manifests_pin_expected_scope() {
    let vcad = load_manifest("vcad_reference_manifest.json");
    let openagents = load_manifest("openagents_start_manifest.json");

    assert_manifest_shape(
        &vcad,
        "vcad_reference",
        "vcad",
        VCAD_COMMIT,
        VCAD_SOURCE_PATHS,
    );
    assert_manifest_shape(
        &openagents,
        "openagents_starting_point",
        "openagents",
        OPENAGENTS_COMMIT,
        OPENAGENTS_SOURCE_PATHS,
    );
}

#[test]
fn parity_baseline_openagents_manifest_hashes_match_pinned_commit() {
    let root = repo_root();
    let manifest = load_manifest("openagents_start_manifest.json");
    for entry in manifest.source_documents {
        let blob = git_show_blob(&root, OPENAGENTS_COMMIT, &entry.path);
        assert_eq!(
            blob.len(),
            entry.bytes,
            "byte count mismatch: {}",
            entry.path
        );
        let actual_sha = sha256_hex(&blob);
        assert_eq!(actual_sha, entry.sha256, "sha256 mismatch: {}", entry.path);
    }
}

#[test]
fn parity_baseline_vcad_manifest_hashes_match_pinned_commit_when_repo_available() {
    let vcad_repo = env::var("VCAD_REPO")
        .map(PathBuf::from)
        .unwrap_or_else(|_| match env::var("HOME") {
            Ok(home) => PathBuf::from(home).join("code/vcad"),
            Err(_) => PathBuf::from("/home/christopherdavid/code/vcad"),
        });

    if !vcad_repo.exists() {
        eprintln!(
            "skipping vcad parity hash verification because repo is missing: {}",
            vcad_repo.display()
        );
        return;
    }

    let manifest = load_manifest("vcad_reference_manifest.json");
    for entry in manifest.source_documents {
        let blob = git_show_blob(&vcad_repo, VCAD_COMMIT, &entry.path);
        assert_eq!(
            blob.len(),
            entry.bytes,
            "byte count mismatch: {}",
            entry.path
        );
        let actual_sha = sha256_hex(&blob);
        assert_eq!(actual_sha, entry.sha256, "sha256 mismatch: {}", entry.path);
    }
}
