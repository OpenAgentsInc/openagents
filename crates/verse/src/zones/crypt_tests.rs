//! The crypt lab as a zone: entry, the walk over its solids, the vault,
//! the camera, and the door out.

use super::crypt::{self, HALF_X, HALF_Z, SPAWN};
use super::everglade_pack::{self, ZonePack};
use super::{Intent, ZoneId, atmosphere};
use crate::controller::{InputState, RADIUS};
use crate::runtime::{Action, WorldRuntime};
use glam::Vec3;
use std::f32::consts::{FRAC_PI_2, PI};
use std::path::Path;
use std::sync::OnceLock;

const DT: f32 = 0.02;

fn pack() -> &'static ZonePack {
    static PACK: OnceLock<ZonePack> = OnceLock::new();
    PACK.get_or_init(|| {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(everglade_pack::PACK_DIRECTORY)
            .join(format!(
                "{}.{}",
                everglade_pack::PACK_SHA256,
                everglade_pack::PACK_EXTENSION
            ));
        ZonePack::load_local(&path).expect("the committed pack loads")
    })
}

/// In the crypt, entered from the plaza.
fn entered() -> WorldRuntime {
    let mut runtime = WorldRuntime::new();
    runtime.install_crypt(pack());
    assert_eq!(
        runtime.zone,
        ZoneId::Crypt,
        "{:?}",
        runtime.zone_state.error
    );
    runtime
}

fn walk(runtime: &mut WorldRuntime, seconds: f32) {
    let forward = InputState {
        forward: true,
        ..InputState::default()
    };
    for _ in 0..(seconds / DT).round() as usize {
        runtime.tick(&forward, DT);
    }
}

#[test]
fn the_crypt_is_a_registered_zone_with_a_plaza_arch() {
    assert_eq!(ZoneId::from_name("crypt"), Some(ZoneId::Crypt));
    assert_eq!(ZoneId::from_name("verse-crypt"), Some(ZoneId::Crypt));
    assert_eq!(ZoneId::Crypt.sign(), "CRYPT");
    atmosphere(ZoneId::Crypt).validate().unwrap();
    assert_eq!(ZoneId::Crypt.portals(), vec![(ZoneId::Plaza, crypt::DOOR)]);
    // The desktop build carries the models, so the plaza has the arch.
    assert!(crypt::EMBEDDED);
    let arch = ZoneId::Plaza
        .portals()
        .into_iter()
        .find(|(zone, _)| *zone == ZoneId::Crypt)
        .expect("the plaza's crypt arch")
        .1;
    let mut runtime = WorldRuntime::new();
    runtime.set_spawn(arch - Vec3::Z * 2.0, 0.0).unwrap();
    let snapshot = runtime.zone_snapshot(1.0);
    assert!(snapshot.portal.near);
    assert!(
        snapshot
            .controls
            .iter()
            .any(|c| c.action == Intent::Enter && c.label == "Enter Crypt")
    );
    // The arch draws no amber geometry inside the crypt itself.
    assert!(super::portal_mesh(ZoneId::Crypt, 0.0).lines.is_empty());
}

#[test]
fn the_crypt_installs_at_the_door_facing_the_hall() {
    let runtime = entered();
    assert_eq!(runtime.player.pos, SPAWN);
    assert!(runtime.player.forward().z < -0.99, "faces the hall");
    let scene = runtime.world.mesh.textured.as_ref().expect("the hall");
    assert_eq!(scene.placements.len(), crypt::LAYOUT.len());
    assert!(!runtime.world.blockers.is_empty());
    assert!(runtime.everglade_hotbar().is_some(), "Everglade's bar");
    assert!(runtime.first_person_allowed());
    assert!(!runtime.companion_present());
    let mesh = runtime.dynamic_mesh();
    let neon = mesh.neon.expect("the hall's stage");
    assert!(neon.key.is_some());
    assert!(neon.lamps.iter().filter(|l| l.intensity > 0.0).count() > 8);
    assert!(mesh.figure.is_some(), "the player's character");
    assert!(!mesh.sprites.is_empty(), "candle halos, steam, and fog");
    assert!(mesh.lines.is_empty(), "no arch at the door");
    let snapshot = runtime.zone_snapshot(1.6);
    assert_eq!(snapshot.id, ZoneId::Crypt);
    assert!(snapshot.caption.starts_with("Crypt"));
}

