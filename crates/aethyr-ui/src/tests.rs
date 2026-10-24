use super::*;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

fn crate_dir() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn repo_file(relative: &str) -> PathBuf {
    crate_dir().join("../..").join(relative)
}

fn read_json(path: &Path) -> Value {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn json_files(dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<_> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .map(|entry| entry.expect("entry").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .collect();
    files.sort();
    files
}

/// Decode, validate, and encode again; the encoding must equal the input,
/// so fixtures written by either side are already canonical.
fn round_trip(value: &Value) -> Result<ValidatedView, ViewError> {
    let validated = View::from_json(&value.to_string())?;
    let again = serde_json::to_value(validated.view()).expect("encodes");
    assert_eq!(&again, value, "round trip changed the view");
    Ok(validated)
}

#[test]
fn generated_files_are_current() {
    for (path, text) in [
        (codegen::SCHEMA_PATH, codegen::schema_text()),
        (codegen::TYPESCRIPT_PATH, codegen::typescript()),
    ] {
        let current = std::fs::read_to_string(repo_file(path)).unwrap_or_default();
        assert!(
            current == text,
            "{path} is stale; run `cargo run -p aethyr-ui -- --write`"
        );
    }
}

#[test]
fn schema_hash_matches_a_known_digest() {
    assert_eq!(
        codegen::sha256_for_test(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    assert_eq!(
        codegen::sha256_for_test(&[b'a'; 1000]),
        "41edece42d63e8d9bf515a9ba6932e1c20cbc9f5a5d134645adb5db1b9737ea3"
    );
}

#[test]
fn the_schema_lists_every_kind_and_intent() {
    let schema = codegen::schema();
    let kinds: Vec<&str> = schema["$defs"]["Element"]["oneOf"]
        .as_array()
        .expect("element variants")
        .iter()
        .map(|variant| {
            variant["properties"]["kind"]["const"]
                .as_str()
                .expect("kind tag")
        })
        .collect();
    assert_eq!(kinds, ELEMENT_KINDS);
    let ts = codegen::typescript();
    for kind in ["activate", "select", "submit", "dismiss", "input"] {
        assert!(ts.contains(&format!("\"{kind}\"")), "{kind}");
    }
    // Styles name tokens; there is no field that can hold a color.
    let style = &schema["$defs"]["Style"]["properties"];
    assert!(style.get("color").is_none() && style.get("background").is_none());
}

#[test]
fn every_kind_fixture_round_trips() {
    let mut seen = Vec::new();
    for path in json_files(&crate_dir().join("fixtures/kinds")) {
        let value = read_json(&path);
        let view = round_trip(&value).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let stem = path.file_stem().and_then(|s| s.to_str()).expect("name");
        let mut pending = vec![&view.view().root];
        let mut found = false;
        while let Some(node) = pending.pop() {
            found |= node.element.kind() == stem;
            pending.extend(node.element.children());
        }
        assert!(found, "{stem}.json has no {stem} node");
        seen.push(stem.to_string());
    }
    let mut kinds: Vec<String> = ELEMENT_KINDS.iter().map(|k| k.to_string()).collect();
    kinds.sort();
    assert_eq!(seen, kinds, "one fixture per node kind");
}

#[test]
fn controller_views_from_typescript_round_trip() {
    let files = json_files(&crate_dir().join("fixtures/ts"));
    assert!(
        files.len() >= 3,
        "run `npm test` in apps/verse-ui to write the controller fixtures"
    );
    for path in files {
        round_trip(&read_json(&path)).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    }
}

#[test]
fn invalid_fixtures_fail_with_their_code() {
    let cases = read_json(&crate_dir().join("fixtures/invalid.json"));
    for case in cases.as_array().expect("cases") {
        let name = case["name"].as_str().expect("name");
        let expected = case["error"].as_str().expect("error");
        match View::from_json(&case["view"].to_string()) {
            Ok(_) => panic!("{name}: accepted"),
            Err(error) => assert_eq!(error.code(), expected, "{name}: {error}"),
        }
    }
}

#[test]
fn intents_are_checked_against_the_current_revision() {
    let fixture = read_json(&crate_dir().join("fixtures/intents.json"));
    let view = round_trip(&fixture["view"]).expect("valid");
    for case in fixture["cases"].as_array().expect("cases") {
        let name = case["name"].as_str().expect("name");
        let intent: Intent = serde_json::from_value(case["intent"].clone()).expect(name);
        let result = view.accept(&intent).map(|node| node.id.clone());
        match case["expect"].as_str().expect("expect") {
            "ok" => assert_eq!(result, Ok(intent.node.clone()), "{name}"),
            code => assert_eq!(result.map_err(|e| e.code()), Err(code), "{name}"),
        }
    }
}

fn text(id: String) -> Value {
    json!({ "id": id, "element": { "kind": "text", "value": "x", "role": "body" } })
}

fn wide(count: usize) -> Value {
    let children: Vec<Value> = (1..count).map(|i| text(format!("n{i}"))).collect();
    json!({ "schema": SCHEMA, "instance": "limits", "revision": 1,
        "root": { "id": "root", "element": { "kind": "stack", "axis": "vertical", "children": children } } })
}

fn deep(depth: usize) -> Value {
    let mut node = text(format!("d{depth}"));
    for level in (1..depth).rev() {
        node = json!({ "id": format!("d{level}"),
            "element": { "kind": "stack", "axis": "vertical", "children": [node] } });
    }
    json!({ "schema": SCHEMA, "instance": "limits", "revision": 1, "root": node })
}

#[test]
fn node_and_depth_limits() {
    assert!(View::from_json(&wide(MAX_NODES).to_string()).is_ok());
    assert_eq!(
        View::from_json(&wide(MAX_NODES + 1).to_string()).map(|_| ()),
        Err(ViewError::NodeLimit)
    );
    assert!(View::from_json(&deep(MAX_DEPTH).to_string()).is_ok());
    assert_eq!(
        View::from_json(&deep(MAX_DEPTH + 1).to_string()).map(|_| ()),
        Err(ViewError::DepthLimit)
    );
    let huge = "x".repeat(MAX_VIEW_BYTES + 1);
    assert_eq!(
        View::from_json(&huge).map(|_| ()),
        Err(ViewError::ViewLimit)
    );
    let long = json!({ "schema": SCHEMA, "instance": "limits", "revision": 1,
        "root": { "id": "t", "element": { "kind": "text", "value": "x".repeat(MAX_TEXT_BYTES + 1), "role": "body" } } });
    assert_eq!(
        View::from_json(&long.to_string())
            .map_err(|e| e.code())
            .map(|_| ()),
        Err("text_limit")
    );
}

#[test]
fn builders_produce_the_canonical_encoding() {
    let view = View::new(
        "entry",
        3,
        Node::new(
            "go",
            Element::Button {
                label: "Continue as guest".into(),
                enabled: true,
                emphasis: Emphasis::Primary,
                href: None,
                leading: None,
                trailing: Some(Glyph::Forward),
            },
        ),
    );
    assert_eq!(
        serde_json::to_value(&view).expect("encodes"),
        json!({ "schema": "aethyr.ui.v1", "instance": "entry", "revision": 3, "root": {
            "id": "go", "element": { "kind": "button", "label": "Continue as guest",
                "emphasis": "primary", "trailing": "forward" } } })
    );
    let validated = view.validate().expect("valid");
    let intent = Intent {
        instance: "entry".into(),
        revision: 3,
        node: "go".into(),
        kind: IntentKind::Activate,
        value: None,
    };
    assert!(validated.accept(&intent).is_ok());
    let secret = Intent {
        value: Some("hunter2".into()),
        kind: IntentKind::Input,
        ..intent
    };
    assert!(!format!("{secret:?}").contains("hunter2"));
}
