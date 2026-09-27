//! Scene tests ported from Genesis examples (see the roadmap in
//! `docs/physics/2026-09-27-genesis-port-roadmap.md`).

use glam::{DQuat, DVec3};

use crate::{Body, BodyKind, Collider, Filter, Ledger, Material, NoField, Shape, Uniform, World};

const G: f64 = 9.81;

fn ground(world: &mut World, material: Material) {
    let floor = world
        .add(Body::new(1.0, DVec3::ONE, DVec3::new(0.0, -0.5, 0.0)).with_kind(BodyKind::Static));
    let half = DVec3::new(20.0, 0.5, 20.0);
    world.add_collider(Collider::new(floor, Shape::Cuboid { half }).with_material(material));
}

/// `examples/collision/contact_manifold.py`: a box driven through tilt, yaw,
/// deep penetration, and slide on a fixed box; the manifold size per phase.
#[test]
fn box_on_box_manifold_through_a_scripted_sweep() {
    const HALF: f64 = 0.1;
    let mut world = World::new(0.01);
    let base = world.add(Body::new(1.0, DVec3::ONE, DVec3::ZERO).with_kind(BodyKind::Static));
    world.add_collider(Collider::new(
        base,
        Shape::Cuboid {
            half: DVec3::splat(HALF),
        },
    ));
    let top = world.add(Body::new(1.0, DVec3::ONE, DVec3::ZERO).with_kind(BodyKind::Kinematic));
    world.add_collider(Collider::new(
        top,
        Shape::Cuboid {
            half: DVec3::splat(HALF),
        },
    ));
    // Detection needs one dynamic side; a unit dynamic stand-in keeps the
    // pose scripted since only detection runs.
    world[top].kind = BodyKind::Dynamic;
    let corners: Vec<DVec3> = (0..8)
        .map(|i| {
            DVec3::new(
                if i & 1 == 0 { -HALF } else { HALF },
                if i & 2 == 0 { -HALF } else { HALF },
                if i & 4 == 0 { -HALF } else { HALF },
            )
        })
        .collect();
    let mut manifold_at = |tilt: f64, yaw: f64, penetration: f64, slide: f64| {
        let q = DQuat::from_rotation_y(yaw) * DQuat::from_rotation_x(tilt);
        let lowest = corners
            .iter()
            .map(|c| (q * *c).y)
            .fold(f64::INFINITY, f64::min);
        world[top].orientation = q;
        world[top].pos = DVec3::new(slide, HALF - penetration - lowest, 0.0);
        let manifolds = world.detect(&|_, _| 0.0);
        manifolds.first().map_or(0, |m| m.points.len())
    };
    assert_eq!(manifold_at(0.0, 0.0, 1e-3, 0.0), 4, "flat face on face");
    assert_eq!(
        manifold_at(5f64.to_radians(), 0.0, 1e-3, 0.0),
        2,
        "rocking on an edge"
    );
    assert_eq!(
        manifold_at(0.0, 45f64.to_radians(), 1e-3, 0.0),
        4,
        "yawed octagon reduced to four"
    );
    assert_eq!(manifold_at(0.0, 20f64.to_radians(), 0.03, 0.0), 4, "deep");
    assert_eq!(manifold_at(0.0, 0.0, 1e-3, 0.1), 4, "half slid off");
    assert_eq!(manifold_at(0.0, 0.0, -0.05, 0.0), 0, "apart");
}

/// Bitmask filtering: `examples/collision/contype.py`.
#[test]
fn filters_choose_which_pairs_touch() {
    let (red, green, blue) = (
        Filter {
            group: 0b001,
            mask: 0b001,
        },
        Filter {
            group: 0b010,
            mask: 0b010,
        },
        Filter {
            group: 0b011,
            mask: 0b011,
        },
    );
    assert!(!red.allows(green));
    assert!(red.allows(blue) && green.allows(blue));
    assert!(Filter::ALL.allows(red) && !Filter::NONE.allows(Filter::ALL));
}

/// Box resting on the ground under gravity, pushed sideways by `load` times
/// the Coulomb limit; returns how far it slid in three seconds.
fn slide(load: f64) -> f64 {
    let material = Material {
        friction: 0.6,
        torsional: 0.0,
        restitution: 0.0,
    };
    let mut world = World::new(0.01);
    ground(&mut world, material);
    let mass = 2.0;
    let half = DVec3::splat(0.1);
    let block = world.add(Body::new(
        mass,
        Body::box_inertia(mass, half * 2.0),
        DVec3::new(0.0, 0.1, 0.0),
    ));
    world.add_collider(Collider::new(block, Shape::Cuboid { half }).with_material(material));
    let g = Uniform(DVec3::new(0.0, -G, 0.0));
    for _ in 0..50 {
        world.step(&g);
    }
    let start = world[block].pos;
    let limit = 0.6 * mass * G;
    for _ in 0..300 {
        world[block].apply_force(DVec3::X * (load * limit));
        world.step(&g);
    }
    (world[block].pos - start).length()
}

