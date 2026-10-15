use glam::{DQuat, DVec3};

use super::*;
use crate::parallel::{SERIAL, THREADS};
use crate::{Body, Collider, ConvexHull, HullRef, NoField, Shape, Uniform};

const G: f64 = 9.81;

fn gravity() -> Uniform {
    Uniform(DVec3::new(0.0, -G, 0.0))
}

/// The isometric bending measure: zero for a flat stencil, unchanged by a
/// rigid motion, growing with the fold, and its gradient is exact.
#[test]
fn the_bending_measure_is_flat_zero_rigid_and_its_gradient_exact() {
    let flat = [
        DVec3::new(0.0, 0.0, 0.0),
        DVec3::new(1.0, 1.0, 0.0),
        DVec3::new(1.0, 0.0, 0.0),
        DVec3::new(0.0, 1.0, 0.0),
    ];
    let (k, scale) = isometric(flat).unwrap();
    let bend = Bend {
        ids: [0, 1, 2, 3],
        k,
        scale,
        rest: 0.0,
    };
    assert!(bending(&bend, flat).0.abs() < 1e-12);
    let fold = |angle: f64| {
        let mut x = flat;
        // Fold corner 3 up about the diagonal 0-1.
        let axis = DVec3::new(1.0, 1.0, 0.0).normalize();
        x[3] = DQuat::from_axis_angle(axis, angle) * x[3];
        x
    };
    let (small, big) = (bending(&bend, fold(0.2)).0, bending(&bend, fold(0.8)).0);
    assert!(small > 0.0 && big > small, "{small} {big}");
    let turn = DQuat::from_euler(glam::EulerRot::XYZ, 0.4, -1.0, 2.0);
    let moved = fold(0.5).map(|p| turn * p + DVec3::new(3.0, -1.0, 2.0));
    assert!((bending(&bend, moved).0 - bending(&bend, fold(0.5)).0).abs() < 1e-12);
    let x = fold(0.6);
    let (_, grads) = bending(&bend, x);
    let h = 1e-6;
    for p in 0..4 {
        for axis in 0..3 {
            let (mut plus, mut minus) = (x, x);
            plus[p][axis] += h;
            minus[p][axis] -= h;
            let numeric = (bending(&bend, plus).0 - bending(&bend, minus).0) / (2.0 * h);
            assert!(
                (numeric - grads[p][axis]).abs() < 1e-6,
                "{p} {axis}: {numeric} vs {}",
                grads[p][axis]
            );
        }
    }
}

/// A cloth hung by two corners in gravity: its energy never grows, it
/// stays the length of its edges, and the pins carry its weight.
#[test]
fn a_hanging_cloth_never_gains_energy() {
    let mut world = World::new(1.0 / 120.0);
    let mut cloth = Cloth::grid(
        DVec3::new(0.0, 2.0, 0.0),
        DVec3::X,
        DVec3::NEG_Y,
        16,
        12,
        0.05,
        ClothSettings::default(),
    );
    cloth.pin_here(0, 0);
    cloth.pin_here(15, 0);
    let pinned = cloth.world_pinned();
    let m = cloth.particle_mass();
    let energy = |c: &Cloth| {
        c.kinetic_energy()
            + c.pos
                .iter()
                .zip(&pinned)
                .filter(|(_, p)| !**p)
                .map(|(x, _)| x.y)
                .sum::<f64>()
                * m
                * G
    };
    let start = energy(&cloth);
    for _ in 0..(120 * 6) {
        cloth.step(&mut world, &gravity());
        assert!(
            energy(&cloth) <= start + 1e-9,
            "{} vs {start}",
            energy(&cloth)
        );
        assert!(cloth.pos.iter().all(|p| p.is_finite()));
    }
    // It has swung down and come to rest.
    assert!(cloth.kinetic_energy() < 1e-4, "{}", cloth.kinetic_energy());
    let lowest = cloth.pos.iter().map(|p| p.y).fold(f64::INFINITY, f64::min);
    assert!(lowest < 2.0 - 0.5, "{lowest}");
    // Edges stretch little under its weight.
    let worst = cloth
        .edges
        .iter()
        .filter(|e| e.stretch)
        .map(|e| cloth.pos[e.a as usize].distance(cloth.pos[e.b as usize]) / e.rest - 1.0)
        .fold(0.0, f64::max);
    assert!(worst < 0.05, "stretched {worst}");
}

