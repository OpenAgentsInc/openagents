//! The hotbar's spells in the loaded glade: each acts on the player as the
//! chamber's rules say, and the hotbar shows it.

use super::hotbar::{COUNT, SLOTS, Slot};
use super::spells::Spell;
use super::tests::entered;
use crate::controller::{AVATAR_HEIGHT, InputState};
use crate::runtime::WorldRuntime;
use crate::zones::Intent;
use glam::Vec3;

const DT: f32 = 0.02;

fn slots(runtime: &WorldRuntime) -> [Slot; COUNT] {
    runtime.everglade_hotbar().expect("inside Everglade")
}

fn slot(runtime: &WorldRuntime, spell: Spell) -> Slot {
    let index = SLOTS
        .iter()
        .position(|(intent, ..)| *intent == spell.intent())
        .expect("the spell has a slot");
    slots(runtime)[index]
}

fn idle(runtime: &mut WorldRuntime, seconds: f32) {
    for _ in 0..(seconds / DT).round() as usize {
        runtime.tick(&InputState::default(), DT);
    }
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

/// Seconds of falling from `at` until landing.
fn fall_time(runtime: &mut WorldRuntime, at: Vec3) -> f32 {
    runtime.player.pos = at;
    runtime.player.set_vertical_speed(0.0);
    let mut seconds = 0.0;
    while runtime.player.airborne() || seconds == 0.0 {
        runtime.tick(&InputState::default(), DT);
        seconds += DT;
        assert!(seconds < 10.0, "never landed");
    }
    seconds
}

#[test]
fn the_hotbar_holds_four_spells_after_movement_with_keys_four_to_seven() {
    let spells: Vec<_> = SLOTS[3..].iter().map(|(intent, ..)| *intent).collect();
    assert_eq!(
        spells,
        [
            Intent::FeatherFall,
            Intent::WallOfStone,
            Intent::WindWall,
            Intent::ReverseGravity
        ]
    );
    for spell in Spell::ALL {
        assert_eq!(Spell::of(spell.intent()), Some(spell));
        assert!((spell.cooldown() - verse_world::spells::ROUND).abs() < 1e-6);
    }
    let size = [800.0, 600.0];
    let [left, top, _, height] = super::hotbar::frame(size, 0.0);
    // The fifth slot, under key 5, is Wall of Stone.
    let point = [left + (8.0 + 42.0 * 4.0 + 18.0), top + height / 2.0];
    assert_eq!(
        super::hotbar::hit(point, size, 0.0),
        Some(Intent::WallOfStone)
    );
    let runtime = entered();
    let bar = slots(&runtime);
    // Standing on the clearing, Feather Fall waits for a fall; the walls
    // and Reverse Gravity can be cast.
    assert!(!bar[3].enabled);
    assert!(bar[4].enabled && bar[5].enabled && bar[6].enabled);
    assert!(bar.iter().all(|s| s.cooldown == 0.0 && !s.active));
}

#[test]
fn feather_fall_caps_a_fall_and_ends_on_landing() {
    let mut runtime = entered();
    assert!(runtime.zone_intent(Intent::FeatherFall).is_err());
    let start = runtime.player.pos;
    let plain = fall_time(&mut runtime, start + Vec3::Y * 15.0);

    runtime.player.pos = start + Vec3::Y * 15.0;
    runtime.player.set_vertical_speed(0.0);
    idle(&mut runtime, 0.2);
    assert!(runtime.player.vertical_speed() < -1.0);
    assert!(slot(&runtime, Spell::FeatherFall).enabled);
    runtime.zone_intent(Intent::FeatherFall).unwrap();
    let warded = slot(&runtime, Spell::FeatherFall);
    assert!(warded.active && !warded.enabled && warded.cooldown > 0.9);
    // Within a fifth of a second the descent is 60 feet per round.
    idle(&mut runtime, 0.3);
    let cap = verse_world::feather_fall::DESCENT_CAP as f32;
    assert!(
        (runtime.player.vertical_speed() + cap).abs() < 0.01,
        "{}",
        runtime.player.vertical_speed()
    );
    let glade = runtime.zone_state.everglade.as_ref().unwrap();
    assert!(
        !glade.spell_mesh(&runtime.player).faces.is_empty(),
        "feathers"
    );
    let before = runtime.player.pos.y;
    idle(&mut runtime, 1.0);
    assert!(((before - runtime.player.pos.y) - cap).abs() < 0.05);
    // The rest of the fall is slow too, and landing ends the ward.
    let mut seconds = 0.2 + 0.3 + 1.0;
    while runtime.player.airborne() {
        runtime.tick(&InputState::default(), DT);
        seconds += DT;
        assert!(seconds < 10.0);
    }
    assert!(seconds > plain * 2.0, "{seconds} against {plain}");
    assert!(!slot(&runtime, Spell::FeatherFall).active);
    let glade = runtime.zone_state.everglade.as_ref().unwrap();
    assert!(glade.spell_mesh(&runtime.player).faces.is_empty());
}

#[test]
fn wall_of_stone_blocks_walking_until_it_ends() {
    let mut runtime = entered();
    let start = runtime.player.pos;
    let forward = runtime.player.forward();
    runtime.zone_intent(Intent::WallOfStone).unwrap();
    let bar = slot(&runtime, Spell::WallOfStone);
    assert!(bar.active && bar.enabled && bar.cooldown > 0.9);
    let glade = runtime.zone_state.everglade.as_ref().unwrap();
    assert!(!glade.spell_mesh(&runtime.player).faces.is_empty());
    // Head-on and at a slant, the panels four meters ahead stop the walk.
    let ahead = |runtime: &WorldRuntime| (runtime.player.pos - start).dot(forward);
    walk(&mut runtime, 2.0);
    assert!(
        ahead(&runtime) > 3.0 && ahead(&runtime) < 3.5,
        "{}",
        ahead(&runtime)
    );
    runtime.player.pos = start;
    runtime.player.yaw += 0.4;
    walk(&mut runtime, 1.0);
    assert!(ahead(&runtime) < 3.5, "{}", ahead(&runtime));
    runtime.player.yaw -= 0.4;
    // A levitating player can come down on the wall's top.
    runtime.player.pos = start + forward * 4.0 + Vec3::Y * 4.0;
    runtime.player.set_vertical_speed(0.0);
    idle(&mut runtime, 1.0);
    assert!(!runtime.player.airborne());
    let top = verse_world::wall_of_stone::Form::Thick.size().y as f32;
    assert!((runtime.player.pos.y - top).abs() < 0.05);
    // Pressing the slot again ends the concentration; the way is open.
    runtime.zone_intent(Intent::WallOfStone).unwrap();
    assert!(!slot(&runtime, Spell::WallOfStone).active);
    idle(&mut runtime, 1.0);
    runtime.player.pos = start;
    walk(&mut runtime, 2.0);
    assert!(ahead(&runtime) > 6.0, "{}", ahead(&runtime));
}

#[test]
fn a_new_concentration_spell_ends_the_last_and_cooldowns_hold() {
    let mut runtime = entered();
    runtime.zone_intent(Intent::WallOfStone).unwrap();
    runtime.zone_intent(Intent::WindWall).unwrap();
    assert!(!slot(&runtime, Spell::WallOfStone).active);
    assert!(slot(&runtime, Spell::WindWall).active);
    // Wall of Stone is still cooling down.
    let stone = slot(&runtime, Spell::WallOfStone);
    assert!(!stone.enabled && stone.cooldown > 0.9);
    assert!(runtime.zone_intent(Intent::WallOfStone).is_err());
    idle(&mut runtime, verse_world::spells::ROUND + 0.1);
    assert_eq!(slot(&runtime, Spell::WallOfStone).cooldown, 0.0);
    runtime.zone_intent(Intent::WallOfStone).unwrap();
    assert!(!slot(&runtime, Spell::WindWall).active);
}

#[test]
fn wind_wall_rises_ahead_and_its_updraft_throws_the_player_up() {
    let mut runtime = entered();
    let start = runtime.player.pos;
    let forward = runtime.player.forward();
    runtime.zone_intent(Intent::WindWall).unwrap();
    let glade = runtime.zone_state.everglade.as_ref().unwrap();
    assert!(glade.spell_mesh(&runtime.player).lines.len() > 40);
    // Standing in the wind keeps the player aloft.
    runtime.player.pos = start + forward * 4.0;
    idle(&mut runtime, 2.0);
    assert!(runtime.player.airborne());
    // Walking into the wind throws the player up over the wall's top, and
    // they come down on its far side.
    runtime.player.pos = start;
    runtime.player.set_vertical_speed(0.0);
    idle(&mut runtime, 1.0);
    let wall_top = verse_world::wind_wall::HEIGHT as f32;
    let forward_input = InputState {
        forward: true,
        ..InputState::default()
    };
    let mut peak = 0.0_f32;
    for _ in 0..(4.0 / DT) as usize {
        runtime.tick(&forward_input, DT);
        peak = peak.max(runtime.player.pos.y);
    }
    assert!(peak > wall_top, "peak {peak}");
    assert!((runtime.player.pos - start).dot(forward) > 6.0);
}

#[test]
fn reverse_gravity_lifts_to_the_top_hovers_and_drops_when_ended() {
    let mut runtime = entered();
    runtime.zone_intent(Intent::Levitate).unwrap();
    idle(&mut runtime, 0.5);
    runtime.zone_intent(Intent::ReverseGravity).unwrap();
    // Falling upward ends the levitation.
    assert!(!runtime.everglade_levitating());
    assert!(slot(&runtime, Spell::ReverseGravity).active);
    idle(&mut runtime, 6.0);
    let top = verse_world::reverse_gravity::HEIGHT as f32;
    assert!(
        (runtime.player.pos.y - top).abs() < 0.2,
        "{}",
        runtime.player.pos.y
    );
    assert!(runtime.player.vertical_speed().abs() < 0.2);
    let glade = runtime.zone_state.everglade.as_ref().unwrap();
    assert!(glade.spell_mesh(&runtime.player).lines.len() >= 2 * 48 * 3);
    // Ending it drops the player; Feather Fall then catches the fall.
    runtime.zone_intent(Intent::ReverseGravity).unwrap();
    idle(&mut runtime, 0.3);
    runtime.zone_intent(Intent::FeatherFall).unwrap();
    idle(&mut runtime, 12.0);
    assert!(!runtime.player.airborne());
    assert!(runtime.player.pos.y < 0.5);
}

#[test]
fn reverse_gravity_inside_the_hall_stops_at_the_roof() {
    let mut runtime = entered();
    let ([cx, cz], [hx, _]) = super::HALL;
    runtime.player.pos = Vec3::new(cx - hx / 2.0, 0.0, cz);
    idle(&mut runtime, 0.2);
    runtime.zone_intent(Intent::ReverseGravity).unwrap();
    idle(&mut runtime, 4.0);
    let head = runtime.player.pos.y + AVATAR_HEIGHT;
    assert!(head > super::layout::WALL_TOP, "{head}");
    // The head rests against the roof's underside.
    let glade = runtime.zone_state.everglade.as_ref().unwrap();
    let (x, z) = (runtime.player.pos.x, runtime.player.pos.z);
    let roof = glade.solids.ceiling(x, z, head - 0.01).expect("a roof");
    assert!((roof - head).abs() < 0.01, "head {head}, roof {roof}");
}
