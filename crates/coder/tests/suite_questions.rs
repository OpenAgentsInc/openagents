//! The `coder-turns-v2` question set is the text this crate sends.
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
//! `coder-turns-v1` is that older set: its five retired questions are no
//! longer sent, and its file and rows stay as the measurement that retired
//! them (`docs/decision-models/measurements/2026-09-20-coder-question-baselines.md`).
//!
//! To print the set for the generator:
//!
//! ```text
//! cargo test -p coder --test suite_questions -- --ignored --nocapture
//! ```

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use coder::classify::{questions, shell_questions};
use coder::delegate::boundary_supported;
use coder::questions::Fill;
use coder::runtime::{PROGRAM_QUESTION, Runtime};
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

/// The repository this crate lives in, whose `questions/` and
/// `programs/` the selection question is built from.
fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Whether this host can put a delegation inside an enforced filesystem
/// boundary. A host without a backend offers no program with a `delegate`
/// step, so the option set it would send is not the production one; the case
/// says so and returns.
fn boundary() -> bool {
    if boundary_supported() {
        return true;
    }
    eprintln!("skipping: this host has no filesystem boundary backend (bwrap on Linux)");
    false
}

/// The program-selection question, exactly as `runtime::select` sends it
/// on a host that resolved this repository's programs.
///
/// The wording is the repository's own `questions/program.json` and the
/// options are the programs in `programs/`, so a suite scored against this
/// is scored against production rather than against a copy of it.
fn wire_selection() -> BTreeMap<String, Value> {
    let root = repository();
    let set = coder::questions::Registry::open(&[root.join("questions")])
        .get(PROGRAM_QUESTION)
        .cloned()
        .expect("the repository carries the selection question");
    let built = set
        .build(&Fill::Options(host().selectable()))
        .expect("the options the host would run fill it");
    serde_json::from_value(serde_json::to_value(&built).expect("the question serializes"))
        .expect("a question set is a map of questions")
}

/// A runtime over this repository, for the option set it would offer.
///
/// The door is a stub that is never called: a `decide` step is admitted
/// only when a door is configured, and whether one answers is not what
/// this is asking.
fn host() -> Runtime {
    let door = jev::Client::new(
        jev::Config::new()
            .api_key("ts-not-called")
            .base_url("http://127.0.0.1:1"),
    )
    .expect("a client builds");
    Runtime::open(Some(&repository()), &repository()).asking(Some(door))
}

/// The committed question set for the program selection, as the Gym holds
/// it.
fn committed_selection() -> BTreeMap<String, Value> {
    let text = std::fs::read_to_string(gym("questions/program-selection-v3.json"))
        .expect("the selection question set is committed");
    let set: Value = serde_json::from_str(&text).expect("the question set parses");
    serde_json::from_value(set["questions"].clone()).expect("the set holds a map of questions")
}

/// The question the selection suite scores is the one the host sends.
///
/// A measurement of a copy of the question measures the copy.
/// `docs/decision-models/measurements/2026-09-19-program-selection.md` reports what this
/// wording scored, and this test is what keeps that report about the
/// wording still in `questions/program.json`.
#[test]
fn the_selection_question_is_the_production_text() {
    if !boundary() {
        return;
    }
    assert_eq!(
        committed_selection(),
        wire_selection(),
        "crates/gym/questions/program-selection-v3.json no longer matches what \
         questions/program.json and programs/ produce; a reword is a new question \
         set with a new id, not an edit to this one"
    );
}

/// Every option the question offers is a program this host resolved, plus
/// `none`. A program that dropped out of the registry would leave the
/// suite scoring an option nothing can run.
#[test]
fn the_selection_question_offers_none_and_the_resolved_programs() {
    if !boundary() {
        return;
    }
    let wire = wire_selection();
    let criteria = wire["program"]["criteria"]
        .as_object()
        .expect("a choice offers criteria")
        .clone();
    assert!(
        criteria.contains_key("none"),
        "a forced choice is how an ordinary question becomes a fan-out"
    );
    let host = host();
    for slug in criteria.keys().filter(|slug| *slug != "none") {
        let program =
            host.survey().programs.get(slug).unwrap_or_else(|| {
                panic!("{slug} is offered and this host resolves no such program")
            });
        host.admit(program).unwrap_or_else(|refused| {
            panic!("{slug} is offered and this host refuses it: {refused}")
        });
    }
    assert_eq!(
        criteria.len(),
        host.selectable().len() + 1,
        "the options are the programs this host would run, plus none"
    );
}

/// The committed question set, as a map of family to question body.
fn committed() -> BTreeMap<String, Value> {
    let text = std::fs::read_to_string(gym("questions/coder-turns-v2.json"))
        .expect("the coder question set is committed");
    let set: Value = serde_json::from_str(&text).expect("the question set parses");
    serde_json::from_value(set["questions"].clone()).expect("the set holds a map of questions")
}

#[test]
fn the_question_set_is_the_production_text() {
    assert_eq!(
        committed(),
        wire_questions(),
        "crates/gym/questions/coder-turns-v2.json no longer matches what classify.rs sends; \
         a reword is a new question set with a new id, not an edit to this one"
    );
}

/// The five `coder-turns-v1` families the baselines record retired are
/// not on the wire, and the two it kept are word for word the v1 text, so
/// a v2 row answers the same question as a v1 row on the same item.
#[test]
fn the_retired_v1_questions_are_not_sent() {
    let text = std::fs::read_to_string(gym("questions/coder-turns-v1.json"))
        .expect("the v1 question set is retained");
    let v1: Value = serde_json::from_str(&text).expect("the v1 question set parses");
    let v1: BTreeMap<String, Value> =
        serde_json::from_value(v1["questions"].clone()).expect("the set holds a map of questions");
    let wire = wire_questions();
    for retired in ["damage", "needs_code", "progress", "risk", "useful"] {
        assert!(v1.contains_key(retired), "{retired} was a v1 question");
        assert!(
            !wire.contains_key(retired),
            "{retired} is retired and still sent"
        );
    }
    for kept in ["action", "shell_outcome"] {
        assert_eq!(
            wire[kept], v1[kept],
            "{kept} was reworded; that is a new set"
        );
    }
    assert_eq!(wire.len(), 2);
}

#[test]
fn the_suite_names_that_set_and_carries_no_text_of_its_own() {
    let text = std::fs::read_to_string(gym("suites/coder-turns-v2.json"))
        .expect("the coder suite is committed");
    let suite: Value = serde_json::from_str(&text).expect("the coder suite parses");
    assert_eq!(suite["questions"], "coder-turns-v2");
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

#[test]
#[ignore = "prints the selection question body for the question set generator"]
fn print_wire_selection() {
    println!(
        "{}",
        serde_json::to_string_pretty(&wire_selection()).expect("the question serializes")
    );
}
