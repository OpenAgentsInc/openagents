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

fn mixed_response_world() -> World {
    let mut world = World::new(1.0 / 120.0);
    world.sleep.enabled = false;
    let bodies = [
        (BodyKind::Static, DVec3::ZERO),
        (BodyKind::Dynamic, DVec3::new(0.7, 0.0, 0.0)),
        (BodyKind::Dynamic, DVec3::new(0.35, 0.6, 0.0)),
        (BodyKind::Kinematic, DVec3::new(0.8, 0.5, 0.0)),
        (BodyKind::Dynamic, DVec3::new(0.3, 0.25, 0.5)),
        (BodyKind::Static, DVec3::new(0.8, 0.25, 0.5)),
    ];
    for (i, (kind, pos)) in bodies.into_iter().enumerate() {
        let mut body = Body::new(1.0, DVec3::new(0.3, 0.4, 0.5), pos).with_kind(kind);
        body.sleeping = i == 1;
        if body.responds() {
            body.vel = DVec3::new(0.08, -0.03, 0.02) * i as f64;
            body.omega = DVec3::new(0.01, 0.02, -0.03);
        }
        let id = world.add(body);
        world.add_collider(Collider::new(id, Shape::Sphere { radius: 0.5 }));
    }
    world
}

#[test]
fn responding_queries_keep_fixed_sleeping_and_kinematic_pairs_in_exhaustive_order() {
    let mut world = mixed_response_world();
    let expected = world.detect(&|_, _| 0.01);
    let (actual, stats) = world.detect_bounded(0.01).unwrap();
    assert_eq!(format!("{actual:?}"), format!("{expected:?}"));
    assert_eq!(
        stats.candidate_pairs, 9,
        "responding pairs are counted once"
    );
    assert_eq!(
        actual.iter().map(|m| (m.a.0, m.b.0)).collect::<Vec<_>>(),
        [
            (0, 2),
            (0, 4),
            (1, 2),
            (1, 4),
            (2, 3),
            (2, 4),
            (2, 5),
            (3, 4),
            (4, 5)
        ]
    );
}

