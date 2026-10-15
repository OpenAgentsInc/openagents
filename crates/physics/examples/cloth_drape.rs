//! Step time for the #11658 acceptance scene: a 32 x 32 cloth (1,024
//! particles, 12 substeps) dropped on a sphere resting on a floor. Prints
//! the mean and worst cloth step while it falls and drapes, and once it
//! rests. Run with `--release`.
use std::time::Instant;

use glam::DVec3;
use physics::{Body, BodyKind, Cloth, ClothSettings, Collider, Shape, Uniform, World};

fn main() {
    let side: u32 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(32);
    let mut world = World::new(1.0 / 120.0);
    let ball = world
        .add(Body::new(1.0, DVec3::ONE, DVec3::new(0.0, 0.5, 0.0)).with_kind(BodyKind::Static));
    world.add_collider(Collider::new(ball, Shape::Sphere { radius: 0.5 }));
    let floor = world
        .add(Body::new(1.0, DVec3::ONE, DVec3::new(0.0, -0.5, 0.0)).with_kind(BodyKind::Static));
    world.add_collider(Collider::new(
        floor,
        Shape::Cuboid {
            half: DVec3::new(5.0, 0.5, 5.0),
        },
    ));
    let mut cloth = Cloth::grid(
        DVec3::new(-0.8, 1.3, -0.8),
        DVec3::X,
        DVec3::Z,
        side,
        side,
        1.6 / f64::from(side - 1),
        ClothSettings::default(),
    );
    let g = Uniform(DVec3::new(0.0, -9.81, 0.0));
    let mut phase = |label: &str, steps: usize| {
        let (mut total, mut worst) = (0.0_f64, 0.0_f64);
        for _ in 0..steps {
            world.step(&g);
            let start = Instant::now();
            cloth.step(&mut world, &g);
            let ms = start.elapsed().as_secs_f64() * 1e3;
            total += ms;
            worst = worst.max(ms);
        }
        println!(
            "{label}: {} particles, {} substeps, mean {:.3} ms, worst {:.3} ms per 1/120 s step",
            cloth.pos.len(),
            cloth.settings.substeps,
            total / steps as f64,
            worst
        );
    };
    phase("falling and draping (3 s)", 360);
    phase("resting (2 s)", 240);
}
