use gym::robot::{self, Case, Partition, Suite};
use robot_lab::{Document, Reference, Store, document::*};
use std::collections::{BTreeMap, BTreeSet};
fn fixture() -> (
    tempfile::TempDir,
    Store,
    Robot,
    Scene,
    Experiment,
    Reference,
) {
    let root = tempfile::tempdir().unwrap();
    let lab = Store::open(root.path(), "robot-tests").unwrap();
    let source = lab
        .put_asset(&b"original fixture"[..], "application/xml", 1024)
        .unwrap();
    let license = lab.put_asset(&b"CC0-1.0"[..], "text/plain", 1024).unwrap();
    let robot = Robot {
        name: "Fixture arm".into(),
        source: Source {
            repository: "https://github.com/OpenAgentsInc/openagents".into(),
            revision: "a".repeat(40),
            files: BTreeMap::from([("arm.urdf".into(), source)]),
            license: "CC0-1.0".into(),
            notices: vec![license],
            converter: "robot-test-v1".into(),
        },
        description: "arm.urdf".into(),
        meters_per_unit: 1.,
        source_axes: Axes::ZUp,
        links: vec![
            Link {
                name: "base".into(),
                inertial: None,
                visuals: vec![],
                collisions: vec![],
            },
            Link {
                name: "tip".into(),
                inertial: Some(Inertial {
                    mass: 1.,
                    center: [0.; 3],
                    tensor: [1., 1., 1., 0., 0., 0.],
                }),
                visuals: vec![],
                collisions: vec![],
            },
        ],
        joints: vec![Joint {
            name: "shoulder".into(),
            parent: "base".into(),
            child: "tip".into(),
            kind: JointKind::Revolute,
            origin: Pose::default(),
            axis: [0., 0., 1.],
            limits: Some(Limits {
                lower: -1.,
                upper: 1.,
                effort: 3.,
                velocity: 2.,
            }),
        }],
        capabilities: BTreeSet::from([
            Capability::Inspect,
            Capability::Replay,
            Capability::Simulate,
        ]),
    };
    let robot_ref = lab
        .save(
            "arm",
            None,
            Document::Robot(robot.clone()),
            "Test",
            "Added arm",
            1,
        )
        .unwrap();
    let scene = Scene {
        name: "Arm and cube".into(),
        robots: vec![RobotInstance {
            id: "arm".into(),
            robot: robot_ref,
            pose: Pose::default(),
            joints: BTreeMap::from([("shoulder".into(), 0.)]),
        }],
        objects: vec![Object {
            id: "cube".into(),
            geometry: Geometry {
                pose: Pose {
                    position: [0.2, 0.5, 0.],
                    ..Default::default()
                },
                shape: Shape::Box { size: [0.05; 3] },
                rgba: [1., 0., 0., 1.],
            },
            mesh: None,
            collider: Some(Shape::Box { size: [0.05; 3] }),
            mass: 0.05,
            friction: 0.6,
        }],
        lights: vec![],
        cameras: vec![Camera {
            id: "front".into(),
            pose: Pose::default(),
            width: 32,
            height: 32,
            intrinsics: [32., 32., 16., 16.],
            robot_link: None,
        }],
    };
    let scene_ref = lab
        .save(
            "table",
            None,
            Document::Scene(scene.clone()),
            "Test",
            "Added cube",
            2,
        )
        .unwrap();
    let build = Build {
        name: "test-build".into(),
        sha256: "b".repeat(64),
    };
    let experiment = Experiment {
        scene: scene_ref,
        task: CubeTask {
            version: Build {
                name: "cube-task".into(),
                sha256: "c".repeat(64),
            },
            object: "cube".into(),
            target_min: [0.3, 0.45, -0.1],
            target_max: [0.5, 0.55, 0.1],
            stable_seconds: 0.2,
            max_linear_speed: 0.01,
            max_angular_speed: 0.01,
        },
        controller: Controller::Scripted {
            build: build.clone(),
            robot: "arm".into(),
            joints: vec!["shoulder".into()],
        },
        backend: build,
        timestep_seconds: 0.1,
        control_every_steps: 1,
        sensor_every_steps: 1,
        seeds: vec![11, 12],
        limits: ResourceLimits {
            max_steps: 6,
            wall_seconds: 10,
            max_artifact_bytes: 1 << 20,
            inference_timeout_ms: 1000,
        },
    };
    let reference = lab
        .save(
            "experiment",
            None,
            Document::Experiment(experiment.clone()),
            "Test",
            "Saved experiment",
            3,
        )
        .unwrap();
    (root, lab, robot, scene, experiment, reference)
}
fn episode(reference: Reference, seed: u64) -> Episode {
    Episode {
        experiment: reference,
        seed,
        mode: EpisodeMode::Simulation,
        steps: (0..6)
            .map(|index| Step {
                index,
                time_seconds: index as f64 * 0.1,
                joints: BTreeMap::from([("shoulder".into(), 0.)]),
                actions: if index == 0 {
                    BTreeMap::new()
                } else {
                    BTreeMap::from([("shoulder".into(), 0.)])
                },
                observation_step: if index == 0 { None } else { Some(index - 1) },
                objects: BTreeMap::from([(
                    "cube".into(),
                    ObjectState {
                        pose: Pose {
                            position: [if index < 2 { 0.2 } else { 0.4 }, 0.5, 0.],
                            ..Default::default()
                        },
                        linear_velocity: [0.; 3],
                        angular_velocity: [0.; 3],
                        held: false,
                    },
                )]),
                contacts: vec![],
                sensors: BTreeMap::new(),
                inference_ms: if index == 0 { None } else { Some(1.) },
                limit_violations: vec![],
            })
            .collect(),
        outcome: Outcome::Complete,
        wall_seconds: 0.7,
        events: vec![],
    }
}
fn suite(experiment: &Experiment) -> Suite {
    let mut suite = Suite {
        schema: robot::SUITE_SCHEMA.into(),
        name: "Fixture cube placement".into(),
        scene_template: experiment.scene.clone(),
        task: experiment.task.clone(),
        metric: robot::metric_build(),
        timestep_seconds: experiment.timestep_seconds,
        control_every_steps: 1,
        sensor_every_steps: 1,
        max_steps: 6,
        cases: BTreeMap::from([(
            Partition::Validation,
            experiment
                .seeds
                .iter()
                .map(|seed| Case {
                    seed: *seed,
                    scene: experiment.scene.clone(),
                    backend: experiment.backend.clone(),
                })
                .collect(),
        )]),
        exposure: BTreeMap::new(),
        digest: String::new(),
    };
    suite.digest = suite.compute_digest().unwrap();
    suite
}
#[test]
fn success_requires_the_whole_cube_release_and_a_continuous_stable_interval() {
    let (_root, _lab, robot, scene, experiment, reference) = fixture();
    let episode = episode(reference, 11);
    let scored = robot::score(&scene, &robot, &experiment, &episode).unwrap();
    assert!(scored.success);
    assert_eq!(scored.completion_seconds, Some(0.4));
    let mut bad = episode.clone();
    for step in &mut bad.steps {
        step.objects.get_mut("cube").unwrap().held = true;
    }
    assert!(
        !robot::score(&scene, &robot, &experiment, &bad)
            .unwrap()
            .success
    );
    for step in &mut bad.steps {
        let cube = step.objects.get_mut("cube").unwrap();
        cube.held = false;
        cube.pose.position[0] = 0.31;
    }
    assert!(
        !robot::score(&scene, &robot, &experiment, &bad)
            .unwrap()
            .success,
        "the center alone isn't enough"
    );
    let mut moving = episode.clone();
    moving.steps[3]
        .objects
        .get_mut("cube")
        .unwrap()
        .linear_velocity = [0.1, 0., 0.];
    assert!(
        !robot::score(&scene, &robot, &experiment, &moving)
            .unwrap()
            .success
    );
    let mut touching = episode.clone();
    for step in &mut touching.steps {
        step.contacts.push(Contact {
            a: "tip".into(),
            b: "cube".into(),
            position: [0.; 3],
            force: [1., 0., 0.],
        });
    }
    let scored = robot::score(&scene, &robot, &experiment, &touching).unwrap();
    assert!(!scored.success);
    assert_eq!(scored.collision_events, 1);
    assert_eq!(scored.robot_collision_events, 1);
    let mut demo = episode;
    demo.mode = EpisodeMode::Demonstration;
    assert!(robot::score(&scene, &robot, &experiment, &demo).is_err());
}
#[test]
fn results_recompute_and_missing_attempts_never_become_failures_or_complete_rates() {
    let (root, lab, _robot, _scene, experiment, reference) = fixture();
    let suite = suite(&experiment);
    let reference = lab
        .save(
            "attempt-one",
            None,
            Document::Episode(episode(reference, 11)),
            "Test",
            "Saved attempt",
            4,
        )
        .unwrap();
    let row = robot::row(&lab, &suite, Partition::Validation, &reference).unwrap();
    let path = root.path().join("results.jsonl");
    let store = gym::store::Store::at(&path);
    store.append(&row).unwrap();
    assert!(store.append(&row).is_err());
    let rows = robot::verify_rows(&lab, &suite, &path).unwrap();
    let summary = robot::summary(&suite, Partition::Validation, &row.controller, &rows).unwrap();
    assert_eq!(summary.expected, 2);
    assert_eq!(summary.recorded, 1);
    assert_eq!(summary.failures, 0);
    assert_eq!(summary.missing_seeds, vec![12]);
    assert_eq!(summary.success_rate, None);
    assert_eq!(summary.interval_95, None);
    let mut tampered = rows[0].clone();
    tampered.metrics.success = false;
    let other = root.path().join("tampered.jsonl");
    gym::store::Store::at(&other).append(&tampered).unwrap();
    assert!(
        robot::verify_rows(&lab, &suite, &other).is_err(),
        "a valid chain doesn't excuse invented metrics"
    );
}
#[test]
fn comparison_and_partitions_keep_exact_cases_and_report_small_sample_uncertainty() {
    let (_root, lab, _robot, _scene, experiment, reference) = fixture();
    let mut suite = suite(&experiment);
    let mut rows = vec![];
    for seed in [11, 12] {
        let reference = lab
            .save(
                &format!("attempt-{seed}"),
                None,
                Document::Episode(episode(reference.clone(), seed)),
                "Test",
                "Saved attempt",
                4,
            )
            .unwrap();
        rows.push(robot::row(&lab, &suite, Partition::Validation, &reference).unwrap());
    }
    let summary =
        robot::summary(&suite, Partition::Validation, &rows[0].controller, &rows).unwrap();
    assert_eq!(summary.success_rate, Some(1.));
    assert!(summary.interval_95.unwrap()[0] < 0.35);
    let mut other = rows.clone();
    for row in &mut other {
        row.controller.sha256 = "d".repeat(64);
        row.metrics.success = false;
        row.metrics.completion_seconds = None;
        row.metrics.failure = Some("Cube stayed outside".into());
    }
    let left = rows[0].controller.clone();
    let right = other[0].controller.clone();
    rows.extend(other);
    let comparison = robot::compare(&suite, Partition::Validation, &left, &right, &rows).unwrap();
    assert_eq!(comparison.paired_seeds, 2);
    assert_eq!(comparison.paired_difference, Some(-1.));
    assert_eq!(comparison.left_only_successes, 2);
    suite.cases.insert(
        Partition::HeldOut,
        suite.cases[&Partition::Validation].clone(),
    );
    suite.digest = suite.compute_digest().unwrap();
    assert!(
        suite.validate().is_err(),
        "training and held-out cases cannot overlap"
    );
}

