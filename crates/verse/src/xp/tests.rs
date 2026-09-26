//! Level curve, snapshot mapping, and board text over throwaway fixtures.

use std::collections::BTreeSet;

use knowledge::xp::XpTrust;

use super::fixture::{Completion, signer};
use super::*;

const AT: u64 = 1_790_000_000;

fn trust(c: &Completion) -> XpTrust {
    XpTrust {
        referees: BTreeSet::from([c.referee.pubkey().to_owned()]),
        runners: BTreeSet::new(),
    }
}

fn base(c: &Completion) -> Vec<Event> {
    vec![c.quest.clone(), c.entry.clone(), c.evidence.clone()]
}

#[test]
fn the_level_curve_is_100_n_to_the_1_5() {
    assert_eq!(xp_to_reach(1), 0);
    assert_eq!(xp_to_reach(2), 100);
    assert_eq!(xp_to_reach(3), 283);
    assert_eq!(xp_to_reach(4), 520);
    assert_eq!(xp_to_reach(5), 800);
    assert_eq!(level_of(0), 1);
    assert_eq!(level_of(99), 1);
    assert_eq!(level_of(100), 2);
    assert_eq!(level_of(282), 2);
    assert_eq!(level_of(283), 3);
    assert_eq!(level_of(10_000), 22);
    let mut last = 0;
    for level in 1..200 {
        let need = xp_to_reach(level);
        assert!(need >= last, "cumulative XP never falls");
        assert_eq!(level_of(need), level);
        last = need;
    }
}

#[test]
fn a_counted_award_credits_xp_titles_and_its_quest_row() {
    let c = Completion::new(11, 12, 13, AT);
    let award = c.award(AT);
    let label = Completion::label(&c.referee, &award, "beat-reference", AT);
    let mut events = base(&c);
    // The same award from two relays counts once.
    events.extend([award.clone(), award, label]);
    let snap = snapshot(&events, &trust(&c));
    let author = c.author.pubkey().to_owned();
    let runner = c.runner.pubkey().to_owned();
    assert_eq!(snap.totals.get(&author), Some(&6));
    assert_eq!(snap.totals.get(&runner), Some(&4));
    assert_eq!(
        snap.xp_of(&[author.clone(), runner.clone(), author.clone()]),
        10
    );
    assert_eq!(snap.counted, 1);
    assert_eq!(
        snap.titles_of(std::slice::from_ref(&author)),
        BTreeSet::from(["beat-reference".to_owned()])
    );

    let [row] = snap.quests.as_slice() else {
        panic!("one quest row, got {:?}", snap.quests);
    };
    assert!(row.trusted && !row.conflict);
    assert_eq!(row.address, "tb4.fix-git.beat-fable-low@1");
    assert_eq!(row.task, "fix-git");
    assert_eq!(row.max_usd_per_run, Some(0.21));
    assert_eq!(
        row.split,
        vec![("author".to_owned(), 6), ("runner".to_owned(), 4)]
    );
    assert_eq!(row.total(), 10);
    assert_eq!((row.awards, row.counted), (1, 1));
    assert_eq!(row.titles, vec!["beat-reference".to_owned()]);
    let reference = row.reference.as_ref().expect("a reference");
    assert_eq!((reference.usd, reference.seconds), (Some(0.21), Some(312)));
}

#[test]
fn untrusted_referees_quests_show_but_their_awards_count_for_nothing() {
    let c = Completion::new(21, 22, 23, AT);
    let mut events = base(&c);
    events.push(c.award(AT));
    let snap = snapshot(&events, &XpTrust::default());
    assert!(snap.totals.is_empty());
    assert_eq!(snap.referees, 0);
    let row = &snap.quests[0];
    assert!(!row.trusted);
    assert_eq!((row.awards, row.counted), (1, 0));
}

