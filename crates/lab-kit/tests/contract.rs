//! The contract against real shapes: the existing Labs' records, the card
//! fixtures every surface renders, and the echo kind end to end.

use lab_kit::echo::Echo;
use lab_kit::{
    AssetClass, AssetEntry, Card, Change, JobSpec, LabKind, Reference, Report, Version, View, Want,
    digest,
};
use serde_json::{Value, json};

fn version(content: Value) -> Version {
    Version {
        document: "d".into(),
        number: 1,
        parent: None,
        author: "test".into(),
        summary: "first".into(),
        created_unix_ms: 0,
        schema: "echo.v1".into(),
        content_sha256: digest(&content),
        content,
        report: Report::default(),
        assets: Vec::new(),
    }
}

#[test]
fn robot_lab_versions_fit() {
    // crates/robot-lab/src/version.rs `Version`, as stored.
    let robot = json!({"schema": "robot-lab.version.v1", "document": "so101-cube", "number": 4,
        "parent": {"document": "so101-cube", "version": "ab".repeat(32)}, "author": "owner",
        "summary": "Cube 10 cm further away", "created_unix_ms": 1_792_000_000_000u64,
        "content_sha256": "cd".repeat(32), "content": {"schema": "robot-lab.experiment.v1"}});
    let common = Version {
        document: robot["document"].as_str().unwrap().into(),
        number: robot["number"].as_u64().unwrap(),
        parent: Some(3),
        author: robot["author"].as_str().unwrap().into(),
        summary: robot["summary"].as_str().unwrap().into(),
        created_unix_ms: robot["created_unix_ms"].as_u64().unwrap(),
        schema: robot["content"]["schema"].as_str().unwrap().into(),
        content: robot["content"].clone(),
        content_sha256: digest(&robot["content"]),
        report: Report::default(),
        assets: vec![],
    };
    let back: Version = serde_json::from_value(serde_json::to_value(&common).unwrap()).unwrap();
    assert_eq!(back, common);
}

#[test]
fn cad_web_versions_fit() {
    // crates/openagents-web/src/cad_web.rs: index.json history entry and the
    // version's doc.json, as stored per account or project.
    let entry = json!({"n": 7, "summary": "Round the top edges 2 mm", "author": "owner",
        "created": 1_792_000_000u64, "mass_kg": 0.041});
    let doc =
        json!({"schema": "part.v1", "features": [{"id": "base", "op": "sketch", "params": {}}]});
    let common = Version {
        document: "fan-bracket".into(),
        number: entry["n"].as_u64().unwrap(),
        parent: Some(6),
        author: entry["author"].as_str().unwrap().into(),
        summary: entry["summary"].as_str().unwrap().into(),
        created_unix_ms: entry["created"].as_u64().unwrap() * 1000,
        schema: "part.v1".into(),
        content_sha256: digest(&doc),
        content: doc,
        report: Report {
            warnings: vec![],
            facts: json!({"mass_kg": entry["mass_kg"]}),
        },
        assets: vec![AssetEntry {
            location: "fonts/geist.ttf".into(),
            sha256: "ef".repeat(32),
            class: AssetClass::Public,
        }],
    };
    assert!(!common.has_private_local());
    assert_eq!(
        common.reference(),
        Reference {
            document: "fan-bracket".into(),
            version: 7
        }
    );
}

#[test]
fn every_card_fixture_reads_and_draws() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/cards");
    let mut seen = 0;
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        let text = std::fs::read_to_string(&path).unwrap();
        let card: Card =
            serde_json::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let state = card.state();
        match path.file_stem().unwrap().to_str().unwrap() {
            "video" => assert!(state.playable && !state.following && card.private_local),
            "video-rendering" => {
                assert!(state.following && !state.playable);
                assert_eq!(
                    state.status.as_deref(),
                    Some("Rendering the preview, about 3 min…")
                );
            }
            "compare" => assert!(state.playable && state.view == View::Compare),
            "report-refused" => assert_eq!(
                state.refusal.as_deref(),
                Some("The tug would weigh 14.2 t; its budget is 12 t.")
            ),
            "unknown-view" => {
                assert_eq!(card.view, View::Unknown);
                assert_eq!(state.view, View::Report);
                assert!(card.page.is_some(), "an unknown view still links somewhere");
            }
            _ => {}
        }
        seen += 1;
    }
    assert_eq!(seen, 8);
}

#[test]
fn echo_kind_round_trip() {
    let base = version(json!({"text": "hello"}));
    let draft = Echo::apply(
        &base,
        &Change::Intent {
            intent: json!({"intent": "set", "text": "hi there"}),
        },
        "Say hi",
        &Value::Null,
    )
    .unwrap();
    assert_eq!(draft.content, json!({"text": "hi there"}));
    assert_eq!(draft.report.facts["chars"], 8);

    let refused = Echo::apply(
        &base,
        &serde_json::from_value(
            json!({"type": "patch", "ops": [{"op": "replace", "path": "/text", "value": " "}]}),
        )
        .unwrap(),
        "Blank",
        &Value::Null,
    )
    .unwrap_err();
    assert_eq!(refused.message, "An echo needs some text.");
    let refused = Echo::apply(
        &base,
        &serde_json::from_value(
            json!({"type": "patch", "ops": [{"op": "add", "path": "/color", "value": "red"}]}),
        )
        .unwrap(),
        "Color",
        &Value::Null,
    )
    .unwrap_err();
    assert_eq!(refused.code, "unknown_field");

    let jobs = Echo::jobs_for(&base, &Want::Card);
    assert_eq!(jobs.len(), 1);
    let card = Echo::card(&base, &[], &[]);
    assert_eq!(card.state().view, View::Report);
    assert!(!card.state().playable);

    let manifest = Echo::manifest();
    assert!(
        manifest
            .tools
            .iter()
            .all(|t| t.name.starts_with(&manifest.tool_prefix))
    );
}

#[test]
fn job_keys_are_idempotent_and_distinct() {
    let base = version(json!({"text": "hello"}));
    let spec = |params: Value| JobSpec {
        params,
        ..Echo::jobs_for(&base, &Want::Card).remove(0)
    };
    let a = spec(json!({"quality": "preview"})).key(&base.content_sha256);
    assert_eq!(
        a,
        spec(json!({"quality": "preview"})).key(&base.content_sha256)
    );
    assert_ne!(
        a,
        spec(json!({"quality": "review"})).key(&base.content_sha256)
    );
    assert_ne!(
        a,
        spec(json!({"quality": "preview"})).key(&digest(&json!({"text": "other"})))
    );
}

mod properties {
    use super::*;
    use proptest::prelude::*;

    fn text() -> impl Strategy<Value = String> {
        "[a-zA-Z0-9 ]{0,24}"
    }

    proptest! {
        /// The same change on the same base always gives the same draft,
        /// and equal content has an equal digest.
        #[test]
        fn apply_is_deterministic(start in text(), next in text()) {
            prop_assume!(!start.trim().is_empty());
            let base = version(json!({"text": start}));
            let change = Change::Intent { intent: json!({"intent": "set", "text": next}) };
            let one = Echo::apply(&base, &change, "s", &Value::Null);
            let two = Echo::apply(&base, &change, "s", &Value::Null);
            prop_assert_eq!(&one, &two);
            if let Ok(draft) = one {
                prop_assert_eq!(digest(&draft.content), digest(&json!({"text": next})));
            }
        }
    }
}
