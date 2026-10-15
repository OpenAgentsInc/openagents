//! Step time for 200 mixed convex hulls (rocks, hexagonal prisms, and box
//! hulls) poured into a fixed box with walls, the #11656 acceptance scene.
//! Prints the mean and worst step while the pile falls and settles, and
//! once it is at rest. Run with `--release`.
use std::time::Instant;

use glam::{DQuat, DVec3};
use physics::{Body, BodyKind, Collider, ConvexHull, HullRef, Shape, Uniform, World};

fn fixed_box(world: &mut World, center: DVec3, half: DVec3) {
    let id = world.add(Body::new(1.0, DVec3::ONE, center).with_kind(BodyKind::Static));
    world.add_collider(Collider::new(id, Shape::Cuboid { half }));
}

fn main() {
    let count: usize = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(200);
    let mut world = World::new(1.0 / 120.0);
    let (wide, wall) = (2.0, 0.25);
    fixed_box(
        &mut world,
        DVec3::new(0.0, -wall, 0.0),
        DVec3::new(wide + 2.0 * wall, wall, wide + 2.0 * wall),
    );
    for (x, z, hx, hz) in [
        (wide + wall, 0.0, wall, wide + 2.0 * wall),
        (-wide - wall, 0.0, wall, wide + 2.0 * wall),
        (0.0, wide + wall, wide, wall),
        (0.0, -wide - wall, wide, wall),
    ] {
        fixed_box(&mut world, DVec3::new(x, 3.0, z), DVec3::new(hx, 3.0, hz));
    }
    let prism = {
        let points: Vec<DVec3> = (0..12)
            .map(|k| {
                let a = std::f64::consts::TAU * f64::from(k / 2) / 6.0;
                DVec3::new(
                    0.12 * a.cos(),
                    if k % 2 == 0 { -0.08 } else { 0.08 },
                    0.12 * a.sin(),
                )
            })
            .collect();
        HullRef::new(ConvexHull::new(&points).expect("prism"))
    };
    let shapes: Vec<HullRef> = (0..8_u64)
        .map(|k| match k % 4 {
            0 | 1 => {
                HullRef::new(ConvexHull::rock(k, DVec3::new(0.16, 0.11, 0.13), 24).expect("rock"))
            }
            2 => prism,
            _ => HullRef::new(ConvexHull::cuboid(DVec3::new(0.12, 0.07, 0.1)).expect("box")),
        })
        .collect();
    for n in 0..count {
        let (layer, cell) = (n / 49, n % 49);
        let pos = DVec3::new(
            (cell % 7) as f64 * 0.5 - 1.5,
            0.6 + layer as f64 * 0.45,
            (cell / 7) as f64 * 0.5 - 1.5,
        );
        let turn = DQuat::from_euler(
            glam::EulerRot::XYZ,
            n as f64 * 0.7,
            n as f64 * 1.3,
            n as f64 * 0.4,
        );
        let (id, _) = world.add_hull_body(shapes[n % shapes.len()], 800.0, pos, turn);
        world[id].vel = DVec3::new(0.0, -12.0 * ((n % 5) as f64 / 4.0), 0.0);
    }
    let g = Uniform(DVec3::new(0.0, -9.81, 0.0));
    let phase = |label: &str, steps: usize, world: &mut World| {
        let (mut total, mut worst, mut contacts) = (0.0_f64, 0.0_f64, 0);
        for _ in 0..steps {
            let start = Instant::now();
            world.step(&g);
            let ms = start.elapsed().as_secs_f64() * 1e3;
            total += ms;
            worst = worst.max(ms);
            contacts = contacts.max(world.stats.contact_points);
        }
        println!(
            "{label}: {count} hulls, {steps} steps, mean {:.3} ms, worst {:.3} ms, \
             up to {contacts} contact points, {} asleep at the end",
            total / steps as f64,
            worst,
            world.stats.asleep
        );
    };
    phase("falling (4 s)", 480, &mut world);
    phase("settling (4 s)", 480, &mut world);
    phase("at rest (2 s)", 240, &mut world);
}
