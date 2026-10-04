//! Levitate at the physics level: a hall with a floor, a pillar, and a far
//! wall, a 75 kg dummy capsule, and a 20 kg crate.

use super::*;
use glam::DQuat;
use physics::ledger::Momentum;
use physics::trace::{Tolerance, Trace};
use physics::{Body, Material, Shape, Uniform};

const DT: f64 = 1.0 / 120.0;
const DUMMY_MASS: f64 = 75.0;
const DUMMY_RADIUS: f64 = 0.35;
const DUMMY_HEIGHT: f64 = 1.8;
/// The far wall's face, m.
const WALL_X: f64 = 11.5;
const WALL_RESTITUTION: f64 = 0.5;

struct Hall {
    world: World,
    dummy: BodyId,
    pillar: BodyId,
}

fn fixed_box(world: &mut World, center: DVec3, half: DVec3, material: Material) -> BodyId {
    let id = world.add(Body::new(1.0, DVec3::ONE, center).with_kind(BodyKind::Static));
    world.add_collider(Collider::new(id, Shape::Cuboid { half }).with_material(material));
    id
}

/// A dummy standing on the floor 0.2 m from the pillar's +x face.
fn hall() -> Hall {
    let mut world = World::new(DT);
    fixed_box(
        &mut world,
        DVec3::new(0.0, -0.5, 0.0),
        DVec3::new(40.0, 0.5, 40.0),
        Material::default(),
    );
    let pillar = fixed_box(
        &mut world,
        DVec3::new(0.0, 5.0, 0.0),
        DVec3::new(0.5, 5.0, 0.5),
        Material::default(),
    );
    fixed_box(
        &mut world,
        DVec3::new(WALL_X + 0.5, 5.0, 0.0),
        DVec3::new(0.5, 5.0, 10.0),
        Material {
            restitution: WALL_RESTITUTION,
            ..Material::default()
        },
    );
    let dummy = add_dummy(&mut world, DVec3::new(0.5 + 0.2 + DUMMY_RADIUS, 0.9, 0.0));
    Hall {
        world,
        dummy,
        pillar,
    }
}

fn add_dummy(world: &mut World, at: DVec3) -> BodyId {
    // A creature stays upright: its inertia is large enough that contacts
    // do not tip it.
    let id = world.add(Body::new(DUMMY_MASS, DVec3::splat(1e6), at));
    world.add_collider(
        Collider::new(
            id,
            Shape::Capsule {
                radius: DUMMY_RADIUS,
                half_length: DUMMY_HEIGHT / 2.0 - DUMMY_RADIUS,
            },
        )
        .at(
            DVec3::ZERO,
            DQuat::from_rotation_x(std::f64::consts::FRAC_PI_2),
        ),
    );
    id
}

fn add_crate(world: &mut World, at: DVec3) -> BodyId {
    let half = DVec3::splat(0.3);
    let id = world.add(Body::new(20.0, Body::box_inertia(20.0, half * 2.0), at));
    world.add_collider(Collider::new(id, Shape::Cuboid { half }));
    id
}

/// One fixed step: the spell's forces, optional movement input, gravity in
/// the ledger, and the world step.
fn step(
    world: &mut World,
    lev: &Levitation,
    id: BodyId,
    input: DVec3,
    ledger: &mut Ledger,
) -> DVec3 {
    lev.drive(world, id, GRAVITY, ledger);
    let dv = lev.steer(world, id, input, &|_| true, ledger);
    if world[id].responds() {
        let pos = world[id].pos;
        ledger.add_impulse(
            terms::GRAVITY,
            DVec3::NEG_Y * GRAVITY * world[id].mass * world.dt,
            pos,
        );
    }
    world.step(&Uniform(DVec3::NEG_Y * GRAVITY));
    dv
}

fn horizontal(v: DVec3) -> DVec3 {
    DVec3::new(v.x, 0.0, v.z)
}

/// Lift the hall's dummy to the full 20 feet and let it settle.
fn lifted() -> (Hall, Levitation) {
    let mut hall = hall();
    let base = resting_height(&hall.world, hall.dummy).unwrap();
    let lev = Levitation::new(base, MAX_RISE, false, 0.0);
    let mut ledger = Ledger::default();
    for _ in 0..(12 * 120) {
        step(&mut hall.world, &lev, hall.dummy, DVec3::ZERO, &mut ledger);
    }
    (hall, lev)
}

