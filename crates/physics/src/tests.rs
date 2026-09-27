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

/// The mouse-interaction grab as a soft weld: a pack grips a heavy part
/// off-center while they move apart and spin relative to each other. The
/// relative motion dies out without overshoot and momentum is exact.
#[test]
fn a_soft_grip_settles_critically_and_conserves_momentum() {
    let mut world = World::new(1.0 / 120.0);
    let mut pack = Body::new(250.0, DVec3::splat(40.0), DVec3::ZERO);
    pack.vel = DVec3::new(0.3, 0.0, 0.0);
    let pack = world.add(pack);
    let mut part = Body::new(
        450.0,
        Body::box_inertia(450.0, DVec3::new(2.2, 2.2, 3.0)),
        DVec3::new(0.0, 0.5, 2.5),
    );
    part.vel = DVec3::new(-0.2, 0.05, 0.1);
    part.omega = DVec3::new(0.0, 0.3, 0.1);
    let part = world.add(part);
    let grip = world.add_joint(
        crate::Joint::weld_here(&world, pack, part, DVec3::new(0.0, 0.2, 1.0))
            .soft(6.0, 1.0)
            .limited(2_000.0, 2_000.0),
    );
    let origin = DVec3::new(5.0, -3.0, 1.0);
    let ledger = Ledger::new(origin, world.momentum(origin));
    let relative = |world: &World| {
        let joint = world.joint(grip).unwrap();
        let (a, b) = joint.anchors(world);
        let (pa, pb) = (&world[pack], &world[part]);
        let va = pa.vel + pa.omega_world().cross(a - pa.pos);
        let vb = pb.vel + pb.omega_world().cross(b - pb.pos);
        (
            (vb - va).length(),
            (pb.omega_world() - pa.omega_world()).length(),
        )
    };
    let (v0, w0) = relative(&world);
    let mut last = (v0, w0);
    let mut rebounds = 0;
    for step in 0..360 {
        world.step(&NoField);
        let error = ledger.error(world.momentum(origin));
        assert!(error.linear < 1e-12 && error.angular < 1e-12, "{error:?}");
        let now = relative(&world);
        // A critically damped response decays without ringing: after the
        // first quarter second it never grows back by more than a sliver.
        if step > 30 && now.0 > last.0 * 1.05 + 1e-4 {
            rebounds += 1;
        }
        last = now;
    }
    assert_eq!(rebounds, 0, "the grip rang");
    assert!(
        last.0 < v0 * 1e-3 && last.1 < w0 * 1e-3,
        "settled: {last:?}"
    );
    assert!(!world.joint(grip).unwrap().saturated);
}

/// Past its force limit a grip reports saturation and never transmits more.
#[test]
fn a_grip_saturates_at_its_limit() {
    let mut world = World::new(1.0 / 120.0);
    let hand = world.add(Body::new(250.0, DVec3::splat(40.0), DVec3::ZERO));
    let mut part = Body::new(180.0, DVec3::splat(60.0), DVec3::new(0.0, 0.0, 1.5));
    part.vel = DVec3::new(0.0, 0.0, 3.0);
    let part = world.add(part);
    let grip = world.add_joint(
        crate::Joint::weld_here(&world, hand, part, DVec3::new(0.0, 0.0, 1.0))
            .soft(6.0, 1.0)
            .limited(400.0, 200.0),
    );
    world.step(&NoField);
    let joint = world.joint(grip).unwrap();
    assert!(joint.saturated);
    assert!(joint.impulse.length() <= 400.0 * world.dt * (1.0 + 1e-9));
    world.remove_joint(grip);
    assert!(world.joint(grip).is_none());
}

/// `examples/rigid/closed_loop.py`'s connect: a hard point joint holds a
/// pendulum's arm length under gravity.
#[test]
fn a_hard_point_joint_holds_a_pendulum() {
    let mut world = World::new(0.005);
    let pivot = world.add(Body::new(1.0, DVec3::ONE, DVec3::ZERO).with_kind(BodyKind::Static));
    let bob = world.add(Body::new(
        2.0,
        DVec3::splat(0.02),
        DVec3::new(1.0, 0.0, 0.0),
    ));
    world.add_joint(crate::Joint::new(
        pivot,
        DVec3::ZERO,
        bob,
        DVec3::new(-1.0, 0.0, 0.0),
        crate::JointKind::Point,
    ));
    let g = Uniform(DVec3::new(0.0, -G, 0.0));
    let mut worst: f64 = 0.0;
    for _ in 0..800 {
        world.step(&g);
        let joint = world.joints().next().unwrap().1;
        let (a, b) = joint.anchors(&world);
        worst = worst.max(a.distance(b));
    }
    assert!(worst < 5e-3, "anchor gap {worst} m");
    assert!(world[bob].pos.y < -0.5, "it swung down");
}

