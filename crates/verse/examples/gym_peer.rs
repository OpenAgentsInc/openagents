//! A headless trainer standing in the Grid's Gym, for simulator checks of
//! the EVALS board and the agents' notes against a relay on this machine.
//!
//! It joins the bare world's presence inside the Gym, switches on Compare
//! notes, and prints every note its hall shows. With `--publish`, it first
//! publishes labeled fixture results: two test sets, three tools, its own
//! result, a second fixture trainer's result, and a check of its result by
//! that trainer. Fixture events go only to a `ws://` relay on this machine.
//!
//! ```sh
//! scripts/verse-relay.sh    # or any local relay
//! cargo run -p verse --no-default-features --example gym_peer -- \
//!   --relay ws://127.0.0.1:7447 --publish --seconds 600
//! ```

use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use nostr::domain::{Event, RelaySigner};
use verse::agent::Agent;
use verse::controller::PlayerController;
use verse::gym_evals::fixture::{self, Spec};
use verse::gym_hall::{Config, Hall, peers_inside};
use verse::identity::Identity;
use verse::net::{In, Link, Out};
use verse::session::{BARE_WORLD, Session};
use verse::world::{GYM_CENTER, GymSite};

fn arg(name: &str) -> Option<String> {
    let args: Vec<String> = std::env::args().collect();
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1).cloned())
}

fn publish(relay: &str, events: &[&Event]) {
    let link = Link::start(relay);
    let (mut sent, mut left, start) = (false, events.len(), Instant::now());
    while start.elapsed() < Duration::from_secs(10) && (!sent || left > 0) {
        for message in link.drain() {
            match message {
                In::Connected if !sent => {
                    for event in events {
                        link.send(Out::Publish((*event).clone()));
                    }
                    sent = true;
                }
                In::Ok {
                    id,
                    accepted,
                    message,
                } => {
                    left = left.saturating_sub(1);
                    println!(
                        "published {} {}",
                        &id[..8],
                        if accepted { "ok" } else { &message }
                    );
                }
                _ => {}
            }
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn main() {
    let relay = arg("--relay").unwrap_or_else(|| "ws://127.0.0.1:7447".into());
    assert!(
        relay.starts_with("ws://127.0.0.1:") || relay.starts_with("ws://localhost:"),
        "fixture results stay on a relay on this machine"
    );
    let seconds: u64 = arg("--seconds").and_then(|s| s.parse().ok()).unwrap_or(300);
    let secret_hex = arg("--secret").unwrap_or_else(|| "5a".repeat(32));
    let secret: secp256k1::SecretKey = secret_hex.parse().expect("a 64-hex secret");
    let identity = Identity::from_secret("peer", secret).expect("a key");
    let me = identity.signer.clone();
    println!("peer {}", me.pubkey());

    if std::env::args().any(|a| a == "--publish") {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let op = RelaySigner::from_secret_hex(&"6b".repeat(32)).unwrap();
        let other = RelaySigner::from_secret_hex(&"7c".repeat(32)).unwrap();
        let find = fixture::release(&op, "starter-find", "1", now - 3600);
        let tests = fixture::release(&op, "starter-tests", "1", now - 3600);
        let finder = fixture::release(&op, "code-finder", "0.3.0", now - 3600);
        let mapper = fixture::release(&op, "project-map", "0.2.0", now - 3600);
        let reader = fixture::release(&op, "test-reader", "1.0.0", now - 3600);
        let spec = |suite, tool, with, without| Spec {
            suite,
            tool,
            with,
            without: Some(without),
            total: 8,
            lock: "fixture-lock",
            checks: None,
        };
        let mine = fixture::result(&me, &spec(&find, &finder, 6, 3), now - 1800);
        let theirs = fixture::result(&other, &spec(&find, &mapper, 5, 5), now - 1500);
        let check = fixture::result(
            &other,
            &Spec {
                checks: Some(&mine.id),
                ..spec(&find, &finder, 6, 3)
            },
            now - 1200,
        );
        let tests_result = fixture::result(&other, &spec(&tests, &reader, 7, 4), now - 900);
        publish(
            &relay,
            &[
                &find,
                &tests,
                &finder,
                &mapper,
                &reader,
                &mine,
                &theirs,
                &check,
                &tests_result,
            ],
        );
    }

    let mut session = Session::start_presence(
        Identity::from_secret("peer", secret).unwrap(),
        &relay,
        BARE_WORLD,
    )
    .expect("presence");
    let site = GymSite::GRID;
    let body = PlayerController::new(site.point(GYM_CENTER), 0.0);
    let agent = Agent::new(&body);
    let mut hall = Hall::new(
        Config {
            relay: relay.clone(),
            world: BARE_WORLD.into(),
            signer: me,
        },
        true,
    );
    hall.set_active(true);
    let start = Instant::now();
    let mut printed = 0;
    while start.elapsed() < Duration::from_secs(seconds) {
        let now = Instant::now();
        session.tick(now, &body, &agent);
        hall.set_peers(peers_inside(site, &session.crowd.shown(now)));
        hall.poll();
        if let Some(snapshot) = hall.snapshot()
            && snapshot.notes.len() != printed
        {
            printed = snapshot.notes.len();
            println!(
                "board: {} results, {} checks; notes:",
                snapshot.board.results, snapshot.board.checks
            );
            for note in snapshot.notes.iter().rev() {
                println!(
                    "  {}{}: {}",
                    note.author_tag,
                    if note.mine { " (us)" } else { "" },
                    note.text
                );
            }
        }
        std::thread::sleep(Duration::from_millis(16));
    }
}
