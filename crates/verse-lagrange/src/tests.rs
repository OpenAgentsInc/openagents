use glam::DVec3;

use crate::orbit::{self, AU, L1, State, StationOrbit};
use crate::station::{self, Command, Input, PartKind, PartState, Station};

#[test]
fn l1_sits_where_the_sun_earth_system_puts_it() {
    let l1 = L1::sun_earth();
    let km = l1.gamma * AU / 1_000.0;
    // L1 is about 1.5 million km sunward of the Earth–Moon barycenter.
    assert!((1.49e6..1.51e6).contains(&km), "{km} km");
    // At L1 the rotating-frame acceleration of a body at rest is zero.
    let rest = State {
        pos: l1.point(),
        vel: DVec3::ZERO,
    };
    assert!(orbit::acceleration(l1.mu, &rest).length() < 1e-12);
    // Richardson's coefficient and the linear frequencies for Sun–Earth L1.
    assert!((l1.c2 - 4.0610).abs() < 2e-3, "c2 {}", l1.c2);
    assert!((l1.lambda - 2.5327).abs() < 2e-3, "lambda {}", l1.lambda);
    assert!((l1.omega_p - 2.0864).abs() < 2e-3, "omega_p {}", l1.omega_p);
    let snapshot = StationOrbit::new().snapshot();
    // Linear periods near 178 days and an instability e-folding near 23 days.
    assert!((170.0..185.0).contains(&snapshot.lissajous_days));
    assert!((20.0..26.0).contains(&snapshot.instability_days));
    // Light takes about five seconds to reach Earth.
    assert!((4.5..5.6).contains(&snapshot.light_seconds));
}

#[test]
fn the_unstable_left_eigenvector_is_exact() {
    let l1 = L1::sun_earth();
    // Seed a pure unstable-mode displacement and check the projection.
    let a = 1.0 + 2.0 * l1.c2;
    let tau = (l1.lambda * l1.lambda - a) / (2.0 * l1.lambda);
    let eps = 1e-7;
    let state = State {
        pos: l1.point() + DVec3::new(eps, eps * tau, 0.0),
        vel: DVec3::new(eps * l1.lambda, eps * l1.lambda * tau, 0.0),
    };
    assert!((l1.unstable_component(&state) - eps).abs() < 1e-15);
    // Linear Lissajous states have no unstable component.
    let orbit = l1.lissajous(1e-4, 5e-4, 0.3, 1.1);
    assert!(l1.unstable_component(&orbit).abs() < 1e-15);
}

#[test]
fn uncontrolled_motion_conserves_jacobi_and_leaves_l1() {
    let mut orbit = StationOrbit::new();
    orbit.keeping_interval = f64::INFINITY;
    let c0 = orbit::jacobi(orbit.l1.mu, &orbit.state);
    // Nudge off the center manifold by 1 cm/s.
    orbit.state.vel.x += 0.01 / AU * orbit::time_unit();
    orbit.advance(400.0 * 86_400.0);
    let offset = (orbit.state.pos - orbit.l1.point()).length() * AU / 1_000.0;
    assert!(
        offset > 1.0e6,
        "the collinear point is unstable: {offset} km"
    );
    let mut probe = StationOrbit::new();
    probe.keeping_interval = f64::INFINITY;
    probe.advance(60.0 * 86_400.0);
    let drift = (orbit::jacobi(probe.l1.mu, &probe.state) - c0).abs();
    assert!(drift / c0 < 1e-9, "Jacobi drift {drift}");
}

#[test]
fn station_keeping_holds_a_lissajous_orbit_for_two_years() {
    let mut orbit = StationOrbit::new();
    let mut max_offset: f64 = 0.0;
    for _ in 0..(730 / 5) {
        orbit.advance(5.0 * 86_400.0);
        let offset = (orbit.state.pos - orbit.l1.point()).length() * AU / 1_000.0;
        max_offset = max_offset.max(offset);
    }
    assert!(max_offset < 600_000.0, "bounded: {max_offset} km");
    let per_year = orbit.keeping_dv / 2.0;
    eprintln!(
        "station-keeping {per_year:.2} m/s per year, {} burns",
        orbit.burns
    );
    // Flight halo controllers spend a few m/s per year. Cancelling only the
    // linear unstable mode costs more, but stays within ten.
    assert!(per_year > 1.0 && per_year < 10.0, "{per_year} m/s per year");
    assert!(orbit.burns > 10);
    let snapshot = orbit.snapshot();
    assert!((1.2e6..1.8e6).contains(&snapshot.earth_distance_km));
    assert!(snapshot.sev_degrees < 10.0);
}

