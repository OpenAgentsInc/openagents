//! The `coder-turns-v1` question set is the text this crate actually sends.
//!
//! `classify.rs` says the question set and the thresholds that read it live
//! in one module so they review together. A suite scored against a copy of
//! the question text has the same failure the thresholds had: it drifts from
//! production without anything saying so. The Gym keeps question text in
//! `crates/gym/questions/<id>.json` under its own digest, and this test pins
//! that file to [`classify::questions`] and [`classify::shell_questions`].
//!
//! Editing a question in this crate therefore fails this test until the
//! question set is regenerated, and regenerating it under the same id is
//! wrong: a reword is a new set with a new id, so every row recorded under
//! the old text stays comparable with the items it was asked about.
//!
//! To print the set for the generator:
//!
//! ```text
//! cargo test -p coder --test suite_questions -- --ignored --nocapture
//! ```

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use coder::classify::{questions, shell_questions};
use serde_json::Value;

/// Where the Gym keeps question text, relative to this crate.
fn gym(path: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../gym")
        .join(path)
}

/// Every production question, by the family id the suite gives it.
///
/// The turn questions keep their own ids. The shell round's `outcome`
/// question is named `shell_outcome`, because `outcome` alone reads as a
/// column of a result row rather than as a question.
fn wire_questions() -> BTreeMap<String, Value> {
    let mut wire = BTreeMap::new();
    for (surface, question) in [("turn", questions()), ("shell", shell_questions())] {
        let body: Value = serde_json::to_value(&question).expect("the question set serializes");
        let fields = body
            .as_object()
            .expect("a question set is an object")
            .clone();
        for (name, value) in fields {
            let family = match (surface, name.as_str()) {
                ("shell", "outcome") => "shell_outcome".to_string(),
                _ => name,
            };
            wire.insert(family, value);
        }
    }
    wire
}

/// The committed question set, as a map of family to question body.
fn committed() -> BTreeMap<String, Value> {
    let text = std::fs::read_to_string(gym("questions/coder-turns-v1.json"))
        .expect("the coder question set is committed");
    let set: Value = serde_json::from_str(&text).expect("the question set parses");
    serde_json::from_value(set["questions"].clone()).expect("the set holds a map of questions")
}

#[test]
fn the_question_set_is_the_production_text() {
    assert_eq!(
        committed(),
        wire_questions(),
        "crates/gym/questions/coder-turns-v1.json no longer matches what classify.rs sends; \
         a reword is a new question set with a new id, not an edit to this one"
    );
}

#[test]
fn the_suite_names_that_set_and_carries_no_text_of_its_own() {
    let text = std::fs::read_to_string(gym("suites/coder-turns-v1.json"))
        .expect("the coder suite is committed");
    let suite: Value = serde_json::from_str(&text).expect("the coder suite parses");
    assert_eq!(suite["questions"], "coder-turns-v1");
    let items = suite["items"].as_array().expect("the suite has items");
    assert!(!items.is_empty(), "the suite has no items");
    let set = committed();
    for item in items {
        assert!(
            item.get("question").is_none(),
            "item {} carries question text the set already holds",
            item["id"]
        );
        let family = item["family"].as_str().expect("an item names its family");
        assert!(
            set.contains_key(family),
            "{family} is not a question this crate sends"
        );
    }
    for family in set.keys() {
        assert!(
            items.iter().any(|item| item["family"] == family.as_str()),
            "no item scores the {family} question"
        );
    }
}

#[test]
#[ignore = "prints the wire question bodies for the question set generator"]
fn print_wire_questions() {
    println!(
        "{}",
        serde_json::to_string_pretty(&wire_questions()).expect("the questions serialize")
    );
}
