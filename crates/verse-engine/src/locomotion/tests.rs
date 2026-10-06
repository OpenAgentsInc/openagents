use super::*;
use crate::{
    animation_graph::{Authored, Semantic},
    assets::{Bone, Clip, Model, RestPose, Skin},
    motion::{Binding, Mode},
};

fn model(scale: f32) -> Model {
    let names = [
        "root", "thigh_l", "calf_l", "foot_l", "spine_03", "hand_l", "thigh_r", "calf_r", "foot_r",
    ];
    let parents = [-1, 0, 1, 2, 0, 4, 0, 6, 7];
    let positions = [
        [0., 0., 0.],
        [-0.15, 1., 0.],
        [0., -0.45, 0.08],
        [0., -0.5, -0.08],
        [0., 1.1, 0.],
        [0.4, 0.2, 0.],
        [0.15, 1., 0.],
        [0., -0.45, 0.08],
        [0., -0.5, -0.08],
    ];
    let rest: Vec<_> = positions
        .iter()
        .map(|p| RestPose {
            translation: (Vec3::from(*p) * scale).to_array(),
            rotation: Quat::IDENTITY.to_array(),
            scale: [1.; 3],
        })
        .collect();
    let mut global = vec![Mat4::IDENTITY; names.len()];
    for i in 0..names.len() {
        let local = Mat4::from_translation(rest[i].translation.into());
        global[i] = if parents[i] < 0 {
            local
        } else {
            global[parents[i] as usize] * local
        };
    }
    Model {
        graph: None,
        markers: vec![],
        source: "admitted-character-fixture".into(),
        source_sha256: String::new(),
        surfaces: vec![],
        height: 1.8 * scale,
        bones: parents
            .into_iter()
            .map(|parent| Bone {
                parent,
                pivot: [0.; 3],
            })
            .collect(),
        skin: Some(Skin {
            names: names.into_iter().map(str::to_owned).collect(),
            rest,
            inverse_bind: global
                .into_iter()
                .map(|m| m.inverse().to_cols_array())
                .collect(),
            basis: Mat4::IDENTITY.to_cols_array(),
        }),
        states: State::ALL
            .into_iter()
            .map(|state| {
                (
                    state,
                    Binding {
                        clip: match state {
                            State::Walk => 1,
                            State::Run => 2,
                            _ => 0,
                        },
                        mode: Mode::Loop,
                        transition_seconds: 0.1,
                    },
                )
            })
            .collect(),
        clips: (0..3)
            .map(|id| Clip {
                id,
                duration: 1.,
                bones: vec![],
            })
            .collect(),
        attachments: vec![],
    }
}
struct Terrain;
impl Support for Terrain {
    fn sample(&self, _: LifeId, p: Vec3, _: f32) -> Result<Option<Ground>, String> {
        let step = if p.x > 0. { 0.18 } else { 0. };
        Ok(Some(Ground {
            position: Vec3::new(p.x, step + p.z * 0.15, p.z),
            normal: Vec3::new(0., 1., -0.15).normalize(),
        }))
    }
}
fn life(generation: u64) -> LifeId {
    LifeId {
        instance: 1,
        actor: 14,
        generation,
    }
}

