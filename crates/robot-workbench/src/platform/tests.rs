use super::*;
use robot_lab::document::*;
use robot_lab::migrate;
use std::collections::BTreeSet;

fn local(dir: &std::path::Path) -> (Store, Reference) {
    let store = Store::open(dir, "openagents").unwrap();
    let source = store
        .put_asset(&b"<robot name=\"test-arm\"/>"[..], "application/xml", 1_024)
        .unwrap();
    let license = store
        .put_asset(&b"Original test fixture; CC0-1.0"[..], "text/plain", 1_024)
        .unwrap();
    let robot = Robot {
        name: "Test arm".into(),
        source: Source {
            repository: "https://github.com/OpenAgentsInc/openagents".into(),
            revision: "a".repeat(40),
            files: BTreeMap::from([("arm.urdf".into(), source)]),
            license: "CC0-1.0".into(),
            notices: vec![license],
            converter: "robot-lab-test-v1".into(),
        },
        description: "arm.urdf".into(),
        meters_per_unit: 1.0,
        source_axes: Axes::ZUp,
        links: vec![
            Link {
                name: "base".into(),
                inertial: None,
                visuals: vec![],
                collisions: vec![],
            },
            Link {
                name: "forearm".into(),
                inertial: Some(Inertial {
                    mass: 1.0,
                    center: [0.0; 3],
                    tensor: [1.0, 1.0, 1.0, 0.0, 0.0, 0.0],
                }),
                visuals: vec![],
                collisions: vec![],
            },
        ],
        joints: vec![Joint {
            name: "shoulder".into(),
            parent: "base".into(),
            child: "forearm".into(),
            kind: JointKind::Revolute,
            origin: Pose::default(),
            axis: [0.0, 0.0, 1.0],
            limits: Some(Limits {
                lower: -1.0,
                upper: 1.0,
                effort: 3.0,
                velocity: 2.0,
            }),
        }],
        capabilities: BTreeSet::from([Capability::Inspect, Capability::Simulate]),
    };
    let robot = store
        .save(
            "test-arm",
            None,
            Document::Robot(robot),
            "Test",
            "Added arm",
            1,
        )
        .unwrap();
    let scene = Scene {
        name: "Arm and cube".into(),
        robots: vec![RobotInstance {
            id: "arm".into(),
            robot,
            pose: Pose::default(),
            joints: BTreeMap::from([("shoulder".into(), 0.0)]),
        }],
        objects: vec![Object {
            id: "cube".into(),
            geometry: Geometry {
                pose: Pose {
                    position: [0.2, 0.5, 0.0],
                    ..Pose::default()
                },
                shape: Shape::Box { size: [0.05; 3] },
                rgba: [1.0, 0.0, 0.0, 1.0],
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
            width: 640,
            height: 480,
            intrinsics: [500.0, 500.0, 320.0, 240.0],
            robot_link: None,
        }],
    };
    let scene = store
        .save(
            "bench",
            None,
            Document::Scene(scene),
            "Test",
            "Added bench",
            2,
        )
        .unwrap();
    let build = Build {
        name: "test-v1".into(),
        sha256: "a".repeat(64),
    };
    let experiment = Experiment {
        scene: scene.clone(),
        task: CubeTask {
            version: build.clone(),
            object: "cube".into(),
            target_min: [0.0; 3],
            target_max: [1.0; 3],
            stable_seconds: 0.1,
            max_linear_speed: 0.01,
            max_angular_speed: 0.01,
        },
        controller: Controller::Scripted {
            build: build.clone(),
            robot: "arm".into(),
            joints: vec!["shoulder".into()],
        },
        backend: build,
        timestep_seconds: 0.01,
        control_every_steps: 2,
        sensor_every_steps: 5,
        seeds: vec![7],
        limits: ResourceLimits {
            max_steps: 100,
            wall_seconds: 10,
            max_artifact_bytes: 1_024,
            inference_timeout_ms: 100,
        },
    };
    let experiment = store
        .save(
            "experiment",
            None,
            Document::Experiment(experiment),
            "Test",
            "Added experiment",
            3,
        )
        .unwrap();
    (store, experiment)
}

/// The worker's input for the experiment: platform versions, experiment
/// first, and every file the local project holds, by checksum.
fn input(store: &Store) -> Value {
    let plan = migrate::plan(store).unwrap();
    let mut versions: Vec<Value> = plan
        .iter()
        .map(|d| {
            let v = d.versions.last().unwrap();
            json!({"document": d.id, "number": 1, "content": v.content,
                "content_sha256": lab_kit::digest(&v.content)})
        })
        .collect();
    versions.sort_by_key(|v| v["content"]["schema"] != "robot-experiment.v1");
    let mut files = BTreeMap::new();
    for d in &plan {
        for v in &d.versions {
            for f in &v.files {
                files.insert(
                    f.sha256.clone(),
                    store.root().join("assets").join(&f.sha256),
                );
            }
        }
    }
    json!({"work": {"assignment": {"lease": {"job": {"id": "job-1", "spec": {"params": {}}}}}},
        "versions": versions, "files": files, "local_assets": {}})
}

#[test]
fn a_job_saves_its_referenced_versions_and_files_locally_and_replays_by_platform_digest() {
    let source = tempfile::tempdir().unwrap();
    let (store, _) = local(source.path());
    let input = input(&store);
    let target = tempfile::tempdir().unwrap();
    let saved = materialize(&input, &target.path().join("robot")).unwrap();
    assert_eq!(saved.seed, 7);
    assert_eq!(saved.platform.document, "experiment");
    assert_eq!(
        saved.platform.version,
        input["versions"][0]["content_sha256"].as_str().unwrap()
    );
    let Document::Experiment(experiment) = saved.store.get(&saved.experiment).unwrap().content
    else {
        panic!("experiment")
    };
    let Document::Scene(scene) = saved.store.get(&experiment.scene).unwrap().content else {
        panic!("scene")
    };
    assert!(matches!(
        saved.store.get(&scene.robots[0].robot).unwrap().content,
        Document::Robot(_)
    ));

    // The replay names the platform experiment, not the local copy.
    let episode = Episode {
        experiment: saved.experiment.clone(),
        seed: 7,
        mode: EpisodeMode::Simulation,
        steps: vec![Step {
            index: 0,
            time_seconds: 0.0,
            joints: BTreeMap::from([("shoulder".into(), 0.0)]),
            actions: BTreeMap::new(),
            observation_step: None,
            objects: BTreeMap::new(),
            contacts: vec![],
            sensors: BTreeMap::new(),
            inference_ms: None,
            limit_violations: vec![],
        }],
        outcome: Outcome::Stopped {
            reason: "Stopped by you".into(),
        },
        wall_seconds: 0.01,
        events: Vec::new(),
    };
    let episode = saved
        .store
        .save("run", None, Document::Episode(episode), "Test", "Ran", 4)
        .unwrap();
    let replay = replay(&saved.store, &episode, &saved.platform).unwrap();
    assert_eq!(replay["schema"], "robot-episode.v1");
    assert_eq!(replay["content"]["experiment"]["document"], "experiment");
    assert_eq!(
        replay["content"]["experiment"]["version"],
        input["versions"][0]["content_sha256"]
    );
    let out = output(&replay).unwrap();
    assert_eq!(out["outputs"][0]["role"], "replay");
    assert_eq!(out["outputs"][0]["media_type"], REPLAY_MEDIA);
}

#[test]
fn a_changed_file_or_an_unreadable_reference_refuses() {
    let source = tempfile::tempdir().unwrap();
    let (store, _) = local(source.path());
    let mut changed = input(&store);
    let (sha, _) = changed["files"]
        .as_object()
        .unwrap()
        .iter()
        .next()
        .map(|(k, v)| (k.clone(), v.clone()))
        .unwrap();
    let other = source.path().join("other");
    std::fs::write(&other, b"not the robot").unwrap();
    changed["files"][&sha] = json!(other);
    let target = tempfile::tempdir().unwrap();
    assert!(materialize(&changed, &target.path().join("a")).is_err());

    let mut missing = input(&store);
    missing["versions"]
        .as_array_mut()
        .unwrap()
        .retain(|v| v["content"]["schema"] != "robot-scene.v1");
    assert_eq!(
        materialize(&missing, &target.path().join("b"))
            .err()
            .unwrap(),
        "This item refers to a version the job can't read."
    );
}
