//! The labeled route sets as Gym suites (#9925, #9936, #9960).
//!
//! `crates/gym/suites/chat-router-v3.json` is generated from
//! `crates/coder/fixtures/chat-router/routes-v3.json`: one `route` item per
//! row, its state the one the router's judgment reads, its truth the
//! labeled route. Held-out rows are the locked partition, so the Gym's
//! ledger records the one read of them. The question text,
//! `crates/gym/questions/chat-router-route-v4.json`, is
//! [`coder::router_eval::route_question`], which is the production router's
//! structured `route` question ([`coder::router::judge::route`]), so Gym
//! scores measure what production asks. These tests fail when the fixture
//! or the router's route question changes and the Gym files were not
//! regenerated:
//!
//! ```text
//! ROUTER_SUITE_WRITE=1 cargo test -p coder --test router_suite -- --ignored
//! ```
//!
//! `chat-router-v1.json` and its question set `chat-router-route-v2.json`,
//! and `chat-router-v2.json` with `chat-router-route-v3.json`, are kept as
//! they were recorded: the twelve- and eighteen-route questions those
//! scores were measured with, and the suites generated from
//! `routes-v1.json` and `routes-v2.json`.

use std::path::{Path, PathBuf};

use coder::generate::{Message, Role};
use coder::router_eval::{
    SUITE, SUITE_QUESTIONS, SUITE_QUESTIONS_V1, SUITE_QUESTIONS_V2, SUITE_V1, SUITE_V2, Set,
    partition_of, route_question,
};
use gym::suite::{Item, Partition, Suite};
use serde_json::{Value, json};

fn gym(path: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../gym")
        .join(path)
}

fn suite(set: &Set, name: &str, questions: &str, gate: &str, description: &str) -> Suite {
    let items: Vec<Item> = set
        .rows
        .iter()
        .map(|row| {
            let transcript: Vec<Message> = row
                .messages
                .iter()
                .map(|m| Message {
                    role: if m.role == "assistant" {
                        Role::Assistant
                    } else {
                        Role::User
                    },
                    text: m.text.clone(),
                })
                .collect();
            Item {
                id: format!("route/{}", row.id),
                family: "route".to_string(),
                kind: "choice".to_string(),
                state: coder::first::state(row.latest(), &transcript),
                question: None,
                truth: row.route.clone(),
                partition: match partition_of(row) {
                    "locked" => Partition::Locked,
                    "calibration" => Partition::Calibration,
                    _ => Partition::Development,
                },
                label_source: None,
                label_rule: Some(
                    "author: the route a correct chat router takes for the latest message, \
                     per docs/coder/design/2026-09-28-chat-router.md"
                        .to_string(),
                ),
            }
        })
        .collect();
    let mut suite = Suite {
        schema: "openagents.gym.suite.v1".to_string(),
        name: name.to_string(),
        description: description.to_string(),
        created: set.created.clone(),
        digest: String::new(),
        tier: Some("scored".to_string()),
        gate: Some(gate.to_string()),
        questions: Some(questions.to_string()),
        sampling: None,
        exposure: None,
        items,
    };
    suite.digest = suite.compute_digest().expect("the items digest");
    suite
}

fn v1() -> Suite {
    suite(
        &Set::v1(),
        SUITE_V1,
        SUITE_QUESTIONS_V1,
        "probability-v2",
        "The chat router's labeled route set (#9925), from \
         crates/coder/fixtures/chat-router/routes-v1.json: realistic first \
         messages across the 12 routes, labeled with the route a correct router \
         takes. The fixture's held-out rows are the locked partition.",
    )
}

fn v2() -> Suite {
    suite(
        &Set::v2(),
        SUITE_V2,
        SUITE_QUESTIONS_V2,
        coder::router_claim::GATE,
        "The chat router's labeled route set for chat-router-v2 (#9936), from \
         crates/coder/fixtures/chat-router/routes-v2.json: the v1 rows and rows for \
         the Gym and eval routes, across 18 routes, labeled with the route a correct \
         router takes. The fixture's held-out rows are the locked partition. Judged \
         by the router-v1 gate (#9959): canned precision primary, route accuracy, \
         canned recall, dispatch precision, and ECE held non-inferior.",
    )
}

fn v3() -> Suite {
    suite(
        &Set::fixture(),
        SUITE,
        SUITE_QUESTIONS,
        coder::router_claim::GATE,
        "The chat router's labeled route set for chat-router-v3 (#9960), from \
         crates/coder/fixtures/chat-router/routes-v3.json: the v2 rows and rows for \
         the capability.missing route and its admitted near misses, across 19 routes, \
         labeled with the route a correct router takes. The fixture's held-out rows \
         are the locked partition.",
    )
}