#[test]
fn two_proportions_plant_on_slopes_and_stairs_and_reset_after_death() {
    for scale in [1., 1.08] {
        let model = model(scale);
        let rig = Rig::admit(&model, &Definition::universal(0)).unwrap();
        let mut playback = Playback::default();
        let mut anchors = None;
        for tick in 0..10 {
            let body = Mat4::from_translation(Vec3::Z * tick as f32 * 0.008);
            playback
                .controls(life(0), body, tick as f64 / 60., None, Aim::default())
                .unwrap();
            let mut palette =
                crate::animation::pose_selected(&model, State::Walk.into(), 0.).unwrap();
            let report = playback
                .adjust(
                    &rig,
                    life(0),
                    body,
                    State::Walk,
                    0.1,
                    &mut palette,
                    Some(&Terrain),
                )
                .unwrap();
            // A low foot keeps its admitted ground anchor while the body advances.
            assert!(report.planted[0], "{report:?}");
            assert!(report.foot_error[0] < 0.005);
            let feet = rig.foot_positions(body, &palette).unwrap();
            if let Some(previous) = anchors {
                assert!(feet[0].distance(previous) < 0.001);
            } else {
                anchors = Some(feet[0]);
            }
        }
        let body = Mat4::IDENTITY;
        playback
            .controls(life(0), body, 1., None, Aim::default())
            .unwrap();
        let mut palette = crate::animation::pose_selected(&model, State::Idle.into(), 0.).unwrap();
        let standing = playback
            .adjust(
                &rig,
                life(0),
                body,
                State::Idle,
                0.,
                &mut palette,
                Some(&Terrain),
            )
            .unwrap();
        assert_eq!(standing.planted, [true, true]);
        let dead = playback
            .adjust(
                &rig,
                life(0),
                body,
                State::Death,
                0.,
                &mut palette,
                Some(&Terrain),
            )
            .unwrap();
        assert_eq!(dead.probes, 0);
        assert_eq!(dead.planted, [false; 2]);
        assert!(
            playback
                .controls(life(1), body, 1.1, Some(2), Aim::default())
                .unwrap()
                .reset
        );
        let mut palette = crate::animation::pose_selected(&model, State::Idle.into(), 0.).unwrap();
        assert_eq!(
            playback
                .adjust(
                    &rig,
                    life(1),
                    body,
                    State::Idle,
                    0.,
                    &mut palette,
                    Some(&Terrain)
                )
                .unwrap()
                .planted,
            [true; 2]
        );
    }
}

#[test]
fn root_motion_is_removed_and_aim_updates_the_same_socket_palette() {
    let mut model = model(1.);
    model.clips[1].bones.push(crate::assets::BoneKeys {
        bone: 0,
        translation: vec![(0., [0.3, 0., 0.2])],
        rotation: vec![(0., Quat::from_rotation_y(0.3).to_array())],
        scale: vec![],
    });
    model.attachments.push(crate::assets::Attachment {
        id: 6,
        bone: 5,
        position: [0.4, 1.3, 0.],
    });
    let rig = Rig::admit(&model, &Definition::universal(0)).unwrap();
    let body = Mat4::from_translation(Vec3::new(2., 0., 3.));
    let mut playback = Playback::default();
    playback
        .controls(
            life(0),
            body,
            0.,
            None,
            Aim {
                yaw: 0.2,
                pitch: 0.1,
            },
        )
        .unwrap();
    let mut palette = crate::animation::pose_selected(&model, State::Walk.into(), 0.).unwrap();
    let report = playback
        .adjust(&rig, life(0), body, State::Walk, 0., &mut palette, None)
        .unwrap();
    assert!((report.root_horizontal_removed - 0.13f32.sqrt()).abs() < 0.0001);
    let root = rig.world(body, &palette, rig.root);
    assert!(root.w_axis.truncate().distance(body.w_axis.truncate()) < 0.0001);
    assert!(
        root.to_scale_rotation_translation()
            .1
            .angle_between(Quat::IDENTITY)
            < 0.001
    );
    let sockets = crate::sockets::Sockets::admit(&model).unwrap();
    let p = sockets
        .point(
            crate::sockets::Palette::admit(&model, &palette).unwrap(),
            body,
            6,
        )
        .unwrap();
    assert!(p.distance(rig.world(body, &palette, 5).w_axis.truncate()) < 0.0001);
    assert!(p.distance(body.transform_point3(Vec3::new(0.4, 1.3, 0.))) > 0.02);
    let json = serde_json::to_value(Definition::universal(0)).unwrap();
    let mut refused = json;
    refused["root_motion"] = serde_json::json!("authority");
    assert!(serde_json::from_value::<Definition>(refused).is_err());
}

