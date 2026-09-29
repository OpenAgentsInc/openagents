//! The NIP-ATIF fixture (`crates/nostr/tests/fixtures/atif`) names each
//! trajectory's `steps_digest`, `step_count`, and `state`. This reads the
//! fixture logs as evidence and checks those values against what this crate
//! renders, so the NIP's worked example can't drift from the format.

use std::path::PathBuf;

use serde_json::Value;

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../nostr/tests/fixtures/atif")
}

fn manifest(name: &str) -> Value {
    let text = std::fs::read_to_string(fixtures().join(name)).expect("manifest fixture");
    serde_json::from_str(&text).expect("manifest JSON")
}

fn check(log: &str, manifest_file: &str) {
    let recording = atif::log::read_whole(&fixtures().join(log)).expect("a whole log");
    let document = recording.document();
    let steps = document["steps"].clone();
    let manifest = manifest(manifest_file);
    assert_eq!(
        manifest["steps_digest"],
        atif::digest(&steps),
        "{log} steps_digest"
    );
    assert_eq!(
        manifest["step_count"],
        steps.as_array().expect("steps").len(),
        "{log} step_count"
    );
    assert_eq!(manifest["state"], document["extra"]["state"], "{log} state");
    assert_eq!(
        manifest["trajectory_id"], document["trajectory_id"],
        "{log} trajectory_id"
    );
    assert_eq!(
        manifest["schema_version"], document["schema_version"],
        "{log} schema_version"
    );
}

#[test]
fn the_nip_atif_fixture_names_what_the_logs_render() {
    check("parent.atif.jsonl", "parent.manifest.json");
    check("child.atif.jsonl", "child.manifest.json");
}

#[test]
fn a_steps_digest_survives_a_second_render() {
    let recording =
        atif::log::read_whole(&fixtures().join("parent.atif.jsonl")).expect("a whole log");
    let one = recording.document();
    let two = recording.document();
    assert_eq!(atif::digest(&one["steps"]), atif::digest(&two["steps"]));
}

/// The fixture is written at the current version. The same log as it was
/// recorded before v1.8 still reads whole and renders the same steps, so a
/// `steps_digest` pinned over a v1.7 recording stays valid.
#[test]
fn the_fixture_as_recorded_at_v1_7_renders_the_same_steps() {
    let path = fixtures().join("parent.atif.jsonl");
    let current = std::fs::read_to_string(&path).expect("fixture log");
    assert!(current.contains(&format!("\"schema_version\":\"{}\"", atif::SCHEMA_VERSION)));
    let recorded = current.replace("\"ATIF-v1.8\"", "\"ATIF-v1.7\"");
    let old = atif::log::read_bytes(&path, recorded.as_bytes()).expect("a v1.7 log");
    assert!(old.whole());
    assert_eq!(old.schema_version, "ATIF-v1.7");
    let new = atif::log::read_whole(&path).expect("a whole log");
    assert_eq!(
        atif::digest(&old.document()["steps"]),
        atif::digest(&new.document()["steps"])
    );
    assert_eq!(
        manifest("parent.manifest.json")["schema_version"],
        "ATIF-v1.8"
    );
}
