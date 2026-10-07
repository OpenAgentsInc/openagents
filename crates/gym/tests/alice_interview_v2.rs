//! The five-category Alice interview suite stays loadable and grounded:
//! every category has items in all three partitions, every item pins the
//! committed fixture and cites records from before the interview, every
//! code-checked answer passes its own check against what it cites, and the
//! suite names the gate that judges it.

use std::collections::{BTreeMap, BTreeSet};

use gym::eval::utc_from_unix;
use gym::interview::{self, Category, ItemState};
use gym::suite::{Partition, Suite};

#[test]
#[ignore = "prints the digest the file records; run on suite edits"]
fn print_alice_interview_v2_digest() {
    let suite: Suite = serde_json::from_str(interview::ALICE_V2_SUITE).unwrap();
    println!("{}", suite.compute_digest().unwrap());
}

#[test]
fn every_category_has_items_in_every_partition() {
    let suite = interview::alice_v2_suite().expect("the committed suite loads");
    assert_eq!(suite.gate.as_deref(), Some(interview::GATE));
    let mut seen: BTreeMap<Category, BTreeSet<Partition>> = BTreeMap::new();
    for item in &suite.items {
        let category = Category::parse(&item.family).expect("a known category");
        let kind = if category.code_checked() {
            interview::KIND_RECALL
        } else {
            interview::KIND_JUDGED
        };
        assert_eq!(item.kind, kind, "{}", item.id);
        seen.entry(category).or_default().insert(item.partition);
    }
    for category in Category::ALL {
        assert_eq!(
            seen.get(&category).map(BTreeSet::len),
            Some(3),
            "{category} lacks a partition"
        );
    }
    // Version 1's memory items carry over unchanged.
    let v1 = interview::alice_v1_suite().unwrap();
    let json = |item: &gym::suite::Item| serde_json::to_value(item).unwrap();
    for item in &v1.items {
        assert!(
            suite.items.iter().any(|other| json(other) == json(item)),
            "{} changed",
            item.id
        );
    }
}

#[test]
fn every_item_is_grounded_in_the_fixture() {
    let suite = interview::alice_v2_suite().unwrap();
    let fixture = interview::alice_v1_fixture().unwrap();
    for item in &suite.items {
        let state = ItemState::of(item).unwrap();
        assert_eq!(state.fixture_digest, fixture.manifest.digest, "{}", item.id);
        assert!(!state.sources.is_empty(), "{} cites nothing", item.id);
        let mut cited = String::new();
        for source in &state.sources {
            let (at, text) = fixture
                .record(source)
                .unwrap_or_else(|| panic!("{}: no record {source}", item.id));
            assert!(at < state.as_of, "{}: {source} is after it", item.id);
            cited.push_str(&utc_from_unix(at)[..10]);
            cited.push(' ');
            cited.push_str(&text.to_ascii_lowercase());
            cited.push('\n');
        }
        assert!(!interview::question(item).is_empty(), "{}", item.id);
        assert!(!item.truth.is_empty(), "{}", item.id);
        if Category::parse(&item.family).is_some_and(Category::code_checked) {
            let check = state.check.as_ref().expect("a checked item has a check");
            assert!(
                interview::grade(check, &item.truth).correct,
                "{}: its truth fails",
                item.id
            );
            assert!(
                !interview::grade(check, "I don't remember that.").correct,
                "{}",
                item.id
            );
            if item.family == Category::Plan.as_str() {
                for group in &check.all {
                    assert!(
                        group.iter().any(|term| cited.contains(term.as_str())),
                        "{}: no cited record holds any of {group:?}:\n{cited}",
                        item.id
                    );
                }
            }
        } else {
            assert!(state.check.is_none(), "{} is judged, not checked", item.id);
        }
    }
}

#[test]
fn the_gate_loads_from_the_compiled_copy_and_the_directory_alike() {
    let compiled = interview::gate().expect("the compiled gate validates");
    let committed = gym::gate::load(interview::GATE).expect("the committed gate loads");
    assert_eq!(compiled.digest(), committed.digest());
    assert!(matches!(compiled.rule, gym::gate::Rule::Interview(_)));
    interview::ab_rule()
        .validate()
        .expect("the interview round's rule validates");
}

/// The retained baseline: one scripted round of all five arms over the
/// development partition (`coder interview round`, recorded in
/// `bench/verse/2026-10-07/alice-interview-v2/`). It stays a verified chain
/// over this suite, and its report names the committed gate. A suite or gate
/// edit makes it stale; rerun the round `docs/verse/generative-agents.md`
/// names under item 7.
#[test]
fn the_retained_baseline_receipt_verifies() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../bench/verse/2026-10-07/alice-interview-v2");
    let rows = gym::store::read_rows(&dir.join("rows.jsonl")).expect("the rows read");
    let gym::store::ChainVerdict::Ok { rows: count, head } = gym::store::verify_chain(&rows) else {
        panic!("the retained chain doesn't verify");
    };
    let suite = interview::alice_v2_suite().unwrap();
    let parsed = interview::rows_of(&rows);
    assert_eq!(parsed.len(), count);
    assert!(parsed.iter().all(|r| r.suite_digest == suite.digest));
    let arms: BTreeSet<&str> = parsed.iter().map(|r| r.arm.as_str()).collect();
    assert_eq!(arms.len(), 5, "{arms:?}");

    let report: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("report.json")).unwrap()).unwrap();
    assert_eq!(report["head"].as_str(), head.as_deref());
    assert_eq!(report["suite_digest"], suite.digest.as_str());
    assert_eq!(report["gate"]["gate_id"], interview::GATE);
    assert_eq!(
        report["gate"]["gate_digest"],
        interview::gate().unwrap().digest().as_str()
    );
    // The judge agreement and the comparison recomputed from the rows match
    // what the report recorded.
    let recomputed = interview::comparison(
        report["group"].as_str().unwrap(),
        &parsed,
        Some(&interview::Agreement::default()),
    );
    assert_eq!(
        serde_json::to_value(&recomputed).unwrap(),
        report["comparison"]
    );
}