#[test]
fn the_field_near_the_station_is_microgravity() {
    let station = Station::new();
    let a = station.field(DVec3::new(50.0, 50.0, 50.0), DVec3::ZERO);
    assert!(a.length() < 1e-10 && a.length() > 0.0, "{a}");
}

fn fly(station: &mut Station, direction: DVec3, seconds: f64) {
    let command = Command {
        direction,
        yaw: station.yaw,
        climb: false,
    };
    for _ in 0..(seconds * 60.0) as usize {
        station.step(1.0 / 60.0, &command);
    }
}

#[test]
fn the_pack_obeys_the_rocket_equation_and_holds_position() {
    let mut station = Station::new();
    station.astronaut_mut().pos = DVec3::new(40.0, 20.0, 40.0);
    let start_prop = station.propellant;
    let dv = station.delta_v_remaining();
    // Ideal rocket equation for 20 kg of nitrogen on 250 kg at Isp 70 s.
    assert!((dv - 70.0 * 9.806_65 * (250.0_f64 / 230.0).ln()).abs() < 1e-9);
    fly(&mut station, DVec3::X, 20.0);
    assert!((station.astronaut().vel.x - station::SPEED_LIMIT).abs() < 0.01);
    let used = start_prop - station.propellant;
    // Accelerating 250 kg to 2 m/s uses m dv / (Isp g0), about 0.73 kg.
    let expected = 250.0 * 2.0 / (70.0 * 9.806_65);
    assert!(
        (used - expected).abs() / expected < 0.03,
        "{used} vs {expected}"
    );
    fly(&mut station, DVec3::ZERO, 20.0);
    assert!(
        station.astronaut().vel.length() < 0.01,
        "hold nulls velocity"
    );
    // Out of propellant, the astronaut drifts with no friction.
    station.propellant = 0.0;
    station.astronaut_mut().vel = DVec3::new(0.0, 0.0, -0.5);
    let before = station.astronaut().pos;
    fly(&mut station, DVec3::X, 4.0);
    assert!((station.astronaut().pos - before - DVec3::new(0.0, 0.0, -2.0)).length() < 1e-3);
}

#[test]
fn carrying_mass_changes_acceleration_and_the_part_moves_with_the_grip() {
    let mut station = Station::new();
    station.astronaut_mut().pos = station::DEPOT + DVec3::new(2.5, 0.0, -5.0) - DVec3::Y * 0.2;
    station.face(-std::f64::consts::FRAC_PI_2);
    let empty = station::THRUST / station.mass();
    let kind = station.grab().unwrap();
    assert_eq!(kind, PartKind::MainEngine);
    let loaded = station::THRUST / station.mass();
    assert!(loaded < empty * 0.4, "the 450 kg engine slows the pack");
    // Fly out of the rack with the engine: the grip drags it along.
    let away = Command {
        direction: DVec3::X,
        yaw: station.yaw,
        climb: false,
    };
    for _ in 0..(6 * 60) {
        station.step(1.0 / 60.0, &away);
    }
    let engine = *station.body(&station.parts[0]);
    let astronaut = *station.astronaut();
    // Thrust through the pair's center of mass is capped by the steering
    // torque budget, so a heavy load accelerates gently.
    assert!(astronaut.vel.x > 0.05, "{}", astronaut.vel);
    // The pair turns a little under the off-center load, so compare the
    // direction of travel rather than whole velocities.
    assert!(
        (engine.vel.x - astronaut.vel.x).abs() < 0.01,
        "held: {} vs {}",
        engine.vel,
        astronaut.vel
    );
    // Release in free space: the part keeps its own motion.
    station.release().unwrap();
    assert_eq!(station.parts[0].state, PartState::Drifting);
    assert_eq!(station.body(&station.parts[0]).vel, engine.vel);
}