/// The acceptance scene: a 32 x 32 cloth dropped on a sphere that rests
/// on a floor, with rocks falling beside it so the world has work to
/// split, stepped `seconds`.
fn drape(threads: Option<usize>, seconds: usize) -> (Cloth, World) {
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
    let rock = HullRef::new(ConvexHull::rock(5, DVec3::splat(0.08), 16).unwrap());
    for k in 0..40 {
        let pos = DVec3::new(
            1.2 + 0.2 * f64::from(k % 5),
            0.3 + 0.2 * f64::from(k / 5),
            0.0,
        );
        world.add_hull_body(rock, 2000.0, pos, DQuat::from_rotation_y(f64::from(k)));
    }
    let mut cloth = Cloth::grid(
        DVec3::new(-0.8, 1.3, -0.8),
        DVec3::X,
        DVec3::Z,
        32,
        32,
        1.6 / 31.0,
        ClothSettings::default(),
    );
    if let Some(n) = threads {
        THREADS.store(n, std::sync::atomic::Ordering::Relaxed);
        SERIAL.store(n == 1, std::sync::atomic::Ordering::Relaxed);
    }
    for _ in 0..(120 * seconds) {
        world.step(&gravity());
        cloth.step(&mut world, &gravity());
    }
    if threads.is_some() {
        THREADS.store(0, std::sync::atomic::Ordering::Relaxed);
        SERIAL.store(false, std::sync::atomic::Ordering::Relaxed);
    }
    (cloth, world)
}

#[test]
fn a_cloth_drapes_over_a_sphere_and_rests_without_jitter() {
    let (mut cloth, mut world) = drape(None, 20);
    let center = DVec3::new(0.0, 0.5, 0.0);
    let thickness = cloth.settings.thickness;
    for p in &cloth.pos {
        assert!(
            p.distance(center) > 0.5 + thickness * 0.5,
            "inside the ball: {p}"
        );
        assert!(p.y > -0.001, "through the floor: {p}");
    }
    // Draped: the middle rests on the ball's crown, the corners hang below
    // the equator.
    let middle = cloth.pos[cloth.index(16, 16) as usize];
    assert!(
        (middle.distance(center) - (0.5 + thickness)).abs() < 0.01,
        "{middle}"
    );
    assert!(
        DVec3::new(middle.x, 0.0, middle.z).length() < 0.25,
        "{middle}"
    );
    for corner in [0, 31, 992, 1023] {
        assert!(cloth.pos[corner].y < 0.5, "{}", cloth.pos[corner]);
    }
    // At rest: another second moves nothing more than a few millimetres.
    let start = cloth.pos.clone();
    let mut fastest: f64 = 0.0;
    for _ in 0..120 {
        world.step(&gravity());
        cloth.step(&mut world, &gravity());
        fastest = fastest.max(cloth.vel.iter().map(|v| v.length()).fold(0.0, f64::max));
    }
    let moved = cloth
        .pos
        .iter()
        .zip(&start)
        .map(|(a, b)| a.distance(*b))
        .fold(0.0, f64::max);
    let worst = (0..cloth.pos.len())
        .max_by(|&a, &b| {
            cloth.pos[a]
                .distance(start[a])
                .total_cmp(&cloth.pos[b].distance(start[b]))
        })
        .unwrap();
    assert!(
        moved < 5e-3,
        "crept {moved} m at {worst} ({}, {}) from {} to {}",
        worst % 32,
        worst / 32,
        start[worst],
        cloth.pos[worst]
    );
    assert!(fastest < 0.03, "jittered at {fastest} m/s");
}

