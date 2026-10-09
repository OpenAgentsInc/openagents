//! The EVALS board: grouping by test set, folded checks, names, credit, and
//! refusals. Events are signed with throwaway keys.

use std::collections::BTreeMap;

use nostr::domain::RelaySigner;

use super::fixture::{self, Spec};
use super::*;

const AT: u64 = 1_790_000_000;

pub(crate) fn signer(label: &str) -> RelaySigner {
    use sha2::Digest;
    let hex: String = sha2::Sha256::digest(label.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    RelaySigner::from_secret_hex(&hex).expect("throwaway key")
}

fn spec<'a>(suite: &'a Event, tool: &'a Event, with: u64, without: u64) -> Spec<'a> {
    Spec {
        suite,
        tool,
        with,
        without: Some(without),
        total: 8,
        lock: "lock-a",
        checks: None,
    }
}

#[test]
fn results_group_by_test_set_with_names_checks_and_credit() {
    let (op, alice, bob, carol) = (
        signer("operator"),
        signer("alice"),
        signer("bob"),
        signer("carol"),
    );
    let find = fixture::release(&op, "starter-find", "1", AT - 900);
    let tests = fixture::release(&op, "starter-tests", "2", AT - 900);
    let finder = fixture::release(&op, "code-finder", "0.3.0", AT - 900);
    let reader = fixture::release(&op, "test-reader", "1.0.0", AT - 900);
    let a = fixture::result(&alice, &spec(&find, &finder, 6, 4), AT - 300);
    let b = fixture::result(&bob, &spec(&find, &reader, 3, 4), AT - 200);
    let c = fixture::result(&carol, &spec(&tests, &reader, 5, 5), AT - 100);
    // Carol checks Alice's result with the same suite, subject, and lock.
    let check = fixture::result(
        &carol,
        &Spec {
            checks: Some(&a.id),
            ..spec(&find, &finder, 6, 4)
        },
        AT - 50,
    );
    // Bob "checks" Alice with another lock: not a check, a result of its own.
    let other = fixture::result(
        &bob,
        &Spec {
            checks: Some(&a.id),
            lock: "lock-b",
            ..spec(&find, &finder, 5, 4)
        },
        AT - 40,
    );
    let events = [
        &find, &tests, &finder, &reader, &a, &b, &c, &check, &other, &a,
    ];
    let publications = verified(events.iter().copied());
    assert_eq!(
        publications.len(),
        5,
        "the duplicate and the releases don't count"
    );
    let names = Names::from_events(events.iter().copied());
    assert_eq!(names.releases.len(), 4);
    let credit = eval_credit([
        ("eval-check", carol.pubkey(), 20),
        ("eval-check", alice.pubkey(), 10),
        ("reproduce", alice.pubkey(), 50),
    ]);
    let board = board(&publications, &names, &credit, alice.pubkey());
    assert_eq!((board.results, board.checks), (4, 1));
    // The test set with the newest result first.
    let sets: Vec<&str> = board.groups.iter().map(|g| g.test_set.as_str()).collect();
    assert_eq!(sets, ["starter-find 1", "starter-tests 2"]);
    let find_rows = &board.groups[0].rows;
    assert_eq!(find_rows.len(), 3);
    assert_eq!(find_rows[0].id, other.id);
    let alice_row = find_rows.iter().find(|r| r.id == a.id).unwrap();
    assert_eq!(alice_row.tool, "code-finder 0.3.0");
    assert_eq!(alice_row.headline, "6 of 8 passed with it, 4 of 8 without");
    assert_eq!(
        (alice_row.verdict, alice_row.verdict_words),
        ("pass", "Better")
    );
    assert_eq!((alice_row.confirmed, alice_row.disputed), (1, 0));
    assert_eq!(alice_row.checks, "Confirmed by 1 check");
    assert_eq!(alice_row.credit_xp, 10, "reproduce XP isn't eval credit");
    assert!(alice_row.mine && !alice_row.hosted);
    let bob_row = find_rows.iter().find(|r| r.id == b.id).unwrap();
    assert_eq!((bob_row.verdict_words, bob_row.mine), ("Worse", false));
    assert_eq!(bob_row.tool, "test-reader 1.0.0");
    assert_eq!(board.groups[1].rows[0].verdict_words, "No clear change");
    assert_eq!(board.groups[1].rows[0].credit_xp, 20);
}

#[test]
fn names_fall_back_to_the_references_without_releases() {
    let (op, alice) = (signer("operator"), signer("alice"));
    let find = fixture::release(&op, "starter-find", "1", AT);
    let finder = fixture::release(&op, "code-finder", "0.3.0", AT);
    let a = fixture::result(&alice, &spec(&find, &finder, 2, 1), AT);
    let publications = verified([&a]);
    let board = board(&publications, &Names::default(), &BTreeMap::new(), "");
    assert_eq!(
        board.groups[0].test_set,
        format!("test set {}", short(&find.id))
    );
    assert_eq!(board.groups[0].rows[0].tool, "code-finder");
    assert_eq!(tool_name("abc:project-map/map"), "project-map/map");
    assert_eq!(tool_name("abc:code-finder/code-finder"), "code-finder");
}

#[test]
fn forged_and_foreign_events_never_reach_the_board() {
    let (op, alice) = (signer("operator"), signer("alice"));
    let find = fixture::release(&op, "starter-find", "1", AT);
    let finder = fixture::release(&op, "code-finder", "0.3.0", AT);
    let good = fixture::result(&alice, &spec(&find, &finder, 2, 1), AT);
    let mut forged = good.clone();
    forged.created_at += 1;
    let mut unmarked = good.clone();
    unmarked.tags.retain(|t| t.value() != Some(MARKER));
    // A release signed by someone other than its package root.
    let mut stolen = find.clone();
    stolen.pubkey = alice.pubkey().to_owned();
    assert!(verified([&forged, &unmarked]).is_empty());
    assert!(release(&stolen).is_none());
    assert!(release(&good).is_none());
    assert_eq!(verified([&good]).len(), 1);
}

#[test]
fn a_result_without_a_baseline_says_so() {
    let (op, alice) = (signer("operator"), signer("alice"));
    let find = fixture::release(&op, "starter-find", "1", AT);
    let finder = fixture::release(&op, "code-finder", "0.3.0", AT);
    let a = fixture::result(
        &alice,
        &Spec {
            without: None,
            ..spec(&find, &finder, 3, 0)
        },
        AT,
    );
    let p = verified([&a]);
    let p = p.values().next().unwrap();
    assert_eq!(headline(p), "3 of 8 passed with it, not run without it");
    assert_eq!(verdict_words(p.verdict()), "No clear change");
}

/// The hosted runner's sample plugins never stand on the board: a subject
/// signed by the runner's key or the starter catalog's is a sample.
#[test]
fn sample_plugin_subjects_are_left_off_the_board() {
    assert!(is_sample(&format!(
        "{}:project-map/project-map",
        eval_ext::hosted::RUNNER
    )));
    assert!(is_sample(&format!(
        "{}:openagents/repo-map",
        "a".repeat(64)
    )));
    assert!(!is_sample(&format!("{}:my-tool/skill", "1".repeat(64))));
    assert!(!is_sample("no-key"));
}
