//! The bare world's presence through the real scene over a loopback relay:
//! a remote player's avatar appears and walks, the local player's movement
//! reaches the peer at the mobile cadence, and pausing stops publishing. The
//! relay is a local fixture, not the production relay.
use super::{Config, PointerPhase, Scene};
use std::time::{Duration, Instant};
use verse::controller::PlayerController;
use verse::session::{BARE_WORLD, Session, Status};

#[path = "../../verse/tests/support/loopback_relay.rs"]
mod loopback_relay;

fn bare_scene(relay: &str) -> Scene {
    let mut scene = Scene::new(Config {
        world_offline: true,
        ..crate::verse_ffi::bare_config(800, 1200, 2.0, false, None)
    })
    .unwrap();
    // The loopback fixture speaks plain `ws://`, which the production relay
    // policy refuses; select it directly. No saved position to restore.
    scene.relay = Some(relay.to_owned());
    scene.restore_spawn = false;
    scene
}

/// Advances the scene and the peer on real time until `done` or `limit`.
fn run(
    scene: &mut Scene,
    peer: &mut Session,
    peer_player: &mut PlayerController,
    clock: Instant,
    limit: Duration,
    mut done: impl FnMut(&mut Scene, &mut Session, &mut PlayerController) -> bool,
) -> bool {
    let start = Instant::now();
    let agent = verse::agent::Agent::new(peer_player);
    while start.elapsed() < limit {
        scene.update(clock.elapsed().as_secs_f64()).unwrap();
        peer.tick(Instant::now(), peer_player, &agent);
        if done(scene, peer, peer_player) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(16));
    }
    false
}

#[test]
fn bare_world_players_see_each_other_move_and_pausing_stops_publishing() {
    let relay = loopback_relay::LoopbackRelay::start();
    let clock = Instant::now();
    let mut scene = bare_scene(&relay.url);
    scene.activate(true).unwrap();
    let session = scene.session.as_ref().expect("an active bare world joins");
    assert_eq!(session.world(), BARE_WORLD);
    let me = scene.public_key.clone();
    let identity =
        verse::identity::Identity::from_secret("peer", verse::identity::random_secret()).unwrap();
    let mut peer = Session::start_presence(identity, &relay.url, BARE_WORLD).unwrap();
    let them = peer.pubkey().to_owned();
    let mut ahead = scene.world.player.pos;
    ahead.z += 4.0;
    let mut peer_player = PlayerController::new(ahead, 0.0);

    // The peer's avatar appears in the scene, live, and is drawn in white
    // and gray.
    let appeared = run(
        &mut scene,
        &mut peer,
        &mut peer_player,
        clock,
        Duration::from_secs(8),
        |scene, peer, _| {
            peer.status == Status::Online
                && scene
                    .session
                    .as_ref()
                    .is_some_and(|s| s.status == Status::Online)
                && scene.packet().live_remote_entities == 1
        },
    );
    assert!(appeared, "the peer's avatar never appeared");
    // The peer carries its key's first letters overhead.
    assert!(!scene.player_tags().vertices.is_empty(), "the peer has no tag");
    let entities = crate::verse_ffi::bare_entities(
        scene
            .session
            .as_mut()
            .unwrap()
            .crowd
            .mesh(Instant::now(), 1.0 / 60.0),
    );
    assert!(!entities.faces.is_empty() || !entities.lines.is_empty());
    assert!(
        entities
            .lines
            .iter()
            .chain(&entities.faces)
            .all(|v| v.color[0] == v.color[1] && v.color[1] == v.color[2])
    );
    assert!(entities.lit.is_empty() && entities.glow.is_empty() && entities.neon.is_none());

    // The peer walks for three seconds, publishing a pose each second, and
    // stops. The scene draws it walking continuously between those poses,
    // one mobile interval in the past, and finally where it stopped.
    peer.set_publish_intervals(verse::session::PublishIntervals {
        moving: Duration::from_secs(1),
        idle: Duration::from_secs(2),
        state: Duration::from_secs(30),
    })
    .unwrap();
    let origin = peer_player.pos;
    let mut target = origin;
    target.x += 6.0;
    let walk = Instant::now();
    let mut seen = Vec::new();
    let arrived = run(
        &mut scene,
        &mut peer,
        &mut peer_player,
        clock,
        Duration::from_secs(12),
        |scene, _, walker| {
            let t = (walk.elapsed().as_secs_f32() / 3.0).min(1.0);
            walker.pos = origin.lerp(target, t);
            walker.speed = if t < 1.0 { 2.0 } else { 0.0 };
            let shown = scene.session.as_ref().unwrap().crowd.shown(Instant::now());
            let Some(avatar) = shown.iter().find(|e| e.pubkey == them && e.id == "avatar") else {
                return false;
            };
            seen.push(avatar.pos.x);
            avatar.pos.distance(target) < 0.05
        },
    );
    assert!(arrived, "the scene never drew the peer where it stopped");
    let largest_step = seen
        .windows(2)
        .map(|pair| (pair[1] - pair[0]).abs())
        .fold(0.0_f32, f32::max);
    assert!(
        seen.iter()
            .any(|x| *x > origin.x + 1.0 && *x < target.x - 1.0)
            && largest_step < 0.5,
        "the avatar jumped instead of walking (largest step {largest_step} m)"
    );

    // The local player walks with the stick; the peer sees the scene's avatar
    // follow at the mobile cadence.
    let from = scene.world.player.pos;
    let [sx, sy] = scene.stick_center();
    scene.pointer(9, PointerPhase::Down, sx, sy).unwrap();
    scene.pointer(9, PointerPhase::Move, sx, sy - 80.0).unwrap();
    run(
        &mut scene,
        &mut peer,
        &mut peer_player,
        clock,
        Duration::from_millis(700),
        |_, _, _| false,
    );
    scene.pointer(9, PointerPhase::Up, sx, sy - 80.0).unwrap();
    let walked_to = scene.world.player.pos;
    assert!(walked_to.distance(from) > 1.0);
    let followed = run(
        &mut scene,
        &mut peer,
        &mut peer_player,
        clock,
        Duration::from_secs(10),
        |_, peer, _| {
            peer.crowd
                .shown(Instant::now())
                .iter()
                .any(|e| e.pubkey == me && e.online && e.pos.distance(walked_to) < 0.05)
        },
    );
    assert!(followed, "the peer never saw the scene's avatar move");

    // The scene published only its avatar's presence in the bare world.
    let mine: Vec<_> = relay
        .published()
        .into_iter()
        .filter(|event| event.pubkey == me)
        .collect();
    assert!(mine.iter().all(|event| {
        matches!(event.kind, verse::mv::FRAME_KIND | verse::mv::STATE_KIND)
            && event.tag_values("w").eq([BARE_WORLD])
            && !event.content.contains("\"agent\"")
    }));

    // Pausing (the tab hidden or the app in the background) closes the
    // connection: nothing more is published and no remote geometry remains.
    scene.activate(false).unwrap();
    assert!(scene.session.is_none());
    assert_eq!(scene.packet().connection.state, "paused");
    std::thread::sleep(Duration::from_millis(300));
    let before = relay.published_by(&me, verse::mv::FRAME_KIND).len();
    run(
        &mut scene,
        &mut peer,
        &mut peer_player,
        clock,
        Duration::from_secs(6),
        |_, _, _| false,
    );
    assert_eq!(relay.published_by(&me, verse::mv::FRAME_KIND).len(), before);
    // Returning starts a fresh presence session.
    scene.activate(true).unwrap();
    assert!(scene.session.is_some());
}