/// `examples/rigid/friction_breakaway.py`: loads below the Coulomb limit
/// hold, loads past it slip.
#[test]
fn friction_holds_below_the_coulomb_limit_and_slips_past_it() {
    for load in [0.25, 0.5, 0.75, 0.95] {
        let drift = slide(load);
        assert!(drift < 5e-3, "load {load} drifted {drift} m");
    }
    let drift = slide(1.05);
    assert!(drift > 5e-2, "past the limit it slides: {drift} m");
}

/// Sphere on the ground spun about the vertical by `load` times the
/// torsional limit; returns the angle swept in three seconds.
fn twist(load: f64) -> f64 {
    let material = Material {
        friction: 1.0,
        torsional: 0.05,
        restitution: 0.0,
    };
    let mut world = World::new(0.01);
    ground(&mut world, material);
    let (mass, radius) = (1.0, 0.1);
    let ball = world.add(Body::new(
        mass,
        DVec3::splat(0.4 * mass * radius * radius),
        DVec3::new(0.0, radius, 0.0),
    ));
    world.add_collider(Collider::new(ball, Shape::Sphere { radius }).with_material(material));
    let g = Uniform(DVec3::new(0.0, -G, 0.0));
    for _ in 0..50 {
        world.step(&g);
    }
    let start = world[ball].orientation;
    let limit = 0.05 * mass * G;
    for _ in 0..300 {
        world[ball].apply_torque(DVec3::Y * (load * limit));
        world.step(&g);
    }
    start.angle_between(world[ball].orientation)
}

/// `examples/rigid/torsional_grasp.py`: a point contact resists spin up to
/// the torsional limit.
#[test]
fn torsional_friction_resists_spin_up_to_its_limit() {
    assert!(twist(0.9) < 5e-3, "held: {}", twist(0.9));
    assert!(twist(1.1) > 5e-2, "slips: {}", twist(1.1));
}

/// A fast body against a thin panel: speculative contacts stop it at the
/// surface instead of letting it pass through in one step.
#[test]
fn fast_bodies_do_not_tunnel_through_thin_panels() {
    for (shape, speed) in [
        (Shape::Sphere { radius: 0.1 }, 2.0),
        (Shape::Sphere { radius: 0.1 }, 40.0),
        (
            Shape::Cuboid {
                half: DVec3::splat(0.2),
            },
            40.0,
        ),
        (
            Shape::Capsule {
                radius: 0.1,
                half_length: 0.5,
            },
            40.0,
        ),
    ] {
        let mut world = World::new(1.0 / 120.0);
        let panel = world.add(Body::new(1.0, DVec3::ONE, DVec3::ZERO).with_kind(BodyKind::Static));
        world.add_collider(Collider::new(
            panel,
            Shape::Cuboid {
                half: DVec3::new(2.0, 2.0, 0.01),
            },
        ));
        let mut body = Body::new(10.0, DVec3::splat(1.0), DVec3::new(0.0, 0.0, 2.0));
        body.vel = DVec3::new(0.0, 0.0, -speed);
        let id = world.add(body);
        world.add_collider(Collider::new(id, shape));
        for _ in 0..120 {
            world.step(&NoField);
            assert!(
                world[id].pos.z > 0.0,
                "{shape:?} at {speed} m/s passed the panel"
            );
        }
    }
}