#[test]
fn the_weight_limit_and_loose_objects_are_enforced() {
    let never = || panic!("objects make no save");
    assert_eq!(
        admit(
            Subject::Object {
                mass: 300.0,
                secured: false
            },
            10.0,
            never
        ),
        Err(Refusal::TooHeavy { mass: 300.0 })
    );
    assert!(
        admit(
            Subject::Object {
                mass: 226.0,
                secured: false
            },
            10.0,
            never
        )
        .is_ok()
    );
    assert!(
        admit(
            Subject::Object {
                mass: 20.0,
                secured: false
            },
            10.0,
            never
        )
        .is_ok()
    );
    assert_eq!(
        admit(
            Subject::Object {
                mass: 20.0,
                secured: true
            },
            10.0,
            never
        ),
        Err(Refusal::Secured)
    );
    // Creatures are not weight-limited.
    assert_eq!(
        admit(
            Subject::Creature {
                willing: true,
                constitution: 0
            },
            10.0,
            never
        ),
        Ok(None)
    );
    assert!((WEIGHT_LIMIT - 226.796).abs() < 1e-3);
    let refusal = admit(
        Subject::Object {
            mass: 300.0,
            secured: false,
        },
        10.0,
        never,
    )
    .unwrap_err();
    assert_eq!(refusal.reason(), "too heavy: 661 lb (300 kg) > 500 lb");
}

#[test]
fn an_unwilling_creature_that_saves_is_unaffected() {
    let cultist = Subject::Creature {
        willing: false,
        constitution: 1,
    };
    let saved = admit(cultist, 10.0, || 14).unwrap_err();
    assert_eq!(
        saved,
        Refusal::Saved(Save {
            roll: 14,
            modifier: 1,
            dc: SPELL_SAVE_DC,
            success: true
        })
    );
    let failed = admit(cultist, 10.0, || 13).unwrap().unwrap();
    assert!(!failed.success);
    assert_eq!(
        admit(cultist, RANGE + 0.1, || 1),
        Err(Refusal::OutOfRange {
            distance: RANGE + 0.1
        })
    );
}

#[test]
fn the_rise_is_twenty_feet_or_less_and_no_faster_than_the_hold_speed() {
    let mut hall = hall();
    let base = resting_height(&hall.world, hall.dummy).unwrap();
    assert!((base - 0.9).abs() < 1e-9, "{base}");
    // Asking for more than 20 feet gets 20 feet.
    let lev = Levitation::new(base, 100.0, false, 0.0);
    assert_eq!(lev.rise, MAX_RISE);
    let mut ledger = Ledger::default();
    let (mut highest, mut fastest) = (f64::MIN, 0.0_f64);
    for _ in 0..(15 * 120) {
        step(&mut hall.world, &lev, hall.dummy, DVec3::ZERO, &mut ledger);
        let body = hall.world[hall.dummy];
        highest = highest.max(body.pos.y - base);
        fastest = fastest.max(body.vel.y);
    }
    assert!(highest <= MAX_RISE + 1e-3, "{highest}");
    assert!(fastest <= RISE_SPEED + 1e-9, "{fastest}");
    let rise = hall.world[hall.dummy].pos.y - base;
    assert!((rise - MAX_RISE).abs() < 1e-3, "{rise}");
}

#[test]
fn input_without_a_surface_in_reach_does_nothing() {
    let mut world = World::new(DT);
    fixed_box(
        &mut world,
        DVec3::new(0.0, -0.5, 0.0),
        DVec3::new(40.0, 0.5, 40.0),
        Material::default(),
    );
    let dummy = add_dummy(&mut world, DVec3::new(6.0, 0.9, 0.0));
    let lev = Levitation::new(0.9, MAX_RISE, true, 0.0);
    let mut ledger = Ledger::default();
    for _ in 0..(10 * 120) {
        step(&mut world, &lev, dummy, DVec3::ZERO, &mut ledger);
    }
    let before = world[dummy].pos;
    for i in 0..(3 * 120) {
        let input = DVec3::new((i as f64).cos(), 0.0, (i as f64).sin());
        let dv = step(&mut world, &lev, dummy, input, &mut ledger);
        assert_eq!(dv, DVec3::ZERO);
    }
    let body = world[dummy];
    assert_eq!(horizontal(body.vel), DVec3::ZERO);
    assert_eq!(horizontal(body.pos), horizontal(before));
    // Lowered to the floor, the floor is in reach: the target can only crawl
    // along it at climbing speed.
    let mut low = lev;
    low.command(-MAX_RISE, 20.0).unwrap();
    for _ in 0..(8 * 120) {
        step(&mut world, &low, dummy, DVec3::ZERO, &mut ledger);
    }
    let dv = step(&mut world, &low, dummy, DVec3::X, &mut ledger);
    assert!((dv - DVec3::X * CLIMB_SPEED).length() < 1e-9, "{dv}");
}

