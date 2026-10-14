use robot_lab::document::*;
use robot_lab::{Document, Store};
use std::collections::{BTreeMap, BTreeSet};
use tempfile::TempDir;
pub fn fixture() -> (TempDir, Store, Reference, Reference, Reference) {
    let directory = tempfile::tempdir().unwrap();
    let store = Store::open(directory.path(), "openagents").unwrap();
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
            name: "hinge".into(),
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
        capabilities: BTreeSet::from([
            Capability::Inspect,
            Capability::Replay,
            Capability::Simulate,
        ]),
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
            robot: robot.clone(),
            pose: Pose::default(),
            joints: BTreeMap::from([("hinge".into(), 0.0)]),
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
            joints: vec!["hinge".into()],
        },
        backend: build,
        timestep_seconds: 0.01,
        control_every_steps: 2,
        sensor_every_steps: 5,
        seeds: vec![7, 8, 9],
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
    (directory, store, robot, scene, experiment)
}