#[test]
fn walls_and_props_block_and_the_floor_carries() {
    let mut runtime = entered();
    walk(&mut runtime, 1.0);
    assert!(runtime.player.pos.z < SPAWN.z - 2.0, "walks into the hall");
    assert!(runtime.player.pos.y.abs() < 1e-4);
    // West, across the study, into the bookshelf's alcove wall.
    runtime
        .set_spawn(Vec3::new(0.0, 0.0, 0.0), -FRAC_PI_2)
        .unwrap();
    walk(&mut runtime, 6.0);
    assert!(runtime.player.pos.x > -HALF_X + RADIUS - 0.01);
    // Into the green cauldron.
    runtime
        .set_spawn(Vec3::new(-1.0, 0.0, -3.4), -FRAC_PI_2)
        .unwrap();
    walk(&mut runtime, 3.0);
    assert!(runtime.player.pos.x > -3.2, "{}", runtime.player.pos);
    // Back out through the door: it doesn't open by walking.
    runtime.set_spawn(SPAWN, 0.0).unwrap();
    walk(&mut runtime, 3.0);
    assert!(runtime.player.pos.z < HALF_Z - RADIUS + 0.01);
}

#[test]
fn the_dais_is_walkable() {
    let mut runtime = entered();
    runtime.set_spawn(Vec3::new(0.0, 0.0, -4.5), PI).unwrap();
    walk(&mut runtime, 4.0);
    assert!(
        (runtime.player.pos.y - 0.18).abs() < 0.01 && runtime.player.pos.z < -6.3,
        "on the dais's step before the sarcophagus: {}",
        runtime.player.pos
    );
}

#[test]
fn the_vault_holds_a_levitating_player_down() {
    let mut runtime = entered();
    runtime.set_spawn(Vec3::new(0.0, 0.0, 2.0), PI).unwrap();
    runtime.everglade_levitate(true).unwrap();
    for _ in 0..400 {
        runtime.tick(&InputState::default(), DT);
    }
    let top = crypt::feet_ceiling(runtime.player.pos.x);
    assert!(runtime.player.pos.y <= top + 1e-3, "{}", runtime.player.pos);
    assert!(runtime.player.pos.y > 3.0, "rose toward the vault");
}

#[test]
fn the_camera_stays_in_the_hall() {
    let mut runtime = entered();
    // At the spawn the camera stands behind the player, toward the door;
    // pulled far back, it would leave through the wall.
    runtime.apply(Action::Zoom { lines: -40.0 }).unwrap();
    runtime.tick(&InputState::default(), DT);
    let eye = runtime.view(1.6).eye;
    assert!(eye.z < HALF_Z && eye.x.abs() < HALF_X, "{eye}");
    assert!(eye.y < crypt::vault_height(eye.x), "{eye}");
    // Zoomed all the way in, the camera is the player's eyes.
    for _ in 0..4 {
        runtime.apply(Action::Zoom { lines: 40.0 }).unwrap();
        runtime.tick(&InputState::default(), 0.5);
    }
    assert!(runtime.first_person() && runtime.hides_avatar());
}

#[test]
fn the_door_leaves_for_the_plaza() {
    let mut plaza = WorldRuntime::new();
    let before = plaza.player.pos;
    plaza.install_crypt(pack());
    // Not from the middle of the hall.
    plaza.set_spawn(Vec3::new(0.0, 0.0, 0.0), 0.0).unwrap();
    assert!(plaza.zone_intent(Intent::Interact).is_err());
    assert_eq!(plaza.zone, ZoneId::Crypt);
    // At the door, the interact control opens it.
    plaza
        .set_spawn(Vec3::new(0.0, 0.0, HALF_Z - 0.6), 0.0)
        .unwrap();
    assert!(plaza.crypt_door_near());
    let snapshot = plaza.zone_snapshot(1.6);
    assert!(snapshot.portal.near);
    assert!(snapshot.caption.contains("door"), "{}", snapshot.caption);
    plaza.zone_intent(Intent::Interact).unwrap();
    assert_eq!(plaza.zone, ZoneId::Plaza);
    assert_eq!(plaza.player.pos, before);
    assert!(plaza.zone_state.crypt.is_none() && plaza.zone_state.everglade.is_none());
}
