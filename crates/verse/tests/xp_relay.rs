//! The quest board and XP read a live relay: a throwaway completion is
//! published, and the board's reader thread derives its XP, title, and
//! quest row.
//!
//! Runs only when `VERSE_TEST_RELAY` names a relay on this machine, for
//! example after `scripts/kb-relay.sh`:
//!
//! ```sh
//! VERSE_TEST_RELAY=ws://127.0.0.1:7490 cargo test -p verse --test xp_relay
//! ```

use std::collections::BTreeSet;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use knowledge::xp::XpTrust;
use verse::net::{In, Link, Out};
use verse::xp::Board;
use verse::xp::fixture::Completion;

#[test]
fn the_board_derives_xp_from_a_live_relay() {
    let Ok(relay) = std::env::var("VERSE_TEST_RELAY") else {
        eprintln!("skipped: set VERSE_TEST_RELAY to run against a relay");
        return;
    };
    assert!(
        relay.starts_with("ws://127.0.0.1:") || relay.starts_with("ws://localhost:"),
        "fixture events stay on a relay on this machine"
    );
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("a clock")
        .as_secs();
    // Fresh keys per run, so an earlier run's quest is never "rewritten".
    let seed = (SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("a clock")
        .as_nanos()
        % 1_000_000_000) as u64
        * 3
        + 1_000;
    let c = Completion::new(seed, seed + 1, seed + 2, now - 10);
    let award = c.award(now);
    let label = Completion::label(&c.referee, &award, "beat-reference", now);

    let link = Link::start(&relay);
    let start = Instant::now();
    let mut waiting: BTreeSet<String> = BTreeSet::new();
    let mut sent = false;
    while start.elapsed() < Duration::from_secs(10) && (!sent || !waiting.is_empty()) {
        for message in link.drain() {
            match message {
                In::Connected if !sent => {
                    for event in [&c.quest, &c.entry, &c.evidence, &award, &label] {
                        waiting.insert(event.id.clone());
                        link.send(Out::Publish(event.clone()));
                    }
                    sent = true;
                }
                In::Ok {
                    id,
                    accepted,
                    message,
                } => {
                    assert!(accepted, "the relay refused {id}: {message}");
                    waiting.remove(&id);
                }
                _ => {}
            }
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(sent && waiting.is_empty(), "the relay stored the fixtures");

    let trust = XpTrust {
        referees: BTreeSet::from([c.referee.pubkey().to_owned()]),
        runners: BTreeSet::new(),
    };
    let mut board = Board::start_with(&relay, trust, None, None);
    board.settle(Duration::from_millis(1500), Duration::from_secs(15));
    let snap = board.snapshot.expect("a snapshot");
    assert_eq!(snap.totals.get(c.author.pubkey()), Some(&6));
    assert_eq!(snap.totals.get(c.runner.pubkey()), Some(&4));
    assert!(
        snap.titles
            .get(c.author.pubkey())
            .is_some_and(|t| t.contains("beat-reference"))
    );
    let row = snap
        .quests
        .iter()
        .find(|q| q.referee == c.referee.pubkey())
        .expect("the quest is on the board");
    assert!(row.trusted);
    assert_eq!((row.awards, row.counted), (1, 1));
}
