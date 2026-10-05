//! The bare world's presence through the real scene over a loopback relay:
//! a remote player's avatar appears and walks, the local player's movement
//! reaches the peer at the mobile cadence, and pausing stops publishing. The
//! relay is a local fixture, not the production relay.
use super::{Config, PointerPhase, Scene};
use std::time::{Duration, Instant};
use verse::controller::PlayerController;
use verse::session::{BARE_WORLD, Session, Status};

#[path = "../../verse/tests/support/loopback_relay.rs"]
pub(crate) mod loopback_relay;

fn bare_scene(relay: &str) -> Scene {
    let mut scene = Scene::new(Config {
        world_offline: true,
        ..crate::verse_ffi::bare_config(800, 1200, 2.0, false, None)
    })
    .unwrap();
    // The loopback fixture speaks plain `ws://`, which the production relay
    // policy refuses; select it directly. No saved position to restore.
    scene.relay = Some(relay.to_owned());
    scene
}

#[test]
fn the_grid_starts_at_its_spawn_rather_than_a_saved_position() {
    let presence = crate::BarePresence {
        secret_hex: "11".repeat(32),
        relay: None,
        name: None,
    };
    let scene = Scene::new(crate::verse_ffi::bare_config(
        800,
        1200,
        2.0,
        false,
        Some(presence),
    ))
    .unwrap();
    assert!(scene.relay.is_some() && !scene.restore_spawn);
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
    // Both players carry their key's first eight characters overhead: six
    // vertices a glyph, eight glyphs a tag.
    assert_eq!(
        scene.player_tags().vertices.len(),
        2 * 8 * 6,
        "a tag is missing"
    );
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

    // The peer walks for three seconds, publishing poses at the Grid's
    // 5 Hz moving rate (#10582), and stops. The scene draws it walking
    // continuously between those poses, behind by the crowd's delay, and
    // finally where it stopped.
    peer.set_publish_intervals(verse::session::PublishIntervals {
        moving: Duration::from_millis(200),
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

/// Walking through the Grid's portal leaves `verse-bare` for a local
/// Lagrange 1 with a neutral panel; its button (or its arch) brings the
/// player back in front of the portal, and presence rejoins `verse-bare`.
#[test]
fn the_grid_portal_moves_presence_to_lagrange_1_and_the_return_rejoins() {
    let relay = loopback_relay::LoopbackRelay::start();
    let clock = Instant::now();
    let mut scene = bare_scene(&relay.url);
    scene.activate(true).unwrap();
    let me = scene.public_key.clone();
    let identity =
        verse::identity::Identity::from_secret("peer", verse::identity::random_secret()).unwrap();
    let mut peer = Session::start_presence(identity, &relay.url, BARE_WORLD).unwrap();
    let mut peer_player = PlayerController::new(scene.world.player.pos, 0.0);
    let joined = run(
        &mut scene,
        &mut peer,
        &mut peer_player,
        clock,
        Duration::from_secs(8),
        |scene, _, _| scene.packet().live_remote_entities == 1,
    );
    assert!(joined, "the peer never appeared");

    // The portal is hidden in the apps; this keeps its path working.
    scene.world.open_grid_portal_for_tests();
    // Face the portal from in front of it and hold the stick forward.
    let gate = scene.world.grid_gate().expect("the Grid's portal");
    let (front, away) = gate.front();
    scene
        .world
        .place_player(front, away + std::f32::consts::PI)
        .unwrap();
    let [sx, sy] = scene.stick_center();
    scene.pointer(1, PointerPhase::Down, sx, sy).unwrap();
    scene.pointer(1, PointerPhase::Move, sx, sy - 80.0).unwrap();
    let entered = run(
        &mut scene,
        &mut peer,
        &mut peer_player,
        clock,
        // Generous for a loaded machine: the loop ends at the crossing.
        Duration::from_secs(20),
        |scene, _, _| !scene.world.is_plaza(),
    );
    assert!(
        entered,
        "walking through the portal did not enter Lagrange 1"
    );
    assert_eq!(scene.world.zone, verse::zones::ZoneId::Lagrange1);
    // The held stick was released by the crossing; presence moved to the
    // zone's own shared world, where the Grid's peer is not.
    assert!(scene.touches.is_empty());
    scene.pointer(1, PointerPhase::Up, sx, sy - 80.0).unwrap();
    let zone_world = verse::zones::ZoneId::Lagrange1.world_id();
    assert_eq!(
        scene.session.as_ref().expect("zone presence").world(),
        zone_world
    );
    let packet = scene.packet();
    assert_eq!(packet.connection.label, "Lagrange 1");
    assert_eq!(packet.remote_entities, 0);

    // Inside a zone the app draws no zone panel.
    assert!(!scene.bare_zone_panel());
    // Another player in the zone's world appears; the Grid's peer does not.
    let identity =
        verse::identity::Identity::from_secret("zoned", verse::identity::random_secret()).unwrap();
    let mut zoned = Session::start_presence(identity, &relay.url, zone_world).unwrap();
    let mut zoned_player = PlayerController::new(scene.world.player.pos, 0.0);
    let met = run(
        &mut scene,
        &mut zoned,
        &mut zoned_player,
        clock,
        Duration::from_secs(8),
        |scene, _, _| scene.packet().live_remote_entities == 1,
    );
    assert!(met, "the zone's other player never appeared");
    assert!(
        relay
            .published()
            .iter()
            .filter(|event| event.pubkey == me)
            .any(|event| event
                .tags
                .iter()
                .any(|t| t.0.len() > 1 && t.0[0] == "w" && t.0[1] == zone_world)),
        "the player's poses never named the zone's world"
    );
    assert!(
        !zoned
            .crowd
            .shown(Instant::now())
            .iter()
            .any(|e| e.pubkey == peer.pubkey()),
        "the Grid's peer leaked into the zone"
    );

    // Leaving (as walking back through the zone's arch does) returns to
    // the Grid in front of the portal.
    scene.zone_intent(verse::zones::Intent::Return).unwrap();
    assert!(scene.world.is_plaza());
    assert_eq!(scene.world.player.pos, gate.front().0);
    let session = scene.session.as_ref().expect("presence rejoins");
    assert_eq!(session.world(), BARE_WORLD);
    let seen_back = run(
        &mut scene,
        &mut peer,
        &mut peer_player,
        clock,
        Duration::from_secs(10),
        |_, peer, _| {
            peer.crowd
                .shown(Instant::now())
                .iter()
                .any(|e| e.pubkey == me && e.online && e.pos.distance(front) < 0.05)
        },
    );
    assert!(seen_back, "the peer never saw the player back on the Grid");
}

/// Walking through the Grid's arch to Everglade loads the zone's pinned pack
/// with the zone panel's progress and Cancel on the Grid, enters Everglade
/// with presence in Everglade's shared world and movement controls drawn, and
/// leaving comes back in front of the arch, where presence rejoins `verse-bare`.
#[test]
fn the_everglade_arch_loads_its_pack_moves_presence_and_the_return_rejoins() {
    use verse::zones::everglade_pack::{PACK_DIRECTORY, PACK_EXTENSION, PACK_SHA256};
    use verse::zones::{Intent, ZoneId};
    let relay = loopback_relay::LoopbackRelay::start();
    // A cache that already holds the pinned pack, as after a first visit.
    let cache = tempfile::tempdir().unwrap();
    let name = format!("{PACK_SHA256}.{PACK_EXTENSION}");
    std::fs::copy(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(PACK_DIRECTORY)
            .join(&name),
        cache.path().join(&name),
    )
    .unwrap();
    let mut scene = Scene::new(Config {
        world_offline: true,
        ..crate::verse_ffi::bare_config_with_gym(
            800,
            1200,
            2.0,
            false,
            None,
            crate::BareGym {
                zone_cache_directory: Some(cache.path().to_string_lossy().into_owned()),
                ..crate::BareGym::default()
            },
        )
    })
    .unwrap();
    scene.relay = Some(relay.url.clone());
    scene.activate(true).unwrap();
    let clock = Instant::now();
    let mut frame = |scene: &mut Scene, limit: Duration, done: fn(&Scene) -> bool| {
        let start = Instant::now();
        while start.elapsed() < limit {
            scene.update(clock.elapsed().as_secs_f64()).unwrap();
            if done(scene) {
                return true;
            }
            std::thread::sleep(Duration::from_millis(16));
        }
        false
    };
    assert!(frame(&mut scene, Duration::from_secs(8), |s| s
        .session
        .is_some()));

    // The Lagrange 1 portal stays hidden; Everglade's arch stands.
    assert!(scene.world.grid_gate().is_none());
    let gate = scene.world.everglade_gate().expect("the Grid's arch");
    let (front, away) = gate.front();
    scene
        .world
        .place_player(front, away + std::f32::consts::PI)
        .unwrap();
    let [sx, sy] = scene.stick_center();
    scene.pointer(1, PointerPhase::Down, sx, sy).unwrap();
    scene.pointer(1, PointerPhase::Move, sx, sy - 80.0).unwrap();
    assert!(
        frame(&mut scene, Duration::from_secs(20), |s| s
            .world
            .zone_loading()
            || !s.world.is_plaza()),
        "walking through the arch did not start the load"
    );
    scene.pointer(1, PointerPhase::Up, sx, sy - 80.0).unwrap();
    if scene.world.is_plaza() {
        // Still on the Grid while the pack loads: presence pauses and the
        // panel shows progress and Cancel in the neutral palette.
        assert!(scene.session.is_none());
        let hud = scene.zone_hud_snapshot();
        assert!(hud.visible);
        assert!(hud.buttons.iter().any(|b| b.action == Intent::Cancel));
        let ui = scene.map_ui();
        assert!(ui.vertices.len() > scene.stick_ui().vertices.len());
    }
    assert!(
        frame(&mut scene, Duration::from_secs(180), |s| !s
            .world
            .is_plaza()),
        "the cached pack did not load: {:?}",
        scene.world.zone_snapshot(1.0).error
    );
    assert_eq!(scene.world.zone, ZoneId::Everglade);
    assert_eq!(
        scene.session.as_ref().expect("zone presence").world(),
        ZoneId::Everglade.world_id()
    );
    assert_eq!(scene.packet().connection.label, "Everglade");

    // At a station the app offers no studio panel until its host connects
    // the studio to a computer.
    let podium = verse::zones::everglade::STATIONS
        .iter()
        .find(|s| s.id == "podium")
        .unwrap()
        .at;
    scene
        .world
        .place_player([podium[0], 0.0, podium[1]].into(), 0.0)
        .unwrap();
    assert!(scene.world.studio_panel_here().is_some());
    // The icon hotbar shares touch routing with the held stick; no zone
    // panel is drawn inside Everglade.
    assert!(!scene.bare_zone_panel());
    assert!(scene.everglade_hotbar_shown());
    let size = scene.lifecycle.viewport().logical_size();
    let bottom = scene.hotbar_bottom();
    let [left, top, width, height] = verse::zones::everglade::hotbar::frame(size, bottom);
    let slot = verse::zones::everglade::hotbar::SLOTS
        .iter()
        .position(|(intent, ..)| *intent == Intent::Levitate)
        .unwrap();
    let step = width / verse::zones::everglade::hotbar::SLOTS.len() as f32;
    let (x, y) = (left + step * (slot as f32 + 0.5), top + height / 2.0);
    assert_eq!(
        verse::zones::everglade::hotbar::hit([x, y], size, bottom),
        Some(Intent::Levitate)
    );
    let [sx, sy] = scene.stick_center();
    scene.pointer(8, PointerPhase::Down, sx, sy).unwrap();
    scene.pointer(9, PointerPhase::Down, x, y).unwrap();
    scene.pointer(9, PointerPhase::Up, x, y).unwrap();
    assert!(scene.touches.contains_key(&8));
    assert!(scene.world.everglade_levitating());
    scene.pointer(8, PointerPhase::Up, sx, sy).unwrap();
    // The spells after Levitate cast from a tap too, with no cooldown:
    // Wall of Stone rises and its slot lights.
    let stone = verse::zones::everglade::hotbar::SLOTS
        .iter()
        .position(|(intent, ..)| *intent == Intent::WallOfStone)
        .unwrap();
    let x = left + step * (stone as f32 + 0.5);
    scene.pointer(10, PointerPhase::Down, x, y).unwrap();
    scene.pointer(10, PointerPhase::Up, x, y).unwrap();
    let slot = scene.world.everglade_hotbar().unwrap()[stone];
    assert!(slot.active && slot.enabled && slot.cooldown == 0.0);
    // A long press shows a slot's card and lifts without casting.
    let wind = verse::zones::everglade::hotbar::SLOTS
        .iter()
        .position(|(intent, ..)| *intent == Intent::WindWall)
        .unwrap();
    let x = left + step * (wind as f32 + 0.5);
    scene.pointer(11, PointerPhase::Down, x, y).unwrap();
    assert_eq!(scene.held_slot_tip(), None);
    scene.slot_touch.as_mut().unwrap().3 -= f64::from(verse::tooltip::LONG_PRESS);
    assert_eq!(scene.held_slot_tip(), Some(wind));
    scene.pointer(11, PointerPhase::Up, x, y).unwrap();
    assert_eq!(scene.held_slot_tip(), None);
    assert!(!scene.world.everglade_hotbar().unwrap()[wind].active);
    assert!(scene.world.everglade_hotbar().unwrap()[stone].active);
    assert!(scene.zone_intent(Intent::Interact).is_err());
    assert!(scene.studio.is_none());

    // Leaving (as walking back through THE GRID arch does) returns to the
    // Grid in front of the arch, and presence rejoins.
    scene.zone_intent(Intent::Return).unwrap();
    assert!(scene.world.is_plaza());
    assert_eq!(scene.world.player.pos, front);
    assert_eq!(
        scene.session.as_ref().expect("presence rejoins").world(),
        BARE_WORLD
    );
}