#[test]
fn capturing_a_drifting_part_conserves_momentum() {
    let mut station = open_space();
    station.propellant = 0.0;
    station.astronaut_mut().vel = DVec3::new(-0.05, 0.0, 0.0);
    let reach = station.hands() + DVec3::new(0.0, 0.0, 1.0);
    release_part(
        &mut station,
        2,
        reach,
        DVec3::new(0.1, 0.02, 0.0),
        DVec3::new(0.0, 0.05, 0.02),
    );
    station.reset_ledger();
    station.grab().unwrap();
    let mut worst = worst_error(&mut station, 4.0, &Command::default());
    let truss = *station.body(&station.parts[2]);
    let astronaut = *station.astronaut();
    let error = station.ledger.error(station.momentum());
    worst.linear = worst.linear.max(error.linear);
    worst.angular = worst.angular.max(error.angular);
    assert!(worst.linear < 1e-10 && worst.angular < 1e-10, "{worst:?}");
    // The pair now moves as one: the truss's velocity at the grip matches the hands.
    let grip = station.world.joint(station.grip.unwrap()).unwrap();
    let (hand, held) = grip.anchors(&station.world);
    let hand_vel = astronaut.vel + astronaut.omega_world().cross(hand - astronaut.pos);
    let held_vel = truss.vel + truss.omega_world().cross(held - truss.pos);
    assert!(
        (hand_vel - held_vel).length() < 1e-3,
        "{hand_vel} vs {held_vel}"
    );
}

#[test]
fn a_grip_pulled_past_its_limit_slips() {
    let mut station = busy_station();
    station.grab().unwrap();
    // Something yanks the engine away at 3 m/s.
    let id = station.parts[0].body;
    station.world[id].vel = DVec3::new(0.0, 0.0, 3.0);
    station.step(1.0 / 60.0, &Command::default());
    assert_eq!(station.parts[0].state, PartState::Drifting);
    assert!(station.grip.is_none());
    assert!(
        station
            .message
            .as_deref()
            .unwrap_or_default()
            .contains("slipped")
    );
}

#[test]
fn latching_requires_position_and_low_closing_speed() {
    let mut station = Station::new();
    station.astronaut_mut().pos = station::DEPOT + DVec3::new(2.5, 0.0, -5.0) - DVec3::Y * 0.2;
    station.face(-std::f64::consts::FRAC_PI_2);
    let kind = station.grab().unwrap();
    // Place the held part right at its slot, moving too fast.
    let offset = kind.slot() - station.body(&station.parts[0]).pos;
    station.translate(offset);
    station.step(
        1.0 / 60.0,
        &Command {
            direction: DVec3::ZERO,
            yaw: station.yaw,
            climb: false,
        },
    );
    station.set_velocity(DVec3::new(1.0, 0.0, 0.0));
    assert!(!station.snapshot().latch_ready);
    let mut slow = station.clone();
    slow.set_velocity(DVec3::new(0.1, 0.0, 0.0));
    assert!(slow.snapshot().latch_ready);
    slow.release().unwrap();
    assert_eq!(slow.parts[0].state, PartState::Installed);
    assert_eq!(slow.snapshot().installed, 1);
    assert_eq!(slow.snapshot().next_part, Some(PartKind::PropellantTank));
    station.release().unwrap();
    assert_eq!(station.parts[0].state, PartState::Drifting);
}

#[test]
fn structure_blocks_the_astronaut() {
    let mut station = Station::new();
    station.astronaut_mut().pos = DVec3::new(0.0, 6.0, 24.0);
    for _ in 0..(30 * 60) {
        station.step(
            1.0 / 60.0,
            &Command {
                direction: -DVec3::Z,
                yaw: std::f64::consts::PI,
                climb: false,
            },
        );
    }
    // The airlock face is at z = 17; the suit stops outside it.
    // Contact allows the solver slop (5 mm) of overlap.
    assert!(station.astronaut().pos.z >= 17.0 + station::ASTRONAUT_RADIUS - 0.01);
}

/// A short EVA: thrust toward the depot, grab the engine, and drift.
fn busy_station() -> Station {
    let mut station = Station::new();
    station.astronaut_mut().pos = station::DEPOT + DVec3::new(2.5, 0.0, -5.0) - DVec3::Y * 0.2;
    station.face(-std::f64::consts::FRAC_PI_2);
    station
}

fn same_physics(a: &Station, b: &Station) {
    assert_eq!(a.world.tick, b.world.tick);
    assert_eq!(a.world.bodies(), b.world.bodies());
    assert_eq!(a.orbit, b.orbit);
    assert_eq!(a.propellant.to_bits(), b.propellant.to_bits());
    assert_eq!(a.parts, b.parts);
}

