//! The player's medium in Everglade's water, stepped as the zone steps it
//! ([`super::super::Everglade::move_controlled`]) over the carved ground.

use super::*;
use crate::controller::{InputState, PlayerController, RUN_SPEED};
use verse_world::social::everglade_water::{
    BANK, POND_DEPTHS, POOL_DEPTH, pond_level, run, surface as water_surface,
};
use verse_world::water::{DEFEAT_LEVEL, hold_seconds};

use super::super::HALF_EXTENT;
use super::super::solids::Solids;

const DT: f32 = 1.0 / 60.0;

/// One step of the zone's walking branch: the medium's pace, the
/// controller over the solids, then the medium.
fn step(swim: &mut Swim, player: &mut PlayerController, solids: &Solids, input: &InputState) {
    player.set_pace(swim.pace());
    let feet = player.pos.y;
    solids.step(player, input, DT, HALF_EXTENT, true);
    swim.after_step(player, input, feet, solids, DT);
}

fn forward() -> InputState {
    InputState {
        forward: true,
        ..InputState::default()
    }
}

/// A player at `(x, z)` on the ground facing `(dx, dz)`.
fn standing(x: f32, z: f32, dx: f32, dz: f32) -> PlayerController {
    let ground = height(x, z);
    let mut player = PlayerController::new(Vec3::new(x, ground, z), dx.atan2(dz));
    player.set_surface_height(ground);
    player
}

/// A swimmer floating at `(x, z)` facing `(dx, dz)`.
fn floating(x: f32, z: f32, dx: f32, dz: f32) -> PlayerController {
    let top = water_surface(x, z).expect("water");
    let mut player = standing(x, z, dx, dz);
    let y = top - FLOAT_DEPTH as f32;
    player.set_surface_height(height(x, z));
    player.hold_altitude(y);
    player
}

#[test]
fn walking_into_each_pond_wades_then_swims_with_no_bank_blocker() {
    let solids = Solids::over(height);
    for (k, &([cx, cz], r)) in PONDS.iter().enumerate() {
        let mut swim = Swim::default();
        let mut player = standing(cx - (r + BANK + 1.0), cz, 1.0, 0.0);
        let mut seen: Vec<Medium> = vec![Medium::Ground];
        for _ in 0..600 {
            let was = swim.medium;
            step(&mut swim, &mut player, &solids, &forward());
            if seen.last() != Some(&swim.medium) {
                seen.push(swim.medium);
            }
            // A step that starts wading goes at half speed.
            if was == Medium::Wading {
                assert_eq!(player.pace(), 0.5, "wading is Difficult Terrain");
            }
            if swim.medium == Medium::Swimming {
                break;
            }
        }
        let wading = seen.iter().position(|m| *m == Medium::Wading);
        let swimming = seen.iter().position(|m| *m == Medium::Swimming);
        assert!(
            matches!((wading, swimming), (Some(w), Some(s)) if w < s),
            "{}: {seen:?}",
            ew::POND_NAMES[k]
        );
        // Nothing stopped the walker at the bank: it is well into the water.
        assert!((player.pos.x - cx).hypot(player.pos.z - cz) < r - 0.5);
    }
}

#[test]
fn diving_reaches_lantern_ponds_three_meter_bed_and_buoyancy_lifts_back() {
    let solids = Solids::over(height);
    let ([cx, cz], _) = PONDS[0];
    let level = pond_level(0);
    let mut swim = Swim::default();
    swim.set_pitch(1.35);
    let mut player = floating(cx - 0.4, cz, 1.0, 0.0);
    let mut deepest = f32::INFINITY;
    let mut dived = false;
    for _ in 0..90 {
        step(&mut swim, &mut player, &solids, &forward());
        deepest = deepest.min(player.pos.y);
        dived |= swim.medium == Medium::Diving;
    }
    assert!(dived);
    assert!(
        deepest <= level - POND_DEPTHS[0] + 0.1,
        "{deepest} vs the bed at {}",
        level - POND_DEPTHS[0]
    );
    assert!(swim.under && swim.bar().is_some());
    // Let go: buoyancy brings the swimmer back to the float line.
    for _ in 0..600 {
        step(&mut swim, &mut player, &solids, &InputState::default());
    }
    assert_eq!(swim.medium, Medium::Swimming);
    assert!(
        (player.pos.y - (level - FLOAT_DEPTH as f32)).abs() < 0.02,
        "{} {:?} bed {}",
        player.pos,
        swim,
        height(player.pos.x, player.pos.z)
    );
    assert!(!swim.under);
}