#[test]
fn publication_is_explicit_bounded_and_contains_only_the_chosen_partition() {
    let (root, lab, _, _, experiment, experiment_ref) = fixture();
    let suite = suite(&experiment);
    let subject = lab
        .save(
            "publication-attempt",
            None,
            Document::Episode(episode(experiment_ref, 11)),
            "Test",
            "Saved attempt",
            5,
        )
        .unwrap();
    let results = root.path().join("results.jsonl");
    gym::store::Store::at(&results)
        .append(&robot::row(&lab, &suite, Partition::Validation, &subject).unwrap())
        .unwrap();
    let suite_path = root.path().join("suite.json");
    std::fs::write(&suite_path, serde_json::to_vec(&suite).unwrap()).unwrap();
    let publication = root.path().join("published.json");
    let args = vec![
        "publish".into(),
        "--lab-root".into(),
        lab.root().display().to_string(),
        "--project".into(),
        "robot-tests".into(),
        "--suite".into(),
        suite_path.display().to_string(),
        "--results".into(),
        results.display().to_string(),
        "--output".into(),
        publication.display().to_string(),
    ];
    let mut output = Vec::new();
    assert!(!publication.exists());
    gym::robot_cli::command(&args, &mut output).unwrap();
    let published: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&publication).unwrap()).unwrap();
    assert_eq!(published["partition"], "validation");
    assert_eq!(published["rows"].as_array().unwrap().len(), 1);
    assert!(
        !std::fs::read_to_string(&publication)
            .unwrap()
            .contains(&lab.root().display().to_string())
    );
    assert!(
        gym::robot_cli::command(&args, &mut output).is_err(),
        "publication cannot overwrite a file"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&publication)
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }
}
