//! Retained named-rig acceptance over the licensed character and gait sources.
#![cfg(feature = "compiler")]
use glam::{DVec3, Mat4, Vec3};
use physics::queries::{ColliderKey, Life, Mesh, MeshCollider, Scene, Triangle, Usage};
use verse_engine::{
    animation_graph::Semantic,
    core::LifeId,
    locomotion::{Aim, Controller, Tier},
    motion::State,
    presentation::Instance,
    sockets::{Palette, Sockets},
};

fn support_scene() -> Scene {
    let mut scene = Scene::default();
    let mut triangles = Vec::new();
    // Left foot climbs a slope; the right foot crosses a stair tread.
    for (x0, x1, raised) in [(-4., 0., false), (0., 4., true)] {
        for (z0, z1) in [(-4., -1.), (-1., 0.), (0., 4.)] {
            let height = |z: f64| {
                if raised {
                    if z0 >= -1. { 0.16 } else { 0. }
                } else {
                    -z * 0.05
                }
            };
            let a = DVec3::new(x0, height(z0), z0);
            let b = DVec3::new(x1, height(z0), z0);
            let c = DVec3::new(x1, height(z1), z1);
            let d = DVec3::new(x0, height(z1), z1);
            triangles.extend([Triangle([a, c, b]), Triangle([a, d, c])]);
        }
    }
    scene
        .insert(MeshCollider {
            key: ColliderKey {
                life: Life {
                    instance: 77,
                    entity: 900,
                    generation: 0,
                },
                shape: 0,
            },
            layers: u32::MAX,
            usage: Usage::Blocking,
            mesh: Mesh::compile(triangles).unwrap(),
        })
        .unwrap();
    scene
}
#[test]
fn named_outfits_share_contacts_sockets_and_marker_timing_across_tiers() {
    let dir = tempfile::tempdir().unwrap();
    let mut pack = verse_content::compiler::original::generate(dir.path()).unwrap();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets/verse/characters/quaternius");
    let support_scene = support_scene();
    let support = verse_world::animation_support::Queries::new(&support_scene);
    let mut evidence = Vec::new();
    for name in ["male-peasant", "female-ranger"] {
        let model =
            verse_content::compiler::characters::appearance(&mut pack, dir.path(), &root, name)
                .unwrap();
        let authored = model.graph.as_ref().unwrap();
        let graph = Semantic::new(authored, &model).unwrap();
        let rig = graph.rig().unwrap();
        let sockets = Sockets::admit(&model).unwrap();
        let mut all_markers = Vec::new();
        let mut sampled = Vec::new();
        let mut authority_replays = Vec::new();
        for tier in [Tier::Full, Tier::Half, Tier::Quarter] {
            let mut controller = Controller::default();
            let mut markers = Vec::new();
            let mut samples = 0;
            let mut contacts = 0;
            let mut moving_contacts = 0;
            let mut slope_contacts = 0;
            let mut stair_contacts = 0;
            let mut max_socket_error: f32 = 0.;
            let mut max_plant_drift: f32 = 0.;
            let mut last_planted = [None; 2];
            let mut position = Vec3::ZERO;
            let mut frames = Vec::new();
            let mut scene = verse_engine::director::Scene::from_json(include_bytes!(
                "../../../assets/verse/original/ritual.json"
            ))
            .unwrap();
            scene.cut_at = 0.;
            scene.cues.clear();
            scene
                .actors
                .iter_mut()
                .find(|a| a.id == 14)
                .unwrap()
                .position = Vec3::new(0., 0., -5.);
            let mut authority = verse_world::play::Game::combat_in(scene, false, 77).unwrap();
            let mut authority_hashes = Vec::new();
            let mut cast_outcomes = Vec::new();
            for tick in 0..300 {
                if [30, 100, 180].contains(&tick) {
                    let result = authority.activate(verse_world::play::Ability::FireBolt);
                    cast_outcomes.push(serde_json::json!({"tick":tick,"result":result}));
                }
                authority.tick(1. / 60., [0.; 2]).unwrap();
                let before_pose = authority.checkpoint().unwrap();
                let state = match tick {
                    0..=14 => State::Idle,
                    15..=134 => State::Walk,
                    135..=179 => State::Run,
                    180..=209 => State::Cast,
                    210..=239 => State::Death,
                    240..=269 => State::Prone,
                    _ => State::Idle,
                };
                let generation = u64::from(tick >= 270);
                if matches!(state, State::Walk | State::Run) {
                    position.z -= if state == State::Run { 4.5 } else { 1.8 } / 60.;
                }
                let life = LifeId {
                    instance: 77,
                    actor: 4,
                    generation,
                };
                let instance = Instance {
                    mount: None,
                    actor: Some(life),
                    model: name.into(),
                    transform: Mat4::from_translation(position) * verse_content::basis(),
                    animation: state.into(),
                    time: if tick >= 180 {
                        (tick - 180) as f32 / 60.
                    } else {
                        tick as f32 / 60.
                    },
                    animation_epoch: Some(generation),
                    emission: Vec3::ONE,
                };
                let (frame, diagnostic) = controller
                    .update(
                        &graph,
                        &instance,
                        tick as f64 / 60.,
                        Aim {
                            yaw: 0.2,
                            pitch: 0.1,
                        },
                        tier,
                        Some(&support),
                    )
                    .unwrap();
                assert_eq!(authority.checkpoint().unwrap(), before_pose);
                use sha2::{Digest, Sha256};
                authority_hashes.push(format!("{:x}", Sha256::digest(&before_pose)));
                samples += usize::from(frame.evaluated);
                markers.extend(frame.markers.clone());
                let feet = rig
                    .foot_positions(instance.transform, &frame.matrices)
                    .unwrap();
                if let Some(report) = &diagnostic.adjustment {
                    if matches!(state, State::Death | State::Prone) {
                        assert_eq!(report.probes, 0);
                    }
                    for side in 0..2 {
                        if report.planted[side] {
                            contacts += 1;
                            let ground = report.contacts[side].unwrap();
                            slope_contacts += usize::from(ground.normal.y < 0.9999);
                            stair_contacts += usize::from(
                                ground.normal.y > 0.9999
                                    && (ground.position.y - 0.16).abs() < 0.001,
                            );
                            moving_contacts +=
                                usize::from(matches!(state, State::Walk | State::Run));
                            assert!(report.foot_error[side] < 0.005);
                            if let Some((old_generation, p)) = last_planted[side] {
                                if generation == old_generation {
                                    max_plant_drift = max_plant_drift.max(feet[side].distance(p));
                                }
                            }
                            last_planted[side] = Some((generation, feet[side]));
                        } else {
                            last_planted[side] = None;
                        }
                    }
                }
                for attachment in &model.attachments {
                    let actual = sockets
                        .point(
                            Palette::admit(&model, &frame.matrices).unwrap(),
                            instance.transform,
                            attachment.id,
                        )
                        .unwrap();
                    let expected = (instance.transform * frame.matrices[attachment.bone])
                        .transform_point3(attachment.position.into());
                    max_socket_error = max_socket_error.max(actual.distance(expected));
                }
                if tick == 270 {
                    assert!(frame.evaluated);
                    assert!(frame.markers.is_empty());
                    assert!(diagnostic.adjustment.as_ref().unwrap().inputs.reset);
                }
                if [14, 30, 134, 170, 200, 230, 260, 270, 299].contains(&tick) {
                    frames.push(diagnostic);
                }
            }
            assert!(contacts > 20, "{name}/{tier:?}: {contacts} contacts");
            assert!(
                moving_contacts > 30,
                "{name}/{tier:?}: {moving_contacts} moving contacts"
            );
            assert!(
                slope_contacts > 5 && stair_contacts > 5,
                "{name}/{tier:?}: slope {slope_contacts}, stair {stair_contacts}"
            );
            assert!(max_socket_error < 0.00001);
            assert!(
                max_plant_drift < 0.005,
                "{name}/{tier:?}: drift {max_plant_drift}"
            );
            let events = authority.events.clone();
            assert!(
                events
                    .iter()
                    .any(|e| matches!(e.kind, verse_world::events::Kind::Damage { .. }))
            );
            authority_replays
                .push(serde_json::json!({"checkpoints":authority_hashes,"events":events,"cast_outcomes":cast_outcomes}));
            sampled.push(samples);
            all_markers.push(markers.clone());
            evidence.push(serde_json::json!({"outfit":name,"tier":tier,"samples":samples,"contacts":contacts,"moving_contacts":moving_contacts,"slope_contacts":slope_contacts,"stair_contacts":stair_contacts,"max_socket_error_m":max_socket_error,"max_plant_drift_m":max_plant_drift,"markers":markers,"frames":frames,"rig":rig.definition,"clips":model.clips.iter().map(|c|(c.id,c.duration)).collect::<Vec<_>>()}));
        }
        assert_eq!(authority_replays[0], authority_replays[1]);
        assert_eq!(authority_replays[0], authority_replays[2]);
        evidence.push(serde_json::json!({"outfit":name,"authority_equal_across_tiers":true,"authority":authority_replays[0]}));
        assert_eq!(all_markers[0], all_markers[1]);
        assert_eq!(all_markers[0], all_markers[2]);
        assert!(sampled[1] < sampled[0] * 3 / 4, "{sampled:?}");
        assert!(sampled[2] < sampled[0] / 2, "{sampled:?}");
    }
    if let Some(path) = std::env::var_os("VERSE_LOCOMOTION_EVIDENCE") {
        std::fs::write(
            path,
            serde_json::to_vec_pretty(
                &serde_json::json!({"schema":"verse.locomotion.named-rigs.v1","fixtures":evidence}),
            )
            .unwrap(),
        )
        .unwrap();
    }
}
