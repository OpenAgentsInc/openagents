//! Isolated scene-candidate and rigid-step evidence; no display, network, or host.
use glam::DVec3;
use physics::queries::{
    Capsule, CapsuleCollider, ColliderKey, Filter, Life, Mesh, MeshCollider, Scene, Usage,
};
use physics::{Body, Collider, NoField, Shape, World};
use serde_json::json;
use std::time::Instant;

fn percentile(values: &mut [f64], p: f64) -> f64 {
    values.sort_by(f64::total_cmp);
    values[((values.len() - 1) as f64 * p).ceil() as usize]
}
fn main() {
    let output = std::env::args().nth(1).expect("output JSON path");
    let mut rows = Vec::new();
    for count in [8, 64, 256, 1024, 4096] {
        let start = Instant::now();
        let mut scene = Scene::default();
        for i in 0..count {
            let key = ColliderKey {
                life: Life {
                    instance: 1,
                    entity: i as u64,
                    generation: 0,
                },
                shape: 0,
            };
            let position = if i == 0 {
                DVec3::ZERO
            } else {
                DVec3::new(10. + (i % 16) as f64 * 4., (i / 16) as f64 * 4., 10.)
            };
            if i % 2 == 0 {
                scene
                    .insert(MeshCollider {
                        key,
                        layers: 1,
                        usage: Usage::Blocking,
                        mesh: Mesh::from_box(
                            position - DVec3::splat(0.3),
                            position + DVec3::splat(0.3),
                        )
                        .unwrap(),
                    })
                    .unwrap();
            } else {
                scene
                    .insert_capsule(CapsuleCollider {
                        key,
                        layers: 1,
                        usage: Usage::Blocking,
                        capsule: Capsule {
                            a: position,
                            b: position + DVec3::Y * 0.2,
                            radius: 0.3,
                        },
                    })
                    .unwrap();
            }
        }
        let build_ms = start.elapsed().as_secs_f64() * 1000.;
        scene.enable_profiling();
        let mut ray = Vec::new();
        let mut overlap = Vec::new();
        let mut sweep = Vec::new();
        let capsule = Capsule {
            a: DVec3::ZERO,
            b: DVec3::Y * 0.2,
            radius: 0.2,
        };
        let mut hit_counts = [0; 3];
        let mut last = Vec::new();
        for _ in 0..200 {
            let t = Instant::now();
            let r = scene
                .ray(-DVec3::X, DVec3::X, 2., Filter::blocking(1))
                .unwrap();
            ray.push(t.elapsed().as_secs_f64() * 1e6);
            let t = Instant::now();
            let o = scene.overlap(capsule, Filter::blocking(1)).unwrap();
            overlap.push(t.elapsed().as_secs_f64() * 1e6);
            let t = Instant::now();
            let s = scene
                .sweep(
                    Capsule {
                        a: capsule.a - DVec3::X,
                        b: capsule.b - DVec3::X,
                        ..capsule
                    },
                    DVec3::X * 2.,
                    Filter::blocking(1),
                )
                .unwrap();
            sweep.push(t.elapsed().as_secs_f64() * 1e6);
            hit_counts = [r.hits.len(), o.hits.len(), s.hits.len()];
            last = vec![
                format!("{:?}", r.stats),
                format!("{:?}", o.stats),
                format!("{:?}", s.stats),
            ];
        }
        let mut world = World::new(1. / 120.);
        world.sleep.enabled = false;
        for i in 0..count {
            let pos = if i < 8 {
                DVec3::new(i as f64 * 1.2, 0., 0.)
            } else {
                DVec3::new(50. + (i % 16) as f64 * 4., (i / 16) as f64 * 4., 50.)
            };
            let mut body = Body::new(1., DVec3::splat(0.4 * 0.3 * 0.3), pos);
            if i >= 8 {
                body.kind = physics::BodyKind::Static;
            } else {
                body.vel = DVec3::X * (if i % 2 == 0 { 0.1 } else { -0.1 });
            }
            let id = world.add(body);
            world.add_collider(Collider::new(id, Shape::Sphere { radius: 0.3 }));
        }
        let initial_momentum = world.momentum(DVec3::ZERO);
        let energy = |w: &World| {
            w.bodies()
                .iter()
                .filter(|b| b.moves())
                .map(|b| 0.5 * b.mass * b.vel.length_squared() + b.rotational_energy())
                .sum::<f64>()
        };
        let initial_energy = energy(&world);
        let mut detect = Vec::new();
        let mut solve = Vec::new();
        let mut total = Vec::new();
        for _ in 0..60 {
            world.step(&NoField);
            detect.push(world.stats.detect.as_secs_f64() * 1e3);
            solve.push(world.stats.solve.as_secs_f64() * 1e3);
            total.push(world.stats.total.as_secs_f64() * 1e3);
        }
        let raw = json!({"detect": detect.clone(), "solve": solve.clone(), "total": total.clone()});
        rows.push(json!({"colliders":count,"scene_build_ms":build_ms,"query_iterations":200,"query_p95_us":{"ray":percentile(&mut ray,0.95),"overlap":percentile(&mut overlap,0.95),"sweep":percentile(&mut sweep,0.95)},"query_last_stats":last,"query_profile":scene.query_profile(),"hit_counts":hit_counts,"rigid_steps":60,"rigid_p95_ms":{"detect":percentile(&mut detect,0.95),"solve":percentile(&mut solve,0.95),"total":percentile(&mut total,0.95)},"rigid_last":world.stats.detection,"rigid_raw_ms":raw,"momentum_residual":world.momentum(DVec3::ZERO)-initial_momentum,"energy_residual":energy(&world)-initial_energy}));
    }
    let mut crowd = Scene::default();
    for i in 0..1024u64 {
        crowd
            .insert_capsule(CapsuleCollider {
                key: ColliderKey {
                    life: Life {
                        instance: 1,
                        entity: i,
                        generation: 0,
                    },
                    shape: 0,
                },
                layers: 1,
                usage: Usage::Blocking,
                capsule: Capsule {
                    a: DVec3::ZERO,
                    b: DVec3::Y * 1.2,
                    radius: 0.3,
                },
            })
            .unwrap();
    }
    crowd.enable_profiling();
    let mut update_ms = Vec::new();
    let mut query_ms = Vec::new();
    let mut crowd_hits = 0;
    for tick in 0..60 {
        let started = Instant::now();
        let mut capsules = Vec::new();
        for i in 0..1024u64 {
            let position = DVec3::new(
                (i % 32) as f64 + 0.08 * (tick as f64 * 0.1 + i as f64).sin(),
                0.,
                (i / 32) as f64,
            );
            let key = ColliderKey {
                life: Life {
                    instance: 1,
                    entity: i,
                    generation: 0,
                },
                shape: 0,
            };
            crowd
                .set_pose(
                    key,
                    physics::queries::Pose {
                        position,
                        ..Default::default()
                    },
                )
                .unwrap();
            capsules.push((
                key.life,
                Capsule {
                    a: position,
                    b: position + DVec3::Y * 1.2,
                    radius: 0.3,
                },
            ));
        }
        update_ms.push(started.elapsed().as_secs_f64() * 1e3);
        let started = Instant::now();
        for (life, capsule) in capsules {
            let mut filter = Filter::blocking(1);
            filter.ignore = Some(life);
            crowd_hits += crowd
                .sweep(capsule, DVec3::X * 0.5, filter)
                .unwrap()
                .hits
                .len();
        }
        query_ms.push(started.elapsed().as_secs_f64() * 1e3);
    }
    assert!(crowd_hits > 0);
    let mut contact_rows = Vec::new();
    for count in [8, 64, 256] {
        let mut world = World::new(1. / 120.);
        world.sleep.enabled = false;
        for i in 0..count {
            let mut body = Body::new(1., DVec3::splat(0.036), DVec3::X * (i as f64 * 0.6));
            body.vel = DVec3::X * if i % 2 == 0 { 0.1 } else { -0.1 };
            let id = world.add(body);
            world.add_collider(Collider::new(id, Shape::Sphere { radius: 0.3 }));
        }
        let initial = world.momentum(DVec3::ZERO);
        let energy = |w: &World| {
            w.bodies()
                .iter()
                .map(|b| 0.5 * b.mass * b.vel.length_squared() + b.rotational_energy())
                .sum::<f64>()
        };
        let initial_energy = energy(&world);
        let mut detect = Vec::new();
        let mut solve = Vec::new();
        let mut total = Vec::new();
        let mut narrow_total = 0;
        let mut point_total = 0;
        for _ in 0..120 {
            world.step(&NoField);
            detect.push(world.stats.detect.as_secs_f64() * 1e3);
            solve.push(world.stats.solve.as_secs_f64() * 1e3);
            total.push(world.stats.total.as_secs_f64() * 1e3);
            narrow_total += world.stats.detection.narrow_phase;
            point_total += world.stats.contact_points;
        }
        let residual = world.momentum(DVec3::ZERO) - initial;
        let energy_residual = energy(&world) - initial_energy;
        assert!(residual.linear.length() < 1e-10 && residual.angular.length() < 1e-10);
        assert!(energy_residual <= 1e-10);
        let raw = json!({"detect": detect.clone(), "solve": solve.clone(), "total": total.clone()});
        contact_rows.push(json!({"bodies":count,"steps":120,"narrow_phase_total":narrow_total,"contact_points_total":point_total,"last_detection":world.stats.detection,
            "raw_ms":raw,"p95_ms":{"detect":percentile(&mut detect,0.95),"solve":percentile(&mut solve,0.95),"total":percentile(&mut total,0.95)},"momentum_residual":residual,"initial_energy":initial_energy,"energy_residual":energy_residual}));
    }
    let crowd_raw = json!({"pose_update_ms":update_ms.clone(),"sweep_tick_ms":query_ms.clone()});
    std::fs::write(output, serde_json::to_vec_pretty(&json!({"schema":"openagents.physics.scene-broadphase.v1","profile":"isolated sparse static background, mixed query meshes/capsules, eight moving spheres","rows":rows,"moving_capsules":{"capsules":1024,"ticks":60,"sweeps_per_tick":1024,"sweep_displacement_m":0.5,"hits":crowd_hits,"pose_update_p95_ms":percentile(&mut update_ms,0.95),"sweep_tick_p95_ms":percentile(&mut query_ms,0.95),"query_profile":crowd.query_profile(),"raw":crowd_raw},"closed_contact_rows":contact_rows})).unwrap()).unwrap();
}
