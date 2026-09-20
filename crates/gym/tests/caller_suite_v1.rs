//! `caller-v1` is the fixture the caller-suite builder emits from
//! `fixtures/caller-v1/records.jsonl`: the suite loads under
//! `Suite::load`, the set covers every item, and the two keyings —
//! per family where the wording is shared, per item where it is not —
//! both resolve.

use std::collections::BTreeSet;
use std::path::Path;

use gym::questions::QuestionSet;
use gym::row::LabelSource;
use gym::suite::{Partition, Suite};

const SUITE: &str = include_str!("fixtures/caller-v1/suite.json");

#[test]
fn caller_v1_loads_and_resolves_both_keyings() {
    // `Suite::load` recomputes the item digest and refuses a mismatch, so
    // a load is the check that the builder's canonicalization is Rust's.
    let suite = Suite::load(SUITE).unwrap();
    assert_eq!(suite.name, "caller-v1");
    assert_eq!(suite.items.len(), 22);
    assert_eq!(suite.questions.as_deref(), Some("caller-v1"));

    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/caller-v1/caller-v1.json");
    let set = QuestionSet::load(&path).unwrap();
    assert!(set.covers(&suite));

    let mut families = BTreeSet::new();
    for item in &suite.items {
        families.insert(item.family.as_str());
        assert!(item.question.is_none(), "{}", item.id);
        assert_eq!(
            item.evidence(),
            LabelSource::Other("fixture-caller".to_string())
        );
        let question = set.ask(item).unwrap();
        match item.family.as_str() {
            // A shared wording answers by family name.
            "routing" | "severity" => {
                assert_eq!(question, &set.questions[&item.family], "{}", item.id)
            }
            // Per-item wording answers by item id.
            "facts" => assert_eq!(question, &set.questions[&item.id], "{}", item.id),
            other => panic!("{other}"),
        }
        match item.kind.as_str() {
            "noul" => assert!(matches!(item.truth.as_str(), "yes" | "no"), "{}", item.id),
            "choice" => assert!(
                question["criteria"]
                    .as_object()
                    .unwrap()
                    .contains_key(&item.truth),
                "{}",
                item.id
            ),
            "score" => {
                let levels = question["criteria"].as_array().unwrap();
                let index: usize = item.truth.parse().unwrap();
                assert!(index < levels.len(), "{}", item.id);
            }
            other => panic!("{other}"),
        }
    }
    assert_eq!(families.len(), 3);

    // Every family reaches all three partitions.
    for family in &families {
        for partition in Partition::ALL {
            assert!(
                suite
                    .items
                    .iter()
                    .any(|item| item.family == *family && item.partition == partition),
                "{family}: {partition}"
            );
        }
    }

    // A paraphrase group stayed in one partition. `group` rides along on
    // the item undigested, so it is read from the file rather than `Item`.
    let raw: serde_json::Value = serde_json::from_str(SUITE).unwrap();
    let grouped: Vec<_> = raw["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|item| item["group"].as_str() == Some("para-a"))
        .collect();
    assert_eq!(grouped.len(), 2);
    assert_eq!(grouped[0]["partition"], grouped[1]["partition"]);
}