#[test]
fn a_draped_scene_is_bit_for_bit_at_one_four_and_eight_threads() {
    let runs: Vec<(Cloth, World)> = [1, 4, 8].into_iter().map(|n| drape(Some(n), 2)).collect();
    for (cloth, world) in &runs[1..] {
        assert_eq!(cloth, &runs[0].0);
        for (a, b) in runs[0].1.bodies().iter().zip(world.bodies()) {
            assert_eq!(
                a.pos.to_array().map(f64::to_bits),
                b.pos.to_array().map(f64::to_bits)
            );
        }
    }
}

/// A banner pinned along its top edge to a free body in zero g: nothing
/// external acts, so the body's momentum plus the cloth's holds; under a
/// wind on the cloth, the body is pulled along.
#[test]
fn a_banner_on_a_free_body_trades_momentum_with_it() {
    let mut world = World::new(1.0 / 120.0);
    let half = DVec3::new(0.6, 0.05, 0.05);
    let mut truss = Body::new(4.0, Body::box_inertia(4.0, half * 2.0), DVec3::ZERO);
    truss.vel = DVec3::new(0.2, 0.0, 0.1);
    truss.omega = DVec3::new(0.0, 0.3, 0.0);
    let id = world.add(truss);
    world.add_collider(Collider::new(id, Shape::Cuboid { half }));
    let mut cloth = Cloth::grid(
        DVec3::new(-0.5, -0.06, 0.0),
        DVec3::X,
        DVec3::new(0.3, -1.0, 0.2),
        11,
        8,
        0.1,
        ClothSettings {
            density: 0.4,
            // No air: a closed system.
            air: 0.0,
            ..ClothSettings::default()
        },
    );
    for i in 0..11 {
        cloth.pin_to_body(&world, id, i, 0);
    }
    let total =
        |world: &World, cloth: &Cloth| world.momentum(DVec3::ZERO) + cloth.momentum(DVec3::ZERO);
    let start = total(&world, &cloth);
    let mut worst: f64 = 0.0;
    let mut pulled: f64 = 0.0;
    for _ in 0..(120 * 4) {
        world.step(&NoField);
        let given = cloth.step(&mut world, &NoField);
        pulled = pulled.max(given.linear.length());
        let now = total(&world, &cloth);
        worst = worst.max((now.linear - start.linear).length());
        assert!(cloth.pos.iter().all(|p| p.is_finite()));
    }
    assert!(pulled > 0.0, "the banner pulled on the truss");
    assert!(worst < 1e-12, "linear momentum drifted {worst}");
    // A steady wind on the cloth drags the truss downwind.
    let before = world[id].vel;
    let wind = |_: DVec3, v: DVec3| (DVec3::new(0.0, 0.0, 3.0) - v) * 0.8;
    for _ in 0..120 {
        world.step(&NoField);
        cloth.step(&mut world, &wind);
    }
    assert!(
        world[id].vel.z > before.z + 0.01,
        "{} vs {}",
        world[id].vel,
        before
    );
}