#[test]
fn unsupported_skeletons_and_bad_inputs_refuse_admission() {
    let model = model(1.);
    let mut missing = Definition::universal(0);
    missing.legs[0].foot = "missing".into();
    assert!(Rig::admit(&model, &missing).is_err());
    let mut bad = model.clone();
    bad.skin.as_mut().unwrap().names[2] = "root".into();
    assert!(Rig::admit(&bad, &Definition::universal(0)).is_err());
    let mut bad = model.clone();
    bad.bones[3].parent = 0;
    assert!(Rig::admit(&bad, &Definition::universal(0)).is_err());
    assert!(
        Aim {
            yaw: f32::NAN,
            pitch: 0.
        }
        .validate()
        .is_err()
    );
    assert!(
        Ground {
            position: Vec3::ZERO,
            normal: Vec3::ZERO
        }
        .validate()
        .is_err()
    );
    let mut playback = Playback::default();
    assert!(
        playback
            .controls(life(0), Mat4::ZERO, 0., None, Aim::default())
            .is_err()
    );
}

#[test]
fn speed_blends_and_crowd_tiers_preserve_marker_delivery_and_lifetime_fences() {
    let mut model = model(1.);
    for clip in 0..3 {
        model.markers.push(crate::markers::ClipTrack {
            clip,
            track: crate::markers::Track {
                duration: 1.,
                markers: vec![
                    crate::markers::Marker {
                        id: 1,
                        seconds: 0.25,
                    },
                    crate::markers::Marker {
                        id: 2,
                        seconds: 0.75,
                    },
                ],
            },
        });
    }
    let authored = Authored::from_locomotion(&model, Definition::universal(0)).unwrap();
    let graph = Semantic::new(&authored, &model).unwrap();
    let mut marker_results = Vec::new();
    let mut evaluations = Vec::new();
    for tier in [Tier::Full, Tier::Half, Tier::Quarter] {
        let mut playback = crate::animation_graph::Playback::default();
        let mut markers = Vec::new();
        let mut sampled = 0;
        for tick in 0..121 {
            let clock = tick as f64 / 60.;
            let state = if tick < 90 { State::Walk } else { State::Cast };
            let values = graph
                .motion_values(
                    state,
                    Inputs {
                        speed: 1.8,
                        ..Default::default()
                    },
                )
                .unwrap();
            let frame = playback
                .update_quality(
                    graph.admitted(),
                    life(0),
                    &values,
                    clock,
                    clock,
                    Some(4),
                    tier.sample(life(0), tick),
                )
                .unwrap();
            sampled += usize::from(frame.evaluated);
            markers.extend(frame.markers);
            assert_eq!(frame.matrices.len(), model.bones.len());
        }
        let values = graph.motion_values(State::Idle, Inputs::default()).unwrap();
        let reborn = playback
            .update_quality(graph.admitted(), life(1), &values, 0., 3., Some(5), false)
            .unwrap();
        assert!(reborn.evaluated);
        assert!(reborn.markers.is_empty());
        evaluations.push(sampled);
        marker_results.push(markers);
    }
    assert_eq!(marker_results[0], marker_results[1]);
    assert_eq!(marker_results[0], marker_results[2]);
    assert!(evaluations[1] < evaluations[0] * 3 / 4);
    assert!(evaluations[2] < evaluations[0] / 2);
}

