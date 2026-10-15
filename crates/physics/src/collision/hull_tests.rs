//! Scene tests for hull colliders (#11656): a resting stack, a pile poured
//! into a box, momentum through hull contacts, thread-count independence,
//! and restore.

use glam::{DQuat, DVec3};

use crate::parallel::{SERIAL, THREADS};
use crate::{
    Body, BodyKind, Collider, ConvexHull, HullRef, Ledger, NoField, Shape, Uniform, World,
};

const G: f64 = 9.81;

fn gravity() -> Uniform {
    Uniform(DVec3::new(0.0, -G, 0.0))
}

fn fixed_box(world: &mut World, center: DVec3, half: DVec3) {
    let id = world.add(Body::new(1.0, DVec3::ONE, center).with_kind(BodyKind::Static));
    world.add_collider(Collider::new(id, Shape::Cuboid { half }));
}

/// A hexagonal prism, axis along y: a hull that is not a box.
fn prism(radius: f64, half_height: f64) -> HullRef {
    let mut points = Vec::new();
    for k in 0..6 {
        let a = std::f64::consts::TAU * f64::from(k) / 6.0;
        for y in [-half_height, half_height] {
            points.push(DVec3::new(radius * a.cos(), y, radius * a.sin()));
        }
    }
    HullRef::new(ConvexHull::new(&points).unwrap())
}

#[test]
fn a_stack_of_five_hulls_rests_for_ten_seconds() {
    let mut world = World::new(1.0 / 120.0);
    fixed_box(
        &mut world,
        DVec3::new(0.0, -0.5, 0.0),
        DVec3::new(10.0, 0.5, 10.0),
    );
    let hull = prism(0.3, 0.1);
    let ids: Vec<_> = (0..5)
        .map(|i| {
            let y = 0.1 + 0.2 * f64::from(i);
            let twist = DQuat::from_rotation_y(0.3 * f64::from(i));
            world
                .add_hull_body(hull, 500.0, DVec3::new(0.002 * f64::from(i), y, 0.0), twist)
                .0
        })
        .collect();
    let g = gravity();
    for _ in 0..120 {
        world.step(&g);
    }
    let settled: Vec<DVec3> = ids.iter().map(|&b| world[b].pos).collect();
    let slop = world.solver.slop;
    for _ in 0..(120 * 10) {
        world.step(&g);
    }
    for (i, (&b, start)) in ids.iter().zip(&settled).enumerate() {
        let drift = (world[b].pos - *start).length();
        assert!(drift < slop, "hull {i} drifted {drift} m");
        // Each rests on the one below, sunk at most the slop.
        let expect = 0.1 + 0.2 * i as f64;
        assert!(
            (world[b].pos.y - expect).abs() < slop * (i as f64 + 1.0),
            "hull {i} at {}",
            world[b].pos.y
        );
    }
    assert_eq!(world.stats.asleep, 5, "the stack sleeps");
}