#[test]
fn labels_need_the_awards_referee_and_a_counted_award() {
    let c = Completion::new(31, 32, 33, AT);
    let award = c.award(AT);
    let stranger = signer(99);
    let mut events = base(&c);
    events.push(award.clone());
    events.push(Completion::label(&stranger, &award, "fake-title", AT));
    let snap = snapshot(&events, &trust(&c));
    assert!(snap.titles.is_empty());

    // A revoked award takes its XP and its labels with it.
    let parts = xp::revocation(&award, "graded against the wrong verifier").unwrap();
    let revocation = c
        .referee
        .sign(AT + 1, parts.kind, parts.tags, parts.content);
    events.push(Completion::label(&c.referee, &award, "beat-reference", AT));
    events.push(revocation);
    let snap = snapshot(&events, &trust(&c));
    assert!(snap.totals.is_empty() && snap.titles.is_empty());
    assert_eq!(snap.revoked, 1);
    assert_eq!(snap.quests[0].counted, 0);
}

#[test]
fn missing_names_the_events_trusted_awards_point_at() {
    let c = Completion::new(41, 42, 43, AT);
    let award = c.award(AT);
    let mut have = BTreeMap::new();
    have.insert(award.id.clone(), award);
    let want = missing(&have, &trust(&c));
    assert_eq!(
        want,
        BTreeSet::from([
            c.quest.id.clone(),
            c.entry.id.clone(),
            c.evidence.id.clone()
        ])
    );
    assert!(missing(&have, &XpTrust::default()).is_empty());
    for e in base(&c) {
        have.insert(e.id.clone(), e);
    }
    assert!(missing(&have, &trust(&c)).is_empty());
}

#[test]
fn the_hud_shows_xp_level_titles_and_the_board() {
    let c = Completion::new(51, 52, 53, AT);
    let award = c.award(AT);
    let mut events = base(&c);
    events.push(Completion::label(&c.referee, &award, "beat-reference", AT));
    events.push(award);
    let board = Board::fixed("wss://relay.openagents.com", snapshot(&events, &trust(&c)));
    let mine = vec![c.author.pubkey().to_owned()];

    let lines = strip(Some(&board), &mine);
    assert_eq!(lines[0].0, "XP 6 · level 1 · 94 XP to level 2");
    assert_eq!(lines[1].0, "titles: beat-reference");
    assert!(lines[2].0.contains("1 quests") && lines[2].0.contains("relay.openagents.com"));

    let text: Vec<String> = board_lines(Some(&board), AT)
        .into_iter()
        .map(|l| l.0)
        .collect();
    let all = text.join("\n");
    assert!(all.contains("Beat Fable 5.1 low's cheapest winning run on fix-git"));
    assert!(all.contains("task fix-git · bar: pass at least 100% of runs, under $0.21 a run"));
    assert!(all.contains("reference: Fable 5.1 low, cheapest winning run · $0.21 · 5m 12s"));
    assert!(all.contains("award 10 XP (author 6, runner 4) · season 2026-q4, open 90 more days"));
    assert!(all.contains("1 award (1 counted)"));
    assert!(all.contains("achievements: beat-reference"));
    assert!(all.contains("can't be spent"));
    for word in ["buy", "spend XP", "balance", "sats"] {
        assert!(
            !all.contains(word),
            "the board never implies XP buys anything: {word}"
        );
    }

    assert_eq!(
        level_tag(board.snapshot.as_ref(), &mine).as_deref(),
        Some("lv 1")
    );
    assert_eq!(
        level_tag(board.snapshot.as_ref(), &["nobody".to_owned()]),
        None
    );
}

#[test]
fn offline_the_hud_says_so() {
    assert!(strip(None, &[])[0].0.contains("offline"));
    assert!(board_lines(None, AT)[0].0.contains("offline"));
    assert_eq!(
        host("wss://relay.openagents.com/path"),
        "relay.openagents.com"
    );
    assert_eq!(host("ws://127.0.0.1:7447"), "127.0.0.1:7447");
}