#[test]
fn a_push_off_in_reach_moves_the_target_no_faster_than_climbing() {
    let (mut hall, lev) = lifted();
    let mut ledger = Ledger::default();
    let surface = push_surface(&hall.world, hall.dummy, &|_| true).unwrap();
    assert_eq!(surface.body, hall.pillar);
    assert!((surface.gap - 0.2).abs() < 1e-6, "{}", surface.gap);
    assert!((surface.normal - DVec3::X).length() < 1e-9);
    let mut fastest = 0.0_f64;
    for _ in 0..60 {
        step(&mut hall.world, &lev, hall.dummy, DVec3::X, &mut ledger);
        fastest = fastest.max(horizontal(hall.world[hall.dummy].vel).length());
    }
    assert!(fastest <= CLIMB_SPEED + 1e-9, "{fastest}");
    assert!(fastest > CLIMB_SPEED * 0.99, "{fastest}");
    // Diagonal input into the pillar keeps only its component along the
    // pillar face: a pull along the surface.
    let (mut hall, lev) = lifted();
    let dv = step(
        &mut hall.world,
        &lev,
        hall.dummy,
        DVec3::new(-1.0, 0.0, 1.0).normalize(),
        &mut Ledger::default(),
    );
    assert!(
        dv.x.abs() < 1e-9 && (dv.z - CLIMB_SPEED).abs() < 1e-9,
        "{dv}"
    );
}

/// Ledger over the dummy alone, from the moment it pushes off.
fn drifting() -> (Hall, Levitation, Ledger) {
    let (mut hall, lev) = lifted();
    let origin = DVec3::ZERO;
    let mut ledger = Ledger::new(origin, Momentum::of(&hall.world[hall.dummy], origin));
    step(&mut hall.world, &lev, hall.dummy, DVec3::X, &mut ledger);
    (hall, lev, ledger)
}

#[test]
fn momentum_persists_and_the_ledger_shows_no_hidden_source() {
    let (mut hall, lev, mut ledger) = drifting();
    let v0 = hall.world[hall.dummy].vel.x;
    assert!((v0 - CLIMB_SPEED).abs() < 0.05, "{v0}");
    let start = hall.world.time();
    // Two seconds of drift across the open hall, well short of the wall.
    for _ in 0..(2 * 120) {
        step(&mut hall.world, &lev, hall.dummy, DVec3::ZERO, &mut ledger);
        let error = ledger.error(Momentum::of(&hall.world[hall.dummy], ledger.origin));
        assert!(error.linear < 1e-9 && error.angular < 1e-9, "{error:?}");
    }
    let t = hall.world.time() - start;
    let v = hall.world[hall.dummy].vel.x;
    let expected = v0 * (-DRIFT_DAMPING * t).exp();
    assert!((v - expected).abs() < 1e-3, "{v} vs {expected}");
    assert!(v > v0 * 0.9, "momentum persists: {v}");
    // Only the named terms moved it.
    let names: Vec<&str> = ledger.external.keys().map(String::as_str).collect();
    assert_eq!(
        names,
        [terms::GRAVITY, terms::DAMPING, terms::HOLD, terms::PUSH_OFF]
    );
}

#[test]
fn a_drifting_target_bounces_off_the_far_wall_at_reduced_speed() {
    let (mut hall, lev, mut ledger) = drifting();
    let (mut incoming, mut outgoing) = (0.0, 0.0);
    for _ in 0..(8 * 120) {
        let before = hall.world[hall.dummy].vel.x;
        step(&mut hall.world, &lev, hall.dummy, DVec3::ZERO, &mut ledger);
        let after = hall.world[hall.dummy].vel.x;
        if before > 0.0 && after < 0.0 {
            (incoming, outgoing) = (before, -after);
        }
    }
    assert!(incoming > 2.0, "reached the wall at {incoming}");
    assert!(outgoing > 0.0, "bounced");
    // Drifting back, still slowed only by the light damping.
    assert!(hall.world[hall.dummy].vel.x < 0.0);
    let ratio = outgoing / incoming;
    assert!(
        (WALL_RESTITUTION - 0.1..WALL_RESTITUTION + 0.05).contains(&ratio),
        "{outgoing} / {incoming} = {ratio}"
    );
}