#[test]
fn a_swimmer_climbs_out_on_a_shallow_bank_and_onto_a_low_lip_only() {
    let ([cx, cz], r) = PONDS[0];
    let level = pond_level(0);
    // Up the bowl's shallow bank onto the land.
    let solids = Solids::over(height);
    let mut swim = Swim::default();
    let mut player = floating(cx, cz, 1.0, 0.0);
    for _ in 0..400 {
        step(&mut swim, &mut player, &solids, &forward());
    }
    assert_eq!(swim.medium, Medium::Ground);
    assert!(player.pos.x - cx > r, "{}", player.pos);
    assert!((player.pos.y - height(player.pos.x, player.pos.z)).abs() < 0.01);
    // A jetty step half a meter over the water: climbed with no check.
    let lip = |top: f32| {
        let mut solids = Solids::over(height);
        solids.add_block(
            crate::controller::Footprint {
                min: [cx + 1.0, cz - 1.0],
                max: [cx + 2.0, cz + 1.0],
            },
            top,
        );
        let mut swim = Swim::default();
        let mut player = floating(cx - 1.0, cz, 1.0, 0.0);
        for _ in 0..90 {
            step(&mut swim, &mut player, &solids, &forward());
            if swim.medium == Medium::Ground {
                break;
            }
        }
        (swim.medium, player.pos)
    };
    let (medium, at) = lip(level + 0.5);
    assert_eq!(medium, Medium::Ground);
    assert!((at.y - (level + 0.5)).abs() < 1e-4, "{at}");
    // Too high a lip holds the swimmer in the water.
    let (medium, at) = lip(level + 0.9);
    assert_eq!(medium, Medium::Swimming, "{at}");
    assert!(at.x < cx + 1.0);
}

#[test]
fn a_swimmer_in_the_plunge_pool_drifts_at_the_flow_fields_speed() {
    let solids = Solids::over(height);
    let [px, pz] = run().pool;
    let top = water_surface(px, pz).unwrap();
    assert!((top - height(px, pz) - POOL_DEPTH).abs() < 0.01);
    let mut swim = Swim::default();
    let mut player = floating(px, pz, 1.0, 0.0);
    // Settle onto the float line first.
    step(&mut swim, &mut player, &solids, &InputState::default());
    assert_eq!(swim.medium, Medium::Swimming);
    let mut path = vec![player.pos];
    for _ in 0..30 {
        step(&mut swim, &mut player, &solids, &InputState::default());
        path.push(player.pos);
    }
    for pair in path.windows(2) {
        let moved = Vec2::new(pair[1].x - pair[0].x, pair[1].z - pair[0].z) / DT;
        let [fx, fz] = ew::current(pair[0].x, pair[0].z);
        let flow = Vec2::new(fx, fz);
        assert!(flow.length() > 0.05, "{flow}");
        assert!((moved - flow).length() < 1e-3, "{moved} vs {flow}");
    }
}

#[test]
fn wading_across_glade_run_goes_at_half_speed() {
    let solids = Solids::over(height);
    let run = run();
    let along = 30.0;
    let [x, z] = run.point_at(along);
    let [tx, tz] = run.tangent_at(along);
    let (nx, nz) = (-tz, tx);
    let off = run.half_at(along) + BANK + 0.6;
    let mut swim = Swim::default();
    let mut player = standing(x - nx * off, z - nz * off, nx, nz);
    let mut waded = 0;
    for _ in 0..240 {
        let before = player.pos;
        let medium = swim.medium;
        step(&mut swim, &mut player, &solids, &forward());
        assert_ne!(swim.medium, Medium::Swimming);
        if medium == Medium::Wading && swim.medium == Medium::Wading {
            let speed = Vec2::new(player.pos.x - before.x, player.pos.z - before.z).length() / DT;
            assert!((speed - RUN_SPEED * 0.5).abs() < 0.05, "{speed}");
            waded += 1;
        }
    }
    assert!(waded > 5, "{waded}");
    // Across and out on the far bank.
    let crossed = (player.pos.x - x) * nx + (player.pos.z - z) * nz;
    assert!(crossed > run.half_at(along) + BANK, "{crossed}");
    assert_eq!(swim.medium, Medium::Ground);
}

