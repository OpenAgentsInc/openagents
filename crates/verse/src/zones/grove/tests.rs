//! The Grove's rules against the pinned pack: the field, the hotbar, and
//! each spell's effect on a dummy.

use super::draw::Effect;
use super::dummies::{Condition, Dummy, FIELD, Kind, POST, RESET_AFTER};
use super::kit::{Area, Land, REPEAT, Spell};
use super::slots;
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
/// Slots: Wall of Stone (Arid, Alt+0) and Reverse Gravity (Ctrl+=).
const STONE_SLOT: usize = 45;
const GRAVITY_SLOT: usize = 35;
/// Wild Shape: Giant Spider (4), Return to Form (5), and the beast's first
/// attack (Shift+1).
const SPIDER: u8 = 3;
const RETURN: u8 = 4;
const BITE: u8 = 12;

/// Casts the spell on slot `index`.
fn cast(runtime: &mut WorldRuntime, index: usize) -> Result<(), String> {
    runtime.zone_intent(Intent::GroveSlot(index as u8))
}

/// The slot `spell` sits on now.
fn slot(runtime: &WorldRuntime, spell: Spell) -> usize {
    let g = grove(runtime);
    slots::slot_of(spell, g.form(), g.land()).unwrap_or_else(|| panic!("{spell:?} is on no slot"))
}

/// Casts `spell` from its slot, failing the test on a refusal.
fn cast_spell(runtime: &mut WorldRuntime, spell: Spell) {
    let index = slot(runtime, spell);
    cast(runtime, index).unwrap_or_else(|e| panic!("{spell:?}: {e}"));
}

