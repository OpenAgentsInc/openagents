//! The Grove's rules against the pinned pack: the field, the hotbar, and
//! each spell's effect on a dummy.

use super::draw::Effect;
use super::dummies::{Dummy, FIELD, Kind, POST, RESET_AFTER};
use super::hotbar;
use super::kit::{REPEAT, Spell};
use super::{Grove, MAX_EFFECTS, MAX_FLOATERS, SPAWN};
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
fn the_grove_opens_with_its_hotbar_and_no_cooldowns() {
    let runtime = entered();
    assert!(runtime.everglade_hotbar().is_none());
    let bar = runtime.grove_bar().expect("the Grove's bar");
    assert!(bar.slots.iter().all(|s| s.cooldown == 0.0));
    assert_eq!(bar.spells, hotbar::ROW);
    assert_eq!(hotbar::key('1'), Some(Intent::GroveSlot(0)));
    assert_eq!(hotbar::key('='), Some(Intent::GroveSlot(11)));
    assert_eq!(
        grove(&runtime).resolve(Intent::GroveSlot(0)),
        Some(Spell::Thunderwave)
    );
    assert_eq!(
        grove(&runtime).resolve(Intent::GroveSlot(11)),
        Some(Spell::LongRest)
    );
    for spell in Spell::ALL {
        let sprite = hotbar::info(spell).0;
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
fn long_rest_ends_the_spells_and_stands_the_dummies_back_up() {
    let mut runtime = entered();
    face(&mut runtime, STRAW, 2.5);
    runtime.zone_intent(Intent::Thunderwave).unwrap();
    runtime.zone_intent(Intent::ReverseGravity).unwrap();
    idle(&mut runtime, 0.5);
    runtime.zone_intent(Intent::LongRest).unwrap();
    assert!(!runtime.grove_bar().unwrap().slots[4].active);
    let grove = grove(&runtime);
    assert!(
        grove
            .dummies
            .iter()
            .all(|d| d.pos == d.home && d.hp == d.kind.max_hp())
    );
    runtime.zone_intent(Intent::Thunderwave).unwrap();
}

#[test]
fn every_press_casts_at_once_with_no_cooldown_or_mana_in_the_way() {
    let mut runtime = entered();
    face(&mut runtime, STRAW, 8.0);
    // Mashed within one frame: every press of every spell casts.
    for _ in 0..12 {
        for (index, spell) in hotbar::ROW.into_iter().enumerate() {
            if !spell.spell() {
                continue;
            }
            runtime
                .zone_intent(Intent::GroveSlot(index as u8))
                .unwrap_or_else(|e| panic!("{spell:?}: {e}"));
            // Misty Step moved the druid; stand back for the next round.
            if spell == Spell::MistyStep {
                face(&mut runtime, STRAW, 8.0);
            }
        }
        // A few milliseconds apart, with the dummies stood back up so
        // each round has its target.
        runtime.tick(&InputState::default(), 0.003);
        grove_mut(&mut runtime)
            .dummies
            .iter_mut()
            .for_each(Dummy::reset);
    }
    let waves = |runtime: &WorldRuntime| {
        grove(runtime)
            .effects()
            .iter()
            .filter(|e| matches!(e, Effect::Wave { .. }))
            .count()
    };
    assert!(waves(&runtime) >= 6);
    // Twenty Thunderwaves in a row, each pressed the same instant.
    let mut runtime = entered();
    face(&mut runtime, STRAW, 2.5);
    for _ in 0..20 {
        runtime.zone_intent(Intent::Thunderwave).unwrap();
    }
    assert_eq!(
        waves(&runtime),
        Effect::Wave {
            origin: Vec3::ZERO,
            forward: Vec3::Z,
            start: 0.0
        }
        .cap()
    );
    // A concentration spell recasts at once, replacing the live one.
    for _ in 0..5 {
        runtime.zone_intent(Intent::WallOfStone).unwrap();
        assert!(runtime.grove_bar().unwrap().slots[3].active);
        runtime.zone_intent(Intent::ReverseGravity).unwrap();
        assert!(runtime.grove_bar().unwrap().slots[4].active);
        assert!(!runtime.grove_bar().unwrap().slots[3].active);
    }
}

#[test]
fn a_held_key_recasts_six_times_a_second_until_let_go() {
    let mut runtime = entered();
    face(&mut runtime, STRAW, 8.0);
    let bolts = |runtime: &WorldRuntime| {
        grove(runtime)
            .effects()
            .iter()
            .filter(|e| matches!(e, Effect::Bolt { .. }))
            .count()
    };
    assert!(runtime.grove_key(Intent::Firebolt, true).unwrap());
    assert_eq!(bolts(&runtime), 1, "the press casts at once");
    // Held for one second: the press and six repeats.
    let mut casts = 1;
    let mut before = bolts(&runtime);
    for _ in 0..60 {
        runtime.tick(&InputState::default(), 1.0 / 60.0);
        let now = bolts(&runtime);
        // A bolt lands in about a quarter second, so count only new ones.
        if now > before {
            casts += now - before;
        }
        before = now;
    }
    let expected = (1.0 / REPEAT).round() as usize + 1;
    assert!((casts as i32 - expected as i32).abs() <= 1, "{casts}");
    runtime.grove_key(Intent::Firebolt, false).unwrap();
    idle(&mut runtime, 1.0);
    assert_eq!(bolts(&runtime), 0, "let go, it stops");
    // Mashing: each press casts, however fast.
    for _ in 0..10 {
        runtime.grove_key(Intent::Thunderwave, true).unwrap();
        runtime.grove_key(Intent::Thunderwave, false).unwrap();
    }
    let waves = grove(&runtime)
        .effects()
        .iter()
        .filter(|e| matches!(e, Effect::Wave { .. }))
        .count();
    assert_eq!(waves, 6, "ten presses, capped at the newest six");
    // Long Rest never repeats, and losing focus lets go.
    runtime.grove_key(Intent::Thunderwave, true).unwrap();
    runtime.grove_release();
    let start = grove(&runtime).effects().len();
    runtime.tick(&InputState::default(), 0.5);
    assert!(grove(&runtime).effects().len() <= start);
}

#[test]
fn spam_keeps_the_effects_and_numbers_bounded() {
    let mut runtime = entered();
    face(&mut runtime, STRAW, 8.0);
    for _ in 0..400 {
        for intent in [
            Intent::Thunderwave,
            Intent::Fireball,
            Intent::Firebolt,
            Intent::Web,
            Intent::GustOfWind,
        ] {
            let _ = runtime.zone_intent(intent);
        }
        runtime.tick(&InputState::default(), 0.004);
        let grove = grove(&runtime);
        assert!(grove.effects().len() <= MAX_EFFECTS);
        assert!(grove.floaters.len() <= MAX_FLOATERS);
    }
    // The frame's geometry stays bounded too.
    let mesh = runtime.dynamic_mesh();
    let blasts = super::thunder::GLOW_QUADS * 6 * 6;
    assert!(mesh.glow.len() <= blasts, "{}", mesh.glow.len());
    assert!(mesh.lines.len() < 60_000, "{}", mesh.lines.len());
}

#[test]
fn wild_shape_becomes_the_giant_spider_with_its_bite_and_web() {
    let mut runtime = entered();
    face(&mut runtime, STRAW, 2.5);
    let figure = |runtime: &WorldRuntime| runtime.dynamic_mesh().figure.expect("a figure");
    let size = figure(&runtime).vertices.len();
    // The beast's attacks refuse in the druid's own shape.
    assert_eq!(
        grove(&runtime).resolve(Intent::GroveSlot(0)),
        Some(Spell::Thunderwave)
    );
    assert!(
        runtime.zone_intent(Intent::GroveSlot(10)).is_err(),
        "no shape to drop"
    );
    runtime.zone_intent(Intent::GroveSlot(9)).unwrap();
    assert_eq!(
        grove(&runtime).form(),
        Some(super::shape::Form::GiantSpider)
    );
    assert_eq!(runtime.player.pace(), 1.25);
    let bar = runtime.grove_bar().unwrap();
    assert_eq!(bar.spells[0], Spell::Bite);
    assert_eq!(bar.spells[1], Spell::SpiderWeb);
    assert!(bar.slots[9].active, "the shape's slot is lit");
    // The spider draws where the druid stood: its vertices are in the
    // figure and near the player, and the druid's are folded away.
    idle(&mut runtime, 0.1);
    let posed = figure(&runtime);
    assert_eq!(posed.vertices.len(), size, "one figure for every shape");
    let near = posed
        .vertices
        .iter()
        .filter(|v| Vec3::from(v.pos).distance(runtime.player.pos) < 3.0)
        .count();
    assert!(near > 1000, "{near} spider vertices by the player");
    // Bites land piercing and poison.
    let mut poisoned = false;
    for _ in 0..20 {
        // The straw dummy falls before twenty bites; past that they refuse.
        let _ = runtime.zone_intent(Intent::GroveSlot(0));
        poisoned |= grove(&runtime).log.iter().any(|l| l.contains("poison"));
        idle(&mut runtime, 0.05);
    }
    let dummy = &grove(&runtime).dummies[STRAW];
    assert!(dummy.hp < dummy.kind.max_hp(), "twenty bites all missed");
    assert!(poisoned, "{:?}", grove(&runtime).log);
    // The web roots a dummy farther off.
    grove_mut(&mut runtime)
        .dummies
        .iter_mut()
        .for_each(Dummy::reset);
    face(&mut runtime, STRAW, 12.0);
    for _ in 0..10 {
        let _ = runtime.zone_intent(Intent::GroveSlot(1));
    }
    let now = grove(&runtime).time;
    assert!(
        grove(&runtime).dummies[STRAW].rooted(now),
        "ten webs all missed"
    );
    // Walking, the spider plays its walk and moves at its pace.
    let start = runtime.player.pos;
    let walk = InputState {
        forward: true,
        ..InputState::default()
    };
    for _ in 0..25 {
        runtime.tick(&walk, DT);
    }
    let speed = runtime.player.pos.distance(start) / (25.0 * DT);
    assert!(speed > crate::controller::RUN_SPEED * 1.15, "{speed}");
    // Return to Form brings the druid back at the druid's own pace.
    runtime.zone_intent(Intent::GroveSlot(10)).unwrap();
    assert_eq!(grove(&runtime).form(), None);
    assert_eq!(runtime.player.pace(), 1.0);
    assert_eq!(runtime.grove_bar().unwrap().spells[0], Spell::Thunderwave);
    // Long Rest and leaving the Grove end the shape too.
    runtime.zone_intent(Intent::GroveSlot(9)).unwrap();
    runtime.zone_intent(Intent::LongRest).unwrap();
    assert_eq!(grove(&runtime).form(), None);
    runtime.zone_intent(Intent::GroveSlot(9)).unwrap();
    runtime.zone_intent(Intent::Return).unwrap();
    assert_eq!(runtime.player.pace(), 1.0);
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