#[test]
fn frame_pacing_does_not_change_the_physics() {
    let command = Command {
        direction: DVec3::new(0.3, 0.1, 1.0),
        yaw: 0.4,
        climb: false,
    };
    let run = |fps: f64| {
        let mut station = busy_station();
        station.grab().unwrap();
        while station.world.tick < 600 {
            station.step(1.0 / fps, &command);
        }
        // Frames that overshoot the target tick leave the rest unrun; step
        // back to the common tick by replaying exact steps from a fresh run.
        let mut exact = busy_station();
        exact.grab().unwrap();
        for _ in 0..station.world.tick {
            exact.advance(&command);
        }
        same_physics(&station, &exact);
        exact
    };
    let at30 = run(30.0);
    let at60 = run(60.0);
    let at144 = run(144.0);
    assert_eq!(at30.world.tick, 600);
    same_physics(&at30, &at60);
    // 144 fps frames end at tick 600 or just past it; compare at 600.
    let mut fresh = busy_station();
    fresh.grab().unwrap();
    for _ in 0..600 {
        fresh.advance(&command);
    }
    same_physics(&at30, &fresh);
    assert!(at144.world.tick >= 600);
}

#[test]
fn a_long_frame_is_capped_and_counted() {
    let mut station = Station::new();
    station.step(0.5, &Command::default());
    assert_eq!(station.world.tick, u64::from(station::MAX_STEPS_PER_FRAME));
    assert!((station.clock.dropped - 0.4).abs() < 1e-9);
}

#[test]
fn a_saved_station_restores_and_continues_identically() {
    let mut station = busy_station();
    station.grab().unwrap();
    let command = Command {
        direction: DVec3::new(1.0, 0.0, 0.2),
        yaw: 0.1,
        climb: false,
    };
    for _ in 0..90 {
        station.step(1.0 / 60.0, &command);
    }
    station.release().unwrap();
    let json = serde_json::to_string(&station.save()).unwrap();
    let mut restored = Station::restore(serde_json::from_str(&json).unwrap()).unwrap();
    for _ in 0..240 {
        station.step(1.0 / 60.0, &command);
        restored.step(1.0 / 60.0, &command);
    }
    same_physics(&station, &restored);
    let mut stale = station.save();
    stale.version = 0;
    assert!(Station::restore(stale).is_err());
}

#[test]
fn a_recorded_session_replays_from_its_start_state() {
    let mut live = busy_station();
    live.record();
    let start = live.clone();
    let push = Command {
        direction: DVec3::new(1.0, 0.0, 0.0),
        yaw: 0.0,
        climb: false,
    };
    // Local input, then operator actions between frames, as NIP-MV applies them.
    live.apply(Input::Grab).unwrap();
    for frame in 0..200 {
        let command = if frame < 80 { push } else { Command::default() };
        live.step(1.0 / 72.0, &command);
        if frame == 120 {
            live.apply(Input::FlyTo {
                target: DVec3::new(-4.0, -6.0, 8.0),
            })
            .unwrap();
        }
        if frame == 170 {
            live.apply(Input::Release).unwrap();
        }
    }
    live.apply(Input::Stop).unwrap();
    let journal = live.journal.clone().unwrap();
    // The journal survives serialization with the state it started from.
    let json = serde_json::to_string(&journal).unwrap();
    let journal: Vec<(u64, Input)> = serde_json::from_str(&json).unwrap();
    let replayed = Station::replay(&start, &journal, live.world.tick);
    same_physics(&live, &replayed);
    assert!(live.parts[0].state == PartState::Drifting);
}

/// Far from structure, tide off, ledger fresh.
fn open_space() -> Station {
    let mut station = Station::new();
    station.tide = false;
    station.astronaut_mut().pos = DVec3::new(40.0, 30.0, 60.0);
    station.reset_ledger();
    station
}

fn worst_error(station: &mut Station, seconds: f64, command: &Command) -> physics::LedgerError {
    let mut worst = physics::LedgerError {
        linear: 0.0,
        angular: 0.0,
    };
    for _ in 0..(seconds * 60.0) as usize {
        station.step(1.0 / 60.0, command);
        let error = station.ledger.error(station.momentum());
        worst.linear = worst.linear.max(error.linear);
        worst.angular = worst.angular.max(error.angular);
    }
    worst
}

