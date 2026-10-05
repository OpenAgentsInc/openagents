//! The Grove's rules against the pinned pack: the field, the hotbar, and
//! each spell's effect on a dummy.

use super::dummies::{Dummy, FIELD, Kind, POST, RESET_AFTER};
use super::hotbar;
use super::kit::{MAX_MANA, Spell};
use super::{Grove, SPAWN};
use crate::controller::InputState;
use crate::runtime::WorldRuntime;
use crate::zones::everglade_pack::{self, ZonePack};
use crate::zones::{Intent, ZoneId};
use glam::Vec3;
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

/// In the Grove, entered from the plaza.
fn entered() -> WorldRuntime {
    let mut runtime = WorldRuntime::new();
    runtime.install_grove(pack());
    assert_eq!(runtime.zone, ZoneId::Grove);
    runtime
}

fn grove(runtime: &WorldRuntime) -> &Grove {
    runtime.zone_state.grove.as_ref().expect("in the Grove")
}

fn grove_mut(runtime: &mut WorldRuntime) -> &mut Grove {
    runtime.zone_state.grove.as_mut().expect("in the Grove")
}

fn idle(runtime: &mut WorldRuntime, seconds: f32) {
    for _ in 0..(seconds / DT).round() as usize {
        runtime.tick(&InputState::default(), DT);
    }
}

/// Stands `back` meters short of dummy `i`, facing it.
fn face(runtime: &mut WorldRuntime, i: usize, back: f32) {
    let home = grove(runtime).dummies[i].home;
    let at = Vec3::new(home.x, 0.0, home.z - back);
    runtime.set_spawn(at, 0.0).unwrap();
}

/// The first straw dummy, 10 m ahead of the spawn.
const STRAW: usize = 0;

#[test]
fn the_field_stands_at_the_demo_distances() {
    let dummies = Dummy::field();
    assert_eq!(dummies.len(), 7);
    let distance = |i: usize| {
        let d = dummies[i].home - SPAWN;
        d.x.hypot(d.z)
    };
    for (i, wanted) in [(0, 10.0), (1, 20.0), (2, 30.0), (3, 15.0), (4, 25.0)] {
        assert!((distance(i) - wanted).abs() < 0.3, "{i}: {}", distance(i));
    }
    assert_eq!(FIELD[5].0, Kind::Big);
    let flying = &dummies[6];
    assert_eq!(flying.kind, Kind::Flying);
    assert!((flying.home.y - POST).abs() < 1e-3);
    // Every dummy stands inside the meadow, clear of the arch.
    for d in &dummies {
        assert!(d.home.x.hypot(d.home.z) < super::MEADOW_RADIUS);
    }
}

#[test]
fn the_grove_opens_with_its_hotbar_and_full_mana() {
    let runtime = entered();
    assert!(runtime.everglade_hotbar().is_none());
    let bar = runtime.grove_bar().expect("the Grove's bar");
    assert_eq!(bar.mana, MAX_MANA);
    assert_eq!(hotbar::key(1), Some(Intent::Thunderwave));
    assert_eq!(hotbar::key(0), Some(Intent::LongRest));
    assert_eq!(hotbar::SPRITES.len(), Spell::ALL.len());
    for sprite in hotbar::SPRITES {
        assert!(crate::imported::icons::icon(sprite).is_some(), "{sprite}");
    }
    assert!(runtime.zone_snapshot(1.0).caption.starts_with("Grove"));
    // The scene draws the dummies with the character in one figure.
    let mesh = runtime.dynamic_mesh();
    let figure = mesh.figure.expect("the character and the dummies");
    figure.validate().unwrap();
    assert!(!mesh.faces.is_empty(), "health bars and names");
}

#[test]
fn thunderwave_damages_and_pushes_a_dummy_that_fails_its_save() {
    let mut runtime = entered();
    face(&mut runtime, STRAW, 2.5);
    grove_mut(&mut runtime)
        .dice
        .force_save(STRAW as u64, 1)
        .unwrap();
    let before = grove(&runtime).dummies[STRAW].pos;
    runtime.zone_intent(Intent::Thunderwave).unwrap();
    idle(&mut runtime, 0.5);
    let dummy = &grove(&runtime).dummies[STRAW];
    assert!(dummy.hp < dummy.kind.max_hp());
    assert!(dummy.pos.distance(before) > 2.5, "{}", dummy.pos);
    assert!(grove(&runtime).kit.mana < MAX_MANA);
}