fn questions() -> Value {
    json!({
        "$comment": [
            "Generated by crates/coder/tests/router_suite.rs from coder::router::judge::route, the structured route question the production chat router asks Jev."
        ],
        "schema": "openagents.gym.question_set.v1",
        "id": SUITE_QUESTIONS,
        "suite": SUITE,
        "questions": { "route": jev::Question::from(route_question()) },
    })
}

fn committed(name: &str) -> Suite {
    Suite::load(
        &std::fs::read_to_string(gym(&format!("suites/{name}.json"))).expect("the suite exists"),
    )
    .expect("the committed suite loads")
}

#[test]
fn the_gym_suites_are_the_labeled_sets() {
    let v3_committed = committed(SUITE);
    assert_eq!(v3_committed.digest, v3().digest, "regenerate the suite");
    assert_eq!(v3_committed.questions.as_deref(), Some(SUITE_QUESTIONS));
    assert_eq!(
        v3_committed.gate.as_deref(),
        Some(coder::router_claim::GATE),
        "regenerate the suite"
    );
    gym::gate::load(coder::router_claim::GATE).expect("the named gate is a committed rule");
    // The v1 and v2 suites are kept as recorded.
    let v2_committed = committed(SUITE_V2);
    assert_eq!(v2_committed.digest, v2().digest);
    assert_eq!(v2_committed.questions.as_deref(), Some(SUITE_QUESTIONS_V2));
    gym::questions::load(SUITE_QUESTIONS_V2).expect("the v2 question set still reads");
    let v1_committed = committed(SUITE_V1);
    assert_eq!(v1_committed.digest, v1().digest);
    assert_eq!(v1_committed.questions.as_deref(), Some(SUITE_QUESTIONS_V1));
    gym::questions::load(SUITE_QUESTIONS_V1).expect("the v1 question set still reads");
}

#[test]
fn the_gym_question_is_the_route_question() {
    let committed: Value = serde_json::from_str(
        &std::fs::read_to_string(gym(&format!("questions/{SUITE_QUESTIONS}.json")))
            .expect("the question set exists"),
    )
    .expect("parses");
    assert_eq!(
        committed["questions"],
        questions()["questions"],
        "regenerate the question set"
    );
    gym::questions::load(SUITE_QUESTIONS).expect("the Gym reads it");
}

/// The question set's digest the worker puts on the wire
/// (`chat-router-v2@<digest>`, [`coder::router::set_id`]) is the Gym's
/// digest of the committed question file, so a judgment, a Gym row, and an
/// eval report name one question by one digest (#9959).
#[test]
fn the_wire_names_the_question_set_by_the_gyms_digest() {
    let served = gym::questions::load(SUITE_QUESTIONS).expect("the Gym reads it");
    assert_eq!(served.digest(), coder::router::set_digest());
    assert_eq!(
        coder::router::set_id(),
        format!("{SUITE}@{}", &served.digest()[..12])
    );
    // A question that reads differently is a different set.
    let mut reworded = served.clone();
    if let Some(question) = reworded.questions.get_mut("route") {
        question["instructions"] = json!("Pick the route.");
    }
    assert_ne!(reworded.digest(), coder::router::set_digest());
}

/// The Gym asks the `route` question the deployed router asks, whatever
/// the bank, facts, command groups, or tools: one source, not a copy.
#[test]
fn the_gym_question_is_the_production_route_question() {
    let facts = coder::router::worker_facts(
        coder::generate::DEFAULT_MODEL,
        Some(coder::generate::DEFAULT_DOOR_URL),
        Some((6, 40)),
        &coder::router::Seams::default(),
    );
    let production = coder::router::judge::questions(
        coder::router::Bank::builtin(),
        &facts,
        &[],
        &[],
        &coder::router::Admitted::builtin(),
    );
    let production = serde_json::to_value(production.get("route")).expect("serializes");
    let served = gym::questions::load(SUITE_QUESTIONS).expect("the Gym reads it");
    for item in &committed(SUITE).items {
        assert_eq!(
            served.ask(item).expect("the set covers every item"),
            &production,
            "{}: the Gym asks the router's route question",
            item.id
        );
    }
}

#[test]
#[ignore = "writes the Gym files"]
fn write_the_gym_suite() {
    if std::env::var_os("ROUTER_SUITE_WRITE").is_none() {
        return;
    }
    std::fs::write(
        gym(&format!("suites/{SUITE}.json")),
        serde_json::to_string_pretty(&v3()).expect("serializes") + "\n",
    )
    .expect("writes the suite");
    std::fs::write(
        gym(&format!("questions/{SUITE_QUESTIONS}.json")),
        serde_json::to_string_pretty(&questions()).expect("serializes") + "\n",
    )
    .expect("writes the question set");
}
