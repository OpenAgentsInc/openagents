use glam::DVec3;

use crate::orbit::{self, AU, L1, State, StationOrbit};
use crate::station::{self, Command, PartKind, PartState, Station};

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
    station.astronaut.pos = DVec3::new(40.0, 20.0, 40.0);
    let start_prop = station.propellant;
    let dv = station.delta_v_remaining();
    // Ideal rocket equation for 20 kg of nitrogen on 250 kg at Isp 70 s.
    assert!((dv - 70.0 * 9.806_65 * (250.0_f64 / 230.0).ln()).abs() < 1e-9);
    fly(&mut station, DVec3::X, 20.0);
    assert!((station.astronaut.vel.x - station::SPEED_LIMIT).abs() < 0.01);
    let used = start_prop - station.propellant;
    // Accelerating 250 kg to 2 m/s uses m dv / (Isp g0), about 0.73 kg.
    let expected = 250.0 * 2.0 / (70.0 * 9.806_65);
    assert!(
        (used - expected).abs() / expected < 0.03,
        "{used} vs {expected}"
    );
    fly(&mut station, DVec3::ZERO, 20.0);
    assert!(station.astronaut.vel.length() < 0.01, "hold nulls velocity");
    // Out of propellant, the astronaut drifts with no friction.
    station.propellant = 0.0;
    station.astronaut.vel = DVec3::new(0.0, 0.0, -0.5);
    let before = station.astronaut.pos;
    fly(&mut station, DVec3::X, 4.0);
    assert!((station.astronaut.pos - before - DVec3::new(0.0, 0.0, -2.0)).length() < 1e-3);
}

#[test]
fn carrying_mass_changes_acceleration_and_momentum_is_conserved() {
    let mut station = Station::new();
    station.astronaut.pos = station::DEPOT + DVec3::new(2.5, 0.0, -5.0) - DVec3::Y * 0.2;
    station.yaw = -std::f64::consts::FRAC_PI_2;
    let empty = station::THRUST / station.mass();
    let kind = station.grab().unwrap();
    assert_eq!(kind, PartKind::MainEngine);
    let loaded = station::THRUST / station.mass();
    assert!(loaded < empty * 0.4, "the 450 kg engine slows the pack");
    // Release in free space: the part keeps the combined velocity and tumbles.
    station.astronaut.vel = DVec3::new(0.3, 0.0, 0.0);
    station.release().unwrap();
    let part = station.parts.iter().find(|p| p.kind == kind).unwrap();
    assert_eq!(part.state, PartState::Drifting);
    assert_eq!(part.body.vel, DVec3::new(0.3, 0.0, 0.0));
    // Capture: momentum before equals momentum after.
    let mut station2 = station.clone();
    station2.astronaut.vel = DVec3::new(-0.2, 0.0, 0.0);
    station2.astronaut.pos = part.body.pos - station2.hands() + station2.astronaut.pos;
    let before = station2.astronaut.vel * station2.astronaut.mass + part.body.vel * part.body.mass;
    station2.grab().unwrap();
    let after = station2.astronaut.vel * station2.astronaut.mass;
    assert!((before - after).length() < 1e-9);
}

#[test]
fn latching_requires_position_and_low_closing_speed() {
    let mut station = Station::new();
    station.astronaut.pos = station::DEPOT + DVec3::new(2.5, 0.0, -5.0) - DVec3::Y * 0.2;
    station.yaw = -std::f64::consts::FRAC_PI_2;
    let kind = station.grab().unwrap();
    // Place the held part right at its slot, moving too fast.
    let offset = kind.slot() - station.parts[0].body.pos;
    station.astronaut.pos += offset;
    station.step(
        1.0 / 60.0,
        &Command {
            direction: DVec3::ZERO,
            yaw: station.yaw,
            climb: false,
        },
    );
    station.astronaut.vel = DVec3::new(1.0, 0.0, 0.0);
    assert!(!station.snapshot().latch_ready);
    let mut slow = station.clone();
    slow.astronaut.vel = DVec3::new(0.1, 0.0, 0.0);
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
    station.astronaut.pos = DVec3::new(0.0, 6.0, 24.0);
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
    assert!(station.astronaut.pos.z >= 17.0 + station::ASTRONAUT_RADIUS - 1e-6);
}
