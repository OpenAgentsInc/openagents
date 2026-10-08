//! The Grid's EVALS board through real bare-world scenes over a loopback
//! relay: it reads published eval results only while the player is inside
//! the Gym, opens on a tap or **See the board**, and with **Compare notes**
//! on, the player's agent answers another trainer's agent standing in the
//! Gym. The relay and the results are local fixtures.
use super::bare_presence_tests::loopback_relay::LoopbackRelay;
use super::{Config, PointerPhase, Request, Scene, WorldTarget};
use nostr::domain::{Event, RelaySigner};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use verse::controller::PlayerController;
use verse::gym_evals::fixture::{self, Spec};
use verse::gym_hall::{Action, Hall};
use verse::net::{In, Link, Out};
use verse::session::{BARE_WORLD, Session};

fn bare_scene(relay: &str, evals_panel: bool) -> Box<Scene> {
    let mut scene = Scene::new(Config {
        world_offline: true,
        ..crate::verse_ffi::bare_config(800, 1200, 2.0, false, None)
    })
    .unwrap();
    // The loopback fixture speaks plain `ws://`; select it directly.
    scene.relay = Some(relay.to_owned());
    scene.evals_panel = evals_panel;
    scene
}

fn publish(relay: &str, events: &[&Event]) {
    let link = Link::start(relay);
    let (mut sent, mut left, start) = (false, events.len(), Instant::now());
    while start.elapsed() < Duration::from_secs(5) && (!sent || left > 0) {
        for message in link.drain() {
            match message {
                In::Connected if !sent => {
                    for event in events {
                        link.send(Out::Publish((*event).clone()));
                    }
                    sent = true;
                }
                In::Ok { accepted, .. } => {
                    assert!(accepted);
                    left -= 1;
                }
                _ => {}
            }
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(sent && left == 0, "the relay stored the fixtures");
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

struct Peer {
    session: Session,
    body: PlayerController,
    agent: verse::agent::Agent,
    hall: Hall,
}

/// Advances the scene and the peer on real time until `done` or `limit`.
fn run(
    scene: &mut Scene,
    peer: &mut Peer,
    clock: Instant,
    limit: Duration,
    mut done: impl FnMut(&mut Scene, &mut Peer) -> bool,
) -> bool {
    let start = Instant::now();
    while start.elapsed() < limit {
        scene.update(clock.elapsed().as_secs_f64()).unwrap();
        let now = Instant::now();
        peer.session.tick(now, &peer.body, &peer.agent);
        let site = verse::world::GymSite::GRID;
        peer.hall.set_peers(verse::gym_hall::peers_inside(
            site,
            &peer.session.crowd.shown(now),
        ));
        peer.hall.poll();
        if done(scene, peer) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(16));
    }
    false
}

#[test]
fn the_evals_board_reads_inside_and_the_agents_compare_notes() {
    let relay = LoopbackRelay::start();
    let clock = Instant::now();
    let mut scene = bare_scene(&relay.url, true);
    let me = RelaySigner::from_secret_hex(&scene.secret.display_secret().to_string()).unwrap();
    assert_eq!(me.pubkey(), scene.public_key);

    // Published results: ours and a peer's on the same test set.
    let bob = RelaySigner::from_secret_hex(&"33".repeat(32)).unwrap();
    let op = RelaySigner::from_secret_hex(&"44".repeat(32)).unwrap();
    let t = now();
    let find = fixture::release(&op, "starter-find", "1", t - 900);
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
    let ours = fixture::result(&me, &spec(&finder, 4, 4), t - 300);
    let theirs = fixture::result(&bob, &spec(&reader, 7, 4), t - 200);
    publish(&relay.url, &[&find, &finder, &reader, &ours, &theirs]);

    scene.activate(true).unwrap();
    scene.update(clock.elapsed().as_secs_f64()).unwrap();
    // Outside the Gym the board reads nothing and doesn't open.
    assert!(!scene.hall.as_ref().unwrap().active());
    assert!(scene.action(Request::InteractEvals).is_err());
    assert!(scene.evals_view().is_none());
    assert!(!scene.packet().evals_active);

    // A peer stands in the Gym, with Compare notes on.
    let site = verse::world::GymSite::GRID;
    let body = PlayerController::new(site.point(verse::world::GYM_CENTER), 0.0);
    let mut peer = Peer {
        session: Session::start_presence(
            verse::identity::Identity::from_secret("peer", "33".repeat(32).parse().unwrap())
                .unwrap(),
            &relay.url,
            BARE_WORLD,
        )
        .unwrap(),
        agent: verse::agent::Agent::new(&body),
        body,
        hall: Hall::new(
            verse::gym_hall::Config {
                relay: relay.url.clone(),
                world: BARE_WORLD.into(),
                signer: bob.clone(),
            },
            true,
        ),
    };
    peer.hall.set_active(true);

    // See the board: the player walks into the Gym before the EVALS board
    // and it opens.
    scene.action(Request::GoEvals).unwrap();
    assert!(scene.evals_open && !scene.results_open && !scene.gym_open);
    assert!(scene.world.evals(scene.aspect()).near);
    assert!(scene.hall.as_ref().unwrap().active());
    let ready = run(
        &mut scene,
        &mut peer,
        clock,
        Duration::from_secs(10),
        |scene, _| {
            scene.evals_view().is_some_and(|v| {
                v.state == "ready"
                    && v.board.results == 2
                    && v.board.groups[0].test_set == "starter-find 1"
            })
        },
    );
    assert!(ready, "the board never read the results");
    let view = scene.evals_view().unwrap();
    let rows = &view.board.groups[0].rows;
    assert_eq!(view.board.groups[0].test_set, "starter-find 1");
    assert!(
        rows.iter()
            .any(|r| r.id == ours.id && r.mine && r.verdict_words == "No clear change")
    );
    assert!(
        rows.iter()
            .any(|r| r.id == theirs.id && !r.mine && r.verdict_words == "Better")
    );
    assert!(!view.notes_on && !scene.packet().gym_notes);

    // Bob's agent opens once it sees us in the Gym; with Compare notes off,
    // our agent only listens.
    let opened = run(
        &mut scene,
        &mut peer,
        clock,
        Duration::from_secs(10),
        |scene, _| scene.evals_view().is_some_and(|v| v.notes.len() == 1),
    );
    assert!(opened, "the peer's note never arrived");
    run(
        &mut scene,
        &mut peer,
        clock,
        Duration::from_millis(1500),
        |_, _| false,
    );
    assert!(relay.published_by(&scene.public_key, 9).is_empty());

    // Switching it on, our agent answers with our result on that test set.
    scene
        .action(Request::Evals {
            command: Action::Notes { on: true },
        })
        .unwrap();
    assert!(scene.packet().gym_notes);
    let answered = run(
        &mut scene,
        &mut peer,
        clock,
        Duration::from_secs(10),
        |scene, peer| {
            scene.evals_view().is_some_and(|v| v.notes.len() == 2)
                && peer.hall.snapshot().is_some_and(|s| s.notes.len() == 2)
        },
    );
    assert!(answered, "our agent never answered");
    let notes = scene.evals_view().unwrap().notes;
    assert!(notes[0].mine && notes[0].answer);
    assert_eq!(
        notes[0].text,
        "We ran starter-find 1 too, with code-finder 0.3.0: 4 of 8 cases passed with it, 4 of 8 \
without. No clear change. test-reader 1.0.0 added more than code-finder 0.3.0 here (+3 cases \
against 0). Why did test-reader 1.0.0 help more on your run?"
    );
    assert_eq!(relay.published_by(&scene.public_key, 9).len(), 1);

    // Closing and tapping the board open it again; walking out closes it
    // and stops reading.
    scene.action(Request::CloseEvals).unwrap();
    assert!(!scene.evals_open);
    let evals = scene.world.evals(scene.aspect());
    let size = scene.lifecycle.viewport().logical_size();
    let [x, y] = [evals.screen_x * size[0], evals.screen_y * size[1]];
    assert_eq!(scene.world_target(x, y), Some(WorldTarget::Evals));
    let t = clock.elapsed().as_secs_f64();
    scene.pointer_at(4, PointerPhase::Down, x, y, t).unwrap();
    scene
        .pointer_at(4, PointerPhase::Up, x, y, t + 0.1)
        .unwrap();
    assert!(scene.evals_open);
    scene.world.set_spawn(verse::world::SPAWN, 0.0).unwrap();
    scene.update(clock.elapsed().as_secs_f64()).unwrap();
    assert!(!scene.evals_open && !scene.hall.as_ref().unwrap().active());
}

#[test]
fn without_the_panel_the_evals_board_never_opens() {
    let relay = LoopbackRelay::start();
    let mut scene = bare_scene(&relay.url, false);
    scene.activate(true).unwrap();
    scene.update(1.0).unwrap();
    assert!(scene.action(Request::GoEvals).is_err());
    let site = scene.world.gym_site().unwrap();
    scene
        .world
        .set_spawn(
            site.point(verse::world::GYM_EVALS_STAND),
            site.yaw_of(std::f32::consts::FRAC_PI_2),
        )
        .unwrap();
    scene.update(1.1).unwrap();
    assert!(!scene.hall.as_ref().unwrap().active());
    let evals = scene.world.evals(scene.aspect());
    let size = scene.lifecycle.viewport().logical_size();
    assert_ne!(
        scene.world_target(evals.screen_x * size[0], evals.screen_y * size[1]),
        Some(WorldTarget::Evals)
    );
    assert!(scene.action(Request::InteractEvals).is_err());
    assert!(!scene.packet().evals_active);
}