#[test]
fn coasting_conserves_momentum() {
    let mut station = open_space();
    // An empty pack cannot hold position, so the astronaut coasts too.
    station.propellant = 0.0;
    station.astronaut_mut().vel = DVec3::new(0.3, -0.1, 0.2);
    // A tumbling truss released nearby.
    let id = station.parts[2].body;
    station.parts[2].state = PartState::Drifting;
    station.world[id].kind = PartState::Drifting.body_kind();
    station.world[id].pos = DVec3::new(30.0, 30.0, 50.0);
    station.world[id].vel = DVec3::new(-0.05, 0.02, 0.1);
    station.world[id].omega = DVec3::new(0.2, 0.9, -0.1);
    station.reset_ledger();
    let worst = worst_error(&mut station, 20.0, &Command::default());
    assert!(worst.linear < 1e-12 && worst.angular < 1e-12, "{worst:?}");
}

#[test]
fn burns_balance_against_their_exhaust() {
    let mut station = open_space();
    let mut worst = worst_error(
        &mut station,
        6.0,
        &Command {
            direction: DVec3::new(1.0, 0.5, -0.3),
            yaw: 0.7,
            climb: false,
        },
    );
    station.fly_to(DVec3::new(20.0, 10.0, 40.0)).unwrap();
    let hold = worst_error(&mut station, 20.0, &Command::default());
    worst.linear = worst.linear.max(hold.linear);
    worst.angular = worst.angular.max(hold.angular);
    assert!(station.ledger.external["exhaust"].linear.length() > 1.0);
    assert!(worst.linear < 1e-9 && worst.angular < 1e-9, "{worst:?}");
}

#[test]
fn structure_and_the_safety_tether_are_named_external_terms() {
    let mut station = Station::new();
    station.tide = false;
    station.astronaut_mut().pos = DVec3::new(0.0, 6.0, 24.0);
    station.reset_ledger();
    let into_airlock = Command {
        direction: -DVec3::Z,
        yaw: std::f64::consts::PI,
        climb: false,
    };
    let worst = worst_error(&mut station, 10.0, &into_airlock);
    assert!(station.ledger.external["structure"].linear.length() > 1.0);
    assert!(worst.linear < 1e-9 && worst.angular < 1e-9, "{worst:?}");
    let mut station = open_space();
    station.astronaut_mut().pos = station::AIRLOCK + DVec3::new(0.0, 0.0, station::EVA_RANGE - 1.0);
    station.reset_ledger();
    let outward = Command {
        direction: DVec3::Z,
        yaw: 0.0,
        climb: false,
    };
    let worst = worst_error(&mut station, 5.0, &outward);
    assert!(station.ledger.external["tether"].linear.length() > 1.0);
    assert!(worst.linear < 1e-9 && worst.angular < 1e-9, "{worst:?}");
}

/// With the soft grip, carrying and releasing conserve linear and angular
/// momentum (the rigid carry that GP-1 pinned as a baseline did not).
#[test]
fn carrying_conserves_momentum() {
    let mut station = busy_station();
    station.tide = false;
    station.reset_ledger();
    station.grab().unwrap();
    let mut worst = worst_error(
        &mut station,
        6.0,
        &Command {
            direction: DVec3::new(0.2, 0.0, 1.0),
            yaw: 0.9,
            climb: false,
        },
    );
    station.release().unwrap();
    let released = station.ledger.error(station.momentum());
    worst.linear = worst.linear.max(released.linear);
    worst.angular = worst.angular.max(released.angular);
    assert!(worst.linear < 1e-9 && worst.angular < 1e-9, "{worst:?}");
}

#[test]
fn attitude_holds_through_a_translation_and_turns_on_command() {
    let mut station = open_space();
    let heading = station.heading_yaw();
    let across = Command {
        direction: DVec3::new(1.0, 0.3, 0.2),
        yaw: heading,
        climb: false,
    };
    let mut worst: f64 = 0.0;
    for _ in 0..(8 * 60) {
        station.step(1.0 / 60.0, &across);
        let off =
            glam::DQuat::from_rotation_y(heading).angle_between(station.astronaut().orientation);
        worst = worst.max(off);
    }
    assert!(
        worst.to_degrees() < 1.0,
        "attitude drifted {:.3} deg",
        worst.to_degrees()
    );
    assert!(station.astronaut().vel.length() > 1.0);
    // A quarter turn completes and settles within ten seconds.
    let before = station.propellant;
    let turn = Command {
        direction: DVec3::ZERO,
        yaw: heading + std::f64::consts::FRAC_PI_2,
        climb: false,
    };
    for _ in 0..(10 * 60) {
        station.step(1.0 / 60.0, &turn);
    }
    let target = glam::DQuat::from_rotation_y(turn.yaw);
    let off = target
        .angle_between(station.astronaut().orientation)
        .to_degrees();
    assert!(off < 1.0, "{off} deg from the commanded heading");
    assert!(station.astronaut().omega.length() < 0.01);
    assert!(
        before - station.propellant > 0.0,
        "turning costs propellant"
    );
    // Hands follow the body: they sit a meter ahead along the new facing.
    let ahead = station.hands() - station.astronaut().pos;
    assert!((ahead.dot(station::heading(turn.yaw)) - 1.0).abs() < 0.03);
}

