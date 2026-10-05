//! Indexed detection retains exhaustive contacts, state, and restored trajectories.
use super::*;
use crate::{Body, BodyKind, NoField};
fn world() -> World {
    let mut world = World::new(1. / 120.);
    for i in 0..48 {
        let pos = DVec3::new(
            (i % 8) as f64 * 1.2,
            ((i / 8) % 3) as f64 * 1.1,
            (i / 24) as f64 * 1.2,
        );
        let mut body = Body::new(1. + i as f64 * 0.01, DVec3::splat(0.4), pos);
        body.orientation = DQuat::from_rotation_y(i as f64 * 0.13);
        body.vel = DVec3::new((i % 3) as f64 - 1., 0., (i % 5) as f64 * 0.1);
        body.omega = DVec3::new(0.1, i as f64 * 0.05, 0.2);
        if i % 7 == 0 {
            body.kind = BodyKind::Static;
        }
        if i % 11 == 0 {
            body.kind = BodyKind::Kinematic;
        }
        if i % 13 == 0 {
            body.sleeping = true;
            body.vel = DVec3::ZERO;
            body.omega = DVec3::ZERO;
        }
        let id = world.add(body);
        let shape = match i % 3 {
            0 => Shape::Sphere { radius: 0.5 },
            1 => Shape::Capsule {
                radius: 0.3,
                half_length: 0.35,
            },
            _ => Shape::Cuboid {
                half: DVec3::new(0.5, 0.4, 0.3),
            },
        };
        world.add_collider(
            Collider::new(id, shape).at(DVec3::X * 0.1, DQuat::from_rotation_x(i as f64 * 0.17)),
        );
    }
    world
}
#[test]
fn bounded_motion_matches_exhaustive_manifolds_after_moves_filters_and_removal() {
    let mut world = world();
    for iteration in 0..100 {
        let i = iteration % world.bodies().len();
        world.bodies_mut()[i].pos.x += 0.07;
        world.bodies_mut()[i].orientation = DQuat::from_rotation_z(iteration as f64 * 0.3);
        world.collider_mut(ColliderId(i as u32)).filter = if iteration % 4 == 0 {
            Filter::NONE
        } else {
            Filter::ALL
        };
        let reaches: Vec<_> = (0..world.colliders().len())
            .map(|i| (i % 7) as f64 * 0.13)
            .collect();
        let expected = world.detect(&|a, b| {
            let index = |c: &Collider| {
                world
                    .colliders()
                    .iter()
                    .position(|other| std::ptr::eq(c, other))
                    .unwrap()
            };
            0.01 + reaches[index(a)] + reaches[index(b)]
        });
        let (actual, stats) = world.detect_motion_profiled(0.01, &reaches).unwrap();
        assert_eq!(format!("{actual:?}"), format!("{expected:?}"));
        assert!(
            stats.candidate_pairs <= world.colliders().len() * (world.colliders().len() - 1) / 2
        );
    }
    let (actual, _) = world.detect_bounded(0.).unwrap();
    assert_eq!(
        format!("{actual:?}"),
        format!("{:?}", world.detect(&|_, _| 0.))
    );
    assert!(world.detect_bounded(f64::NAN).is_err());
    assert!(world.detect_motion_profiled(0., &[0.]).is_err());
}
#[test]
fn indexed_steps_replay_exhaustive_state_through_wake_reuse_and_restore() {
    let mut actual = world();
    let mut expected = actual.clone();
    expected.exhaustive_detection = true;
    for step in 0..120 {
        if step == 20 {
            for world in [&mut actual, &mut expected] {
                world.remove_body(BodyId(1));
            }
        }
        if step == 25 {
            for world in [&mut actual, &mut expected] {
                world.bodies_mut()[1] = Body::new(1., DVec3::splat(0.4), DVec3::new(0.6, 0., 0.));
                world.add_collider(Collider::new(BodyId(1), Shape::Sphere { radius: 0.3 }));
            }
        }
        if step == 50 {
            for world in [&mut actual, &mut expected] {
                world.bodies_mut()[2].vel = DVec3::X * 80.;
                world.bodies_mut()[3].omega = DVec3::Y * 40.;
            }
        }
        actual.step(&NoField);
        expected.step(&NoField);
        let bytes = serde_json::to_vec(&actual).unwrap();
        assert_eq!(
            bytes,
            serde_json::to_vec(&expected).unwrap(),
            "state at step {step}"
        );
        if step % 17 == 0 {
            actual = serde_json::from_slice(&bytes).unwrap();
            expected = serde_json::from_slice(&bytes).unwrap();
            expected.exhaustive_detection = true;
        }
    }
}
#[test]
fn sparse_background_prunes_pairs_and_tracks_kind_changes_and_reused_leaves() {
    let mut world = World::new(1. / 120.);
    for i in 0..4096 {
        let kind = if i == 0 {
            BodyKind::Dynamic
        } else {
            BodyKind::Static
        };
        let id = world.add(Body::new(1., DVec3::ONE, DVec3::X * (i as f64 * 4.)).with_kind(kind));
        world.add_collider(Collider::new(id, Shape::Sphere { radius: 0.5 }));
    }
    let (contacts, stats) = world.detect_bounded(0.01).unwrap();
    assert!(contacts.is_empty());
    assert_eq!(stats.candidate_pairs, 0);
    assert!(stats.scene_nodes < 5000);
    world.bodies_mut()[4095].kind = BodyKind::Dynamic;
    world.bodies_mut()[4095].pos = DVec3::X * 0.9;
    let (contacts, stats) = world.detect_bounded(0.01).unwrap();
    assert_eq!(contacts.len(), 1);
    assert_eq!(contacts[0].a, ColliderId(0));
    assert_eq!(contacts[0].b, ColliderId(4095));
    assert_eq!(stats.narrow_phase, 1);
    world.remove_body(BodyId(4095));
    let (contacts, _) = world.detect_bounded(0.01).unwrap();
    assert!(contacts.is_empty());
}

#[test]
fn scripted_motion_wakes_near_sleepers_without_enumerating_distant_pairs() {
    let mut world = World::new(1. / 120.);
    world.sleep.enabled = false;
    for i in 0..4096 {
        let mut body = Body::new(
            1.,
            DVec3::ONE,
            DVec3::X * if i == 1 { 0.9 } else { i as f64 * 4. },
        );
        if i == 0 {
            body.kind = BodyKind::Kinematic;
            body.vel = DVec3::X;
        } else {
            body.sleeping = true;
        }
        let id = world.add(body);
        world.add_collider(Collider::new(id, Shape::Sphere { radius: 0.5 }));
    }
    world.step(&NoField);
    assert!(!world.bodies()[1].sleeping);
    assert!(world.bodies()[2..].iter().all(|body| body.sleeping));
    assert_eq!(world.stats.detection.wake_candidate_pairs, 2);
    assert!(world.stats.detection.wake_scene_nodes < 64);
    assert_eq!(world.stats.detection.narrow_phase, 1);
}
