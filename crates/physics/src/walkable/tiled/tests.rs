use super::super::tests::{add, fixture};
use super::*;

#[test]
fn tile_seams_match_admitted_graph_and_pinned_round_trip() {
    let (mut scene, config) = fixture();
    add(
        &mut scene,
        1,
        DVec3::new(-0.6, 0., -0.6),
        DVec3::new(0.6, 4., 0.6),
    );
    let tiled = Navigation::compile_tiled(&scene, config, 4, &[]).unwrap();
    let flat = Navigation::compile_tile(&scene, config).unwrap();
    let graph = |nav: &Navigation| {
        let mut edges = nav
            .nodes
            .iter()
            .flat_map(|node| {
                node.links.iter().map(|next| {
                    (
                        node.feet.to_array().map(f64::to_bits),
                        nav.nodes[*next].feet.to_array().map(f64::to_bits),
                    )
                })
            })
            .collect::<Vec<_>>();
        edges.sort_unstable();
        edges
    };
    assert_eq!(graph(&flat), graph(&tiled));
    assert_eq!(tiled.tiles.len(), 16);
    let (bytes, digest) = tiled.cooked().unwrap();
    let restored = Navigation::from_cooked(&bytes, digest, 7).unwrap();
    assert_eq!(graph(&restored), graph(&tiled));
    assert_eq!(
        tiled.tiles.iter().map(|t| t.digest).collect::<Vec<_>>(),
        restored.tiles.iter().map(|t| t.digest).collect::<Vec<_>>()
    );
    let mut corrupt = bytes.clone();
    *corrupt.last_mut().unwrap() ^= 1;
    assert!(Navigation::from_cooked(&corrupt, digest, 7).is_err());
    let changed = Navigation::compile_tiled(
        &scene,
        Config {
            character: Settings {
                radius: 0.4,
                ..config.character
            },
            ..config
        },
        4,
        &[],
    )
    .unwrap();
    assert_ne!(changed.tiles[0].digest, tiled.tiles[0].digest);
    let rebound = tiled.bind_instance(8).cooked().unwrap();
    assert_eq!(rebound, (bytes, digest));
    let start = DVec3::new(-2., SKIN, 0.);
    let goal = DVec3::new(2., SKIN, 0.);
    let mut scratch = SearchScratch::default();
    let blockers = Blockers::new(7);
    let first = tiled
        .path_with_scratch(
            &scene,
            &blockers,
            7,
            start,
            goal,
            None,
            Budget::default(),
            &mut scratch,
        )
        .unwrap()
        .unwrap();
    let capacity = scratch.span_capacity();
    for _ in 0..12 {
        let again = restored
            .path_with_scratch(
                &scene,
                &blockers,
                7,
                start,
                goal,
                None,
                Budget::default(),
                &mut scratch,
            )
            .unwrap()
            .unwrap();
        assert_eq!(first.points, again.points);
        assert_eq!(capacity, scratch.span_capacity());
    }
}
#[test]
fn grounded_transition_refuses_gaps_and_changes_identity() {
    let (scene, config) = fixture();
    let baseline = Navigation::compile_tiled(&scene, config, 4, &[]).unwrap();
    let transition = Transition {
        id: 1,
        from: DVec3::new(-1., SKIN, -1.),
        to: DVec3::new(1., SKIN, 1.),
        bidirectional: true,
        kind: TransitionKind::Grounded,
    };
    let linked = Navigation::compile_tiled(&scene, config, 4, &[transition]).unwrap();
    assert_ne!(baseline.cooked().unwrap().1, linked.cooked().unwrap().1);
    assert!(Navigation::compile_tiled(&scene, config, 4, &[transition, transition]).is_err());
    assert!(
        Navigation::compile_tiled(
            &scene,
            config,
            4,
            &[Transition {
                to: DVec3::new(100., 0., 0.),
                ..transition
            }]
        )
        .is_err()
    );
    let mut split = Scene::default();
    add(
        &mut split,
        0,
        DVec3::new(-5., -1., -5.),
        DVec3::new(-0.6, 0., 5.),
    );
    add(
        &mut split,
        1,
        DVec3::new(0.6, -1., -5.),
        DVec3::new(5., 0., 5.),
    );
    assert!(
        Navigation::compile_tiled(&split, config, 4, &[transition])
            .unwrap_err()
            .contains("collision admission")
    );
}
#[test]
fn local_source_edit_changes_only_overlapping_tile_identities() {
    let (mut scene, config) = fixture();
    let before = Navigation::compile_tiled(&scene, config, 4, &[]).unwrap();
    add(
        &mut scene,
        2,
        DVec3::new(-3.7, 0., -3.7),
        DVec3::new(-3.3, 2., -3.3),
    );
    let after = Navigation::compile_tiled(&scene, config, 4, &[]).unwrap();
    let changes = before
        .tiles
        .iter()
        .zip(&after.tiles)
        .filter(|(a, b)| a.digest != b.digest)
        .count();
    assert!(changes > 0 && changes < before.tiles.len(), "{changes}");
    let affected = before
        .affected_tiles(DVec3::new(-3.7, 0., -3.7), DVec3::new(-3.3, 2., -3.3))
        .unwrap();
    assert!(affected.len() < before.tiles.len());
}
#[test]
fn scheduler_bounds_failed_search_reservations_fairness_and_restore() {
    let mut scheduler = Scheduler::default();
    let budget = Budget {
        nodes: 16_384,
        ..Default::default()
    };
    let life = |entity| Life {
        instance: 7,
        entity,
        generation: 1,
    };
    scheduler.begin_tick(1);
    for id in 1..=40 {
        assert_eq!(scheduler.request(life(id), budget).unwrap(), id <= 4);
    }
    assert_eq!(scheduler.used(), (4, 65_536, 2_000_000));
    assert_eq!(scheduler.pending(), 36);
    let mut restored: Scheduler =
        serde_json::from_slice(&serde_json::to_vec(&scheduler).unwrap()).unwrap();
    restored.validate(7).unwrap();
    for tick in 2..=10 {
        scheduler.begin_tick(tick);
        restored.begin_tick(tick);
        for id in 1..=40 {
            assert_eq!(
                scheduler.request(life(id), budget).unwrap(),
                restored.request(life(id), budget).unwrap()
            );
        }
        assert_eq!(scheduler.used().0, 4);
    }
    scheduler.begin_tick(100);
    assert_eq!(scheduler.pending(), 0);
    assert!(scheduler.request(life(100), budget).unwrap());
}
#[test]
fn crowd_steering_is_instance_scoped_stable_and_bounded() {
    let life = |entity| Life {
        instance: 7,
        entity,
        generation: 1,
    };
    let a = CrowdAgent {
        life: life(2),
        feet: DVec3::ZERO,
        radius: 0.35,
    };
    let b = CrowdAgent {
        life: life(1),
        feet: DVec3::new(0.8, 0., 0.),
        radius: 0.35,
    };
    let mut crowd = Crowd::default();
    crowd.rebuild([a, b]).unwrap();
    let motion = crowd.steer(a, DVec3::X * 0.2).unwrap();
    assert!(motion.length() <= 0.2);
    assert_ne!(motion, DVec3::X * 0.2);
    crowd.rebuild([b, a]).unwrap();
    assert_eq!(motion, crowd.steer(a, DVec3::X * 0.2).unwrap());
    crowd
        .rebuild([CrowdAgent {
            life: Life {
                instance: 8,
                ..b.life
            },
            ..b
        }])
        .unwrap();
    assert_eq!(crowd.steer(a, DVec3::X * 0.2).unwrap(), DVec3::X * 0.2);
    assert!(crowd.steer(a, DVec3::splat(f64::NAN)).is_err());
}
#[test]
fn pinned_malformed_graph_is_refused_before_indexing() {
    let (scene, config) = fixture();
    let mut nav = Navigation::compile(&scene, config).unwrap();
    nav.nodes[0].links.push(usize::MAX);
    let (bytes, digest) = nav.cooked().unwrap();
    assert!(
        Navigation::from_cooked(&bytes, digest, 7)
            .unwrap_err()
            .contains("graph bounds")
    );
}
#[test]
fn transient_hard_wall_bounds_fence_compiled_edges_and_budget() {
    let (mut scene, config) = fixture();
    let nav = Navigation::compile(&scene, config).unwrap();
    let start = DVec3::new(-2., SKIN, 0.);
    let goal = DVec3::new(2., SKIN, 0.);
    let bounds = crate::kinematic::Aabb {
        min: DVec3::new(-0.3, 0., -5.),
        max: DVec3::new(0.3, 3., 5.),
    };
    add(&mut scene, 2, bounds.min, bounds.max);
    let durable = Blockers::new(7);
    let query = durable.with_obstacles([bounds]).unwrap();
    assert!(
        nav.path(&scene, &query, 7, start, goal, None, Budget::default())
            .unwrap()
            .is_none()
    );
    assert_eq!(
        serde_json::to_vec(&durable).unwrap(),
        serde_json::to_vec(&query).unwrap()
    );
    assert!(
        durable
            .with_obstacles(std::iter::repeat_n(bounds, 1025))
            .is_err()
    );
    assert!(
        durable
            .with_obstacles([crate::kinematic::Aabb {
                min: DVec3::splat(f64::NAN),
                ..bounds
            }])
            .is_err()
    );
    assert!(
        nav.path(
            &scene,
            &query,
            7,
            start,
            goal,
            None,
            Budget {
                work_units: 1,
                ..Default::default()
            }
        )
        .unwrap_err()
        .contains("work budget exceeded")
    );
}