#[test]
fn controller_seeks_replacements_and_rejections_preserve_explicit_baselines() {
    let mut model = model(1.);
    model.graph = Some(Authored::from_locomotion(&model, Definition::universal(0)).unwrap());
    let graph = Semantic::new(model.graph.as_ref().unwrap(), &model).unwrap();
    let mut controller = Controller::default();
    let mut instance = crate::presentation::Instance {
        mount: None,
        actor: Some(life(0)),
        model: "fixture".into(),
        transform: Mat4::IDENTITY,
        animation: State::Walk.into(),
        time: 1.,
        animation_epoch: Some(1),
        emission: Vec3::ONE,
    };
    controller
        .update(
            &graph,
            &instance,
            0.,
            Aim::default(),
            Tier::Full,
            Some(&Terrain),
        )
        .unwrap();
    instance.transform = Mat4::from_translation(Vec3::Z * 0.03);
    instance.time = 1.01;
    let (_, moving) = controller
        .update(
            &graph,
            &instance,
            1. / 60.,
            Aim::default(),
            Tier::Full,
            Some(&Terrain),
        )
        .unwrap();
    assert!(moving.adjustment.as_ref().unwrap().inputs.speed > 1.7);
    let (_, repeat) = controller
        .update(
            &graph,
            &instance,
            1. / 60.,
            Aim::default(),
            Tier::Full,
            Some(&Terrain),
        )
        .unwrap();
    assert_eq!(moving.phase, repeat.phase);
    assert_eq!(moving.parameters, repeat.parameters);
    assert!(
        controller
            .update(
                &graph,
                &instance,
                2. / 60.,
                Aim {
                    yaw: f32::NAN,
                    pitch: 0.
                },
                Tier::Full,
                None
            )
            .is_err()
    );
    instance.time = 0.;
    let (seek, report) = controller
        .update(
            &graph,
            &instance,
            2. / 60.,
            Aim::default(),
            Tier::Quarter,
            Some(&Terrain),
        )
        .unwrap();
    assert!(seek.evaluated && seek.markers.is_empty());
    assert!(report.adjustment.as_ref().unwrap().inputs.reset);
    assert_eq!(report.phase, 0.);
    let replacement = Semantic::new(model.graph.as_ref().unwrap(), &model).unwrap();
    let (_, report) = controller
        .update(
            &replacement,
            &instance,
            3. / 60.,
            Aim::default(),
            Tier::Quarter,
            Some(&Terrain),
        )
        .unwrap();
    assert!(report.adjustment.as_ref().unwrap().inputs.reset);
    assert_eq!(report.phase, 0.);
    let (_, gap) = controller
        .update(
            &replacement,
            &instance,
            3.,
            Aim::default(),
            Tier::Quarter,
            Some(&Terrain),
        )
        .unwrap();
    assert!(gap.adjustment.as_ref().unwrap().inputs.reset);
}

#[test]
fn cadence_accepts_the_largest_admitted_actor_identity() {
    let life = LifeId {
        instance: 1,
        actor: u64::MAX,
        generation: 1,
    };
    assert!(Tier::Full.sample(life, u64::MAX));
    let _ = Tier::Quarter.sample(life, u64::MAX);
}

#[test]
fn controller_crowd_cadence_tracks_frames_at_thirty_sixty_and_one_twenty_hertz() {
    let model = model(1.);
    let authored = Authored::from_locomotion(&model, Definition::universal(0)).unwrap();
    let graph = Semantic::new(&authored, &model).unwrap();
    for hz in [30., 60., 120.] {
        let mut counts = Vec::new();
        for tier in [Tier::Full, Tier::Half, Tier::Quarter] {
            let mut controller = Controller::default();
            let mut samples = 0;
            for tick in 0..120 {
                let time = tick as f64 / hz;
                let instance = crate::presentation::Instance {
                    mount: None,
                    actor: Some(life(0)),
                    model: "fixture".into(),
                    transform: Mat4::from_translation(Vec3::Z * time as f32 * 1.8),
                    animation: State::Walk.into(),
                    time: time as f32,
                    animation_epoch: Some(1),
                    emission: Vec3::ONE,
                };
                let (frame, _) = controller
                    .update(&graph, &instance, time, Aim::default(), tier, None)
                    .unwrap();
                samples += usize::from(frame.evaluated);
            }
            counts.push(samples);
        }
        assert_eq!(counts[0], 120);
        assert!((60..=62).contains(&counts[1]), "{hz}: {counts:?}");
        assert!((30..=32).contains(&counts[2]), "{hz}: {counts:?}");
    }
}

