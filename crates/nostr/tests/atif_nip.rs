//! NIP-ATIF fixture checks (`nips/openagents/NIP-ATIF.md`).
//!
//! The fixture under `tests/fixtures/atif` is a parent trajectory split into
//! two chunks and a delegated child in one. These tests check the bodies
//! against their schemas and the byte rules the NIP states: chunks
//! concatenate to the exact log bytes, every digest and size matches, log
//! chunks end on a record boundary, a chunk fits one NIP-44 plaintext, and
//! the parent binds the child the child names. `crates/atif`
//! (`nip_atif_fixture`) checks the `steps_digest` values against the ATIF
//! renderer.

use std::collections::BTreeMap;
use std::path::PathBuf;

use nostr::contracts::{digest_bytes, jcs, prepare_closure, validate_instance};
use nostr::kinds;
use serde_json::Value;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn fixture(name: &str) -> Vec<u8> {
    let path = root().join("tests/fixtures/atif").join(name);
    std::fs::read(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

fn json(name: &str) -> Value {
    serde_json::from_slice(&fixture(name)).expect("fixture JSON")
}

fn schema_check(schema: &str, instance: &Value) {
    let path = root().join("../../nips/openagents/schemas").join(schema);
    let bytes = std::fs::read(&path).expect("schema");
    let digest = digest_bytes(&bytes);
    let mut documents = BTreeMap::new();
    documents.insert(digest.clone(), bytes);
    let closure = prepare_closure(&documents).expect("a supported schema");
    validate_instance(&closure, &digest, instance)
        .unwrap_or_else(|e| panic!("{schema} refuses the fixture: {e:?}"));
}

/// Checks one trajectory's manifest and chunks; returns the manifest.
fn trajectory(name: &str) -> Value {
    let log = fixture(&format!("{name}.atif.jsonl"));
    let manifest = json(&format!("{name}.manifest.json"));
    schema_check("atif-manifest.v1.json", &manifest);

    assert_eq!(manifest["artifact"]["digest"], digest_bytes(&log));
    assert_eq!(manifest["artifact"]["size"], log.len());
    assert_eq!(manifest["form"], "log");

    let listed = manifest["chunks"].as_array().expect("chunks");
    let mut joined = Vec::new();
    for (index, entry) in listed.iter().enumerate() {
        let chunk = json(&format!("{name}.chunk-{index}.json"));
        schema_check("atif-chunk.v1.json", &chunk);
        let text = chunk["text"].as_str().expect("text").as_bytes();
        assert_eq!(chunk["index"], index);
        assert_eq!(entry["index"], index);
        assert_eq!(chunk["count"], listed.len());
        assert_eq!(chunk["trajectory_id"], manifest["trajectory_id"]);
        assert_eq!(chunk["artifact"], manifest["artifact"]["digest"]);
        assert_eq!(chunk["digest"], digest_bytes(text));
        assert_eq!(entry["digest"], chunk["digest"]);
        assert_eq!(entry["size"], text.len());
        assert!(text.len() <= 32_768, "a chunk's text is at most 32 KiB");
        assert!(
            text.ends_with(b"\n"),
            "a log chunk ends on a record boundary"
        );
        let body = jcs(&chunk).expect("canonical chunk body");
        assert!(
            body.len() <= 65_535,
            "a chunk body fits one NIP-44 plaintext"
        );
        joined.extend_from_slice(text);
    }
    assert_eq!(joined, log, "chunks concatenate to the exact log bytes");
    manifest
}

#[test]
fn the_fixture_manifests_and_chunks_match_their_bytes_and_schemas() {
    trajectory("parent");
    trajectory("child");
}

#[test]
fn the_parent_binds_the_child_the_child_names() {
    let parent = trajectory("parent");
    let child = trajectory("child");
    let bound = &parent["children"][0];
    assert_eq!(bound["trajectory_id"], child["trajectory_id"]);
    assert_eq!(bound["steps_digest"], child["steps_digest"]);
    assert_eq!(bound["artifact"], child["artifact"]["digest"]);
    assert_eq!(child["parent"]["trajectory_id"], parent["trajectory_id"]);
    assert_eq!(child["parent"]["step_id"], bound["step_id"]);
    assert!(
        bound["step_id"].as_u64().expect("step") <= parent["step_count"].as_u64().expect("count")
    );
}

#[test]
fn a_tampered_chunk_no_longer_matches_its_manifest() {
    let manifest = json("parent.manifest.json");
    let mut chunk = json("parent.chunk-1.json");
    let text = chunk["text"]
        .as_str()
        .expect("text")
        .replace("typo", "tipo");
    chunk["text"] = Value::String(text.clone());
    schema_check("atif-chunk.v1.json", &chunk);
    assert_ne!(
        manifest["chunks"][1]["digest"],
        digest_bytes(text.as_bytes())
    );
}

#[test]
fn the_nip_atif_kinds_are_registered() {
    assert_eq!(kinds::ATIF_DECLARATION, 3_198);
    assert_eq!(kinds::ATIF_CHUNK, 3_199);
    assert_eq!(kinds::claim_of(3_198).map(|c| c.owner), Some("NIP-ATIF"));
    assert_eq!(kinds::claim_of(3_199).map(|c| c.owner), Some("NIP-ATIF"));
}
