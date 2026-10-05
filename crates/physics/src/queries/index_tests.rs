//! Indexed queries retain exhaustive hit ordering across scene changes.
use super::*;
fn key(i: u64) -> ColliderKey {
    ColliderKey {
        life: Life {
            instance: 1 + i % 2,
            entity: i,
            generation: i % 7,
        },
        shape: 0,
    }
}
fn same(a: Result<Results, String>, b: Result<Results, String>) {
    match (a, b) {
        (Ok(a), Ok(b)) => {
            assert_eq!(a.truncated, b.truncated);
            assert_eq!(format!("{:?}", a.hits), format!("{:?}", b.hits));
        }
        (Err(a), Err(b)) => assert_eq!(a, b),
        (a, b) => panic!("Query outcomes differ: {a:?} / {b:?}"),
    }
}
#[test]
fn transformed_mixed_shapes_filters_limits_and_updates_match_exhaustive_queries() {
    let mut scene = Scene::default();
    for i in 0..96 {
        let position = DVec3::new(
            (i % 8) as f64 * 2. - 7.,
            (i / 8) as f64 * 0.2,
            (i / 8) as f64 * 2. - 10.,
        );
        let usage = match i % 4 {
            0 => Usage::Blocking,
            1 => Usage::Damage,
            2 => Usage::Trigger,
            _ => Usage::Selection,
        };
        if i % 3 == 0 {
            scene
                .insert_capsule(CapsuleCollider {
                    key: key(i),
                    capsule: Capsule {
                        a: DVec3::ZERO,
                        b: DVec3::Y * 1.2,
                        radius: 0.3,
                    },
                    layers: 1 << (i % 3),
                    usage,
                })
                .unwrap();
        } else {
            scene
                .insert(MeshCollider {
                    key: key(i),
                    mesh: Mesh::from_box(-DVec3::splat(0.5), DVec3::splat(0.5)).unwrap(),
                    layers: 1 << (i % 3),
                    usage,
                })
                .unwrap();
        }
        scene
            .set_pose(
                key(i),
                Pose {
                    position,
                    rotation: DQuat::from_rotation_z(i as f64 * 0.11)
                        * DQuat::from_rotation_y(i as f64 * 0.07),
                },
            )
            .unwrap();
    }
    for iteration in 0..300u64 {
        let changed = iteration % 96;
        scene
            .set_pose(
                key(changed),
                Pose {
                    position: DVec3::new(
                        (iteration % 19) as f64 - 9.,
                        0.2,
                        ((iteration * 13) % 23) as f64 - 11.,
                    ),
                    rotation: DQuat::from_rotation_x(iteration as f64 * 0.2),
                },
            )
            .unwrap();
        let mut reference = scene.clone();
        reference.exhaustive = true;
        let filter = Filter {
            instance: 1 + iteration % 2,
            layers: if iteration % 5 == 0 { 1 } else { u32::MAX },
            ignore: (iteration % 7 == 0).then_some(key(changed).life),
            exclude: (iteration % 11 == 0).then_some(key((changed + 1) % 96)),
            usages: if iteration % 3 == 0 {
                15
            } else {
                Usage::Blocking.bit()
            },
            limit: if iteration % 4 == 0 { 1 } else { 64 },
        };
        let origin = DVec3::new(
            (iteration % 17) as f64 - 8.,
            0.3,
            ((iteration * 7) % 21) as f64 - 10.,
        );
        let capsule = Capsule {
            a: origin,
            b: origin + DVec3::Y * 1.2,
            radius: 0.4,
        };
        let delta = DVec3::new(
            (iteration % 9) as f64 - 4.,
            0.5,
            ((iteration * 3) % 13) as f64 - 6.,
        );
        same(
            scene.ray(origin, delta.normalize(), 15., filter),
            reference.ray(origin, delta.normalize(), 15., filter),
        );
        same(
            scene.overlap(capsule, filter),
            reference.overlap(capsule, filter),
        );
        same(
            scene.sweep(capsule, delta, filter),
            reference.sweep(capsule, delta, filter),
        );
    }
    let old = key(0);
    scene.remove_capsule(old).unwrap();
    assert_eq!(scene.triangle_count, 64 * 12);
    let replacement = ColliderKey {
        life: Life {
            generation: old.life.generation + 1,
            ..old.life
        },
        ..old
    };
    scene
        .insert(MeshCollider {
            key: replacement,
            mesh: Mesh::from_box(-DVec3::ONE, DVec3::ONE).unwrap(),
            layers: 1,
            usage: Usage::Blocking,
        })
        .unwrap();
    let result = scene
        .overlap(
            Capsule {
                a: DVec3::ZERO,
                b: DVec3::Y,
                radius: 0.3,
            },
            Filter::blocking(1),
        )
        .unwrap();
    assert!(result.hits.iter().any(|h| h.collider == replacement));
    assert!(!result.hits.iter().any(|h| h.collider == old));
    assert!(
        scene
            .set_pose(
                replacement,
                Pose {
                    position: DVec3::splat(f64::NAN),
                    ..Default::default()
                }
            )
            .is_err()
    );
    let mut reference = scene.clone();
    reference.exhaustive = true;
    same(
        scene.ray(-DVec3::X * 3., DVec3::X, 6., Filter::blocking(1)),
        reference.ray(-DVec3::X * 3., DVec3::X, 6., Filter::blocking(1)),
    );
    scene.remove(replacement).unwrap();
    assert_eq!(scene.triangle_count, 64 * 12);
}
#[test]
fn foreign_instances_and_distant_shapes_do_not_enter_local_candidates() {
    let mut scene = Scene::default();
    for i in 0..4096u64 {
        let mut key = key(i);
        key.life.instance = if i < 2048 { 1 } else { 2 };
        let position = if i == 0 || i >= 2048 {
            DVec3::ZERO
        } else {
            DVec3::new(100. + i as f64, 100., 100.)
        };
        scene
            .insert_capsule(CapsuleCollider {
                key,
                layers: 1,
                usage: Usage::Blocking,
                capsule: Capsule {
                    a: position,
                    b: position + DVec3::Y,
                    radius: 0.3,
                },
            })
            .unwrap();
    }
    let result = scene
        .ray(-DVec3::X, DVec3::X, 2., Filter::blocking(1))
        .unwrap();
    assert_eq!(result.hits.len(), 1);
    assert_eq!(result.stats.scene_candidates, 1);
    assert_eq!(result.stats.capsule_tests, 1);
    assert!(result.stats.scene_nodes < 64);
}