#[test]
fn plumes_come_from_the_firing_thrusters() {
    let mut station = open_space();
    let push = Command {
        direction: DVec3::X,
        yaw: station.heading_yaw(),
        climb: false,
    };
    station.step(1.0 / 60.0, &push);
    assert!(!station.plumes.is_empty());
    for plume in &station.plumes {
        let from = DVec3::from(plume.pos) - station.astronaut().pos;
        assert!(from.length() < 1.0, "plume {from} is on the pack");
        // Thrust along +X means exhaust toward -X.
        assert!(DVec3::from(plume.dir).x < -0.99);
    }
}

/// Set part `index` drifting at `pos` with the given motion.
fn release_part(station: &mut Station, index: usize, pos: DVec3, vel: DVec3, omega: DVec3) {
    let id = station.parts[index].body;
    station.parts[index].state = PartState::Drifting;
    station.world[id].kind = PartState::Drifting.body_kind();
    station
        .world
        .collider_mut(station.parts[index].collider)
        .filter = PartState::Drifting.filter();
    station.world[id].pos = pos;
    station.world[id].vel = vel;
    station.world[id].omega = omega;
}

#[test]
fn a_spinning_tank_glances_off_a_solar_array_edge() {
    let mut station = open_space();
    station.propellant = 0.0;
    // The tank lies across the array's inboard edge (x = 13), flying at the
    // pack's top speed toward the array face.
    release_part(
        &mut station,
        1,
        DVec3::new(12.0, 6.0, 4.0),
        DVec3::new(0.0, 0.0, -station::SPEED_LIMIT),
        DVec3::new(0.1, 0.3, 0.2),
    );
    let tank = station.parts[1].body;
    station.world[tank].orientation = glam::DQuat::from_rotation_y(std::f64::consts::FRAC_PI_2);
    station.reset_ledger();
    let spin = station.world[tank].angular_momentum();
    let mut deepest: f64 = 0.0;
    for _ in 0..(6 * 60) {
        station.step(1.0 / 60.0, &Command::default());
        for c in &station.world.contacts {
            deepest = deepest.min(c.separation);
        }
        let error = station.ledger.error(station.momentum());
        assert!(error.linear < 1e-9 && error.angular < 1e-9, "{error:?}");
    }
    let tank = station.world[tank];
    assert!(deepest > -0.05, "penetrated {deepest} m");
    assert!(station.ledger.external["structure"].linear.length() > 100.0);
    assert!(tank.vel.z > 0.0, "bounced back: {}", tank.vel);
    assert!(
        (tank.angular_momentum() - spin).length() > 50.0,
        "the edge hit changed the spin"
    );
}

#[test]
fn free_parts_collide_with_each_other_and_conserve_momentum() {
    let mut station = open_space();
    station.propellant = 0.0;
    release_part(
        &mut station,
        2,
        DVec3::new(-30.0, 30.0, 40.0),
        DVec3::new(1.5, 0.0, 0.1),
        DVec3::new(0.0, 0.2, 0.0),
    );
    release_part(
        &mut station,
        5,
        DVec3::new(-26.0, 30.3, 40.0),
        DVec3::new(-0.5, 0.0, 0.0),
        DVec3::ZERO,
    );
    station.reset_ledger();
    let before = station.body(&station.parts[5]).vel;
    for _ in 0..(5 * 60) {
        station.step(1.0 / 60.0, &Command::default());
        let error = station.ledger.error(station.momentum());
        assert!(error.linear < 1e-12 && error.angular < 1e-12, "{error:?}");
    }
    assert!(
        !station.ledger.external.contains_key("structure"),
        "no fixed body involved"
    );
    assert!(
        (station.body(&station.parts[5]).vel - before).length() > 0.5,
        "the truss struck the avionics bay"
    );
}

/// A held part at its slot, at rest, in `station`.
fn at_slot() -> Station {
    let mut station = busy_station();
    let kind = station.grab().unwrap();
    let offset = kind.slot() - station.body(&station.parts[0]).pos;
    station.translate(offset);
    station.set_velocity(DVec3::ZERO);
    station
}

