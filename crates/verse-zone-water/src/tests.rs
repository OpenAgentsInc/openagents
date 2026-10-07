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

/// Every water patch is a whole grid, and the sea is the clipmap ocean
/// over a field that covers the cove with the depth its ground gives.
#[test]
fn the_water_surface_is_valid() {
    let surface = sea::surface();
    surface.validate().unwrap();
    assert_eq!(surface.patches.len(), 4);
    let ocean = surface.ocean.as_ref().expect("the sea is an ocean");
    assert_eq!(ocean.body, sea::SEA);
    let field = ocean.field.as_ref().expect("the sea has a field");
    for (x, z) in [(0.0, -20.0), (16.0, -30.0), (-60.0, -90.0), (0.0, 30.0)] {
        let depth = LEVEL - ground(x, z);
        let t = field.sample(x, z);
        assert!(
            (t.depth - depth).abs() < 0.35,
            "({x}, {z}): {t:?} vs {depth}"
        );
    }
    // The shore lies between the reef's shallows and open water.
    assert!(field.sample(0.0, -5.0).shore < field.sample(0.0, -120.0).shore);
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

/// A lab with no floats.
fn quiet_lab() -> WaterLab {
    let mut lab = WaterLab::new();
    lab.floats = Floats::new();
    lab
}

/// Holds the Water Orb's key for `seconds` with the caster at `feet`
/// facing `forward`.
fn hold_orb(lab: &mut WaterLab, feet: Vec3, forward: Vec3, seconds: f32) {
    lab.press(Slot::WaterOrb, false, feet, forward, 0.0);
    let steps = (seconds * 60.0).round() as usize;
    for _ in 0..steps {
        lab.tick(1.0 / 60.0, feet, forward);
    }
}

/// The orb grows with the time the key is held, drawing from the sea, and
/// stops at its largest.
#[test]
fn the_orb_grows_while_held_and_stops_at_its_largest() {
    assert!((orb::grown(1.0, orb::GROW_FROM_AIR) - (orb::MIN_RADIUS + 0.9)).abs() < 1e-5);
    assert_eq!(orb::grown(100.0, orb::GROW_FROM_WATER), orb::MAX_RADIUS);
    let mut lab = quiet_lab();
    let feet = spawn();
    hold_orb(&mut lab, feet, Vec3::NEG_Z, 1.0);
    let first = lab.forming().unwrap().clone();
    // The beach faces the bay: it draws from the sea.
    assert_eq!(first.source, orb::Source::Sea);
    let want = orb::MIN_RADIUS + orb::GROW_FROM_WATER;
    assert!((first.radius - want).abs() < 0.1, "{}", first.radius);
    for _ in 0..60 {
        lab.tick(1.0 / 60.0, feet, Vec3::NEG_Z);
    }
    let later = lab.forming().unwrap().radius;
    assert!(later > first.radius + 1.4, "{later}");
    for _ in 0..600 {
        lab.tick(1.0 / 60.0, feet, Vec3::NEG_Z);
    }
    assert_eq!(lab.forming().unwrap().radius, orb::MAX_RADIUS);
    assert!(lab.caption(feet).contains("12.0 m across (largest)"));
    // Its water draws through the water pass.
    let mesh = lab.mesh(feet + Vec3::new(0.0, 3.0, 6.0));
    assert!(mesh.liquid.len() > 3000);
}

/// Letting go throws the orb along the aim: it flies toward the point the
/// pointer's ray meets and bursts near it.
#[test]
fn letting_go_throws_the_orb_along_the_aim() {
    let mut lab = quiet_lab();
    let feet = spawn();
    hold_orb(&mut lab, feet, Vec3::NEG_Z, 0.6);
    // The pointer aims down at the sea, ahead and to the left.
    let eye = feet + Vec3::new(0.0, 3.0, 4.0);
    let spot = Vec3::new(-8.0, LEVEL, -16.0);
    lab.set_aim(Some((eye, (spot - eye).normalize())));
    let line = lab
        .release(Slot::WaterOrb, false, feet, Vec3::NEG_Z)
        .unwrap();
    assert!(line.contains("hurl"), "{line}");
    let thrown = lab.orbs[0].clone();
    let orb::State::Flying { target } = thrown.state else {
        panic!("{:?}", thrown.state);
    };
    // It aims where the ray meets the rolling surface, near the spot.
    let ray = (spot - eye).normalize();
    let off_ray = (target - eye).cross(ray).length();
    assert!(off_ray < 0.2, "{target}");
    assert!(
        Vec2::new(target.x - spot.x, target.z - spot.z).length() < 2.5,
        "{target}"
    );
    let heading = Vec2::new(thrown.velocity.x, thrown.velocity.z).normalize();
    let toward = Vec2::new(target.x - thrown.center.x, target.z - thrown.center.z).normalize();
    assert!(heading.dot(toward) > 0.999);
    assert!(thrown.velocity.y > 0.0, "it arcs");
    // It flies, then bursts near the target.
    let mut last = thrown.center;
    for _ in 0..240 {
        if let Some(orb) = lab.orbs.first() {
            last = orb.center;
        }
        lab.tick(1.0 / 60.0, feet, Vec3::NEG_Z);
    }
    assert!(lab.orbs.is_empty());
    assert!(
        Vec2::new(last.x - spot.x, last.z - spot.z).length() < 3.0,
        "{last}"
    );
    assert!(lab.log.iter().any(|l| l.contains("crashes into the water")));
}

/// An orb bursting on the water throws the floating bodies around it out
/// and up and spills a ring of ripples; on the beach it soaks the sand.
#[test]
fn a_burst_splashes_and_throws_bodies_back() {
    let mut lab = quiet_lab();
    let spot = Vec3::new(0.0, LEVEL, -16.0);
    lab.floats.spawn(
        FloatKind::Crate,
        spot + Vec3::new(3.0, 0.2, 0.0),
        0.0,
        Vec3::ZERO,
    );
    for _ in 0..120 {
        lab.tick(1.0 / 60.0, spawn(), Vec3::NEG_Z);
    }
    let before = lab.floats.states().next().unwrap().1;
    let mut orb = orb::Orb::new(99, spot + Vec3::Y * 6.0, lab.time);
    orb.radius = 3.0;
    orb.state = orb::State::Flying { target: spot };
    orb.velocity = Vec3::new(0.0, -6.0, 0.0);
    lab.orbs.push(orb);
    let mut fastest = 0.0f32;
    let mut ripples = 0;
    for _ in 0..30 {
        lab.tick(1.0 / 60.0, spawn(), Vec3::NEG_Z);
        fastest = fastest.max(lab.floats.states().next().unwrap().2.length());
        let young = lab
            .water
            .ripples
            .iter()
            .filter(|r| lab.time - r.start < 0.6 && r.strength > 0.015)
            .count();
        ripples = ripples.max(young);
    }
    assert!(lab.orbs.is_empty());
    let after = lab.floats.states().next().unwrap().1;
    assert!(fastest > 3.0, "{fastest}");
    assert!(after.x > before.x + 0.2, "{before} -> {after}");
    assert!(ripples >= 4, "{ripples}");
    // On the beach it soaks the ground as wide as its splash.
    let sand = Vec3::new(2.0, ground(2.0, 10.0), 10.0);
    let mut orb = orb::Orb::new(100, sand + Vec3::Y * 4.0, lab.time);
    orb.radius = 2.0;
    orb.state = orb::State::Flying { target: sand };
    orb.velocity = Vec3::new(0.0, -6.0, 0.0);
    lab.orbs.push(orb);
    for _ in 0..30 {
        lab.tick(1.0 / 60.0, spawn(), Vec3::NEG_Z);
    }
    let (wet, amount) = lab.spells.wet.unwrap();
    assert!(wet.distance(Vec2::new(sand.x, sand.z)) < 1.0);
    assert!(amount > 0.9 && lab.spells.wet_radius > 2.0);
}

/// Stands the caster so a hovering orb about 4 m in radius surrounds the
/// beach's straw dummy, drops a crate inside it, and returns the lab and
/// the caster's feet.
fn orb_round_the_dummy() -> (WaterLab, Vec3) {
    let mut lab = quiet_lab();
    let dummy = lab.targets[0].dummy.pos;
    let z = dummy.z + 5.2;
    let feet = Vec3::new(dummy.x, ground(dummy.x, z), z);
    let seconds = (4.0 - orb::MIN_RADIUS) / orb::GROW_FROM_WATER;
    hold_orb(&mut lab, feet, Vec3::NEG_Z, seconds);
    lab.release(Slot::WaterOrb, true, feet, Vec3::NEG_Z)
        .unwrap();
    let center = lab.orbs[0].center;
    lab.floats.spawn(
        FloatKind::Crate,
        center + Vec3::new(1.0, 1.0, 0.0),
        0.0,
        Vec3::ZERO,
    );
    (lab, feet)
}

/// A hovering orb carries what it engulfs: a dummy and a crate inside it
/// stay inside, borne up off the ground and nearly still.
#[test]
fn what_the_orb_engulfs_stays_inside_and_floats() {
    let (mut lab, feet) = orb_round_the_dummy();
    assert!(lab.orbs[0].hovering());
    for _ in 0..(60 * 6) {
        lab.tick(1.0 / 60.0, feet, Vec3::NEG_Z);
    }
    let orb = lab.orbs[0].clone();
    assert_eq!(orb.dummies, vec![0]);
    assert_eq!(orb.floats.len(), 1);
    let (_, crate_at, crate_vel) = lab.floats.states().next().unwrap();
    assert!(orb.contains(crate_at, 0.0), "{crate_at} vs {}", orb.center);
    assert!(crate_vel.length() < 2.5, "{crate_vel}");
    let dummy = &lab.targets[0].dummy;
    assert!(orb.contains(dummy.center(), 0.0));
    assert!(dummy.pos.y > ground(dummy.pos.x, dummy.pos.z) + 0.8);
    // It drifts with the orb's water rather than falling.
    assert!(lab.targets[0].vel.length() < 1.5);
}

/// A Thunderbolt on the orb shocks the dummy and the crate inside it, and
/// no dummy outside it.
#[test]
fn a_bolt_on_the_orb_shocks_exactly_what_is_inside() {
    let (mut lab, feet) = orb_round_the_dummy();
    for _ in 0..120 {
        lab.tick(1.0 / 60.0, feet, Vec3::NEG_Z);
    }
    let before: Vec<f32> = lab.targets.iter().map(|t| t.dummy.hp).collect();
    let orb = lab.orbs[0].clone();
    // The pointer aims at the orb from above the caster.
    let eye = feet + Vec3::new(0.0, 3.0, 3.0);
    lab.set_aim(Some((eye, (orb.center - eye).normalize())));
    let line = lab.press(Slot::Thunderbolt, false, feet, Vec3::NEG_Z, 0.0);
    assert!(line.contains("electrifies 2 inside"), "{line}");
    assert!(lab.orbs[0].charge > 0.99);
    assert!(!lab.jolts.is_empty());
    for (k, target) in lab.targets.iter().enumerate() {
        if k == 0 {
            assert!(target.dummy.hp < before[k], "the dummy inside is shocked");
        } else {
            assert_eq!(target.dummy.hp, before[k], "dummy {k} is outside");
        }
    }
    assert!(lab.floaters.iter().any(|f| f.text.parse::<i32>().is_ok()));
    // The struck orb lights the cove, and its arcs draw.
    assert!(lab.stage().lamps.iter().any(|l| l.intensity > 0.0));
    assert!(!lab.mesh(feet + Vec3::Y * 2.0).lines.is_empty());
}

/// Lightning in the sea conducts 6 m through the same water: a wading
/// dummy 5 m away takes half on a failed save, one 6.5 m away takes
/// nothing, and the river conducts to its own wader alone.
#[test]
fn lightning_conducts_six_meters_through_the_same_water() {
    let mut lab = quiet_lab();
    for _ in 0..30 {
        lab.tick(1.0 / 60.0, spawn(), Vec3::NEG_Z);
    }
    let wader = lab.targets[3].dummy.pos;
    let hp = |lab: &WaterLab| -> Vec<f32> { lab.targets.iter().map(|t| t.dummy.hp).collect() };
    let at = |lab: &WaterLab, x: f32, z: f32| {
        let h = lab.surface_at(Vec2::new(x, z)).unwrap().height;
        Vec3::new(x, h, z)
    };
    // 6.5 m away: out of reach.
    let before = hp(&lab);
    let point = at(&lab, wader.x - 6.5, wader.z);
    let hit = lab.resolve(point, None);
    assert!(
        matches!(hit, bolt::Hit::Water { river: false, .. }),
        "{hit:?}"
    );
    lab.strike(hit);
    assert_eq!(hp(&lab), before);
    // 5 m away, the save failed: half the bolt.
    lab.dice.force_save(3, 1).unwrap();
    let point = at(&lab, wader.x - 5.0, wader.z);
    let line = lab.strike(lab.resolve(point, None));
    assert!(line.contains("1 through the water"), "{line}");
    let after = hp(&lab);
    let lost = before[3] - after[3];
    assert!((6.0..=60.0).contains(&lost), "{lost}");
    for k in [0, 1, 2, 4, 5] {
        assert_eq!(after[k], before[k], "dummy {k}");
    }
    // The river: its wader 4.6 m downstream of the strike, and nobody else.
    let river = lab.targets[5].dummy.pos;
    assert!(terrain::fresh_water(Vec2::new(river.x, river.z)).is_some());
    lab.dice.force_save(5, 1).unwrap();
    let height = terrain::fresh_water(Vec2::new(-37.02, 23.5)).unwrap().0;
    let hit = lab.resolve(Vec3::new(-37.02, height, 23.5), None);
    assert!(
        matches!(hit, bolt::Hit::Water { river: true, .. }),
        "{hit:?}"
    );
    let before = hp(&lab);
    lab.strike(hit);
    let after = hp(&lab);
    assert!(after[5] < before[5]);
    for k in 0..5 {
        assert_eq!(after[k], before[k], "dummy {k}");
    }
}

/// `Y` turns the sea from calm to moderate to storm and back, each rougher
/// than the last offshore, and the floats' surface over open water is the
/// gameplay band `physics::water` samples.
#[test]
fn the_sea_turns_through_its_states() {
    let mut lab = quiet_lab();
    assert_eq!(sea::SEAS[lab.sea], "moderate");
    // Heights at points across the outer bay over twelve seconds, longer
    // than a storm wave's period.
    let spread = |lab: &mut WaterLab| -> f32 {
        let mut heights = Vec::new();
        for _ in 0..120 {
            lab.tick_world(0.1);
            for x in [-60.0, -20.0, 20.0, 60.0] {
                heights.push(lab.surface_at(Vec2::new(x, -150.0)).unwrap().height);
            }
        }
        let mean = heights.iter().sum::<f32>() / heights.len() as f32;
        (heights.iter().map(|h| (h - mean).powi(2)).sum::<f32>() / heights.len() as f32).sqrt()
    };
    let moderate = spread(&mut lab);
    assert_eq!(lab.turn_sea(), "Storm");
    let storm = spread(&mut lab);
    assert_eq!(lab.turn_sea(), "Calm sea");
    let calm = spread(&mut lab);
    assert!(
        calm < moderate && moderate < storm,
        "{calm} {moderate} {storm}"
    );
    // The floats' surface is the gameplay band physics samples, scaled
    // only by shoaling, which is one over deep water.
    lab.set_sea(1);
    let water = lab.frame_water();
    let spectrum = water.sea_body().spectrum.unwrap();
    let waves = physics::water::WaveSet::calm()
        .with_spectrum(spectrum)
        .unwrap();
    let tick = spectrum.tick_at(f64::from(water.time));
    let rest = Vec2::new(10.0, -200.0);
    let (moved, _) = water.sea_body().spectral(rest, 1.0e4, water.time).unwrap();
    let exact = waves.at_rest(rest.as_dvec2(), &waves.phases(tick));
    assert!((f64::from(moved.y) - exact.height).abs() < 1e-4);
    assert!((f64::from(moved.x) - exact.horizontal.x).abs() < 1e-4);
}