#[test]
fn a_blown_object_keeps_drifting_and_leaving_sixty_feet_ends_the_spell() {
    let mut world = World::new(DT);
    fixed_box(
        &mut world,
        DVec3::new(0.0, -0.5, 0.0),
        DVec3::new(60.0, 0.5, 60.0),
        Material::default(),
    );
    let caster = DVec3::new(0.0, 0.9, 0.0);
    let crate_ = add_crate(&mut world, DVec3::new(10.0, 0.3, 0.0));
    let distance = caster.distance(world[crate_].pos);
    assert!(
        admit(
            Subject::Object {
                mass: 20.0,
                secured: false
            },
            distance,
            || 0
        )
        .is_ok()
    );
    let base = resting_height(&world, crate_).unwrap();
    let mut lev = Levitation::new(base, 2.0, false, 0.0);
    let mut ledger = Ledger::default();
    for _ in 0..(6 * 120) {
        step(&mut world, &lev, crate_, DVec3::ZERO, &mut ledger);
    }
    // A scripted Thunderwave-like shove: an external impulse.
    let at = world[crate_].pos;
    world[crate_].apply_impulse_at(DVec3::X * 20.0 * 4.0, at);
    let mut ended_at = None;
    for _ in 0..(10 * 120) {
        let now = world.time();
        lev.update(now, caster.distance(world[crate_].pos));
        if !lev.holding() && ended_at.is_none() {
            ended_at = Some(caster.distance(world[crate_].pos));
        }
        step(&mut world, &lev, crate_, DVec3::ZERO, &mut ledger);
    }
    let ended_at = ended_at.expect("the spell ended");
    assert_eq!(lev.phase, Phase::Descending(End::OutOfRange));
    assert!(
        (RANGE..RANGE + 0.1).contains(&ended_at),
        "ended {ended_at} m from the caster"
    );
    // Still drifting after the spell ended, slowed only by the floor.
    assert!(world[crate_].pos.x > 10.0 + RANGE - 10.0);
}

#[test]
fn the_end_of_the_spell_floats_the_target_down_gently() {
    let (mut hall, mut lev) = lifted();
    let mut ledger = Ledger::default();
    let top = hall.world[hall.dummy].pos.y;
    lev.end(End::Concentration);
    assert!(lev.gentle());
    let mut fastest = 0.0_f64;
    for _ in 0..(5 * 120) {
        step(&mut hall.world, &lev, hall.dummy, DVec3::ZERO, &mut ledger);
        fastest = fastest.max(-hall.world[hall.dummy].vel.y);
    }
    assert!(fastest <= FEATHER_FALL_SPEED + 1e-9, "{fastest}");
    assert!(fastest > FEATHER_FALL_SPEED * 0.99);
    let landed = hall.world[hall.dummy].pos.y;
    assert!((landed - lev.base).abs() < 0.02, "{landed}");
    // A 20-foot drop that would deal 2d6 lands without damage.
    assert!(top - landed > 6.0);
    lev.land();
    assert_eq!(lev.phase, Phase::Done(End::Concentration));
    assert!(lev.gentle());
}

#[test]
fn altitude_changes_by_twenty_feet_once_per_turn() {
    let (mut hall, mut lev) = lifted();
    let mut ledger = Ledger::default();
    let now = hall.world.time();
    assert_eq!(lev.command(-ALTITUDE_STEP, now), Ok(0.0));
    assert!(matches!(
        lev.command(ALTITUDE_STEP, now + 1.0),
        Err(AltitudeRefusal::ThisTurn { .. })
    ));
    assert_eq!(lev.command(7.0, now + TURN), Err(AltitudeRefusal::TooFar));
    for _ in 0..(6 * 120) {
        step(&mut hall.world, &lev, hall.dummy, DVec3::ZERO, &mut ledger);
    }
    let low = hall.world[hall.dummy].pos.y;
    assert!((low - lev.base).abs() < 0.1, "{low}");
    assert_eq!(lev.command(ALTITUDE_STEP, now + TURN), Ok(MAX_RISE));
    for _ in 0..(6 * 120) {
        step(&mut hall.world, &lev, hall.dummy, DVec3::ZERO, &mut ledger);
    }
    let high = hall.world[hall.dummy].pos.y;
    assert!((high - lev.base - MAX_RISE).abs() < 0.1, "{high}");
    // A self-target moves freely within the band.
    let mut own = Levitation::new(0.9, MAX_RISE, true, 0.0);
    assert_eq!(own.command(-1.0, 1.0), Ok(MAX_RISE - 1.0));
    assert_eq!(own.command(1.0, 1.0), Ok(MAX_RISE));
}

#[test]
fn a_checkpoint_mid_drift_replays_identically() {
    let (mut hall, lev, mut ledger) = drifting();
    for _ in 0..90 {
        step(&mut hall.world, &lev, hall.dummy, DVec3::ZERO, &mut ledger);
    }
    let saved = serde_json::to_string(&(&hall.world, &lev, &ledger)).unwrap();
    let (mut world, restored, mut restored_ledger): (World, Levitation, Ledger) =
        serde_json::from_str(&saved).unwrap();
    assert_eq!(world, hall.world);
    assert_eq!(restored, lev);
    let (mut a, mut b) = (Trace::default(), Trace::default());
    // Through the wall bounce.
    for _ in 0..(6 * 120) {
        step(&mut hall.world, &lev, hall.dummy, DVec3::ZERO, &mut ledger);
        step(
            &mut world,
            &restored,
            hall.dummy,
            DVec3::ZERO,
            &mut restored_ledger,
        );
        a.record(&hall.world);
        b.record(&world);
    }
    a.compare(&b, Tolerance::EXACT).unwrap();
    assert_eq!(ledger, restored_ledger);
}