#[test]
fn misaligned_or_spinning_docks_are_refused() {
    let ready = at_slot();
    assert!(ready.latch_ready());
    // Half a turn about the keel still fits; a quarter turn does not.
    for (turn, fits) in [
        (std::f64::consts::PI, true),
        (std::f64::consts::FRAC_PI_2, false),
    ] {
        let mut station = ready.clone();
        let id = station.parts[0].body;
        station.world[id].orientation = glam::DQuat::from_rotation_z(turn);
        assert_eq!(station.latch_ready(), fits, "turned {turn} rad");
    }
    let mut tilted = ready.clone();
    let id = tilted.parts[0].body;
    tilted.world[id].orientation = glam::DQuat::from_rotation_x(20f64.to_radians());
    tilted.release().unwrap();
    assert_eq!(tilted.parts[0].state, PartState::Drifting);
    assert!(
        tilted
            .message
            .as_deref()
            .unwrap_or_default()
            .starts_with("Misaligned by 20")
    );
    let mut spinning = ready.clone();
    let id = spinning.parts[0].body;
    spinning.world[id].omega = DVec3::new(0.0, 0.0, 0.2);
    spinning.release().unwrap();
    assert_eq!(spinning.parts[0].state, PartState::Drifting);
    assert!(
        spinning
            .message
            .as_deref()
            .unwrap_or_default()
            .starts_with("Spinning")
    );
}

#[test]
fn a_latched_part_stays_welded_under_impact() {
    let mut station = at_slot();
    station.release().unwrap();
    assert_eq!(station.parts[0].state, PartState::Installed);
    let slot = PartKind::MainEngine.slot();
    // Fly clear and let the weld settle the engine onto its seat.
    station.translate(DVec3::new(0.0, 0.0, 30.0));
    for _ in 0..60 {
        station.step(1.0 / 60.0, &Command::default());
    }
    // Throw the aft keel truss into the engine at the pack's top speed.
    release_part(
        &mut station,
        2,
        slot + DVec3::new(4.0, 0.3, 0.0),
        DVec3::new(-station::SPEED_LIMIT, 0.0, 0.0),
        DVec3::new(0.0, 0.3, 0.0),
    );
    let mut worst: (f64, f64) = (0.0, 0.0);
    let mut rebound: f64 = f64::NEG_INFINITY;
    for _ in 0..(4 * 60) {
        station.step(1.0 / 60.0, &Command::default());
        rebound = rebound.max(station.body(&station.parts[2]).vel.x);
        let engine = station.body(&station.parts[0]);
        worst.0 = worst.0.max(engine.pos.distance(slot));
        worst.1 = worst
            .1
            .max(engine.orientation.angle_between(glam::DQuat::IDENTITY));
    }
    assert!(rebound > 0.0, "the truss bounced off");
    assert!(worst.0 < 0.01 && worst.1.to_degrees() < 0.5, "{worst:?}");
}

#[test]
fn the_safety_tether_stops_the_astronaut_without_a_jump_or_energy() {
    let mut station = open_space();
    station.propellant = 0.0;
    let out = DVec3::new(0.6, 0.0, 0.8);
    station.astronaut_mut().pos = station::AIRLOCK + out * (station::EVA_RANGE - 2.0);
    station.astronaut_mut().vel = out * 1.5;
    station.reset_ledger();
    let energy = |s: &Station| 0.5 * s.astronaut().mass * s.astronaut().vel.length_squared();
    let start = energy(&station);
    let momentum = station.momentum();
    let mut last = station.astronaut().pos;
    let mut farthest: f64 = 0.0;
    for _ in 0..(6 * 60) {
        station.step(1.0 / 60.0, &Command::default());
        let pos = station.astronaut().pos;
        // No teleport: each frame moves no farther than the speed allows.
        assert!(
            pos.distance(last) <= 1.5 / 60.0 + 1e-9,
            "jumped {} m",
            pos.distance(last)
        );
        last = pos;
        farthest = farthest.max(pos.distance(station::AIRLOCK));
        assert!(energy(&station) <= start * (1.0 + 1e-9), "energy grew");
    }
    // The tether arrests at most TETHER_TENSION, so a fast arrival
    // stretches it a little rather than stopping in one step.
    assert!(farthest < station::EVA_RANGE + 0.15, "{farthest} m");
    let tension = station.ledger.external["tether"];
    assert!(tension.linear.length() > 100.0);
    // The recorded tension is the whole momentum change.
    let change = station.momentum() - momentum;
    assert!((change.linear - tension.linear).length() < 1e-9);
    assert_eq!(station.message.as_deref(), Some("Safety tether taut"));
}