#[test]
fn a_held_breath_runs_out_then_exhaustion_defeats_the_swimmer_onto_the_bank() {
    let solids = Solids::over(height);
    let ([cx, cz], _) = PONDS[0];
    let mut swim = Swim::default();
    swim.set_pitch(1.35);
    let mut player = floating(cx, cz, 1.0, 0.0);
    // Keep diving in place: forward along the bed, turning so it stays in
    // the deep middle.
    let limit = hold_seconds(CON_MODIFIER);
    let mut t = 0.0_f32;
    let mut levels = Vec::new();
    let mut defeated_at = None;
    while defeated_at.is_none() && t < limit + 60.0 {
        let input = InputState {
            forward: true,
            left: true,
            ..InputState::default()
        };
        step(&mut swim, &mut player, &solids, &input);
        t += DT;
        if swim.breath.levels() > levels.len() as u8 {
            levels.push(t);
        }
        if swim.defeats > 0 {
            defeated_at = Some(t);
        }
    }
    assert_eq!(levels.len(), usize::from(DEFEAT_LEVEL) - 1, "{levels:?}");
    let under_at = limit;
    for (k, at) in levels.iter().enumerate() {
        let expected = under_at + 6.0 * (k + 1) as f32;
        // Diving down takes a moment before the eye is under.
        assert!(*at >= expected && *at < expected + 1.0, "{k}: {at}");
    }
    let defeated = defeated_at.expect("the sixth level defeats");
    assert!((defeated - levels[0] - 30.0).abs() < 0.1, "{defeated}");
    // On the bank, dry, with no suffocation levels left.
    assert!(water_surface(player.pos.x, player.pos.z).is_none());
    assert_eq!(swim.breath.levels(), 0);
    assert_eq!(swim.medium, Medium::Ground);
}

#[test]
fn the_water_bakes_over_the_carved_beds_and_draws_from_below_when_the_eye_is_in_it() {
    let surface = surface().unwrap();
    assert_eq!(surface.patches.len(), PONDS.len() + 2);
    for (k, &([cx, cz], _)) in PONDS.iter().enumerate() {
        let patch = &surface.patches[k];
        let center = patch
            .vertices
            .iter()
            .min_by(|a, b| {
                let d = |v: &WaterVertex| (v.pos[0] - cx).hypot(v.pos[2] - cz);
                d(a).total_cmp(&d(b))
            })
            .unwrap();
        assert!((center.depth - POND_DEPTHS[k]).abs() < 0.1, "{k}");
        assert!(
            patch.vertices.iter().any(|v| v.depth < 0.0),
            "the patch ends at its shore"
        );
    }
    let run_patch = &surface.patches[RUN_BODY];
    assert!(run_patch.vertices.iter().any(|v| v.foam > 0.8));
    assert!(
        run_patch
            .vertices
            .iter()
            .filter(|v| v.depth > 0.0)
            .all(|v| v.flow != [0.0; 2] || v.shore < 0.5)
    );
    let sheet = surface.patches.last().unwrap();
    assert!(sheet.vertices.iter().all(|v| v.kind == Kind::Fall.code()));
    let mut water = frame(3.0);
    assert_eq!(water.count, PONDS.len() + 1);
    assert!(water.valid());
    let ([cx, cz], _) = PONDS[2];
    see_from(&mut water, Vec3::new(cx, pond_level(2) - 1.0, cz));
    assert!(water.bodies[2].eye_inside);
    assert!(!water.bodies[0].eye_inside && !water.bodies[RUN_BODY].eye_inside);
    see_from(&mut water, Vec3::new(cx, pond_level(2) + 1.0, cz));
    assert!(!water.bodies[2].eye_inside);
}

#[test]
fn the_frame_carries_the_surface_over_the_eye_and_motes_under_it() {
    let mut water = frame(3.0);
    let ([cx, cz], _) = PONDS[1];
    // Over the pond the frame has the surface the renderer splits the view
    // at, level here, and motes only once the eye is under it.
    see_from(&mut water, Vec3::new(cx, pond_level(1) + 0.5, cz));
    let eye = water.eye.expect("the surface over the eye");
    assert_eq!(eye.body, 1);
    assert!((eye.height - pond_level(1)).abs() < 0.01, "{eye:?}");
    assert!(eye.slope.iter().all(|s| s.abs() < 0.05));
    assert!(water.valid());
    assert!(motes(&water, Vec3::new(cx, pond_level(1) + 0.5, cz)).is_empty());
    let under = Vec3::new(cx, pond_level(1) - 1.0, cz);
    see_from(&mut water, under);
    let specks = motes(&water, under);
    assert!(!specks.is_empty());
    assert!(specks.iter().all(|m| m.at.y < pond_level(1)));
    // On dry land there is none.
    see_from(&mut water, Vec3::new(cx + 60.0, 5.0, cz + 60.0));
    assert!(water.eye.is_none());
}

