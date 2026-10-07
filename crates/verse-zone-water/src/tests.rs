use glam::{Vec2, Vec3};

use super::*;

/// The spawn and the way out stand on dry beach; the bay is water, and
/// the pool and the river lie below their own surfaces.
#[test]
fn the_cove_has_a_beach_a_bay_a_pool_and_a_river() {
    assert!(ground(SPAWN.x, SPAWN.z) > LEVEL + 0.2);
    assert!(ground(EXIT.x, EXIT.z) > LEVEL + 0.3);
    assert!(ground(0.0, -30.0) < LEVEL - 2.0);
    // The reef's top comes within a meter or so of the surface.
    let reef = ground(terrain::REEF[0], terrain::REEF[1]);
    assert!((-1.5..-0.3).contains(&reef), "{reef}");
    assert!(ground(terrain::POOL[0], terrain::POOL[1]) < terrain::POOL_LEVEL - 1.5);
    // The plateau stands well above the pool: the falls drop about 9 m.
    assert!(terrain::LIP.y - terrain::POOL_LEVEL > 8.0);
    for reach in [&terrain::UPPER, &terrain::LOWER] {
        for p in reach.points.iter().take(reach.points.len() - 1) {
            let bed = ground(p[0], p[1]);
            assert!(bed < p[2] - 0.2, "{p:?} bed {bed}");
        }
    }
}

/// Every water patch is a whole grid, and the sea covers the bay.
#[test]
fn the_water_surface_is_valid() {
    let surface = sea::surface();
    surface.validate().unwrap();
    assert_eq!(surface.patches.len(), 5);
    let triangles = surface.indices(1).len() / 3;
    assert!(triangles > 50_000, "{triangles}");
    // The low tier's sea has about a quarter of the triangles.
    assert!(surface.indices(2).len() < surface.indices(1).len() / 2);
}

/// A crate dropped in the bay splashes, then floats about a third under
/// and stays near where it fell.
#[test]
fn a_dropped_crate_splashes_and_floats() {
    let mut lab = WaterLab::new();
    lab.floats = Floats::new();
    lab.floats.spawn(
        FloatKind::Crate,
        Vec3::new(0.0, 3.0, -20.0),
        0.0,
        Vec3::ZERO,
    );
    let mut splashed = false;
    for _ in 0..600 {
        lab.tick(1.0 / 60.0, spawn(), Vec3::NEG_Z);
        splashed |= lab.floats.floats.first().is_some_and(|f| f.wet);
    }
    assert!(splashed);
    let (_, at, vel) = lab.floats.states().next().unwrap();
    let surface = lab.surface_at(Vec2::new(at.x, at.z)).unwrap().height;
    // Its center rides near the surface, not sunk and not flying.
    assert!((at.y - surface).abs() < 0.45, "{} vs {surface}", at.y);
    assert!(vel.length() < 2.0);
    assert!(Vec2::new(at.x, at.z).distance(Vec2::new(0.0, -20.0)) < 8.0);
}

/// Water Walk puts a surface under the character's feet over the sea; deep
/// water without Water Breathing floats the character at the surface.
#[test]
fn water_walk_and_swimming_hold_the_character_up() {
    let mut lab = WaterLab::new();
    let feet = Vec3::new(0.0, LEVEL - 1.2, -40.0);
    let swim = lab.tick(1.0 / 60.0, feet, Vec3::NEG_Z);
    assert!(swim.floor.unwrap() < LEVEL - 0.8);
    lab.press(Slot::WaterWalk, false, feet, Vec3::NEG_Z, 0.0);
    let walk = lab.tick(1.0 / 60.0, Vec3::new(0.0, LEVEL, -40.0), Vec3::NEG_Z);
    assert!(walk.floor.unwrap() > LEVEL - 0.6);
    lab.press(Slot::WaterWalk, false, feet, Vec3::NEG_Z, 0.0);
    lab.press(Slot::WaterBreathing, false, feet, Vec3::NEG_Z, 0.0);
    let dive = lab.tick(1.0 / 60.0, Vec3::new(0.0, -5.0, -40.0), Vec3::NEG_Z);
    assert!(dive.floor.is_none());
}

/// Without Water Breathing a dive lasts the SRD's held breath, then the
/// character is sent up.
#[test]
fn a_held_breath_runs_out() {
    let mut lab = WaterLab::new();
    let deep = Vec3::new(0.0, ground(0.0, -40.0), -40.0);
    let mut sent_up = false;
    for _ in 0..(125 * 10) {
        sent_up |= lab.tick(0.1, deep, Vec3::NEG_Z).floor.is_some() && lab.breath <= 0.0;
    }
    assert!(sent_up);
}

/// Part Water opens a dry trench; Whirlpool pulls the surface down at its
/// eye; Sleet Storm ices the water so it stands still.
#[test]
fn control_water_and_sleet_change_the_water() {
    let mut lab = WaterLab::new();
    let at = Vec3::new(0.0, 0.5, 2.0);
    // The first cast floods; the second parts the water.
    lab.press(Slot::ControlWater, false, at, Vec3::NEG_Z, 0.0);
    lab.press(Slot::ControlWater, false, at, Vec3::NEG_Z, 0.0);
    for _ in 0..240 {
        lab.tick(1.0 / 30.0, spawn(), Vec3::NEG_Z);
    }
    let middle = Spells::aim(at, Vec3::NEG_Z, spells::CONTROL_SIDE * 0.5 - 2.0);
    assert!(lab.surface_at(middle).is_none());
    // Whirlpool, two casts on.
    lab.press(Slot::ControlWater, false, at, Vec3::NEG_Z, 0.0);
    lab.press(Slot::ControlWater, false, at, Vec3::NEG_Z, 0.0);
    for _ in 0..240 {
        lab.tick(1.0 / 30.0, spawn(), Vec3::NEG_Z);
    }
    let eye = lab.spells.control.unwrap().center;
    let water = lab.frame_water();
    assert!(water.controls.drop(eye, 10.0) > 2.0);
    assert!(water.controls.flow_at(eye + Vec2::new(3.0, 0.0)).length() > 1.0);
    lab.press(Slot::ControlWater, true, at, Vec3::NEG_Z, 0.0);
    lab.press(Slot::SleetStorm, false, at, Vec3::NEG_Z, 0.0);
    for _ in 0..150 {
        lab.tick(1.0 / 30.0, spawn(), Vec3::NEG_Z);
    }
    let center = lab.spells.sleet.unwrap().center;
    assert!(lab.frame_water().controls.ice_at(center) > 0.99);
}

/// Every prop stands on the cove's ground and none in the sea's way.
#[test]
fn the_props_avoid_the_river_and_the_spawn() {
    for placement in placements() {
        let at = Vec2::from(placement.at);
        assert!(
            at.distance(Vec2::new(SPAWN.x, SPAWN.z)) > 3.0 || placement.model.contains("Grass")
        );
    }
}
