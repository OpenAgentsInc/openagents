//! `openagents xp verify-card` end to end against a relay on this machine:
//! fixture awards, a trainer profile, and a signed card are published; the
//! built binary re-derives the card and exits 0, then refuses an inflated
//! card with exit 1.
//!
//! Runs only when `VERSE_TEST_RELAY` names a relay on this machine, for
//! example after `scripts/kb-relay.sh`:
//!
//! ```sh
//! VERSE_TEST_RELAY=ws://127.0.0.1:7490 cargo test -p openagents-cli --test verify_card
//! ```

use std::collections::BTreeSet;
use std::process::Command;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use nostr::domain::Event;
use nostr::xp;
use verse::net::{In, Link, Out};
use verse::xp::{XpTrust, fixture, snapshot, trainer_card};

fn publish(relay: &str, events: &[Event]) {
    let link = Link::start(relay);
    let start = Instant::now();
    let mut waiting: BTreeSet<String> = BTreeSet::new();
    let mut sent = false;
    while start.elapsed() < Duration::from_secs(10) && (!sent || !waiting.is_empty()) {
        for message in link.drain() {
            match message {
                In::Connected if !sent => {
                    for event in events {
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
}

fn verify(relay: &str, card: &Event, home: &std::path::Path) -> (i32, serde_json::Value) {
    let file = home.join("card.json");
    std::fs::write(&file, serde_json::to_string(card).unwrap()).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_openagents"))
        .args(["--json", "xp", "verify-card"])
        .arg(&file)
        .args(["--xp-relay", relay])
        .env("HOME", home)
        .output()
        .expect("openagents runs");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let value = serde_json::from_str(stdout.lines().last().unwrap_or("{}")).unwrap_or_else(|_| {
        panic!(
            "json output: {stdout} {}",
            String::from_utf8_lossy(&out.stderr)
        )
    });
    (out.status.code().unwrap_or(-1), value)
}

#[test]
fn a_card_verifies_against_the_relay_and_an_inflated_one_is_refused() {
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
        .unwrap()
        .as_secs();
    // Fresh keys per run.
    let seed = u64::from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .subsec_nanos(),
    ) + 7;
    let referee = fixture::signer(seed);
    let trainer = fixture::signer(seed + 1);
    let events = fixture::tutorial_events(&referee, &trainer, 2, now - 10);
    let trust = XpTrust {
        referees: BTreeSet::from([referee.pubkey().to_owned()]),
        runners: BTreeSet::new(),
    };
    let snap = snapshot(&events, &trust);
    let card = trainer_card(
        &snap,
        trainer.pubkey(),
        &trust,
        std::slice::from_ref(&relay),
        now,
    );
    let parts = xp::card(&card).unwrap();
    let signed = trainer.sign(now, parts.kind, parts.tags, parts.content);
    let mut all = events.clone();
    all.push(signed.clone());
    publish(&relay, &all);

    let home = tempfile::tempdir().unwrap();
    let (code, value) = verify(&relay, &signed, home.path());
    assert_eq!(code, 0, "{value}");
    assert_eq!(value["matches"], true);
    assert_eq!(value["derived"]["xp"], 100);

    let mut inflated = card.clone();
    inflated.xp = 5_000;
    inflated.level = 12;
    let parts = xp::card(&inflated).unwrap();
    let forged = trainer.sign(now + 1, parts.kind, parts.tags, parts.content);
    let (code, value) = verify(&relay, &forged, home.path());
    assert_eq!(code, 1, "{value}");
    assert_eq!(value["matches"], false);
    assert!(value["derived"]["differences"].to_string().contains("5000"));
}
