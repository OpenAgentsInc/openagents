//! Carved models collide by their own triangles: a character walks up the
//! bandshell's steps onto its stage and can't enter its shell, walks around
//! the fountain's basin, stops at a closed building's walls, and lands on
//! its roof.

use crate::controller::{InputState, PlayerController};
use crate::zones::everglade::layout::{self, generated::Instance};
use crate::zones::everglade::solids::{self, Solids};
use crate::zones::everglade::tests::pack;
use crate::zones::everglade::{HALF_EXTENT, height};
use glam::Vec3;
use std::sync::OnceLock;

fn solids() -> &'static Solids {
    static SOLIDS: OnceLock<Solids> = OnceLock::new();
    SOLIDS.get_or_init(|| solids::build(pack(), &layout::placements()).expect("the solids build"))
}

fn instance(name: &str) -> Instance {
    layout::generated()
        .into_iter()
        .find(|i| i.model.name == name)
        .unwrap_or_else(|| panic!("{name} is placed"))
}

/// A player at `local` in `instance`'s frame, on whatever is under it,
/// facing `toward` (also local).
fn player_at(instance: &Instance, local: [f32; 2], toward: [f32; 2]) -> PlayerController {
    let [x, z] = instance.world(local);
    let [tx, tz] = instance.world(toward);
    let yaw = (tx - x).atan2(tz - z);
    let mut player = PlayerController::new(Vec3::new(x, 0.0, z), yaw);
    player.pos.y = solids().floor(x, z, height(x, z));
    player.set_surface_height(player.pos.y);
    player
}

fn walk(player: &mut PlayerController, seconds: f32) {
    let input = InputState {
        forward: true,
        ..InputState::default()
    };
    for _ in 0..(seconds * 60.0) as usize {
        solids().step(player, &input, 1.0 / 60.0, HALF_EXTENT, true);
    }
}

/// `player`'s position in `instance`'s frame, x and z.
fn local(instance: &Instance, player: &PlayerController) -> [f32; 2] {
    let d = Vec3::new(
        player.pos.x - instance.at[0],
        0.0,
        player.pos.z - instance.at[1],
    );
    let back = glam::Quat::from_rotation_y(-instance.yaw) * d;
    [back.x / instance.scale, back.z / instance.scale]
}

#[test]
fn the_bandshell_stage_is_climbed_by_its_steps_and_its_shell_is_solid() {
    let shell = instance("generated/bandshell");
    let base = height(shell.at[0], shell.at[1]);
    // Up the steps at the front, onto the stage near the lectern.
    let mut player = player_at(&shell, [0.0, 4.5], [0.0, 0.0]);
    walk(&mut player, 2.0);
    let on = local(&shell, &player);
    assert!(
        player.pos.y > base + 0.7,
        "on the stage at {} m, local {on:?}",
        player.pos.y - base
    );
    // On to the back: the shell stops the walk at its inner face.
    walk(&mut player, 3.0);
    let at = local(&shell, &player);
    assert!(
        at[1] > -3.39,
        "inside the stage, not through the shell: {at:?}"
    );
    assert!(player.pos.y > base + 0.7);
    // From behind, the shell's back stops a walker on the ground.
    let mut behind = player_at(&shell, [0.0, -6.0], [0.0, 0.0]);
    walk(&mut behind, 3.0);
    let at = local(&shell, &behind);
    assert!(at[1] < -3.0, "outside the shell's back: {at:?}");
    assert!(behind.pos.y < base + 0.3);
}

#[test]
fn an_open_gazebo_lets_a_walker_in_and_a_closed_door_does_not() {
    let gazebo = instance("generated/gazebo");
    let mut player = player_at(&gazebo, [0.0, 3.0], [0.0, -2.7]);
    walk(&mut player, 1.5);
    let at = local(&gazebo, &player);
    assert!(at[1] < -1.5, "walked in under the roof: {at:?}");
    // The library's front door is shut.
    let library = instance("generated/library");
    let front = library.model.front;
    let mut player = player_at(&library, [front[0], front[1] + 1.0], [front[0], -5.0]);
    walk(&mut player, 3.0);
    let at = local(&library, &player);
    assert!(at[1] > 0.0, "stopped at the door: {at:?}");
}

#[test]
fn the_fountain_basin_turns_a_walker_aside() {
    let fountain = instance("kit/fountain");
    let mut player = player_at(&fountain, [0.0, 7.0], [0.0, 0.0]);
    walk(&mut player, 3.0);
    let [x, z] = local(&fountain, &player);
    assert!(
        x.hypot(z) > 2.2,
        "stopped at the rim, {} m from the middle",
        x.hypot(z)
    );
}

#[test]
fn a_closed_building_stops_a_walker_and_holds_up_a_lander() {
    let library = instance("generated/library");
    // At its long side wall, from outside.
    let mut player = player_at(&library, [10.0, -5.0], [0.0, -5.0]);
    walk(&mut player, 3.0);
    let at = local(&library, &player);
    assert!(at[0] > 6.0, "stopped outside the wall: {at:?}");
    // From high above its middle, the first thing underfoot is its roof.
    let [x, z] = library.world([0.0, -5.0]);
    let base = height(x, z);
    let roof = solids().floor(x, z, base + 40.0);
    assert!(roof > base + 7.0, "the roof is {} m up", roof - base);
}