/// Mixed hulls (rocks, prisms, and box hulls) poured into a box with walls.
/// The fastest are thrown down at `speed`.
fn pour(count: usize, speed: f64, threads: Option<usize>) -> World {
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
    let shapes: Vec<HullRef> = (0..8_u64)
        .map(|k| match k % 4 {
            0 | 1 => HullRef::new(ConvexHull::rock(k, DVec3::new(0.16, 0.11, 0.13), 24).unwrap()),
            2 => prism(0.12, 0.08),
            _ => HullRef::new(ConvexHull::cuboid(DVec3::new(0.12, 0.07, 0.1)).unwrap()),
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
        world[id].vel = DVec3::new(0.0, -speed * ((n % 5) as f64 / 4.0), 0.0);
        world[id].omega = DVec3::new(1.0, -2.0, 0.5) * ((n % 3) as f64);
    }
    if let Some(n) = threads {
        THREADS.store(n, std::sync::atomic::Ordering::Relaxed);
        SERIAL.store(n == 1, std::sync::atomic::Ordering::Relaxed);
    }
    let g = gravity();
    for _ in 0..(120 * 4) {
        world.step(&g);
    }
    if threads.is_some() {
        THREADS.store(0, std::sync::atomic::Ordering::Relaxed);
        SERIAL.store(false, std::sync::atomic::Ordering::Relaxed);
    }
    world
}

#[test]
fn two_hundred_hulls_poured_into_a_box_settle_inside_it() {
    let speed = 12.0;
    let world = pour(200, speed, None);
    let mut resting = 0;
    for b in world
        .bodies()
        .iter()
        .filter(|b| b.kind == BodyKind::Dynamic)
    {
        assert!(b.pos.is_finite());
        // Inside the walls and above the floor: nothing tunneled.
        assert!(
            b.pos.x.abs() < 2.0 && b.pos.z.abs() < 2.0,
            "escaped: {}",
            b.pos
        );
        assert!(b.pos.y > 0.0, "fell through the floor: {}", b.pos);
        if b.sleeping || b.vel.length() < 0.05 {
            resting += 1;
        }
    }
    assert!(resting >= 190, "{resting} of 200 at rest");
}

#[test]
fn a_hull_pile_steps_bit_for_bit_at_one_four_and_eight_threads() {
    let runs: Vec<World> = [1, 4, 8]
        .into_iter()
        .map(|n| pour(120, 6.0, Some(n)))
        .collect();
    for other in &runs[1..] {
        for (a, b) in runs[0].bodies().iter().zip(other.bodies()) {
            for (x, y) in [(a.pos, b.pos), (a.vel, b.vel), (a.omega, b.omega)] {
                assert_eq!(
                    x.to_array().map(f64::to_bits),
                    y.to_array().map(f64::to_bits)
                );
            }
            let (p, q) = (a.orientation.to_array(), b.orientation.to_array());
            assert_eq!(p.map(f64::to_bits), q.map(f64::to_bits));
        }
    }
}

/// GP-1's ledger: hulls, a box, and a capsule collide in free space; total
/// momentum holds to rounding through every hull contact.
#[test]
fn hull_collisions_conserve_momentum() {
    let mut world = World::new(1.0 / 120.0);
    let rock = HullRef::new(ConvexHull::rock(3, DVec3::new(0.3, 0.2, 0.25), 30).unwrap());
    let (a, _) = world.add_hull_body(rock, 900.0, DVec3::new(-1.0, 0.05, 0.0), DQuat::IDENTITY);
    world[a].vel = DVec3::new(3.0, 0.0, 0.1);
    world[a].omega = DVec3::new(0.0, 1.0, 2.0);
    let (b, _) = world.add_hull_body(
        prism(0.3, 0.2),
        700.0,
        DVec3::new(0.2, 0.0, 0.1),
        DQuat::from_rotation_x(0.4),
    );
    let (c, _) = world.add_hull_body(
        rock,
        900.0,
        DVec3::new(1.2, -0.1, 0.0),
        DQuat::from_rotation_z(1.0),
    );
    let cube = world.add(Body::new(
        5.0,
        Body::box_inertia(5.0, DVec3::splat(0.4)),
        DVec3::new(2.2, 0.0, -0.05),
    ));
    world.add_collider(Collider::new(
        cube,
        Shape::Cuboid {
            half: DVec3::splat(0.2),
        },
    ));
    let tank = world.add(Body::new(
        8.0,
        Body::shell_inertia(8.0, 0.2, 0.8),
        DVec3::new(3.0, 0.1, 0.0),
    ));
    world.add_collider(Collider::new(
        tank,
        Shape::Capsule {
            radius: 0.2,
            half_length: 0.3,
        },
    ));
    let origin = DVec3::new(0.3, -2.0, 1.0);
    let ledger = Ledger::new(origin, world.momentum(origin));
    let mut touched = 0;
    for _ in 0..360 {
        world.step(&NoField);
        touched += world
            .contacts
            .iter()
            .filter(|c| c.impulse.length() > 0.0)
            .count();
        let error = ledger.error(world.momentum(origin));
        assert!(error.linear < 1e-12 && error.angular < 1e-12, "{error:?}");
    }
    assert!(touched > 0, "the hulls collided");
    for id in [b, c, cube] {
        assert!(
            world[id].vel.x > 0.0,
            "momentum passed down the line: {:?}",
            world
                .bodies()
                .iter()
                .map(|b| (b.pos, b.vel))
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn a_world_with_hulls_restores_and_continues_bit_for_bit() {
    let mut world = pour(30, 3.0, None);
    let json = serde_json::to_string(&world).unwrap();
    let mut restored: World = serde_json::from_str(&json).unwrap();
    assert_eq!(restored, world);
    let g = gravity();
    for _ in 0..60 {
        world.step(&g);
        restored.step(&g);
    }
    assert_eq!(restored.bodies(), world.bodies());
}

#[test]
fn raycasts_and_closest_points_see_a_hull() {
    let mut world = World::new(0.01);
    let (_, collider) = world.add_hull_body(
        HullRef::new(ConvexHull::cuboid(DVec3::splat(0.5)).unwrap()),
        1000.0,
        DVec3::new(0.0, 2.0, 0.0),
        DQuat::from_rotation_y(0.3),
    );
    let hit = world
        .raycast(DVec3::new(0.0, 10.0, 0.0), DVec3::NEG_Y, 20.0, &|_| true)
        .unwrap();
    assert!((hit.distance - 7.5).abs() < 1e-9, "{hit:?}");
    assert!((hit.normal - DVec3::Y).length() < 1e-9);
    let c = world.colliders()[collider.0 as usize];
    let near = c.closest_point(&world, DVec3::new(0.0, 5.0, 0.0));
    assert!((near.y - 2.5).abs() < 1e-9, "{near}");
    let inside = DVec3::new(0.1, 2.1, 0.0);
    assert_eq!(c.closest_point(&world, inside), inside);
}

#[test]
fn hull_mass_properties_drive_the_body() {
    let mut world = World::new(0.01);
    let hull = HullRef::new(ConvexHull::cuboid(DVec3::new(0.5, 0.25, 0.1)).unwrap());
    let (id, _) = world.add_hull_body(hull, 1000.0, DVec3::ZERO, DQuat::IDENTITY);
    let body = world[id];
    assert!((body.mass - 100.0).abs() < 1e-9);
    let mut moments = body.inertia.to_array();
    moments.sort_by(f64::total_cmp);
    let mut expect = Body::box_inertia(100.0, DVec3::new(1.0, 0.5, 0.2)).to_array();
    expect.sort_by(f64::total_cmp);
    for k in 0..3 {
        assert!((moments[k] - expect[k]).abs() < 1e-9);
    }
}
