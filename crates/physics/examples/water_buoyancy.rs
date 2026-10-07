//! Times water's forces on 64 floating bodies against the 1 ms step budget.
//!
//! Run with `cargo run --release -p physics --example water_buoyancy`. The
//! bodies (spheres, capsules, and cuboids in equal numbers) float on an
//! ocean with the full eight Gerstner terms, the costliest surface, so
//! every cuboid fits its plane to four corner samples each step.

use std::time::{Duration, Instant};

use glam::{DQuat, DVec2, DVec3};
use physics::water::{self, Water, WaterBody, WaterId, WaterSet, WaveSet};
use physics::{Body, Collider, Shape, Uniform, World};

const DT: f64 = 1.0 / 120.0;
const BODIES: usize = 64;
const STEPS: usize = 120 * 20;

fn main() {
    let waves = WaveSet::seeded(11, DT, 120 * 60, 8, DVec2::new(1.0, 0.4), 12.0, 0.2, 0.6)
        .expect("valid waves");
    let set = WaterSet::new(
        vec![WaterBody::ocean(WaterId(0), 0.0).with_waves(waves)],
        8.0,
    );
    let mut world = World::new(DT);
    for i in 0..BODIES {
        let shape = match i % 3 {
            0 => Shape::Sphere { radius: 0.35 },
            1 => Shape::Capsule {
                radius: 0.25,
                half_length: 0.5,
            },
            _ => Shape::Cuboid {
                half: DVec3::new(0.4, 0.2, 0.6),
            },
        };
        let mass = 600.0 * water::volume(&shape);
        let at = DVec3::new((i % 8) as f64 * 3.0, 0.5, (i / 8) as f64 * 3.0);
        let mut body = Body::new(mass, DVec3::splat(mass * 0.1), at);
        body.orientation = DQuat::from_rotation_y(i as f64 * 0.4);
        body.prev_orientation = body.orientation;
        let id = world.add(body);
        world.add_collider(Collider::new(id, shape));
    }
    let gravity = Uniform(DVec3::new(0.0, -9.81, 0.0));
    let mut forces = Vec::with_capacity(STEPS);
    let mut steps = Duration::ZERO;
    for _ in 0..STEPS {
        let tick = world.tick;
        let started = Instant::now();
        let pushes = water::apply(&mut world, &set, tick, DT);
        forces.push(started.elapsed());
        std::hint::black_box(pushes);
        let started = Instant::now();
        world.step(&gravity);
        steps += started.elapsed();
    }
    // Skip the first second while the bodies drop in.
    let mut settled = forces[120..].to_vec();
    settled.sort();
    let mean = settled.iter().sum::<Duration>() / settled.len() as u32;
    let p99 = settled[settled.len() * 99 / 100];
    let afloat = world
        .bodies()
        .iter()
        .filter(|b| {
            let surface = set
                .sample(b.pos.x, b.pos.z, world.tick)
                .map_or(0.0, |s| s.height);
            (b.pos.y - surface).abs() < 0.6
        })
        .count();
    println!("water forces on {BODIES} bodies, {} steps", settled.len());
    println!("  mean {:>8.1} us per step", mean.as_secs_f64() * 1e6);
    println!("  p99  {:>8.1} us per step", p99.as_secs_f64() * 1e6);
    println!(
        "  max  {:>8.1} us per step",
        settled.last().unwrap().as_secs_f64() * 1e6
    );
    println!(
        "  world step mean {:.1} us; {afloat} of {BODIES} bodies afloat at the end",
        steps.as_secs_f64() * 1e6 / STEPS as f64
    );
}
