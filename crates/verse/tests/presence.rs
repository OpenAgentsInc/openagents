//! Presence-only sessions over a loopback relay: two players in the bare world
//! see each other's avatars move, publish nothing but avatar presence, stay
//! apart from the plaza world, and stop publishing once dropped.

#[path = "support/loopback_relay.rs"]
mod loopback_relay;

use std::time::{Duration, Instant};

use glam::Vec3;
use loopback_relay::LoopbackRelay;
use verse::agent::Agent;
use verse::controller::PlayerController;
use verse::identity::{self, Identity};
use verse::mv;
use verse::session::{BARE_WORLD, Session, Status, WORLD};

fn identity(name: &str) -> Identity {
    Identity::from_secret(name, identity::random_secret()).unwrap()
}

/// Ticks every session until `done` holds or the deadline passes.
fn run(
    sessions: &mut [(&mut Session, &mut PlayerController, &Agent)],
    deadline: Duration,
    mut step: impl FnMut(&mut [(&mut Session, &mut PlayerController, &Agent)]) -> bool,
) -> bool {
    let start = Instant::now();
    while start.elapsed() < deadline {
        let now = Instant::now();
        for (session, player, agent) in sessions.iter_mut() {
            session.tick(now, player, agent);
        }
        if step(sessions) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    false
}

#[test]
fn presence_round_trips_in_the_bare_world_only() {
    let relay = LoopbackRelay::start();
    let mut a = Session::start_presence(identity("a"), &relay.url, BARE_WORLD).unwrap();
    let mut b = Session::start_presence(identity("b"), &relay.url, BARE_WORLD).unwrap();
    let mut plaza = Session::start_with_identity(identity("plaza"), &relay.url).unwrap();
    assert_eq!((a.world(), plaza.world()), (BARE_WORLD, WORLD));
    let (a_key, b_key) = (a.pubkey().to_owned(), b.pubkey().to_owned());
    let mut pa = PlayerController::new(Vec3::new(2.0, 0.0, 2.0), 0.0);
    let mut pb = PlayerController::new(Vec3::new(-2.0, 0.0, 2.0), 0.0);
    let mut pp = PlayerController::new(Vec3::new(0.0, 0.0, -2.0), 0.0);
    let (ga, gb, gp) = (Agent::new(&pa), Agent::new(&pb), Agent::new(&pp));

    // Both come online and see each other's live avatar where it stands.
    let online = run(
        &mut [(&mut a, &mut pa, &ga), (&mut b, &mut pb, &gb)],
        Duration::from_secs(5),
        |s| {
            let now = Instant::now();
            let sees = |viewer: &Session, key: &str, at: Vec3| {
                viewer.crowd.shown(now).iter().any(|e| {
                    e.pubkey == key && e.id == "avatar" && e.online && e.pos.distance(at) < 0.1
                })
            };
            s.iter()
                .all(|(session, ..)| session.status == Status::Online)
                && sees(s[1].0, &a_key, Vec3::new(2.0, 0.0, 2.0))
                && sees(s[0].0, &b_key, Vec3::new(-2.0, 0.0, 2.0))
        },
    );
    assert!(online, "the two bare-world players never saw each other");

    // A moves; B sees the avatar follow it.
    let moved = run(
        &mut [(&mut a, &mut pa, &ga), (&mut b, &mut pb, &gb)],
        Duration::from_secs(5),
        |s| {
            s[0].1.pos = Vec3::new(6.0, 0.0, 9.0);
            s[0].1.speed = 1.0;
            s[1].0.crowd.shown(Instant::now()).iter().any(|e| {
                e.pubkey == a_key && e.online && e.pos.distance(Vec3::new(6.0, 0.0, 9.0)) < 0.1
            })
        },
    );
    assert!(moved, "B never saw A's avatar move");

    // A plaza player on the same relay sees neither, and neither sees it.
    run(
        &mut [(&mut plaza, &mut pp, &gp)],
        Duration::from_millis(500),
        |_| false,
    );
    let now = Instant::now();
    assert!(plaza.crowd.shown(now).is_empty());
    assert!(a.crowd.shown(now).iter().all(|e| e.pubkey == b_key));

    // Presence carries the avatar alone: no agent, name, profile, chat,
    // gesture, or other event kind.
    for key in [&a_key, &b_key] {
        let events: Vec<_> = relay
            .published()
            .into_iter()
            .filter(|event| &event.pubkey == key)
            .collect();
        assert!(!events.is_empty());
        for event in &events {
            assert!(
                matches!(event.kind, mv::FRAME_KIND | mv::STATE_KIND),
                "unexpected kind {}",
                event.kind
            );
            assert_eq!(event.tag_values("w").collect::<Vec<_>>(), [BARE_WORLD]);
            let content: serde_json::Value = serde_json::from_str(&event.content).unwrap();
            if event.kind == mv::FRAME_KIND {
                let entities = content["e"].as_array().unwrap();
                assert!(entities.iter().all(|e| e["id"] == "avatar"));
            } else {
                assert_eq!(content["id"], "avatar");
                assert!(content.get("name").is_none());
            }
        }
    }

    // Dropping a session (the tab pausing) stops its publishing.
    drop(a);
    std::thread::sleep(Duration::from_millis(300));
    let before = relay.published_by(&a_key, mv::FRAME_KIND).len();
    run(
        &mut [(&mut b, &mut pb, &gb)],
        Duration::from_secs(1),
        |_| false,
    );
    assert_eq!(relay.published_by(&a_key, mv::FRAME_KIND).len(), before);
    assert!(!relay.published_by(&b_key, mv::FRAME_KIND).is_empty());
}
