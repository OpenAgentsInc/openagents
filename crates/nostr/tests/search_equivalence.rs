//! The pure crate's half of the NIP-50 search oracle. The relay's
//! `store_postgres` suite judges the same fixture through its replay and
//! COUNT SQL, so a divergence between `search_matches` and the SQL fails a
//! test in whichever crate drifted.

use nostr::domain::{Event, Filter, search_matches};

const SHARED_CORPUS: &str = include_str!("../../../tests/fixtures/nip50/search-equivalence.json");
const CONTENT_CASES: &str = include_str!("../../../tests/fixtures/nip50/search.json");

fn event(kind: u16, content: &str) -> Event {
    Event {
        id: "0".repeat(64),
        pubkey: "1".repeat(64),
        created_at: 1,
        kind,
        tags: Vec::new(),
        content: content.to_owned(),
        sig: "0".repeat(128),
    }
}

fn search_filter(search: &str) -> Filter {
    Filter {
        search: Some(search.to_owned()),
        ..Filter::default()
    }
}

#[test]
fn shared_corpus_matches_through_search_matches_and_filter() {
    let fixture: serde_json::Value = serde_json::from_str(SHARED_CORPUS).unwrap();
    let corpus = fixture["corpus"].as_array().unwrap();
    let searches = fixture["searches"].as_array().unwrap();
    assert!(!corpus.is_empty() && !searches.is_empty());
    let events = corpus
        .iter()
        .map(|entry| {
            let kind = u16::try_from(entry["kind"].as_u64().unwrap()).unwrap();
            event(kind, entry["content"].as_str().unwrap())
        })
        .collect::<Vec<_>>();

    for case in searches {
        let search = case["search"].as_str().unwrap();
        let valid = case["valid"].as_bool().unwrap();
        let expected = case["matches"]
            .as_array()
            .unwrap()
            .iter()
            .map(|index| usize::try_from(index.as_u64().unwrap()).unwrap())
            .collect::<Vec<_>>();
        let filter = search_filter(search);
        assert_eq!(
            filter.validate().is_ok(),
            valid,
            "validity of search {search:?}"
        );
        let matched = events
            .iter()
            .enumerate()
            .filter(|(_, event)| search_matches(search, event.kind, &event.content))
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        assert_eq!(matched, expected, "search_matches for search {search:?}");
        let through_filter = events
            .iter()
            .enumerate()
            .filter(|(_, event)| filter.matches(event))
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        assert_eq!(
            through_filter, expected,
            "Filter::matches for search {search:?}"
        );
    }
}

#[test]
fn content_cases_match_and_validate() {
    let fixture: serde_json::Value = serde_json::from_str(CONTENT_CASES).unwrap();
    let content = fixture["content"].as_str().unwrap();
    let event = event(1, content);
    for case in fixture["cases"].as_array().unwrap() {
        let search = case["search"].as_str().unwrap();
        let filter = search_filter(search);
        assert_eq!(
            filter.validate().is_ok(),
            case["valid"].as_bool().unwrap(),
            "validity of search {search:?}"
        );
        assert_eq!(
            search_matches(search, event.kind, &event.content),
            case["matches"].as_bool().unwrap(),
            "search_matches for search {search:?}"
        );
        assert_eq!(
            filter.matches(&event),
            case["matches"].as_bool().unwrap(),
            "Filter::matches for search {search:?}"
        );
    }
}