#[test]
fn a_swimmer_writes_its_wake_into_the_ripple_field_and_splashes_in_and_drips_out() {
    let solids = Solids::over(height);
    let ([cx, cz], _) = PONDS[0];
    let mut swim = Swim::default();
    let mut player = floating(cx - 2.0, cz, 1.0, 0.0);
    // Treading water rings the surface gently.
    for _ in 0..240 {
        step(&mut swim, &mut player, &solids, &InputState::default());
    }
    assert_eq!(swim.medium, Medium::Swimming);
    let still = swim.mover().expect("a swimmer crosses the surface");
    assert!(Vec2::from(still.velocity).length() < 0.2);
    // Stroking, it leaves a wake: a moving source the field draws a
    // Kelvin wedge behind.
    for _ in 0..90 {
        step(&mut swim, &mut player, &solids, &forward());
    }
    let stroke = swim.mover().unwrap();
    assert!(Vec2::from(stroke.velocity).length() > verse_pbr::water::ripple::WAKE_SPEED);
    assert!(stroke.strength > still.strength && stroke.foam > still.foam);
    let mut water = frame(500.0);
    swim.ring(&mut water);
    assert_eq!(water.sources().len(), 1);
    assert!(water.valid() && water.wet.is_some());
    // The field's water: the pond is wet, dry ground beside it is not.
    let wet = water.wet.unwrap();
    assert!((wet.0)(cx, cz).is_some());
    assert!((wet.0)(cx, cz - 20.0).is_none());
    // Dry ground makes none.
    let mut walker = Swim::default();
    let mut player = standing(cx, cz - 20.0, 0.0, -1.0);
    for _ in 0..60 {
        step(&mut walker, &mut player, &solids, &forward());
    }
    assert!(walker.mover().is_none());
    // Jumping in from a height splashes.
    let mut diver = Swim::default();
    let top = water_surface(cx, cz).unwrap();
    let mut player = PlayerController::new(Vec3::new(cx, top + 2.5, cz), 0.0);
    player.set_surface_height(height(cx, cz));
    for _ in 0..120 {
        step(&mut diver, &mut player, &solids, &InputState::default());
    }
    let events = diver.take_events();
    assert!(
        events
            .iter()
            .any(|e| matches!(e, WaterEvent::Splash { speed, .. } if *speed > 3.0)),
        "{events:?}"
    );
}

/// A stress scenario: a crowd jumping in at once. The water's effects stay
/// within each tier's particle budget, skipping what would overrun it, and
/// a bigger, faster body draws a bigger splash.
#[test]
fn splashes_stay_within_each_tier_s_particle_budget() {
    use verse_engine::quality::Tier;
    for (tier, budget) in [(Tier::Low, 64), (Tier::Medium, 256), (Tier::High, 512)] {
        let mut fx = WaterFx::new(tier);
        assert_eq!(fx.budget(), budget);
        let mut peak = 0;
        for frame in 0..120 {
            for k in 0..6 {
                let at = Vec3::new(k as f32, 0.0, frame as f32 * 0.1);
                let name = ["water_entry_splash", "water_wade", "water_droplets"][k % 3];
                fx.start(name, crate::fx::Spawn::at(at));
            }
            fx.particles.tick(DT, |_, _| -10.0);
            peak = peak.max(fx.len());
        }
        assert!(peak <= budget, "{tier:?}: {peak} of {budget}");
        assert!(peak > budget / 2, "{tier:?}: the budget was used: {peak}");
        assert!(fx.skipped > 0);
    }
    assert!(splash_scale(8.0, 80.0) > splash_scale(2.0, 80.0));
    assert!(splash_scale(4.0, 300.0) > splash_scale(4.0, 20.0));
}

#[test]
fn every_pond_admits_reflection_and_refraction_above_and_at_eye_level() {
    use verse_pbr::water::{body_bounds, screen};
    let surface = surface().unwrap();
    let vertices: Vec<_> = surface
        .patches
        .iter()
        .flat_map(|p| p.vertices.iter().copied())
        .collect();
    let bounds = body_bounds(&vertices);
    let water = frame(0.0);
    for tier in [
        verse_engine::quality::Tier::Medium,
        verse_engine::quality::Tier::High,
    ] {
        let plan = screen::Plan::of(tier);
        assert!(plan.copies && plan.mirror_divisor > 0);
        for (k, &([x, z], radius)) in PONDS.iter().enumerate() {
            for height in [0.5, 8.0] {
                let level = pond_level(k);
                let eye = Vec3::new(x, level + height, z + radius + 1.0);
                let target = Vec3::new(x, level, z);
                let view = glam::Mat4::perspective_rh(60.0_f32.to_radians(), 1.6, 0.1, 200.0)
                    * glam::Mat4::look_at_rh(eye, target, Vec3::Y);
                assert_eq!(
                    screen::pick(&water.bodies[..water.count], &bounds, view, eye),
                    Some(k),
                    "{tier:?}, {}, eye height {height}, bounds {:?}",
                    ew::POND_NAMES[k],
                    bounds[k]
                );
            }
        }
    }
}
