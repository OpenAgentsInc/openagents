//! The Alice interview suite stays loadable and grounded: every item pins
//! the committed fixture, its canonical answer passes its own check, and
//! every required term is in a fixture record the item cites.

use gym::eval::utc_from_unix;
use gym::interview::{self, Category, ItemState};
use gym::suite::{Partition, Suite};

const MONTHS: [&str; 12] = [
    "january",
    "february",
    "march",
    "april",
    "may",
    "june",
    "july",
    "august",
    "september",
    "october",
    "november",
    "december",
];

/// A record as a person dates it: the ISO date, the spelled date, and the text.
fn dated(at: u64, text: &str) -> String {
    let iso = &utc_from_unix(at)[..10];
    let month: usize = iso[5..7].parse().unwrap();
    let day: u32 = iso[8..10].parse().unwrap();
    format!("{iso} {} {day} {text}", MONTHS[month - 1]).to_ascii_lowercase()
}

#[test]
#[ignore = "prints the digest the file records; run on suite edits"]
fn print_alice_interview_digest() {
    let suite: Suite = serde_json::from_str(interview::ALICE_V1_SUITE).unwrap();
    println!("{}", suite.compute_digest().unwrap());
}

#[test]
fn the_suite_pins_the_fixture_and_its_partitions() {
    let suite = interview::alice_v1_suite().expect("the committed suite loads");
    let fixture = interview::alice_v1_fixture().expect("the committed fixture loads");
    assert!(fixture.manifest.synthetic);
    assert!((150..=400).contains(&fixture.manifest.journal_rows));
    let counts = suite.counts();
    for partition in Partition::ALL {
        assert!(
            counts[&partition] >= 5,
            "{partition} holds {}",
            counts[&partition]
        );
    }
    for item in &suite.items {
        let state = ItemState::of(item).unwrap();
        assert_eq!(state.fixture, fixture.manifest.name);
        assert_eq!(state.fixture_digest, fixture.manifest.digest, "{}", item.id);
        assert_eq!(Category::parse(&item.family), Some(Category::Memory));
        assert_eq!(item.kind, interview::KIND_RECALL);
        assert!(!interview::question(item).is_empty(), "{}", item.id);
    }
}

#[test]
fn every_answer_is_checkable_and_grounded_in_the_fixture() {
    let suite = interview::alice_v1_suite().unwrap();
    let fixture = interview::alice_v1_fixture().unwrap();
    for item in &suite.items {
        let state = ItemState::of(item).unwrap();
        let check = state.check.as_ref().expect("a memory item has a check");
        let truth = interview::grade(check, &item.truth);
        assert!(truth.correct, "{}: its truth fails {truth:?}", item.id);
        assert!(
            !interview::grade(check, "I don't remember that.").correct,
            "{}",
            item.id
        );
        assert!(!state.sources.is_empty(), "{} cites nothing", item.id);
        let cited: Vec<String> = state
            .sources
            .iter()
            .map(|source| {
                let (at, text) = fixture
                    .record(source)
                    .unwrap_or_else(|| panic!("{}: no record {source}", item.id));
                assert!(
                    at < state.as_of,
                    "{}: {source} is after the interview",
                    item.id
                );
                dated(at, &text)
            })
            .collect();
        let cited = cited.join("\n");
        for group in &check.all {
            assert!(
                group.iter().any(|term| cited.contains(term.as_str())),
                "{}: no cited record holds any of {group:?}:\n{cited}",
                item.id
            );
        }
    }
}