#[test]
fn gust_of_wind_pushes_a_dummy_down_its_line() {
    let mut runtime = entered();
    face(&mut runtime, STRAW, 6.0);
    grove_mut(&mut runtime)
        .dice
        .force_save(STRAW as u64, 1)
        .unwrap();
    let before = grove(&runtime).dummies[STRAW].pos;
    runtime.zone_intent(Intent::GustOfWind).unwrap();
    idle(&mut runtime, 0.5);
    let after = grove(&runtime).dummies[STRAW].pos;
    assert!(after.z - before.z > 4.0, "{before} to {after}");
}

#[test]
fn wind_wall_rises_through_the_target_and_lifts_it() {
    let mut runtime = entered();
    face(&mut runtime, STRAW, 7.0);
    grove_mut(&mut runtime)
        .dice
        .force_save(STRAW as u64, 1)
        .unwrap();
    runtime.zone_intent(Intent::WindWall).unwrap();
    let ground = grove(&runtime).dummies[STRAW].home.y;
    idle(&mut runtime, 0.3);
    let dummy = &grove(&runtime).dummies[STRAW];
    assert!(dummy.hp < dummy.kind.max_hp());
    assert!(dummy.pos.y > ground + 0.5, "{}", dummy.pos.y);
}

#[test]
fn wall_of_stone_rises_at_the_target_and_shoves_it_past_the_wall() {
    let mut runtime = entered();
    face(&mut runtime, STRAW, 8.0);
    let before = grove(&runtime).dummies[STRAW].pos;
    runtime.zone_intent(Intent::WallOfStone).unwrap();
    idle(&mut runtime, 0.5);
    let after = grove(&runtime).dummies[STRAW].pos;
    assert!(after.z > before.z + 0.5, "{before} to {after}");
    // It slides behind the wall, so walking on reaches the stone first.
    assert!(runtime.grove_bar().unwrap().slots[3].active);
}

#[test]
fn reverse_gravity_lifts_the_dummies_around_the_druid() {
    let mut runtime = entered();
    face(&mut runtime, STRAW, 5.0);
    runtime.zone_intent(Intent::ReverseGravity).unwrap();
    idle(&mut runtime, 2.0);
    let dummy = &grove(&runtime).dummies[STRAW];
    assert!(dummy.pos.y > dummy.home.y + 3.0, "{}", dummy.pos.y);
    // The flying target on its post stays put.
    let flying = &grove(&runtime).dummies[6];
    assert_eq!(flying.pos, flying.home);
}

#[test]
fn fire_bolt_rolls_attacks_that_land_damage() {
    let mut runtime = entered();
    face(&mut runtime, STRAW, 8.0);
    for _ in 0..6 {
        runtime.zone_intent(Intent::Firebolt).unwrap();
        idle(&mut runtime, 1.1);
    }
    let dummy = &grove(&runtime).dummies[STRAW];
    assert!(dummy.hp < dummy.kind.max_hp(), "six bolts all missed");
    // Fire Bolt costs no mana.
    assert!(grove(&runtime).kit.mana >= MAX_MANA - 1e-3);
    assert!(
        grove(&runtime)
            .log
            .iter()
            .any(|l| l.starts_with("Fire Bolt"))
    );
}

#[test]
fn fireball_burns_the_target_and_the_warded_dummy_resists_fire() {
    let mut runtime = entered();
    face(&mut runtime, STRAW, 10.0);
    grove_mut(&mut runtime)
        .dice
        .force_save(STRAW as u64, 1)
        .unwrap();
    runtime.zone_intent(Intent::Fireball).unwrap();
    idle(&mut runtime, 1.0);
    let dummy = &grove(&runtime).dummies[STRAW];
    // 8d6 is at least 8 on a failed save.
    assert!(dummy.hp <= dummy.kind.max_hp() - 8.0, "{}", dummy.hp);
    assert_eq!(Kind::Warded.multiplier(super::kit::Damage::Fire), 0.5);
    assert_eq!(Kind::Straw.multiplier(super::kit::Damage::Fire), 1.0);
}

