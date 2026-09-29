//! The Gym hall over a loopback relay with simulated players: the EVALS
//! board reads published results, and two players' agents standing in the
//! Grid's Gym compare notes, one opener and one answer, grounded in their
//! own results. A third player who didn't opt in reads the notes and says
//! nothing; a note that cites someone else's result shows nowhere.

#[path = "support/loopback_relay.rs"]
mod loopback_relay;

use std::collections::BTreeSet;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use glam::Vec3;
use loopback_relay::LoopbackRelay;
use nostr::domain::{Event, RelaySigner};
use verse::agent::Agent;
use verse::controller::PlayerController;
use verse::gym_evals::fixture::{self, Spec};
use verse::gym_hall::{Config, Hall, peers_inside};
use verse::gym_notes::{self, Plan};
use verse::identity::{self, Identity};
use verse::net::{In, Link, Out};
use verse::session::{BARE_WORLD, Session};
use verse::world::{GYM_CENTER, GymSite};

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn publish(relay: &str, events: &[&Event]) {
    let link = Link::start(relay);
    let mut waiting: BTreeSet<String> = BTreeSet::new();
    let (mut sent, start) = (false, Instant::now());
    while start.elapsed() < Duration::from_secs(5) && (!sent || !waiting.is_empty()) {
        for message in link.drain() {
            match message {
                In::Connected if !sent => {
                    for event in events {
                        waiting.insert(event.id.clone());
                        link.send(Out::Publish((*event).clone()));
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
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(sent && waiting.is_empty(), "the relay stored the fixtures");
}

struct Player {
    identity: Identity,
    session: Session,
    body: PlayerController,
    agent: Agent,
    hall: Hall,
}

impl Player {
    fn new(relay: &str, at: Vec3, opted_in: bool) -> Self {
        let secret = identity::random_secret();
        let identity = Identity::from_secret("p", secret).unwrap();
        let session = Session::start_presence(
            Identity::from_secret("p", secret).unwrap(),
            relay,
            BARE_WORLD,
        )
        .unwrap();
        let body = PlayerController::new(at, 0.0);
        let agent = Agent::new(&body);
        let hall = Hall::new(
            Config {
                relay: relay.to_owned(),
                world: BARE_WORLD.to_owned(),
                signer: identity.signer.clone(),
            },
            opted_in,
        );
        Self {
            identity,
            session,
            body,
            agent,
            hall,
        }
    }

    fn key(&self) -> String {
        self.identity.signer.pubkey().to_owned()
    }
}

/// Ticks every player's presence and hall until `done` holds.
fn run(
    players: &mut [&mut Player],
    deadline: Duration,
    mut done: impl FnMut(&[&mut Player]) -> bool,
) -> bool {
    let start = Instant::now();
    while start.elapsed() < deadline {
        let now = Instant::now();
        for p in players.iter_mut() {
            p.session.tick(now, &p.body, &p.agent);
            let peers = peers_inside(GymSite::GRID, &p.session.crowd.shown(now));
            p.hall.set_peers(peers);
            p.hall.poll();
        }
        if done(players) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    false
}

fn notes(p: &Player) -> Vec<gym_notes::Shown> {
    p.hall
        .snapshot()
        .map(|s| s.notes.clone())
        .unwrap_or_default()
}

#[test]
fn two_agents_in_the_gym_compare_notes_over_the_relay() {
    let relay = LoopbackRelay::start();
    let site = GymSite::GRID;
    let inside = |dx: f32, dz: f32| site.point(GYM_CENTER + Vec3::new(dx, 0.0, dz));
    let mut alice = Player::new(&relay.url, inside(-2.0, 1.0), true);
    let mut bob = Player::new(&relay.url, inside(2.0, -1.0), true);
    let mut carol = Player::new(&relay.url, inside(0.0, 3.0), false);

    // Published results: Alice and Bob on the same test set with different
    // tools, and Bob on another test set.
    let t = now();
    let op = RelaySigner::from_secret_hex(&"11".repeat(32)).unwrap();
    let find = fixture::release(&op, "starter-find", "1", t - 900);
    let tests = fixture::release(&op, "starter-tests", "1", t - 900);
    let finder = fixture::release(&op, "code-finder", "0.3.0", t - 900);
    let reader = fixture::release(&op, "test-reader", "1.0.0", t - 900);
    let spec = |suite, tool, with, without| Spec {
        suite,
        tool,
        with,
        without: Some(without),
        total: 8,
        lock: "lock",
        checks: None,
    };
    let a = fixture::result(&alice.identity.signer, &spec(&find, &finder, 6, 3), t - 300);
    let b = fixture::result(&bob.identity.signer, &spec(&find, &reader, 5, 4), t - 400);
    let b2 = fixture::result(&bob.identity.signer, &spec(&tests, &reader, 2, 2), t - 200);
    publish(&relay.url, &[&find, &tests, &finder, &reader, &a, &b, &b2]);
    // Mallory claims Alice's result as his own.
    let mallory = RelaySigner::from_secret_hex(&"22".repeat(32)).unwrap();
    let forged = mallory.sign(
        t - 5,
        9,
        gym_notes::tags(BARE_WORLD, &relay.url, &Plan::Open { ours: a.id.clone() }),
        "We got 8 of 8.".into(),
    );
    publish(&relay.url, &[&forged]);

    // Everyone comes online and sees the other two inside the Gym.
    let (ka, kb, kc) = (alice.key(), bob.key(), carol.key());
    assert!(
        run(
            &mut [&mut alice, &mut bob, &mut carol],
            Duration::from_secs(8),
            |p| {
                let seen = |i: usize, keys: [&String; 2]| {
                    let shown = p[i].session.crowd.shown(Instant::now());
                    let here = peers_inside(GymSite::GRID, &shown);
                    keys.iter().all(|k| here.contains(*k))
                };
                seen(0, [&kb, &kc]) && seen(1, [&ka, &kc]) && seen(2, [&ka, &kb])
            }
        ),
        "the three players never saw each other in the Gym"
    );

    // Alice's agent opens first.
    alice.hall.set_active(true);
    assert!(
        run(
            &mut [&mut alice, &mut bob, &mut carol],
            Duration::from_secs(8),
            |p| {
                notes(p[0]).iter().any(|n| n.mine && !n.answer)
                    && p[0].hall.snapshot().is_some_and(|s| {
                        s.board.groups.len() == 2 && s.board.groups[0].test_set == "starter-tests 1"
                    })
            }
        ),
        "Alice's agent never opened"
    );
    let board = alice.hall.snapshot().unwrap().board.clone();
    assert_eq!(board.results, 3);
    assert_eq!(board.groups[0].test_set, "starter-tests 1");
    assert!(board.groups[1].rows.iter().any(|r| r.id == a.id && r.mine));

    // Bob's agent answers with his result on the same test set.
    bob.hall.set_active(true);
    carol.hall.set_active(true);
    let opener = "We tested code-finder 0.3.0 on starter-find 1: 6 of 8 cases passed with it, 3 of 8 \
without. It helped. Has anyone here run starter-find 1?";
    let answer = "We ran starter-find 1 too, with test-reader 1.0.0: 5 of 8 cases passed with it, 4 of \
8 without. It helped. code-finder 0.3.0 added more than test-reader 1.0.0 here (+3 cases against \
+1). Why did code-finder 0.3.0 help more on your run?";
    assert!(
        run(
            &mut [&mut alice, &mut bob, &mut carol],
            Duration::from_secs(10),
            |p| {
                p.iter().all(|player| {
                    let shown = notes(player);
                    shown.len() == 2 && shown[0].text == answer && shown[1].text == opener
                })
            }
        ),
        "the notes never reached all three players"
    );
    for player in [&alice, &bob, &carol] {
        let shown = notes(player);
        let texts: Vec<&str> = shown.iter().map(|n| n.text.as_str()).collect();
        assert_eq!(
            texts,
            [answer, opener],
            "newest first, the same on every phone"
        );
        assert_eq!(
            player.hall.snapshot().unwrap().held,
            1,
            "Mallory's note is held back"
        );
    }
    assert!(notes(&bob)[0].mine && notes(&alice)[1].mine);

    // Then they wait: no more notes, and Carol, who didn't opt in, never
    // spoke. Each note cites only its author's own result.
    run(
        &mut [&mut alice, &mut bob, &mut carol],
        Duration::from_secs(3),
        |_| false,
    );
    let spoken: Vec<Event> = relay
        .published()
        .into_iter()
        .filter(|e| e.kind == 9)
        .collect();
    assert_eq!(
        spoken.len(),
        3,
        "the forged note, one opener, and one answer"
    );
    assert!(relay.published_by(&kc, 9).is_empty());
    for event in spoken.iter().filter(|e| e.pubkey != mallory.pubkey()) {
        let note = gym_notes::parse(event, BARE_WORLD).unwrap();
        let own = if event.pubkey == ka { &a.id } else { &b.id };
        assert_eq!(note.sources, std::slice::from_ref(own));
        assert!(
            !event.content.contains("nsec") && event.content.chars().count() <= gym_notes::MAX_TEXT
        );
    }

    // Leaving the Gym stops the reader; the board shows offline.
    alice.hall.set_active(false);
    assert_eq!(alice.hall.view().state, "offline");
}

/// The same exchange through the production relay's code, when
/// `VERSE_TEST_RELAY` names one on this machine (for example
/// `scripts/verse-relay.sh`): its `#t`, `#w`, and `#z` filters, kind `9`
/// storage, and `ids` fetches.
///
/// ```sh
/// VERSE_TEST_RELAY=ws://127.0.0.1:7447 cargo test -p verse --test gym_hall
/// ```
#[test]
fn the_exchange_holds_on_a_live_local_relay() {
    let Ok(relay) = std::env::var("VERSE_TEST_RELAY") else {
        eprintln!("skipped: set VERSE_TEST_RELAY to run against a relay");
        return;
    };
    assert!(
        relay.starts_with("ws://127.0.0.1:") || relay.starts_with("ws://localhost:"),
        "fixture events stay on a relay on this machine"
    );
    let site = GymSite::GRID;
    let inside = |dx: f32, dz: f32| site.point(GYM_CENTER + Vec3::new(dx, 0.0, dz));
    let mut alice = Player::new(&relay, inside(-2.0, 1.0), true);
    let mut bob = Player::new(&relay, inside(2.0, -1.0), true);
    let t = now();
    // Fresh keys per run keep earlier runs' results apart.
    let op = RelaySigner::from_secret_hex(&format!("{:064x}", t as u128 * 7 + 1)).unwrap();
    let set = format!("live-set-{t}");
    let find = fixture::release(&op, &set, "1", t - 900);
    let finder = fixture::release(&op, "code-finder", "0.3.0", t - 900);
    let reader = fixture::release(&op, "test-reader", "1.0.0", t - 900);
    let spec = |tool, with, without| Spec {
        suite: &find,
        tool,
        with,
        without: Some(without),
        total: 8,
        lock: "lock",
        checks: None,
    };
    let a = fixture::result(&alice.identity.signer, &spec(&finder, 6, 3), t - 300);
    let b = fixture::result(&bob.identity.signer, &spec(&reader, 5, 4), t - 200);
    publish(&relay, &[&find, &finder, &reader, &a, &b]);
    alice.hall.set_active(true);
    assert!(
        run(&mut [&mut alice, &mut bob], Duration::from_secs(15), |p| {
            notes(p[0]).iter().any(|n| n.mine && !n.answer)
        }),
        "Alice's agent never opened"
    );
    bob.hall.set_active(true);
    // A persistent relay keeps earlier runs' notes; look at this run's.
    let (ka, kb) = (alice.key(), bob.key());
    assert!(
        run(&mut [&mut alice, &mut bob], Duration::from_secs(15), |p| {
            p.iter().all(|player| {
                let shown: Vec<_> = notes(player)
                    .into_iter()
                    .filter(|n| n.author == ka || n.author == kb)
                    .collect();
                shown.len() == 2
                    && shown[0].answer
                    && shown[0].text.starts_with(&format!("We ran {set} 1 too"))
            })
        }),
        "the answer never reached both players: {:?} / {:?}",
        alice
            .hall
            .snapshot()
            .map(|s| (s.notes.clone(), s.held, s.board.results)),
        bob.hall
            .snapshot()
            .map(|s| (s.notes.clone(), s.held, s.board.results))
    );
}