#[test]
fn first_walking_frame_preserves_the_supplied_pose_phase() {
    let mut model = model(1.);
    model.clips[1].bones.push(crate::assets::BoneKeys {
        bone: 5,
        translation: vec![],
        scale: vec![],
        rotation: vec![
            (0., Quat::IDENTITY.to_array()),
            (1., Quat::from_rotation_x(1.).to_array()),
        ],
    });
    let authored = Authored::from_locomotion(&model, Definition::universal(0)).unwrap();
    let graph = Semantic::new(&authored, &model).unwrap();
    let mut controller = Controller::default();
    let instance = crate::presentation::Instance {
        mount: None,
        actor: Some(life(0)),
        model: "fixture".into(),
        transform: Mat4::IDENTITY,
        animation: State::Walk.into(),
        time: 0.25,
        animation_epoch: Some(1),
        emission: Vec3::ONE,
    };
    let (frame, diagnostic) = controller
        .update(&graph, &instance, 4., Aim::default(), Tier::Quarter, None)
        .unwrap();
    let expected = crate::animation::pose_selected(&model, State::Walk.into(), 0.25).unwrap();
    assert!(
        frame
            .matrices
            .iter()
            .zip(expected)
            .all(|(a, b)| a.abs_diff_eq(b, 0.00001))
    );
    assert_eq!(diagnostic.phase, 0.25);
    assert!(frame.markers.is_empty());
}

#[test]
fn forward_pause_and_simultaneous_state_and_owner_changes_do_not_replay_markers() {
    let mut model = model(1.);
    model.markers.push(crate::markers::ClipTrack {
        clip: 1,
        track: crate::markers::Track {
            duration: 1.,
            markers: vec![
                crate::markers::Marker {
                    id: crate::markers::FOOTSTEP_LEFT,
                    seconds: 0.25,
                },
                crate::markers::Marker {
                    id: crate::markers::FOOTSTEP_RIGHT,
                    seconds: 0.75,
                },
            ],
        },
    });
    let authored = Authored::from_locomotion(&model, Definition::universal(0)).unwrap();
    let graph = Semantic::new(&authored, &model).unwrap();
    let mut controller = Controller::default();
    let mut instance = crate::presentation::Instance {
        mount: None,
        actor: Some(life(0)),
        model: "fixture".into(),
        transform: Mat4::IDENTITY,
        animation: State::Walk.into(),
        time: 0.7,
        animation_epoch: Some(1),
        emission: Vec3::ONE,
    };
    let (_, before) = controller
        .update(&graph, &instance, 0., Aim::default(), Tier::Full, None)
        .unwrap();
    instance.time = 3.8;
    let (frame, after) = controller
        .update(&graph, &instance, 4., Aim::default(), Tier::Quarter, None)
        .unwrap();
    assert!(frame.markers.is_empty());
    assert!(after.source_epoch > before.source_epoch);
    assert!(after.selection_epoch > before.selection_epoch);
    let mut playback = crate::animation_graph::Playback::default();
    let input = Inputs {
        speed: 1.8,
        ..Default::default()
    };
    playback
        .update_quality(
            graph.admitted(),
            life(0),
            &graph.motion_values(State::Walk, input).unwrap(),
            0.1,
            0.,
            Some(1),
            true,
        )
        .unwrap();
    let changed = playback
        .update_quality(
            graph.admitted(),
            life(0),
            &graph.motion_values(State::Run, input).unwrap(),
            3.8,
            1. / 60.,
            Some(2),
            false,
        )
        .unwrap();
    assert!(changed.evaluated);
    assert!(changed.markers.is_empty());
}

#[test]
fn positive_pitch_raises_the_upper_body_aim() {
    let model = model(1.);
    let rig = Rig::admit(&model, &Definition::universal(0)).unwrap();
    let mut playback = Playback::default();
    playback
        .controls(
            life(0),
            Mat4::IDENTITY,
            0.,
            None,
            Aim {
                yaw: 0.,
                pitch: 0.3,
            },
        )
        .unwrap();
    let mut pose = crate::animation::pose_selected(&model, State::Idle.into(), 0.).unwrap();
    playback
        .adjust(
            &rig,
            life(0),
            Mat4::IDENTITY,
            State::Idle,
            0.,
            &mut pose,
            None,
        )
        .unwrap();
    let direction = rig
        .world(Mat4::IDENTITY, &pose, rig.spine)
        .transform_vector3(Vec3::X);
    assert!(direction.y > 0.29);
}