#[test]
fn latched_parts_sleep_and_free_parts_never_do() {
    let mut station = at_slot();
    station.release().unwrap();
    station.translate(DVec3::new(0.0, 0.0, 30.0));
    // A free part drifting away slowly, nowhere near structure.
    release_part(
        &mut station,
        3,
        DVec3::new(-40.0, 30.0, 40.0),
        DVec3::new(0.002, 0.0, 0.0),
        DVec3::ZERO,
    );
    for _ in 0..(5 * 60) {
        station.step(1.0 / 60.0, &Command::default());
        assert!(
            !station.body(&station.parts[3]).sleeping,
            "a free part slept"
        );
    }
    assert!(
        station.body(&station.parts[0]).sleeping,
        "the latched engine sleeps"
    );
    assert!(station.snapshot().awake_bodies < station.world.bodies().len());
    // Striking the sleeping engine wakes it; the weld still holds it.
    release_part(
        &mut station,
        2,
        PartKind::MainEngine.slot() + DVec3::new(4.0, 0.3, 0.0),
        DVec3::new(-station::SPEED_LIMIT, 0.0, 0.0),
        DVec3::ZERO,
    );
    let mut woke = false;
    for _ in 0..(3 * 60) {
        station.step(1.0 / 60.0, &Command::default());
        woke |= !station.body(&station.parts[0]).sleeping;
    }
    assert!(woke);
    assert!(
        station
            .body(&station.parts[0])
            .pos
            .distance(PartKind::MainEngine.slot())
            < 0.01
    );
}

#[test]
fn the_hud_reads_impact_g_load_spin_and_proximity() {
    let mut station = Station::new();
    station.tide = false;
    // Facing the airlock (-z) from 6 m away, at rest.
    station.astronaut_mut().pos = DVec3::new(0.0, 6.0, 23.5);
    station.face(std::f64::consts::PI);
    station.step(
        1.0 / 60.0,
        &Command {
            direction: DVec3::ZERO,
            yaw: station.yaw,
            climb: false,
        },
    );
    let s = station.snapshot();
    let ahead = s.proximity_m.unwrap();
    assert!(
        (ahead - (23.5 - 17.0 - station::ASTRONAUT_RADIUS)).abs() < 0.5,
        "{ahead}"
    );
    assert!(s.impact_n == 0.0 && s.g_load < 1e-9);
    // Thrusting ahead reads the pack's acceleration: 40 N on about 250 kg.
    let forward = Command {
        direction: -DVec3::Z,
        yaw: station.yaw,
        climb: false,
    };
    station.step(1.0 / 60.0, &forward);
    let g = station.snapshot().g_load;
    let expected = station::THRUST / station.mass() / station::G0;
    assert!((g - expected).abs() / expected < 0.05, "{g} vs {expected}");
    // Hitting the airlock registers an impact; the lines show it.
    let mut hit = 0.0_f64;
    for _ in 0..(20 * 60) {
        station.step(1.0 / 60.0, &forward);
        hit = hit.max(station.snapshot().impact_n);
    }
    assert!(hit > 10.0, "{hit}");
    assert!(
        station
            .debug_lines()
            .iter()
            .any(|l| l.kind == physics::DebugKind::Thrust)
    );
    // A commanded turn reads as spin.
    station.step(
        1.0 / 60.0,
        &Command {
            direction: DVec3::ZERO,
            yaw: 0.0,
            climb: false,
        },
    );
    for _ in 0..30 {
        station.step(
            1.0 / 60.0,
            &Command {
                direction: DVec3::ZERO,
                yaw: 0.0,
                climb: false,
            },
        );
    }
    assert!(station.snapshot().spin_deg_s > 1.0);
}

#[test]
fn grabbing_closes_on_the_part_surface_facing_the_hands() {
    let mut station = busy_station();
    station.grab().unwrap();
    let grip = station.world.joint(station.grip.unwrap()).unwrap();
    let (_, held) = grip.anchors(&station.world);
    let engine = station.body(&station.parts[0]);
    // The engine is a 2.2 m box: the grab point lies on its face toward the
    // hands, 1.1 m from its center along x.
    let local = engine.orientation.inverse() * (held - engine.pos);
    assert!((local.x - 1.1).abs() < 1e-6, "{local}");
}