/// `examples/ipc/ipc_momentum.py` for rigid contact: zero gravity, bodies
/// colliding off-center conserve linear and angular momentum.
#[test]
fn collisions_between_free_bodies_conserve_momentum() {
    let mut world = World::new(1.0 / 120.0);
    let mut cube = Body::new(
        10.0,
        Body::box_inertia(10.0, DVec3::splat(0.4)),
        DVec3::new(-1.0, 0.1, 0.0),
    );
    cube.vel = DVec3::new(4.0, 0.0, 0.0);
    cube.omega = DVec3::new(0.0, 0.0, 1.0);
    let cube = world.add(cube);
    world.add_collider(Collider::new(
        cube,
        Shape::Cuboid {
            half: DVec3::splat(0.2),
        },
    ));
    let tank = world.add(Body::new(
        30.0,
        Body::shell_inertia(30.0, 0.3, 1.2),
        DVec3::new(0.5, 0.0, 0.2),
    ));
    world.add_collider(Collider::new(
        tank,
        Shape::Capsule {
            radius: 0.3,
            half_length: 0.3,
        },
    ));
    let truss = world.add(Body::new(
        15.0,
        Body::box_inertia(15.0, DVec3::new(0.3, 0.3, 2.0)),
        DVec3::new(1.4, 0.0, 0.0),
    ));
    world.add_collider(Collider::new(
        truss,
        Shape::Cuboid {
            half: DVec3::new(0.15, 0.15, 1.0),
        },
    ));
    let origin = DVec3::new(0.3, -2.0, 1.0);
    let ledger = Ledger::new(origin, world.momentum(origin));
    let mut touched = 0;
    for _ in 0..240 {
        world.step(&NoField);
        touched += world
            .contacts
            .iter()
            .filter(|c| c.impulse.length() > 0.0)
            .count();
        let error = ledger.error(world.momentum(origin));
        assert!(error.linear < 1e-12 && error.angular < 1e-12, "{error:?}");
    }
    assert!(touched > 0, "the bodies collided");
    assert!(world[truss].vel.x > 0.1, "momentum passed down the line");
}

/// The audit's experiment: a spinning tank strikes a panel edge. The panel
/// is fixed, so the contact is an external impulse; the tank's momentum
/// changes by exactly the reported impulses, and its spin changes.
#[test]
fn a_spinning_tank_striking_a_panel_edge_takes_angular_impulse() {
    let mut world = World::new(1.0 / 120.0);
    let panel = world
        .add(Body::new(1.0, DVec3::ONE, DVec3::new(0.0, 0.0, 0.0)).with_kind(BodyKind::Static));
    world.add_collider(Collider::new(
        panel,
        Shape::Cuboid {
            half: DVec3::new(3.0, 0.15, 2.0),
        },
    ));
    let mut tank = Body::new(
        320.0,
        Body::shell_inertia(320.0, 1.3, 3.6),
        DVec3::new(3.2, 2.0, 0.0),
    );
    tank.orientation = DQuat::from_rotation_x(std::f64::consts::FRAC_PI_2);
    tank.vel = DVec3::new(-0.3, -0.8, 0.0);
    tank.omega = DVec3::new(0.05, 0.1, 0.4);
    let tank = world.add(tank);
    world.add_collider(
        Collider::new(
            tank,
            Shape::Capsule {
                radius: 1.3,
                half_length: 0.5,
            },
        )
        .with_material(Material {
            friction: 0.4,
            torsional: 0.0,
            restitution: 0.3,
        }),
    );
    let origin = DVec3::ZERO;
    let mut ledger = Ledger::new(origin, world.momentum(origin));
    let spin = world[tank].angular_momentum();
    for _ in 0..360 {
        world.step(&NoField);
        for c in &world.contacts {
            let sign = if c.body_b == tank { 1.0 } else { -1.0 };
            ledger.add_impulse("panel", c.impulse * sign, c.point);
            ledger.add(
                "panel",
                crate::Momentum {
                    linear: DVec3::ZERO,
                    angular: c.twist * sign,
                },
            );
        }
    }
    let error = ledger.error(world.momentum(origin));
    assert!(error.linear < 1e-12 && error.angular < 1e-12, "{error:?}");
    let hit = ledger.external["panel"];
    assert!(hit.linear.length() > 100.0, "{hit:?}");
    assert!(
        (world[tank].angular_momentum() - spin).length() > 10.0,
        "spin changed"
    );
    assert!(world[tank].vel.y > 0.0, "bounced off");
}

/// `examples/collision/pyramid.py`, small: a stack of boxes stays put.
#[test]
fn a_small_stack_settles_and_stays() {
    let mut world = World::new(0.01);
    ground(&mut world, Material::default());
    let half = DVec3::splat(0.125);
    let boxes: Vec<_> = (0..3)
        .map(|i| {
            let id = world.add(Body::new(
                1.0,
                Body::box_inertia(1.0, half * 2.0),
                DVec3::new(0.01 * f64::from(i), 0.125 + 0.25 * f64::from(i), 0.0),
            ));
            world.add_collider(Collider::new(id, Shape::Cuboid { half }));
            id
        })
        .collect();
    let g = Uniform(DVec3::new(0.0, -G, 0.0));
    for _ in 0..100 {
        world.step(&g);
    }
    let settled: Vec<DVec3> = boxes.iter().map(|b| world[*b].pos).collect();
    for _ in 0..300 {
        world.step(&g);
    }
    for (b, start) in boxes.iter().zip(settled) {
        let drift = (world[*b].pos - start).length();
        assert!(drift < 0.01, "box drifted {drift} m");
        assert!(world[*b].pos.y > 0.1, "no sinking");
    }
}
