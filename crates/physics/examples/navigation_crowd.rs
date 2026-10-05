//! Scratch multilevel navigation fixture. No network, display, or owner state.
use glam::DVec3;
use physics::{
    character::{Character, Settings},
    queries::{CapsuleCollider, ColliderKey, Filter, Life, Mesh, MeshCollider, Scene, Usage},
    walkable::{Blockers, Budget, Config, Crowd, CrowdAgent, Navigation, Scheduler, SearchScratch},
};
use serde_json::json;
use std::{collections::BTreeMap, time::Instant};
fn add(scene: &mut Scene, id: u32, min: DVec3, max: DVec3) {
    scene
        .insert(MeshCollider {
            key: ColliderKey {
                life: Life {
                    instance: 7,
                    entity: 0,
                    generation: 0,
                },
                shape: id,
            },
            layers: 1,
            usage: Usage::Blocking,
            mesh: Mesh::from_box(min, max).unwrap(),
        })
        .unwrap();
}
fn fixture() -> (Scene, Config) {
    let mut scene = Scene::default();
    add(
        &mut scene,
        0,
        DVec3::new(-12., -1., -12.),
        DVec3::new(12., 0., 12.),
    );
    add(
        &mut scene,
        1,
        DVec3::new(-8., 0., 3.),
        DVec3::new(8., 1.2, 10.),
    );
    for i in 0..6 {
        add(
            &mut scene,
            2 + i,
            DVec3::new(-3., 0., -0.6 + i as f64 * 0.6),
            DVec3::new(3., (i + 1) as f64 * 0.2, i as f64 * 0.6),
        );
    }
    add(
        &mut scene,
        8,
        DVec3::new(-12., 0., -2.3),
        DVec3::new(-1.5, 3., -1.9),
    );
    add(
        &mut scene,
        9,
        DVec3::new(1.5, 0., -2.3),
        DVec3::new(12., 3., -1.9),
    );
    (
        scene,
        Config {
            instance: 7,
            layers: 1,
            min: DVec3::new(-10., -0.1, -10.),
            max: DVec3::new(10., 3., 10.),
            cell: 0.5,
            character: Settings::default(),
            work_budget: 40_000_000,
        },
    )
}
fn quantile(samples: &[f64], p: f64) -> f64 {
    let mut samples = samples.to_vec();
    samples.sort_by(f64::total_cmp);
    if samples.is_empty() {
        0.
    } else {
        samples[((samples.len() - 1) as f64 * p).ceil() as usize]
    }
}
fn run() -> serde_json::Value {
    let (mut scene, config) = fixture();
    let cook = Instant::now();
    let nav = Navigation::compile_tiled(&scene, config, 8, &[]).unwrap();
    let cook_ms = cook.elapsed().as_secs_f64() * 1000.;
    let (cooked, digest) = nav.cooked().unwrap();
    let loaded = Navigation::from_cooked(&cooked, digest, 7).unwrap();
    let budget = Budget {
        nodes: 16_384,
        ..Default::default()
    };
    let mut scratch = SearchScratch::default();
    let mut scheduler = Scheduler::default();
    let mut crowd = Crowd::default();
    let mut blockers = Blockers::new(7);
    let mut actors = (1..=40)
        .map(|id| {
            (
                id,
                Character::new(DVec3::new(
                    -4. + ((id - 1) % 8) as f64,
                    0.,
                    -9. + ((id - 1) / 8) as f64,
                )),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let life = |id| Life {
        instance: 7,
        entity: id,
        generation: 1,
    };
    let goal = |id| {
        DVec3::new(
            -2. + ((id - 1) % 5) as f64,
            1.20002,
            3.5 + ((id - 1) / 5) as f64 * 0.85,
        )
    };
    let mut paths: BTreeMap<u64, (Vec<DVec3>, usize)> = BTreeMap::new();
    let mut planned = BTreeMap::<u64, u64>::new();
    let mut stuck = BTreeMap::<u64, u64>::new();
    for (id, actor) in &actors {
        scene
            .insert_capsule(CapsuleCollider {
                key: ColliderKey {
                    life: life(*id),
                    shape: 0,
                },
                capsule: config.character.capsule(actor.feet),
                layers: 1,
                usage: Usage::Blocking,
            })
            .unwrap();
    }
    let mut route_ms = vec![];
    let mut tick_ms = vec![];
    let mut reached = vec![];
    let mut results = vec![];
    let mut trace = vec![];
    let mut no_path = 0;
    let mut exhausted = 0;
    let mut routed = std::collections::BTreeSet::new();
    let mut replanned = 0;
    let mut max_plans = 0;
    for tick in 0..360u64 {
        let time = Instant::now();
        scheduler.begin_tick(tick);
        if tick == 90 {
            let min = DVec3::new(-0.4, 0., -5.4);
            let max = DVec3::new(0.4, 2.5, -4.6);
            blockers.upsert(life(1000), min, max).unwrap();
            scene
                .insert(MeshCollider {
                    key: ColliderKey {
                        life: life(1000),
                        shape: 0,
                    },
                    layers: 1,
                    usage: Usage::Blocking,
                    mesh: Mesh::from_box(min, max).unwrap(),
                })
                .unwrap();
            let affected = nav.affected_tiles(min, max).unwrap();
            let mut invalidate = vec![];
            for (id, (points, cursor)) in &paths {
                if nav
                    .route_tiles(actors[id].feet, &points[*cursor..])
                    .unwrap()
                    .iter()
                    .any(|tile| affected.contains(tile))
                {
                    invalidate.push(*id);
                }
            }
            replanned = invalidate.len();
            for id in invalidate {
                paths.remove(&id);
            }
        }
        // A temporary closed door makes goals unreachable, then reopening requests fresh routes.
        if tick == 180 {
            let min = DVec3::new(-1.5, 0., -2.3);
            let max = DVec3::new(1.5, 3., -1.9);
            blockers.upsert(life(1001), min, max).unwrap();
            scene
                .insert(MeshCollider {
                    key: ColliderKey {
                        life: life(1001),
                        shape: 0,
                    },
                    layers: 1,
                    usage: Usage::Blocking,
                    mesh: Mesh::from_box(min, max).unwrap(),
                })
                .unwrap();
            paths.clear();
        }
        if tick == 210 {
            blockers.remove(life(1001)).unwrap();
            scene.remove(ColliderKey {
                life: life(1001),
                shape: 0,
            });
            paths.clear();
        }
        crowd
            .rebuild(actors.iter().map(|(id, actor)| CrowdAgent {
                life: life(*id),
                feet: actor.feet,
                radius: 0.35,
            }))
            .unwrap();
        for (id, actor) in &mut actors {
            if (paths.get(id).is_some_and(|(points, _)| points.is_empty())
                || stuck.get(id).is_some_and(|steps| *steps >= 8))
                && actor.feet.distance(goal(*id)) > 0.2
                && planned.get(id).is_some_and(|last| tick - last >= 15)
            {
                paths.remove(id);
            }
            if !paths.contains_key(id) && scheduler.request(life(*id), budget).unwrap() {
                let start = Instant::now();
                let result = nav.path_with_scratch(
                    &scene,
                    &blockers,
                    7,
                    actor.feet,
                    goal(*id),
                    Some(life(*id)),
                    budget,
                    &mut scratch,
                );
                route_ms.push(start.elapsed().as_secs_f64() * 1000.);
                planned.insert(*id, tick);
                stuck.insert(*id, 0);
                match result {
                    Ok(Some(path)) => {
                        routed.insert(*id);
                        paths.insert(*id, (path.points, 0));
                        results.push(json!([tick, id, "ready"]));
                    }
                    Ok(None) => {
                        no_path += 1;
                        paths.insert(*id, (vec![], 0));
                        results.push(json!([tick, id, "no_path"]));
                    }
                    Err(e) if e.contains("work budget exceeded") => {
                        exhausted += 1;
                        paths.insert(*id, (vec![], 0));
                        results.push(json!([tick, id, "exhausted"]));
                    }
                    Err(e) => panic!("{e}"),
                }
            }
            if let Some((points, cursor)) = paths.get_mut(id) {
                while points
                    .get(*cursor)
                    .is_some_and(|p| p.distance(actor.feet) < 0.1)
                {
                    *cursor += 1;
                }
                if let Some(next) = points.get(*cursor) {
                    let delta = *next - actor.feet;
                    let horizontal = DVec3::new(delta.x, 0., delta.z);
                    let motion = horizontal.normalize_or_zero() * horizontal.length().min(0.08);
                    let motion = crowd
                        .steer(
                            CrowdAgent {
                                life: life(*id),
                                feet: actor.feet,
                                radius: 0.35,
                            },
                            motion,
                        )
                        .unwrap();
                    let previous = actor.feet;
                    actor
                        .step(
                            &scene,
                            Filter {
                                ignore: Some(life(*id)),
                                ..Filter::blocking(7)
                            },
                            config.character,
                            motion * 30.,
                            false,
                            1. / 30.,
                        )
                        .unwrap();
                    if actor.feet.distance(previous) < 1e-5 {
                        *stuck.entry(*id).or_default() += 1;
                    } else {
                        stuck.insert(*id, 0);
                    }
                    let key = ColliderKey {
                        life: life(*id),
                        shape: 0,
                    };
                    scene.remove_capsule(key);
                    scene
                        .insert_capsule(CapsuleCollider {
                            key,
                            capsule: config.character.capsule(actor.feet),
                            layers: 1,
                            usage: Usage::Blocking,
                        })
                        .unwrap();
                    let overlap = scene
                        .overlap(
                            config.character.capsule(actor.feet),
                            Filter {
                                ignore: Some(life(*id)),
                                ..Filter::blocking(7)
                            },
                        )
                        .unwrap();
                    assert!(
                        overlap.hits.iter().all(|hit| hit.penetration <= 0.000021),
                        "crowd penetrated admitted geometry"
                    );
                    assert!(
                        DVec3::new(actor.feet.x - previous.x, 0., actor.feet.z - previous.z)
                            .length()
                            < 0.081,
                        "unadmitted movement"
                    );
                }
            }
        }
        max_plans = max_plans.max(scheduler.used().0);
        assert!(scheduler.used().0 <= 4);
        assert!(scheduler.used().2 <= 2_000_000);
        tick_ms.push(time.elapsed().as_secs_f64() * 1000.);
        trace.push(
            actors
                .iter()
                .map(|(id, a)| (*id, a.feet.to_array()))
                .collect::<Vec<_>>(),
        );
    }
    for (id, actor) in &actors {
        if actor.feet.distance(goal(*id)) < 0.2 {
            reached.push(*id);
        }
    }
    // Independent work exhaustion and disconnected island proofs.
    for id in 1..=40 {
        scene.remove_capsule(ColliderKey {
            life: life(id),
            shape: 0,
        });
    }
    let mut probe = SearchScratch::default();
    let a = DVec3::new(-2., 0., -8.);
    let b = DVec3::new(0., 1.20002, 8.);
    assert!(
        loaded
            .path_with_scratch(
                &scene,
                &blockers,
                7,
                a,
                b,
                None,
                Budget {
                    work_units: 1,
                    ..budget
                },
                &mut probe
            )
            .unwrap_err()
            .contains("work budget exceeded")
    );
    let mut disconnected = scene.clone();
    add(
        &mut disconnected,
        20,
        DVec3::new(-12., 0., -2.3),
        DVec3::new(12., 3., -1.9),
    );
    let closed = blockers
        .with_obstacles([physics::kinematic::Aabb {
            min: DVec3::new(-12., 0., -2.3),
            max: DVec3::new(12., 3., -1.9),
        }])
        .unwrap();
    assert!(
        loaded
            .path_with_scratch(&disconnected, &closed, 7, a, b, None, budget, &mut probe)
            .unwrap()
            .is_none()
    );
    assert_eq!(
        routed.len(),
        40,
        "every crowd actor receives a valid admitted route"
    );
    assert!(replanned > 0);
    assert!(no_path > 0);
    json!({"profile":"original-multilevel-stair-door-construction-v1","actors":40,"ticks":360,"dt":1./30.,"tile_count":nav.tiles().len(),"spans":nav.nodes().len(),"cook_ms":cook_ms,"cooked_bytes":cooked.len(),"cooked_graph":String::from_utf8(cooked).unwrap(),"manifest_digest":digest.iter().map(|b|format!("{b:02x}")).collect::<String>(),"tiles":nav.tiles().iter().map(|t|json!({"coordinate":t.coordinate,"digest":t.digest.iter().map(|b|format!("{b:02x}")).collect::<String>(),"spans":t.spans})).collect::<Vec<_>>(),"route_ms":route_ms,"route_p99_ms":quantile(&route_ms,0.99),"tick_ms":tick_ms,"tick_p99_ms":quantile(&tick_ms,0.99),"max_plans_per_tick":max_plans,"scratch_spans":scratch.span_capacity(),"routed_actors":routed.len(),"reached_actors":reached,"construction_replans":replanned,"no_path":no_path,"exhausted":exhausted,"results":results,"trace":trace})
}
fn main() {
    let first = run();
    let second = run();
    assert_eq!(first["trace"], second["trace"]);
    assert_eq!(first["results"], second["results"]);
    assert_eq!(first["manifest_digest"], second["manifest_digest"]);
    let mut result = first;
    result["replay_equal"] = json!(true);
    println!("{}", serde_json::to_string(&result).unwrap());
}
