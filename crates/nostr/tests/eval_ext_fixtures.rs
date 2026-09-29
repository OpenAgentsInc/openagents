//! Extension evaluation fixture checks (`nips/openagents/NIP-EVAL.md`,
//! "Extension evaluation profile"; `NIP-CJ.md`, "Conversation jobs").
//!
//! `fixtures/eval-ext/<schema>/valid/*.json` must pass both the schema
//! under `nips/openagents/schemas/<schema>.v1.json` and the crate's typed
//! parser; `invalid/*.json` must be refused by both. The schema and the
//! parser are two readers of one contract, so each fixture checks that
//! they agree.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use nostr::cj_conversation::{parse_card, parse_draft};
use nostr::contracts::{digest_bytes, prepare_closure, validate_instance};
use nostr::eval_ext::{parse_case_manifest_value, parse_profile};
use serde_json::Value;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn schema_accepts(schema: &str, instance: &Value) -> bool {
    let path = root()
        .join("../../nips/openagents/schemas")
        .join(format!("{schema}.v1.json"));
    let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let digest = digest_bytes(&bytes);
    let mut documents = BTreeMap::new();
    documents.insert(digest.clone(), bytes);
    let closure = prepare_closure(&documents).expect("a supported schema");
    validate_instance(&closure, &digest, instance).is_ok()
}

fn parser_accepts(schema: &str, instance: &Value) -> bool {
    match schema {
        "eval-case" => parse_case_manifest_value(instance).is_ok(),
        "ext-eval" => parse_profile(instance).is_ok(),
        "eval-draft" => parse_draft(instance).is_ok(),
        "cj-card" => parse_card(instance).is_ok(),
        other => panic!("no parser for {other}"),
    }
}

fn fixtures(dir: &Path) -> Vec<(String, Value)> {
    let mut out: Vec<(String, Value)> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("read {}: {e}", dir.display()))
        .map(|entry| {
            let path = entry.expect("entry").path();
            let bytes = std::fs::read(&path).expect("fixture");
            let name = path.file_name().unwrap().to_string_lossy().to_string();
            (name, serde_json::from_slice(&bytes).expect("fixture JSON"))
        })
        .collect();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

#[test]
fn schemas_and_parsers_agree_on_every_fixture() {
    let base = root().join("fixtures/eval-ext");
    let mut checked = 0;
    for schema in ["eval-case", "ext-eval", "eval-draft", "cj-card"] {
        let valid = fixtures(&base.join(schema).join("valid"));
        let invalid = fixtures(&base.join(schema).join("invalid"));
        assert!(
            !valid.is_empty() && !invalid.is_empty(),
            "{schema} has fixtures"
        );
        for (name, value) in &valid {
            assert!(
                schema_accepts(schema, value),
                "{schema}.v1.json refuses valid/{name}"
            );
            assert!(
                parser_accepts(schema, value),
                "the parser refuses {schema} valid/{name}"
            );
            checked += 1;
        }
        for (name, value) in &invalid {
            assert!(
                !schema_accepts(schema, value),
                "{schema}.v1.json accepts invalid/{name}"
            );
            assert!(
                !parser_accepts(schema, value),
                "the parser accepts {schema} invalid/{name}"
            );
            checked += 1;
        }
    }
    assert!(checked >= 40);
}