#[test]
fn misty_step_blinks_toward_the_target() {
    let mut runtime = entered();
    face(&mut runtime, STRAW, 9.0);
    let start = runtime.player.pos;
    runtime.zone_intent(Intent::MistyStep).unwrap();
    let target = grove(&runtime).dummies[STRAW].pos;
    assert!(runtime.player.pos.distance(target) < start.distance(target) - 5.0);
}

#[test]
fn web_roots_a_dummy_so_pushes_no_longer_move_it() {
    let mut runtime = entered();
    face(&mut runtime, STRAW, 6.0);
    grove_mut(&mut runtime)
        .dice
        .force_save(STRAW as u64, 1)
        .unwrap();
    runtime.zone_intent(Intent::Web).unwrap();
    let now = grove(&runtime).time;
    assert!(grove(&runtime).dummies[STRAW].rooted(now));
    idle(&mut runtime, 1.1);
    let before = grove(&runtime).dummies[STRAW].pos;
    grove_mut(&mut runtime)
        .dice
        .force_save(STRAW as u64, 1)
        .unwrap();
    runtime.zone_intent(Intent::GustOfWind).unwrap();
    idle(&mut runtime, 0.5);
    assert_eq!(grove(&runtime).dummies[STRAW].pos, before);
}

#[test]
fn repeated_roots_diminish_and_the_third_is_immune() {
    let mut dummy = Dummy::new(Kind::Straw, [0.0, 0.0]);
    assert_eq!(dummy.root(12.0, 0.0), 12.0);
    assert_eq!(dummy.root(12.0, 1.0), 6.0);
    assert_eq!(dummy.root(12.0, 2.0), 0.0);
    // Past the 15-second window the full duration returns.
    assert_eq!(dummy.root(12.0, 20.0), 12.0);
}

#[test]
fn a_dummy_resets_after_ten_seconds_untouched() {
    let mut dummy = Dummy::new(Kind::Straw, [0.0, 0.0]);
    dummy.damage(30.0, super::kit::Damage::Fire, 0.0);
    dummy.push(Vec3::Z, 2.0, 0.0);
    let mut now = 0.0;
    while now < RESET_AFTER - 0.1 {
        now += DT;
        dummy.tick(DT, now, None);
    }
    assert!(dummy.hp < dummy.kind.max_hp());
    for _ in 0..10 {
        now += DT;
        dummy.tick(DT, now, None);
    }
    assert_eq!(dummy.hp, dummy.kind.max_hp());
    assert_eq!(dummy.pos, dummy.home);
}

#[test]
fn long_rest_refills_mana_cooldowns_and_the_dummies() {
    let mut runtime = entered();
    face(&mut runtime, STRAW, 2.5);
    runtime.zone_intent(Intent::Thunderwave).unwrap();
    idle(&mut runtime, 1.1);
    runtime.zone_intent(Intent::Fireball).unwrap_or_default();
    idle(&mut runtime, 0.5);
    assert!(grove(&runtime).kit.mana < MAX_MANA);
    assert!(
        runtime.zone_intent(Intent::Thunderwave).is_err(),
        "on cooldown"
    );
    runtime.zone_intent(Intent::LongRest).unwrap();
    let grove = grove(&runtime);
    assert_eq!(grove.kit.mana, MAX_MANA);
    assert!(
        grove
            .dummies
            .iter()
            .all(|d| d.pos == d.home && d.hp == d.kind.max_hp())
    );
    runtime.zone_intent(Intent::Thunderwave).unwrap();
}

#[test]
fn spells_outside_the_grove_are_refused_and_return_leaves_it() {
    let mut plaza = WorldRuntime::new();
    assert!(plaza.zone_intent(Intent::Thunderwave).is_err());
    let mut runtime = entered();
    runtime.zone_intent(Intent::Return).unwrap();
    assert_eq!(runtime.zone, ZoneId::Plaza);
    assert!(runtime.zone_state.grove.is_none());
    assert!(runtime.grove_bar().is_none());
}

#[test]
fn zone_names_include_the_grove() {
    assert_eq!(ZoneId::from_name("grove"), Some(ZoneId::Grove));
    assert_eq!(ZoneId::from_name("verse-grove"), Some(ZoneId::Grove));
}