/// Collisions with every shape: dropped onto a capsule, a box, a hull, and
/// a distance grid, no particle ends inside any of them.
#[test]
fn a_cloth_rests_on_capsules_boxes_hulls_and_grids() {
    let mut world = World::new(1.0 / 120.0);
    let fixed = |world: &mut World, pos: DVec3| {
        world.add(Body::new(1.0, DVec3::ONE, pos).with_kind(BodyKind::Static))
    };
    let a = fixed(&mut world, DVec3::new(-0.6, 0.3, 0.0));
    world.add_collider(Collider::new(
        a,
        Shape::Capsule {
            radius: 0.15,
            half_length: 0.4,
        },
    ));
    let b = fixed(&mut world, DVec3::new(0.0, 0.25, 0.0));
    world.add_collider(Collider::new(
        b,
        Shape::Cuboid {
            half: DVec3::splat(0.25),
        },
    ));
    let c = fixed(&mut world, DVec3::new(0.6, 0.3, 0.0));
    let hull = HullRef::new(ConvexHull::rock(9, DVec3::splat(0.3), 24).unwrap());
    world.add_collider(Collider::new(c, Shape::Hull { hull }));
    let (v, t) = crate::collision::sdf_tests::revolve(
        &[(0.0, -0.2), (1.4, -0.2), (1.4, 0.0), (0.0, 0.0)],
        32,
    );
    let grid = crate::SdfRef::new(crate::SdfGrid::bake(&v, &t, 0.04, 0.08).unwrap());
    let ground = fixed(&mut world, DVec3::ZERO);
    world.add_collider(Collider::new(ground, Shape::Sdf { grid }));
    let mut cloth = Cloth::grid(
        DVec3::new(-1.0, 0.9, -0.4),
        DVec3::X,
        DVec3::Z,
        24,
        10,
        2.0 / 23.0,
        ClothSettings::default(),
    );
    for _ in 0..(120 * 4) {
        world.step(&gravity());
        cloth.step(&mut world, &gravity());
    }
    let surfaces: Vec<_> = world
        .colliders()
        .iter()
        .map(|c| c.surface(&world))
        .collect();
    for p in &cloth.pos {
        for s in &surfaces {
            assert!(s.distance(*p).0 > 0.0, "inside a collider: {p}");
        }
    }
    // It rests on the box's top.
    let on_box = cloth
        .pos
        .iter()
        .filter(|p| p.x.abs() < 0.2 && p.z.abs() < 0.2);
    for p in on_box {
        assert!((p.y - 0.5).abs() < 0.03, "{p}");
    }
}

#[test]
fn self_collision_keeps_a_folded_cloth_apart() {
    let mut world = World::new(1.0 / 120.0);
    let floor = world
        .add(Body::new(1.0, DVec3::ONE, DVec3::new(0.0, -0.5, 0.0)).with_kind(BodyKind::Static));
    world.add_collider(Collider::new(
        floor,
        Shape::Cuboid {
            half: DVec3::new(5.0, 0.5, 5.0),
        },
    ));
    // A long strip that falls folded onto itself over a bar.
    let bar = world
        .add(Body::new(1.0, DVec3::ONE, DVec3::new(0.0, 0.4, 0.0)).with_kind(BodyKind::Static));
    world.add_collider(Collider::new(
        bar,
        Shape::Capsule {
            radius: 0.03,
            half_length: 1.0,
        },
    ));
    let settings = ClothSettings {
        self_collision: true,
        thickness: 0.015,
        ..ClothSettings::default()
    };
    let mut cloth = Cloth::grid(
        DVec3::new(-0.6, 0.5, -0.2),
        DVec3::X,
        DVec3::Z,
        30,
        6,
        0.04,
        settings,
    );
    for _ in 0..(120 * 3) {
        world.step(&gravity());
        cloth.step(&mut world, &gravity());
    }
    let columns = cloth.columns as usize;
    let mut closest = f64::INFINITY;
    for a in 0..cloth.pos.len() {
        for b in (a + 1)..cloth.pos.len() {
            let (ia, ja, ib, jb) = (a % columns, a / columns, b % columns, b / columns);
            if ia.abs_diff(ib) <= 1 && ja.abs_diff(jb) <= 1 {
                continue;
            }
            closest = closest.min(cloth.pos[a].distance(cloth.pos[b]));
        }
    }
    assert!(closest > 0.02, "folds pass through each other: {closest}");
}

#[test]
fn a_restored_cloth_continues_bit_for_bit() {
    let mut world = World::new(1.0 / 120.0);
    let mut cloth = Cloth::grid(
        DVec3::new(0.0, 1.0, 0.0),
        DVec3::X,
        DVec3::NEG_Y,
        8,
        8,
        0.1,
        ClothSettings::default(),
    );
    cloth.pin_here(0, 0);
    for _ in 0..60 {
        cloth.step(&mut world, &gravity());
    }
    let mut restored: Cloth =
        serde_json::from_str(&serde_json::to_string(&cloth).unwrap()).unwrap();
    assert_eq!(restored, cloth);
    let mut other = world.clone();
    for _ in 0..120 {
        cloth.step(&mut world, &gravity());
        restored.step(&mut other, &gravity());
    }
    assert_eq!(restored, cloth);
}
