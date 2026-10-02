// The checked-in fixture tree covers the pinned contract: one spec fixture per
// operation, and redacted captures that name a known operation and source.
mod support;

use std::collections::BTreeSet;

fn inventory() -> Vec<serde_json::Value> {
    serde_json::from_str(boat::OPERATIONS).expect("inventory")
}

#[test]
fn every_operation_has_a_spec_fixture_from_the_pinned_spec() {
    let ops = inventory();
    assert_eq!(ops.len(), 69);
    for op in &ops {
        let id = op["operation_id"].as_str().expect("operation id");
        let fixture = support::fixture(id);
        assert_eq!(fixture["operation_id"], op["operation_id"]);
        assert_eq!(fixture["method"], op["method"], "{id}");
        assert_eq!(fixture["path"], op["path"], "{id}");
        assert_eq!(fixture["sdk_method"], op["rust_method"], "{id}");
        assert_eq!(fixture["sdk_response_type"], op["response"], "{id}");
        assert_eq!(
            fixture["source"]["spec_sha256"].as_str(),
            Some(boat::SPEC_SHA256),
            "{id} was written from a different specification; run schema/fixtures.py"
        );
        assert!(fixture["source"]["sdk_commit"].is_string(), "{id}");
        assert!(fixture["request"]["target"].is_string(), "{id}");
        let success = &fixture["success"];
        assert!(
            !success["schema_sample"].is_null()
                || success["binary"].as_bool() == Some(true)
                || success["published_examples"]
                    .as_object()
                    .is_some_and(|m| !m.is_empty()),
            "{id} needs a success body"
        );
    }
    let index = support::read_fixture("spec/index.json");
    assert_eq!(index["operations"].as_array().map(Vec::len), Some(69));
}

#[test]
fn observed_captures_are_ported_named_and_redacted() {
    let known: BTreeSet<String> = inventory()
        .iter()
        .map(|op| op["operation_id"].as_str().expect("id").to_owned())
        .collect();
    let captures = support::observed();
    assert_eq!(captures.len(), 96, "the coder-box capture set");
    let mut covered = BTreeSet::new();
    for (name, capture) in &captures {
        let id = capture["operation_id"].as_str().expect("operation id");
        assert!(known.contains(id), "{name}: {id} is not in the inventory");
        assert!(capture["status"].is_number(), "{name}");
        assert!(capture["case"].is_string(), "{name}");
        assert!(capture["captured_at"].is_string(), "{name}");
        assert!(capture["source"]["sdk_commit"].is_string(), "{name}");
        assert!(capture["redactions"].is_array(), "{name}");
        assert!(capture["ported"]["from"].is_string(), "{name}");
        let path = capture["path"].as_str().expect("path");
        assert!(
            !path.contains("/boxes") && !path.contains("{boxId}"),
            "{name}"
        );
        let text = capture["body"].to_string();
        for old in [
            "\"box\":",
            "\"boxes\":",
            "\"boxId\":",
            "\"type\":\"box.",
            "activeBoxes",
        ] {
            assert!(!text.contains(old), "{name} keeps the Box name {old}");
        }
        if (200..300).contains(&capture["status"].as_u64().unwrap_or(0)) {
            covered.insert(id.to_owned());
        }
    }
    assert!(covered.len() >= 30, "success captures cover {covered:?}");
}

#[test]
fn no_fixture_holds_a_key_or_request_secret() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures");
    let mut stack = vec![dir];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).expect("directory") {
            let path = entry.expect("entry").path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let text = std::fs::read_to_string(&path).expect("text");
            for word in text.split(|c: char| !c.is_ascii_alphanumeric() && c != '_') {
                // Real keys are long; the one fake key in a 401 capture is short.
                assert!(
                    !(word.starts_with("boat_") || word.starts_with("box_live")) || word.len() < 24,
                    "{} holds what looks like a key",
                    path.display()
                );
            }
        }
    }
}