/// A tether to a fixed anchor catches a body flying outward: it stops at the
/// length, gains no energy, and the reported tension is the momentum change.
#[test]
fn a_tether_catches_without_adding_energy() {
    let mut world = World::new(1.0 / 120.0);
    let anchor = world.add(Body::new(1.0, DVec3::ONE, DVec3::ZERO).with_kind(BodyKind::Static));
    let mut body = Body::new(250.0, DVec3::splat(40.0), DVec3::new(9.0, 0.0, 0.0));
    body.vel = DVec3::new(3.0, 0.5, 0.0);
    let body = world.add(body);
    let tether = world.add_joint(crate::Joint::new(
        anchor,
        DVec3::ZERO,
        body,
        DVec3::ZERO,
        crate::JointKind::Tether { length: 10.0 },
    ));
    let energy = |w: &World| 0.5 * w[body].mass * w[body].vel.length_squared();
    let start = energy(&world);
    let mut received = DVec3::ZERO;
    let mut farthest: f64 = 0.0;
    let momentum = world[body].momentum();
    for _ in 0..240 {
        world.step(&NoField);
        received += world.joint(tether).unwrap().impulse;
        farthest = farthest.max(world[body].pos.length());
        assert!(energy(&world) <= start * (1.0 + 1e-9), "energy grew");
    }
    assert!(farthest < 10.01, "stretched to {farthest} m");
    assert!(received.length() > 100.0, "the tether pulled");
    assert!((world[body].momentum() - momentum - received).length() < 1e-9);
}

/// A hard weld to a fixed anchor holds a body in place when something
/// strikes it.
#[test]
fn a_hard_weld_holds_under_impact() {
    let mut world = World::new(1.0 / 120.0);
    let anchor = world.add(Body::new(1.0, DVec3::ONE, DVec3::ZERO).with_kind(BodyKind::Static));
    let held = world.add(Body::new(
        450.0,
        Body::box_inertia(450.0, DVec3::new(2.2, 2.2, 3.0)),
        DVec3::new(0.0, 0.0, 5.0),
    ));
    world.add_collider(Collider::new(
        held,
        Shape::Cuboid {
            half: DVec3::new(1.1, 1.1, 1.5),
        },
    ));
    world.add_joint(crate::Joint::new(
        anchor,
        DVec3::new(0.0, 0.0, 5.0),
        held,
        DVec3::ZERO,
        crate::JointKind::Weld {
            relative: DQuat::IDENTITY,
        },
    ));
    let mut hammer = Body::new(
        180.0,
        Body::box_inertia(180.0, DVec3::new(1.2, 1.2, 4.0)),
        DVec3::new(4.0, 0.8, 5.0),
    );
    hammer.vel = DVec3::new(-2.0, 0.0, 0.0);
    hammer.omega = DVec3::new(0.0, 0.5, 0.0);
    let hammer = world.add(hammer);
    world.add_collider(Collider::new(
        hammer,
        Shape::Cuboid {
            half: DVec3::new(0.6, 0.6, 2.0),
        },
    ));
    let mut worst: (f64, f64) = (0.0, 0.0);
    let mut rebound: f64 = f64::NEG_INFINITY;
    for _ in 0..360 {
        world.step(&NoField);
        rebound = rebound.max(world[hammer].vel.x);
        worst.0 = worst
            .0
            .max(world[held].pos.distance(DVec3::new(0.0, 0.0, 5.0)));
        worst.1 = worst
            .1
            .max(world[held].orientation.angle_between(DQuat::IDENTITY));
    }
    assert!(rebound > 0.0, "the hammer bounced off");
    assert!(worst.0 < 0.01 && worst.1 < 0.5f64.to_radians(), "{worst:?}");
}

/// The microgravity rule: a body drifting free at a millimeter per second is
/// still moving, so it never sleeps.
#[test]
fn a_slowly_drifting_body_never_sleeps() {
    let mut world = World::new(1.0 / 120.0);
    let mut body = Body::new(180.0, DVec3::splat(60.0), DVec3::ZERO);
    body.vel = DVec3::new(0.001, 0.0, 0.0);
    let id = world.add(body);
    world.add_collider(Collider::new(
        id,
        Shape::Cuboid {
            half: DVec3::splat(0.5),
        },
    ));
    for _ in 0..(120 * 60) {
        world.step(&NoField);
        assert!(!world[id].sleeping);
    }
    assert!((world[id].pos.x - 0.06).abs() < 1e-9);
}