/// Makes dummy `i` fail its next saving throws.
fn fail_saves(runtime: &mut WorldRuntime, i: usize) {
    for _ in 0..4 {
        grove_mut(runtime).dice.force_save(i as u64, 1).unwrap();
    }
}

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
    // Four rows of twelve: 46 abilities and two empty slots.
    assert_eq!(bar.spells.iter().flatten().count(), 46);
    assert_eq!(super::hotbar::key('1', 0), Some(Intent::GroveSlot(0)));
    assert_eq!(super::hotbar::key('=', 3), Some(Intent::GroveSlot(47)));
    assert_eq!(
        grove(&runtime).resolve(Intent::GroveSlot(17)),
        Some(Spell::Thunderwave)
    );
    assert_eq!(
        grove(&runtime).resolve(Intent::GroveSlot(11)),
        Some(Spell::LongRest)
    );
    // The default land is Arid.
    assert_eq!(bar.spells[40], Some(Spell::FireBolt));
    assert_eq!(bar.spells[45], Some(Spell::WallOfStone));
    for spell in Spell::ALL {
        let sprite = slots::info(spell).0;
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
    assert!(runtime.grove_bar().unwrap().slots[STONE_SLOT].active);
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
    assert!(!runtime.grove_bar().unwrap().slots[GRAVITY_SLOT].active);
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
        for index in 0..slots::COUNT {
            let Some(spell) = grove(&runtime).slot_spell(index) else {
                continue;
            };
            // Melee reaches only 3.5 m; the rest cast from 8 m.
            let melee = spell.def().area == Area::Single && spell.def().range < 6.0;
            if !spell.repeats() || spell.beast() || melee {
                continue;
            }
            // Pushes and lifts don't carry the target out of reach.
            grove_mut(&mut runtime)
                .dummies
                .iter_mut()
                .for_each(Dummy::reset);
            runtime
                .zone_intent(Intent::GroveSlot(index as u8))
                .unwrap_or_else(|e| panic!("{spell:?}: {e}"));
            // Misty Step moved the druid; stand back for the next round.
            face(&mut runtime, STRAW, 8.0);
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
    // The newest blasts survive the field's cap on live effects.
    assert!(waves(&runtime) >= 1);
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
        assert!(runtime.grove_bar().unwrap().slots[STONE_SLOT].active);
        runtime.zone_intent(Intent::ReverseGravity).unwrap();
        assert!(runtime.grove_bar().unwrap().slots[GRAVITY_SLOT].active);
        assert!(!runtime.grove_bar().unwrap().slots[STONE_SLOT].active);
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
        grove(&runtime).resolve(Intent::GroveSlot(BITE)),
        Some(Spell::ProduceFlame)
    );
    assert!(
        runtime.zone_intent(Intent::GroveSlot(RETURN)).is_err(),
        "no shape to drop"
    );
    runtime.zone_intent(Intent::GroveSlot(SPIDER)).unwrap();
    assert_eq!(
        grove(&runtime).form(),
        Some(super::shape::Form::GiantSpider)
    );
    assert_eq!(runtime.player.pace(), 1.25);
    let bar = runtime.grove_bar().unwrap();
    assert_eq!(bar.spells[usize::from(BITE)], Some(Spell::SpiderBite));
    assert_eq!(bar.spells[usize::from(BITE) + 1], Some(Spell::SpiderWeb));
    assert!(
        bar.slots[usize::from(SPIDER)].active,
        "the shape's slot is lit"
    );
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
        let _ = runtime.zone_intent(Intent::GroveSlot(BITE));
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
        let _ = runtime.zone_intent(Intent::GroveSlot(BITE + 1));
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
    runtime.zone_intent(Intent::GroveSlot(RETURN)).unwrap();
    assert_eq!(grove(&runtime).form(), None);
    assert_eq!(runtime.player.pace(), 1.0);
    assert_eq!(
        runtime.grove_bar().unwrap().spells[usize::from(BITE)],
        Some(Spell::ProduceFlame)
    );
    // Long Rest and leaving the Grove end the shape too.
    runtime.zone_intent(Intent::GroveSlot(SPIDER)).unwrap();
    runtime.zone_intent(Intent::LongRest).unwrap();
    assert_eq!(grove(&runtime).form(), None);
    runtime.zone_intent(Intent::GroveSlot(SPIDER)).unwrap();
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

#[test]
fn choose_land_swaps_row_four_through_the_four_lands() {
    let mut runtime = entered();
    let choose = slot(&runtime, Spell::ChooseLand);
    for (land, first) in [
        (Land::Polar, Spell::RayOfFrost),
        (Land::Temperate, Spell::ShockingGrasp),
        (Land::Tropical, Spell::AcidSplash),
        (Land::Arid, Spell::FireBolt),
    ] {
        cast(&mut runtime, choose).unwrap();
        assert_eq!(grove(&runtime).land(), land);
        let bar = runtime.grove_bar().unwrap();
        assert_eq!(bar.spells[40], Some(first));
        assert_eq!(&bar.spells[40..46], &land.spells().map(Some)[..]);
        assert!(
            grove(&runtime)
                .log
                .last()
                .unwrap()
                .starts_with("Choose Land")
        );
    }
    // A named intent follows its spell to wherever its land puts it.
    cast(&mut runtime, choose).unwrap();
    assert_eq!(grove(&runtime).slot_of(Intent::Firebolt), None);
}

/// Casts `spell` at the straw dummy from `back` m until `landed` holds,
/// the dummy failing its saves; returns the attempts it took.
fn lands(spell: Spell, back: f32, landed: impl Fn(&Grove) -> bool) -> usize {
    let mut runtime = entered();
    for attempt in 1..=12 {
        grove_mut(&mut runtime)
            .dummies
            .iter_mut()
            .for_each(Dummy::reset);
        face(&mut runtime, STRAW, back);
        fail_saves(&mut runtime, STRAW);
        cast_spell(&mut runtime, spell);
        idle(&mut runtime, 1.3);
        if landed(grove(&runtime)) {
            return attempt;
        }
    }
    panic!("{spell:?} never landed: {:?}", grove(&runtime).log);
}

fn hurt(g: &Grove) -> bool {
    g.dummies[STRAW].hp < g.dummies[STRAW].kind.max_hp()
}

fn has(g: &Grove, condition: Condition) -> bool {
    g.dummies[STRAW].has(condition, g.time)
}

#[test]
fn the_demo_spells_land_with_their_damage_and_conditions() {
    lands(Spell::ProduceFlame, 8.0, hurt);
    lands(Spell::StarryWisp, 8.0, |g| {
        hurt(g) && has(g, Condition::Starlit)
    });
    lands(Spell::PoisonSpray, 6.0, hurt);
    lands(Spell::Entangle, 8.0, |g| has(g, Condition::Restrained));
    lands(Spell::FaerieFire, 8.0, |g| has(g, Condition::Outlined));
    lands(Spell::IceKnife, 8.0, |g| {
        hurt(g) && g.log.iter().any(|l| l.contains("cold"))
    });
    lands(Spell::Moonbeam, 8.0, |g| hurt(g) && !g.auras().is_empty());
    lands(Spell::CallLightning, 8.0, |g| {
        hurt(g) && g.log.iter().any(|l| l.contains("lightning"))
    });
    lands(Spell::IceStorm, 8.0, |g| {
        hurt(g) && g.log.iter().any(|l| l.contains("bludgeoning and"))
    });
    lands(Spell::WallOfFire, 8.0, hurt);
    lands(Spell::Sunbeam, 8.0, |g| {
        hurt(g) && has(g, Condition::Blinded)
    });
    lands(Spell::Sunburst, 8.0, |g| {
        hurt(g) && has(g, Condition::Blinded)
    });
}

#[test]
fn healing_word_restores_a_hurt_dummy() {
    let mut runtime = entered();
    face(&mut runtime, STRAW, 8.0);
    grove_mut(&mut runtime).dummies[STRAW].damage(50.0, super::kit::Damage::Fire, 0.0);
    let before = grove(&runtime).dummies[STRAW].hp;
    cast_spell(&mut runtime, Spell::HealingWord);
    let after = grove(&runtime).dummies[STRAW].hp;
    // 2d4 + 5 is 7 to 13.
    assert!(
        (7.0..=13.0).contains(&(after - before)),
        "{before} to {after}"
    );
}

#[test]
fn spike_growth_cuts_a_dummy_pushed_through_it() {
    let mut runtime = entered();
    face(&mut runtime, STRAW, 8.0);
    cast_spell(&mut runtime, Spell::SpikeGrowth);
    idle(&mut runtime, 0.2);
    assert_eq!(
        grove(&runtime).dummies[STRAW].hp,
        100.0,
        "thorns wait for movement"
    );
    fail_saves(&mut runtime, STRAW);
    cast_spell(&mut runtime, Spell::GustOfWind);
    idle(&mut runtime, 0.6);
    let dummy = &grove(&runtime).dummies[STRAW];
    assert!(
        dummy.hp < dummy.kind.max_hp(),
        "pushed 4.5 m through thorns"
    );
    assert!(
        grove(&runtime)
            .log
            .iter()
            .any(|l| l.starts_with("Spike Growth"))
    );
}

#[test]
fn concentration_keeps_one_maintained_area_and_recasting_moves_it() {
    let mut runtime = entered();
    face(&mut runtime, STRAW, 8.0);
    cast_spell(&mut runtime, Spell::Moonbeam);
    cast_spell(&mut runtime, Spell::Moonbeam);
    assert_eq!(grove(&runtime).auras().len(), 1);
    cast_spell(&mut runtime, Spell::WallOfFire);
    let auras = grove(&runtime).auras();
    assert_eq!(auras.len(), 1);
    assert_eq!(auras[0].spell, Spell::WallOfFire);
    // Entangle isn't maintained, so it stands beside the wall.
    cast_spell(&mut runtime, Spell::Entangle);
    assert_eq!(grove(&runtime).auras().len(), 2);
    // Each ends when its time is up.
    idle(&mut runtime, 10.5);
    assert!(grove(&runtime).auras().is_empty());
}

#[test]
fn conditions_tick_diminish_and_amplify_damage() {
    let mut dummy = Dummy::new(Kind::Straw, [0.0, 0.0]);
    assert_eq!(dummy.afflict(Condition::Paralyzed, 6.0, 0.0), 6.0);
    assert_eq!(dummy.afflict(Condition::Asleep, 6.0, 1.0), 3.0);
    assert_eq!(
        dummy.afflict(Condition::Prone, 1.5, 2.0),
        0.0,
        "the third is immune"
    );
    // Soft conditions don't diminish.
    assert_eq!(dummy.afflict(Condition::Blinded, 2.0, 2.0), 2.0);
    let tags: Vec<_> = dummy.conditions(2.5).into_iter().map(|(c, _)| c).collect();
    assert_eq!(
        tags,
        [Condition::Paralyzed, Condition::Asleep, Condition::Blinded]
    );
    // Damage wakes a sleeper; Faerie Fire's outline adds a fifth.
    dummy.afflict(Condition::Outlined, 10.0, 3.0);
    assert_eq!(dummy.damage(10.0, super::kit::Damage::Fire, 3.0), 12);
    assert!(!dummy.has(Condition::Asleep, 3.0));
    // The armored dummy resists piercing.
    let mut armored = Dummy::new(Kind::Armored, [0.0, 0.0]);
    assert_eq!(armored.damage(10.0, super::kit::Damage::Piercing, 0.0), 5);
    // Poison bites each second in the Grove.
    let mut runtime = entered();
    grove_mut(&mut runtime).dummies[STRAW].afflict(Condition::Poisoned, 3.5, 0.0);
    idle(&mut runtime, 3.2);
    let hp = grove(&runtime).dummies[STRAW].hp;
    assert!((88.0..=94.0).contains(&hp), "{hp}");
}

#[test]
fn the_bear_wolf_and_eagle_take_their_shapes_and_attacks() {
    use super::shape::Form;
    let mut runtime = entered();
    face(&mut runtime, STRAW, 2.5);
    for (form, pace, first) in [
        (Form::BrownBear, 0.85, Spell::BearBite),
        (Form::DireWolf, 1.5, Spell::WolfBite),
        (Form::GiantEagle, 1.4, Spell::EagleTalons),
    ] {
        cast_spell(&mut runtime, form.spell());
        idle(&mut runtime, 0.05);
        assert_eq!(grove(&runtime).form(), Some(form));
        assert_eq!(runtime.player.pace(), pace);
        let bar = runtime.grove_bar().unwrap();
        assert_eq!(bar.spells[usize::from(BITE)], Some(first));
        let figure = runtime.dynamic_mesh().figure.expect("a figure");
        figure.validate().unwrap();
    }
    // The eagle flies: it starts aloft, and Jump climbs.
    assert!(runtime.everglade_levitating());
    let low = runtime.player.pos.y;
    let climb = InputState {
        jump: true,
        ..InputState::default()
    };
    for _ in 0..50 {
        runtime.tick(&climb, DT);
    }
    assert!(
        runtime.player.pos.y > low + 1.0,
        "{low} to {}",
        runtime.player.pos.y
    );
    // Back on the ground in the druid's shape.
    cast(&mut runtime, usize::from(RETURN)).unwrap();
    assert!(!runtime.everglade_levitating());
    // The wolf's bite knocks a dummy down.
    let mut runtime = entered();
    face(&mut runtime, STRAW, 2.5);
    cast_spell(&mut runtime, Spell::WildShapeWolf);
    let mut down = false;
    for _ in 0..12 {
        let _ = cast(&mut runtime, usize::from(BITE));
        down |= has(grove(&runtime), Condition::Prone);
    }
    assert!(down, "{:?}", grove(&runtime).log);
}

#[test]
fn placeholders_and_controls_say_what_they_do() {
    let mut runtime = entered();
    face(&mut runtime, STRAW, 8.0);
    cast_spell(&mut runtime, Spell::WildCompanion);
    assert!(
        grove(&runtime)
            .log
            .last()
            .unwrap()
            .contains("labeled burst")
    );
    cast_spell(&mut runtime, Spell::SpeakWithAnimals);
    assert!(
        grove(&runtime)
            .log
            .last()
            .unwrap()
            .starts_with("A sparrow says")
    );
    let (status, lines) = runtime.grove_log().unwrap();
    assert_eq!(status, "Land: Arid · Form: Druid");
    assert_eq!(lines.len(), 2);
}

#[test]
fn spamming_every_spell_keeps_everything_bounded() {
    let mut runtime = entered();
    face(&mut runtime, STRAW, 8.0);
    let repeated: Vec<usize> = (0..slots::COUNT)
        .filter(|&i| {
            grove(&runtime)
                .slot_spell(i)
                .is_some_and(|s| s.repeats() && !s.beast())
        })
        .collect();
    for round in 0..60 {
        for &index in &repeated {
            let _ = cast(&mut runtime, index);
            face(&mut runtime, STRAW, 8.0);
        }
        runtime.tick(&InputState::default(), 0.05);
        let g = grove(&runtime);
        assert!(g.effects().len() <= MAX_EFFECTS, "round {round}");
        assert!(g.floaters.len() <= MAX_FLOATERS);
        assert!(g.auras().len() <= super::aura::MAX_AURAS);
        assert!(g.particles() <= crate::fx::system::MAX_PARTICLES);
        assert!(g.log.len() <= super::LOG);
    }
    let mesh = runtime.dynamic_mesh();
    assert!(mesh.lines.len() < 80_000, "{}", mesh.lines.len());
    assert!(mesh.sprites.len() <= crate::fx::system::MAX_PARTICLES);
}

/// In the dragon's shape, the transformation over, standing `back` m
/// short of dummy `i`.
fn dragon(back: f32, i: usize) -> WorldRuntime {
    let mut runtime = entered();
    face(&mut runtime, i, back);
    cast_spell(&mut runtime, Spell::Shapechange);
    idle(
        &mut runtime,
        super::dragon::SWAP + super::dragon::GROW + 0.1,
    );
    face(&mut runtime, i, back);
    runtime
}

#[test]
fn shapechange_becomes_the_dragon_after_its_transformation_and_returns() {
    use super::shape::Form;
    let mut runtime = entered();
    face(&mut runtime, STRAW, 8.0);
    cast_spell(&mut runtime, Spell::Shapechange);
    // The vortex rises and the druid dissolves into it before the dragon
    // bursts out.
    assert_eq!(grove(&runtime).form(), None);
    assert!(grove(&runtime).particles() == 0 || grove(&runtime).druid_morph().is_some());
    idle(&mut runtime, 0.6);
    assert_eq!(grove(&runtime).form(), None);
    let (k, _) = grove(&runtime).druid_morph().expect("the druid dissolving");
    assert!(k < 1.0, "{k}");
    assert!(grove(&runtime).particles() > 50);
    let shapechange = slot(&runtime, Spell::Shapechange);
    assert!(
        cast(&mut runtime, shapechange).is_err(),
        "a second cast waits for the first"
    );
    idle(&mut runtime, 0.5);
    assert_eq!(grove(&runtime).form(), Some(Form::Dragon));
    assert!(
        grove(&runtime)
            .log
            .iter()
            .any(|l| l.contains("shape of a dragon"))
    );
    idle(&mut runtime, 1.5);
    assert!(grove(&runtime).druid_morph().is_none());
    assert_eq!(runtime.player.pace(), Form::Dragon.pace());
    // The camera stands back to fit the dragon.
    assert!(runtime.grove_camera() > 2.0, "{}", runtime.grove_camera());
    let near = runtime.view(1.6).eye.distance(runtime.player.pos);
    // The dragon's actions take row 2's first five slots.
    let bar = runtime.grove_bar().unwrap();
    assert_eq!(
        &bar.spells[usize::from(BITE)..usize::from(BITE) + 5],
        &Form::Dragon
            .attacks()
            .iter()
            .map(|s| Some(*s))
            .collect::<Vec<_>>()[..]
    );
    assert!(bar.slots[slot(&runtime, Spell::Shapechange)].active);
    let figure = runtime.dynamic_mesh().figure.expect("a figure");
    figure.validate().unwrap();
    // The dragon stands about three times the druid's height.
    let top = figure
        .vertices
        .iter()
        .map(|v| v.pos[1])
        .filter(|y| *y > -50.0)
        .fold(f32::MIN, f32::max);
    assert!(top - runtime.player.pos.y > 4.0, "{top}");
    // Shapechange keeps spellcasting: the druid's spells still cast.
    cast_spell(&mut runtime, Spell::FireBolt);
    assert!(
        grove(&runtime)
            .effects()
            .iter()
            .any(|e| matches!(e, Effect::Bolt { .. }))
    );
    // Return to Form runs the transformation back.
    cast(&mut runtime, usize::from(RETURN)).unwrap();
    assert_eq!(grove(&runtime).form(), Some(Form::Dragon));
    idle(&mut runtime, 1.4);
    assert_eq!(grove(&runtime).form(), None);
    assert_eq!(runtime.player.pace(), 1.0);
    assert!(!runtime.everglade_levitating());
    idle(&mut runtime, 2.0);
    assert!(runtime.grove_camera() < 1.2);
    assert!(runtime.view(1.6).eye.distance(runtime.player.pos) < near);
    let figure = runtime.dynamic_mesh().figure.expect("a figure");
    figure.validate().unwrap();
}

#[test]
fn fire_breath_burns_the_dummies_in_its_cone() {
    let mut runtime = dragon(6.0, STRAW);
    for i in 0..grove(&runtime).dummies.len() {
        fail_saves(&mut runtime, i);
    }
    cast_spell(&mut runtime, Spell::FireBreath);
    // The breath draws in, then the fire leaves the jaws.
    assert_eq!(grove(&runtime).dummies[STRAW].hp, 100.0);
    idle(&mut runtime, 0.7);
    let g = grove(&runtime);
    let straw = &g.dummies[STRAW];
    assert!(straw.hp < 100.0 - 18.0, "{}", straw.hp);
    assert!(has(g, Condition::Burning));
    assert!(g.log.iter().any(|l| l.starts_with("Fire Breath hits")));
    // The far straw dummy, 30 m out, is past the cone's end.
    assert_eq!(g.dummies[2].hp, g.dummies[2].kind.max_hp());
    // Burning bites each second.
    let before = grove(&runtime).dummies[STRAW].hp;
    idle(&mut runtime, 2.1);
    let after = grove(&runtime).dummies[STRAW].hp;
    assert!(after < before, "{before} {after}");
}

#[test]
fn the_dragon_bites_sweeps_buffets_and_roars() {
    // The bite lands at the jaws' reach.
    let mut runtime = dragon(5.0, STRAW);
    let mut hit = false;
    let bite = slot(&runtime, Spell::DragonBite);
    for _ in 0..8 {
        let _ = cast(&mut runtime, bite);
        hit |= grove(&runtime).dummies[STRAW].hp < 100.0;
    }
    assert!(hit, "{:?}", grove(&runtime).log);
    // Tail Sweep knocks down what it hits.
    let mut runtime = dragon(3.0, STRAW);
    fail_saves(&mut runtime, STRAW);
    cast_spell(&mut runtime, Spell::TailSweep);
    assert!(grove(&runtime).dummies[STRAW].hp < 100.0);
    assert!(has(grove(&runtime), Condition::Prone));
    // Wing Buffet throws a dummy back.
    let mut runtime = dragon(3.0, STRAW);
    fail_saves(&mut runtime, STRAW);
    let before = grove(&runtime).dummies[STRAW]
        .pos
        .distance(runtime.player.pos);
    cast_spell(&mut runtime, Spell::WingBuffet);
    idle(&mut runtime, 0.5);
    let after = grove(&runtime).dummies[STRAW]
        .pos
        .distance(runtime.player.pos);
    assert!(after > before + 4.0, "{before} to {after}");
    // Roar frightens every dummy in reach that fails its save.
    let mut runtime = dragon(6.0, STRAW);
    fail_saves(&mut runtime, STRAW);
    cast_spell(&mut runtime, Spell::Roar);
    assert!(
        grove(&runtime).dummies[STRAW].has(Condition::Frightened, grove(&runtime).time),
        "{:?}",
        grove(&runtime).log
    );
    assert!(
        runtime.grove_shake().length() > 0.0 || {
            idle(&mut runtime, 0.05);
            runtime.grove_shake().length() > 0.0
        }
    );
}

#[test]
fn the_dragon_takes_off_with_jump_and_outflies_the_eagle() {
    use super::shape::Form;
    let mut runtime = dragon(8.0, STRAW);
    // It lands as it takes shape.
    assert!(!runtime.everglade_levitating());
    let ground = runtime.player.pos.y;
    let climb = InputState {
        jump: true,
        ..InputState::default()
    };
    for _ in 0..50 {
        runtime.tick(&climb, DT);
    }
    assert!(runtime.everglade_levitating());
    let dragon_climb = runtime.player.pos.y - ground;
    // The eagle's climb over the same second.
    let mut eagle = entered();
    face(&mut eagle, STRAW, 8.0);
    cast_spell(&mut eagle, Spell::WildShapeEagle);
    idle(&mut eagle, 0.1);
    let low = eagle.player.pos.y;
    for _ in 0..50 {
        eagle.tick(&climb, DT);
    }
    let eagle_climb = eagle.player.pos.y - low;
    assert!(
        dragon_climb > eagle_climb * 1.5,
        "{dragon_climb} vs {eagle_climb}"
    );
    // Aloft it flies at its air pace, faster than the eagle's.
    idle(&mut runtime, 0.05);
    assert_eq!(runtime.player.pace(), Form::Dragon.air_pace());
    assert!(Form::Dragon.air_pace() > Form::GiantEagle.air_pace());
    let start = runtime.player.pos;
    let ahead = InputState {
        forward: true,
        ..InputState::default()
    };
    for _ in 0..50 {
        runtime.tick(&ahead, DT);
    }
    let flown = (runtime.player.pos - start).with_y(0.0).length();
    assert!(flown > 10.0, "{flown}");
    // X dives.
    let high = runtime.player.pos.y;
    for _ in 0..50 {
        runtime.everglade_climb(-1.0, DT);
        runtime.tick(&InputState::default(), DT);
    }
    assert!(runtime.player.pos.y < high - 3.0);
    let figure = runtime.dynamic_mesh().figure.expect("a figure");
    figure.validate().unwrap();
}

#[test]
fn spamming_shapechange_and_the_dragon_stays_bounded() {
    let mut runtime = entered();
    face(&mut runtime, STRAW, 6.0);
    let dragon_slots: Vec<usize> = (0..5).map(|k| usize::from(BITE) + k).collect();
    let shapechange = slot(&runtime, Spell::Shapechange);
    for round in 0..80 {
        let _ = cast(&mut runtime, shapechange);
        for &index in &dragon_slots {
            let _ = cast(&mut runtime, index);
        }
        if round % 17 == 16 {
            let _ = cast(&mut runtime, usize::from(RETURN));
        }
        face(&mut runtime, STRAW, 6.0);
        runtime.tick(&InputState::default(), 0.05);
        let g = grove(&runtime);
        assert!(g.effects().len() <= MAX_EFFECTS, "round {round}");
        assert!(g.floaters.len() <= MAX_FLOATERS);
        assert!(g.particles() <= crate::fx::system::MAX_PARTICLES);
        assert!(g.log.len() <= super::LOG);
    }
    let mesh = runtime.dynamic_mesh();
    assert!(mesh.sprites.len() <= crate::fx::system::MAX_PARTICLES);
    mesh.figure.expect("a figure").validate().unwrap();
    // Long Rest ends it all.
    cast_spell(&mut runtime, Spell::LongRest);
    assert_eq!(grove(&runtime).form(), None);
    assert!(grove(&runtime).druid_morph().is_none());
}

#[test]
fn the_flying_target_sits_low_on_its_post_and_nothing_hangs_after_reverse_gravity() {
    let mut runtime = entered();
    let flying = grove(&runtime)
        .dummies
        .iter()
        .position(|d| d.kind == Kind::Flying)
        .unwrap();
    let home = grove(&runtime).dummies[flying].home;
    let ground = crate::zones::everglade::height(home.x, home.z);
    assert!(
        (4.0..=6.5).contains(&(home.y - ground)),
        "{}",
        home.y - ground
    );
    // Reverse Gravity over the field, with a straw dummy rooted in it.
    face(&mut runtime, STRAW, 2.0);
    grove_mut(&mut runtime).dummies[STRAW].afflict(Condition::Restrained, 1.0, 0.0);
    cast(&mut runtime, GRAVITY_SLOT).unwrap();
    idle(&mut runtime, 0.6);
    // Others fall upward; the rooted one stays on the ground.
    let straw = &grove(&runtime).dummies[STRAW];
    let under = crate::zones::everglade::height(straw.pos.x, straw.pos.z);
    assert!(straw.pos.y < under + 0.05, "{}", straw.pos.y - under);
    // The root ends with gravity still reversed; the dummy rises, and a
    // second press ends the spell, so every dummy comes back down.
    idle(&mut runtime, 1.5);
    cast(&mut runtime, GRAVITY_SLOT).unwrap();
    // What was still rising arcs over and lands.
    idle(&mut runtime, 8.0);
    let g = grove(&runtime);
    for d in &g.dummies {
        if d.anchored() {
            assert_eq!(d.pos.y, d.home.y, "the target never leaves its post");
        } else {
            let floor = crate::zones::everglade::height(d.pos.x, d.pos.z);
            assert!(
                d.pos.y < floor + 0.05,
                "{:?} at {}",
                d.kind,
                d.pos.y - floor
            );
        }
    }
}

#[test]
fn walls_and_areas_draw_particles_and_no_guide_lines() {
    let mut runtime = entered();
    face(&mut runtime, STRAW, 8.0);
    idle(&mut runtime, 0.1);
    let quiet = runtime.dynamic_mesh().lines.len();
    for spell in [
        Spell::WallOfFire,
        Spell::WallOfThorns,
        Spell::Moonbeam,
        Spell::SpikeGrowth,
        Spell::Entangle,
        Spell::IceStorm,
        Spell::Sunbeam,
        Spell::ConeOfCold,
    ] {
        // Cone of Cold is a land spell: choose Polar for it.
        if spell == Spell::ConeOfCold {
            cast_spell(&mut runtime, Spell::ChooseLand);
        }
        let index = slot(&runtime, spell);
        let _ = cast(&mut runtime, index);
        face(&mut runtime, STRAW, 8.0);
        idle(&mut runtime, 0.3);
        let g = grove(&runtime);
        assert!(g.particles() > 0, "{spell:?}");
        // Only the dummies' own marks (the target ring, roots, outlines)
        // draw lines, as before the spell.
        let lines = runtime.dynamic_mesh().lines.len();
        let rooted = g.dummies.iter().any(|d| d.rooted(g.time));
        assert!(lines <= quiet || rooted, "{spell:?}: {quiet} to {lines}");
    }
}
