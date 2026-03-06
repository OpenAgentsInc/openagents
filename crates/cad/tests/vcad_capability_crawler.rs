#![allow(clippy::all, clippy::expect_used, clippy::panic, clippy::pedantic, clippy::print_stderr, clippy::print_stdout, clippy::unwrap_used)]

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use openagents_cad::parity::vcad_crawler::{
    VCAD_PARITY_ISSUE_ID, VCAD_PARITY_PINNED_COMMIT, VcadCapabilityInventory,
    crawl_vcad_capabilities,
};

fn repo_root() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(Path::parent)
        .expect("crate should be in <repo>/crates/cad")
        .to_path_buf()
}

fn fixture_path() -> PathBuf {
    repo_root()
        .join("crates/cad/parity")
        .join("vcad_capabilities_inventory.json")
}

fn load_fixture() -> VcadCapabilityInventory {
    let path = fixture_path();
    let raw = fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed reading fixture {}: {error}", path.display()));
    serde_json::from_str(&raw)
        .unwrap_or_else(|error| panic!("failed parsing fixture {}: {error}", path.display()))
}

fn default_vcad_repo() -> PathBuf {
    if let Ok(path) = env::var("VCAD_REPO") {
        return PathBuf::from(path);
    }
    if let Ok(home) = env::var("HOME") {
        return PathBuf::from(home).join("code/vcad");
    }
    PathBuf::from("/home/christopherdavid/code/vcad")
}

#[test]
fn vcad_capability_inventory_fixture_is_well_formed() {
    let fixture = load_fixture();
    assert_eq!(fixture.manifest_version, 1);
    assert_eq!(fixture.issue_id, VCAD_PARITY_ISSUE_ID);
    assert_eq!(fixture.vcad_commit, VCAD_PARITY_PINNED_COMMIT);
    assert!(fixture.docs.len() > 10, "expected docs capabilities");
    assert!(fixture.crates.len() > 10, "expected crate capabilities");
    assert!(
        fixture.commands.len() >= 10,
        "expected at least core CLI commands"
    );
    assert_eq!(fixture.summary.docs_capability_count, fixture.docs.len());
    assert_eq!(fixture.summary.crate_count, fixture.crates.len());
    assert_eq!(fixture.summary.command_count, fixture.commands.len());
    assert!(
        fixture
            .commands
            .iter()
            .any(|command| command.cli_command == "export")
    );
    assert!(
        fixture
            .commands
            .iter()
            .any(|command| command.cli_command == "import")
    );
    assert!(
        fixture
            .commands
            .iter()
            .any(|command| command.cli_command == "info")
    );
}

#[test]
fn vcad_capability_inventory_fixture_matches_live_repo_when_available() {
    let vcad_repo = default_vcad_repo();
    if !vcad_repo.exists() {
        eprintln!(
            "skipping vcad capability crawl fixture parity check: missing repo {}",
            vcad_repo.display()
        );
        return;
    }

    let crawled = crawl_vcad_capabilities(&vcad_repo, VCAD_PARITY_PINNED_COMMIT)
        .expect("live crawl should succeed");
    let crawled_json = format!(
        "{}\n",
        serde_json::to_string_pretty(&crawled).expect("serialize crawled inventory")
    );
    let fixture_json = fs::read_to_string(fixture_path()).expect("read fixture JSON");
    assert_eq!(crawled_json, fixture_json);
}
