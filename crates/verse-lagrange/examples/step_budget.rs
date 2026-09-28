//! Physics step budget for Lagrange 1 (GP-8, #9785).
//!
//! Runs a busy scene for 30 simulated seconds and prints per-step wall-clock
//! time: the astronaut carrying the engine under thrust and attitude
//! control, with the other five parts thrown at the station and each other.
//! Run it in release on the machine you want to measure:
//!
//! ```sh
//! cargo run --release -p verse-lagrange --example step_budget
//! ```

use std::time::Instant;

use glam::DVec3;
use verse_lagrange::{Command, PartKind, PartState, Station, station};

fn main() {
    let mut station = Station::new();
    station.astronaut_mut().pos = station::DEPOT + DVec3::new(2.5, 0.0, -5.0) - DVec3::Y * 0.2;
    station.face(-std::f64::consts::FRAC_PI_2);
    station.grab().expect("the engine is in reach");
    for (i, index) in (1..PartKind::ALL.len()).enumerate() {
        let id = station.parts[index].body;
        station.parts[index].state = PartState::Drifting;
        let collider = station.parts[index].collider;
        station.world[id].kind = PartState::Drifting.body_kind();
        station.world.collider_mut(collider).filter = PartState::Drifting.filter();
        let at = DVec3::new(i as f64 * 5.0 - 10.0, 12.0, 4.0);
        station.world[id].pos = at;
        station.world[id].vel = (DVec3::new(0.0, 6.0, 5.0) - at).normalize() * station::SPEED_LIMIT;
        station.world[id].omega = DVec3::new(0.1, 0.3, -0.2);
    }
    let command = Command {
        direction: DVec3::new(0.3, 0.2, 1.0),
        yaw: 0.5,
        climb: false,
    };
    let steps = 30 * 120;
    let mut times = Vec::with_capacity(steps);
    let mut contacts = 0;
    let (mut detect, mut solve, mut world_total) = (0.0, 0.0, 0.0);
    for _ in 0..steps {
        let start = Instant::now();
        station.advance(&command);
        times.push(start.elapsed().as_secs_f64() * 1_000.0);
        let st = station.world.stats;
        detect += st.detect.as_secs_f64();
        solve += st.solve.as_secs_f64();
        world_total += st.total.as_secs_f64();
        contacts = contacts.max(station.world.contacts.len());
    }
    times.sort_by(f64::total_cmp);
    let mean = times.iter().sum::<f64>() / times.len() as f64;
    let at = |q: f64| times[((times.len() - 1) as f64 * q) as usize];
    println!(
        "{steps} steps of {:.2} ms simulated: mean {mean:.3} ms, median {:.3} ms, p99 {:.3} ms, max {:.3} ms; up to {contacts} contact points; {} bodies awake at the end",
        station::PHYSICS_DT * 1_000.0,
        at(0.5),
        at(0.99),
        at(1.0),
        station.world.stats.awake
    );
    let per_step = |seconds: f64| seconds * 1_000.0 / steps as f64;
    println!(
        "rigid-body world: {:.3} ms per step (detect {:.3}, solve {:.3}); the rest is the station's own work",
        per_step(world_total),
        per_step(detect),
        per_step(solve)
    );
}