#[test]
fn responding_queries_replay_exact_motion_through_sleep_scripted_wake_and_reset() {
    let initial = mixed_response_world();
    let pristine = serde_json::to_vec(&initial).unwrap();
    let mut actual = initial.clone();
    let mut expected = initial;
    expected.exhaustive_detection = true;
    for step in 0..24 {
        if step == 12 {
            actual = serde_json::from_slice(&pristine).unwrap();
            expected = serde_json::from_slice(&pristine).unwrap();
            expected.exhaustive_detection = true;
        }
        for world in [&mut actual, &mut expected] {
            if step == 2 {
                for body in world.bodies_mut() {
                    if body.kind == BodyKind::Dynamic {
                        body.sleeping = true;
                    }
                    body.vel = DVec3::ZERO;
                    body.omega = DVec3::ZERO;
                }
            } else if matches!(step, 0 | 3 | 12) {
                world[BodyId(3)].vel = DVec3::X * 0.2;
            }
            if step == 8 {
                world.remove_body(BodyId(4));
                world.collider_mut(ColliderId(2)).filter = Filter::NONE;
            } else if step == 9 {
                world.collider_mut(ColliderId(2)).filter = Filter::ALL;
            }
        }
        actual.step(&NoField);
        expected.step(&NoField);
        assert_eq!(
            format!("{:?}", actual.contacts),
            format!("{:?}", expected.contacts),
            "contact order and impulses at step {step}"
        );
        let bytes = serde_json::to_vec(&actual).unwrap();
        assert_eq!(
            bytes,
            serde_json::to_vec(&expected).unwrap(),
            "state at step {step}"
        );
        if step == 0 {
            assert!(
                !actual[BodyId(1)].sleeping,
                "the scripted body wakes its neighbor"
            );
        } else if step == 2 {
            assert!(actual.contacts.is_empty(), "no body can respond");
        }
        if step == 6 {
            actual = serde_json::from_slice(&bytes).unwrap();
            expected = serde_json::from_slice(&bytes).unwrap();
            expected.exhaustive_detection = true;
        }
    }
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
fn unchanged_geometry_is_cached_and_public_mutations_refresh_it() {
    let mut world = world();
    let (_, first) = world.detect_bounded(0.01).unwrap();
    assert_eq!(first.geometry_updates, world.colliders().len());
    let (_, unchanged) = world.detect_bounded(0.01).unwrap();
    assert_eq!(unchanged.geometry_updates, 0);

    world[BodyId(1)].pos += DVec3::X * 0.4;
    world[BodyId(2)].orientation = DQuat::from_rotation_x(0.8);
    world.collider_mut(ColliderId(3)).offset += DVec3::Z;
    world.collider_mut(ColliderId(4)).rotation = DQuat::from_rotation_y(0.9);
    world.collider_mut(ColliderId(5)).shape = Shape::Sphere { radius: 1.2 };
    let (actual, changed) = world.detect_bounded(0.01).unwrap();
    assert_eq!(changed.geometry_updates, 5);
    assert_eq!(
        format!("{actual:?}"),
        format!("{:?}", world.detect(&|_, _| 0.01))
    );

    world.collider_mut(ColliderId(1)).filter = Filter::NONE;
    world[BodyId(1)].pos = DVec3::splat(f64::NAN);
    let (_, disabled) = world.detect_bounded(0.01).unwrap();
    assert_eq!(disabled.geometry_updates, 0);
    world[BodyId(1)].pos = DVec3::ZERO;
    world.collider_mut(ColliderId(1)).filter = Filter::ALL;
    let (actual, enabled) = world.detect_bounded(0.01).unwrap();
    assert_eq!(enabled.geometry_updates, 1);
    assert_eq!(
        format!("{actual:?}"),
        format!("{:?}", world.detect(&|_, _| 0.01))
    );

    let mut reassigned = world[BodyId(1)];
    reassigned.kind = BodyKind::Static;
    let reassigned = world.add(reassigned);
    world.collider_mut(ColliderId(1)).body = reassigned;
    let (actual, same_pose) = world.detect_bounded(0.01).unwrap();
    assert_eq!(same_pose.geometry_updates, 0);
    assert_eq!(
        format!("{actual:?}"),
        format!("{:?}", world.detect(&|_, _| 0.01))
    );
    world[reassigned].kind = BodyKind::Dynamic;
    world[reassigned].sleeping = true;
    let (actual, sleeping) = world.detect_bounded(0.01).unwrap();
    assert_eq!(sleeping.geometry_updates, 0);
    assert_eq!(
        format!("{actual:?}"),
        format!("{:?}", world.detect(&|_, _| 0.01))
    );
    world[reassigned].sleeping = false;
    world[reassigned].pos += DVec3::Z * 0.7;
    let (actual, moved) = world.detect_bounded(0.01).unwrap();
    assert_eq!(moved.geometry_updates, 1);
    assert_eq!(
        format!("{actual:?}"),
        format!("{:?}", world.detect(&|_, _| 0.01))
    );

    let mut restored: World = serde_json::from_slice(&serde_json::to_vec(&world).unwrap()).unwrap();
    let (actual, restored_stats) = restored.detect_bounded(0.01).unwrap();
    assert_eq!(restored_stats.geometry_updates, restored.colliders().len());
    assert_eq!(
        format!("{actual:?}"),
        format!("{:?}", world.detect(&|_, _| 0.01))
    );
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
    assert!(
        stats.scene_nodes < 128,
        "static bodies do not start queries"
    );
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
fn a_wide_floor_does_not_query_airborne_chunks_but_keeps_motion_contacts() {
    let mut world = World::new(1.0 / 120.0);
    let floor = world.add(Body::new(1.0, DVec3::ONE, -DVec3::Y * 0.1).with_kind(BodyKind::Static));
    world.add_collider(Collider::new(
        floor,
        Shape::Cuboid {
            half: DVec3::new(80.0, 0.1, 80.0),
        },
    ));
    for i in 0..699 {
        let body = world.add(Body::new(
            1.0,
            DVec3::ONE,
            DVec3::new(
                (i % 27) as f64 * 2.0 - 26.0,
                6.0,
                (i / 27) as f64 * 2.0 - 26.0,
            ),
        ));
        world.add_collider(Collider::new(
            body,
            Shape::Cuboid {
                half: DVec3::splat(0.3),
            },
        ));
    }
    let approaching = world.add(Body::new(1.0, DVec3::ONE, DVec3::Y * 0.32));
    world.add_collider(Collider::new(
        approaching,
        Shape::Cuboid {
            half: DVec3::splat(0.3),
        },
    ));
    let mut reaches = vec![0.0; world.colliders().len()];
    *reaches.last_mut().unwrap() = 0.04;
    let (actual, stats) = world.detect_motion_profiled(0.01, &reaches).unwrap();
    let expected = world.detect(&|a, b| {
        0.01 + if a.body == approaching || b.body == approaching {
            0.04
        } else {
            0.0
        }
    });
    assert!(!expected.is_empty());
    assert_eq!(format!("{actual:?}"), format!("{expected:?}"));
    assert_eq!(stats.candidate_pairs, 1);
    assert_eq!(stats.narrow_phase, 1);
}

#[test]
fn oriented_margin_bounds_preserve_corner_contacts_from_sat() {
    let mut world = World::new(1.0 / 120.0);
    let rotations = [
        DQuat::from_rotation_z(std::f64::consts::FRAC_PI_4),
        DQuat::from_rotation_z(std::f64::consts::FRAC_PI_6)
            * DQuat::from_rotation_y(std::f64::consts::FRAC_PI_4),
    ];
    let extent_x = rotations.map(|rotation| {
        let axes = DMat3::from_quat(rotation);
        axes.x_axis.x.abs() + axes.y_axis.x.abs() + axes.z_axis.x.abs()
    });
    // The world-space gap exceeds the margin, but the closest edges are
    // within the margin along every separating axis.
    let distance = extent_x[0] + extent_x[1] + 1.05;
    for (x, rotation) in [0.0, distance].into_iter().zip(rotations) {
        let mut body = Body::new(1.0, DVec3::ONE, DVec3::X * x);
        body.orientation = rotation;
        let id = world.add(body);
        world.add_collider(Collider::new(id, Shape::Cuboid { half: DVec3::ONE }));
    }
    let expected = world.detect(&|_, _| 1.0);
    assert!(
        !expected.is_empty(),
        "the existing SAT admits this corner gap"
    );
    let (actual, _) = world.detect_bounded(1.0).unwrap();
    assert_eq!(format!("{actual:?}"), format!("{expected:?}"));
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
