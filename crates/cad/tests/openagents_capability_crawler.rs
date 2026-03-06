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

use openagents_cad::parity::openagents_crawler::{
    OPENAGENTS_PARITY_ISSUE_ID, OPENAGENTS_PARITY_PINNED_COMMIT, OpenagentsCapabilityInventory,
    crawl_openagents_capabilities,
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
        .join("openagents_capabilities_inventory.json")
}

fn load_fixture() -> OpenagentsCapabilityInventory {
    let path = fixture_path();
    let raw = fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed reading fixture {}: {error}", path.display()));
    serde_json::from_str(&raw)
        .unwrap_or_else(|error| panic!("failed parsing fixture {}: {error}", path.display()))
}

#[test]
fn openagents_capability_inventory_fixture_is_well_formed() {
    let fixture = load_fixture();
    assert_eq!(fixture.manifest_version, 1);
    assert_eq!(fixture.issue_id, OPENAGENTS_PARITY_ISSUE_ID);
    assert_eq!(fixture.openagents_commit, OPENAGENTS_PARITY_PINNED_COMMIT);
    assert!(
        fixture.docs.len() > 20,
        "expected docs capability inventory"
    );
    assert!(
        fixture.crates.len() > 10,
        "expected CAD crate/module inventory"
    );
    assert!(
        fixture.commands.len() >= 9,
        "expected CAD intent/tool commands"
    );
    assert_eq!(fixture.summary.docs_capability_count, fixture.docs.len());
    assert_eq!(fixture.summary.crate_count, fixture.crates.len());
    assert_eq!(fixture.summary.command_count, fixture.commands.len());
}

#[test]
fn openagents_capability_inventory_fixture_matches_live_crawl() {
    let root = repo_root();
    let crawled = crawl_openagents_capabilities(&root, OPENAGENTS_PARITY_PINNED_COMMIT)
        .expect("openagents crawl should succeed");
    let crawled_json = format!(
        "{}\n",
        serde_json::to_string_pretty(&crawled).expect("serialize crawled inventory")
    );
    let fixture_json = fs::read_to_string(fixture_path()).expect("read fixture JSON");
    assert_eq!(crawled_json, fixture_json);
}
