//! The Gym suite `background-cache-dir-v1` and its question set are the
//! production judgment: each item's state is what [`Unknown::state`]
//! builds from the labeled fixture, and the questions are [`QUESTION`]
//! and [`ALL_KINDS`] word for word. Rebuild the Gym files with
//! `python3 crates/gym/suites/build_background_cache_dir_v1.py`.

use std::path::Path;

use serde_json::Value;

use crate::judged::{ALL_KINDS, CACHE_DIR, QUESTION, Unknown};

const FIXTURE: &str = include_str!("../fixtures/cache-dir-v1.json");
const SUITE: &str = include_str!("../../gym/suites/background-cache-dir-v1.json");
const QUESTIONS: &str = include_str!("../../gym/questions/background-cache-dir-v1.json");

#[test]
fn the_suite_states_are_what_production_shows_jev() {
    let home = Path::new("/home/person");
    let fixture: Value = serde_json::from_str(FIXTURE).unwrap();
    let suite: Value = serde_json::from_str(SUITE).unwrap();
    let rows = fixture["rows"].as_array().unwrap();
    assert!(rows.len() >= 20);
    for row in rows {
        let id = row["id"].as_str().unwrap();
        let rel = row["path"].as_str().unwrap().strip_prefix("~/").unwrap();
        let unknown = Unknown {
            path: home.join(rel),
            bytes: row["bytes"].as_u64().unwrap(),
            age_secs: row["age_days"].as_u64().unwrap() * 86_400,
            top: row["top"]
                .as_array()
                .unwrap()
                .iter()
                .map(|e| (e[0].as_str().unwrap().to_owned(), e[1].as_u64().unwrap()))
                .collect(),
            markers: row["markers"]
                .as_array()
                .unwrap()
                .iter()
                .map(|m| m.as_str().unwrap().to_owned())
                .collect(),
            users: usize::try_from(row["users"].as_u64().unwrap()).unwrap(),
        };
        let item = suite["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["id"] == format!("cache/{id}"))
            .unwrap();
        assert_eq!(item["state"]["directory"], unknown.state(home), "{id}");
        let kind = row["kind"].as_str().unwrap();
        assert!(ALL_KINDS.iter().any(|(k, _)| *k == kind), "{id}");
        // A yes is only ever a disposable kind.
        if row["cache"].as_bool().unwrap() {
            assert!(crate::judged::KINDS.contains(&kind), "{id}");
        }
    }
}

#[test]
fn the_question_set_is_the_production_question() {
    let set: Value = serde_json::from_str(QUESTIONS).unwrap();
    let cache = &set["questions"]["cache"];
    assert_eq!(cache["instructions"], QUESTION);
    assert_eq!(cache["decision"]["threshold"], CACHE_DIR.default);
    let criteria = set["questions"]["kind"]["criteria"].as_object().unwrap();
    assert_eq!(criteria.len(), ALL_KINDS.len());
    for (kind, what) in ALL_KINDS {
        assert_eq!(criteria[kind]["what"], what);
    }
}