/// `examples/rigid/hibernation.py`: a settled body sleeps and holds its
/// pose exactly; a moving body striking it, a force, or a new joint wakes it.
#[test]
fn a_settled_body_sleeps_and_wakes_on_contact_force_or_joint() {
    let mut world = World::new(0.01);
    ground(&mut world, Material::default());
    let half = DVec3::splat(0.2);
    let block = world.add(Body::new(
        5.0,
        Body::box_inertia(5.0, half * 2.0),
        DVec3::new(0.0, 0.2, 0.0),
    ));
    world.add_collider(Collider::new(block, Shape::Cuboid { half }));
    let g = Uniform(DVec3::new(0.0, -G, 0.0));
    let mut slept_at = None;
    for step in 0..300 {
        world.step(&g);
        if world[block].sleeping && slept_at.is_none() {
            slept_at = Some(step);
        }
    }
    assert!(slept_at.is_some_and(|s| s < 150), "slept at {slept_at:?}");
    assert!(world.slept.is_empty() || world.slept.iter().all(|(_, m)| m.linear.length() < 1.0));
    let resting = world[block];
    for _ in 0..100 {
        world.step(&g);
    }
    assert_eq!(world[block].pos, resting.pos, "asleep, the pose is exact");
    assert_eq!(world.stats.awake, 0);
    // A force wakes it.
    world[block].apply_force(DVec3::X * 200.0);
    world.step(&g);
    assert!(!world[block].sleeping && world[block].vel.x > 0.0);
    for _ in 0..300 {
        world.step(&g);
    }
    assert!(world[block].sleeping, "settled again");
    // A moving body striking it wakes it.
    let mut ball = Body::new(
        2.0,
        DVec3::splat(0.02),
        DVec3::new(-1.0, 0.2, 0.0) + DVec3::X * world[block].pos.x,
    );
    ball.vel = DVec3::new(3.0, 0.0, 0.0);
    let ball = world.add(ball);
    world.add_collider(Collider::new(ball, Shape::Sphere { radius: 0.1 }));
    let mut woke = false;
    for _ in 0..60 {
        world.step(&g);
        woke |= !world[block].sleeping;
    }
    assert!(woke, "the strike woke it");
    // A new joint wakes both bodies.
    for _ in 0..400 {
        world.step(&g);
    }
    assert!(world[block].sleeping);
    let anchor = world
        .add(Body::new(1.0, DVec3::ONE, DVec3::new(0.0, 3.0, 0.0)).with_kind(BodyKind::Static));
    world.add_joint(crate::Joint::new(
        anchor,
        DVec3::ZERO,
        block,
        DVec3::ZERO,
        crate::JointKind::Tether { length: 1.0 },
    ));
    assert!(!world[block].sleeping);
}

/// Fifty boxes dropped onto the ground settle and sleep, and a settled step
/// costs a fraction of an awake one.
#[test]
fn a_settled_pile_sleeps_and_steps_cheaply() {
    let mut world = World::new(0.01);
    ground(&mut world, Material::default());
    let half = DVec3::splat(0.1);
    for i in 0..50 {
        let (x, z) = (
            f64::from(i % 10) * 0.5 - 2.25,
            f64::from(i / 10) * 0.5 - 1.0,
        );
        let id = world.add(Body::new(
            1.0,
            Body::box_inertia(1.0, half * 2.0),
            DVec3::new(x, 0.3, z),
        ));
        world.add_collider(Collider::new(id, Shape::Cuboid { half }));
    }
    let g = Uniform(DVec3::new(0.0, -G, 0.0));
    let mut awake_time = Vec::new();
    for _ in 0..40 {
        world.step(&g);
        awake_time.push(world.stats.total);
    }
    for _ in 0..300 {
        world.step(&g);
    }
    assert_eq!(world.stats.awake, 0, "all fifty asleep");
    let mut asleep_time = Vec::new();
    for _ in 0..40 {
        world.step(&g);
        asleep_time.push(world.stats.total);
    }
    awake_time.sort();
    asleep_time.sort();
    let (awake, asleep) = (awake_time[20], asleep_time[20]);
    assert!(
        asleep * 2 < awake,
        "median step {asleep:?} asleep vs {awake:?} awake"
    );
    assert_eq!(
        world.stats.contact_points, 0,
        "no contacts solved while asleep"
    );
}
