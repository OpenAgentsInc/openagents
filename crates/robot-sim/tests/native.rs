#![cfg(feature = "mujoco")]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use robot_sim::mujoco::{LibraryPin, Runtime, Servo};
use robot_sim::runner::Backend;

fn library() -> LibraryPin {
    let path = PathBuf::from(
        std::env::var_os("ROBOT_LAB_MUJOCO_LIBRARY")
            .expect("Set ROBOT_LAB_MUJOCO_LIBRARY to the qualified MuJoCo 3.3.7 library"),
    );
    let bytes = std::fs::read(&path).unwrap();
    LibraryPin {
        path,
        sha256: robot_lab::store::digest(&bytes),
    }
}

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

#[test]
#[ignore = "requires the configured MuJoCo 3.3.7 library"]
fn owned_model_advances_contacts_limits_reset_and_y_up_poses() {
    let scratch = tempfile::tempdir().unwrap();
    let runtime = Runtime::open(&library(), scratch.path()).unwrap();
    let mut model = runtime
        .load(&fixtures(), "contact-arm.xml", 1.0 / 240.0)
        .unwrap();
    assert_eq!(model.dimensions(), (8, 7, 4));
    model
        .control_joint(
            "hinge",
            Servo {
                kp: 4.0,
                kd: 0.4,
                effort: 2.4,
                speed: 2.0,
            },
        )
        .unwrap();
    model.observe_object("cube", "cube-body").unwrap();
    let original = model.reset(7).unwrap();
    assert!((original.objects["cube"].pose.position[1] - 0.8).abs() < 1e-9);
    assert!(
        model
            .validate_action(&BTreeMap::from([("hinge".into(), 0.6)]))
            .is_err()
    );
    model
        .apply(&BTreeMap::from([("hinge".into(), 0.3)]))
        .unwrap();
    let mut had_contact = false;
    for _ in 0..240 {
        let state = model.step().unwrap();
        had_contact |= !state.contacts.is_empty();
    }
    let state = model.snapshot().unwrap();
    assert!(had_contact);
    assert!((state.objects["cube"].pose.position[1] - 0.775).abs() < 0.005);
    assert!((state.joints["hinge"] - 0.3).abs() < 0.05);
    assert!((model.time() - 1.0).abs() < 1e-9);
    let first_export = model.export_mjcf().unwrap();
    let _other = runtime.load(&fixtures(), "contact-arm.xml", 0.01).unwrap();
    assert_eq!(
        model.export_mjcf().unwrap(),
        first_export,
        "another load cannot relabel a conversion"
    );
    let restored = model.reset(7).unwrap();
    assert_eq!(restored.joints, original.joints);
    assert_eq!(restored.objects, original.objects);
    drop(model);
    drop(_other);
    assert_eq!(std::fs::read_dir(scratch.path()).unwrap().count(), 0);
}

#[test]
#[ignore = "requires the configured MuJoCo 3.3.7 library"]
fn pinned_so101_urdf_compiles_in_the_native_backend() {
    let scratch = tempfile::tempdir().unwrap();
    let runtime = Runtime::open(&library(), scratch.path()).unwrap();
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../robot-description/assets/so101");
    let (pin, robot) = robot_description::pin::load_pinned(&source).unwrap();
    let mut model = runtime
        .load(&source, &pin.description, 1.0 / 240.0)
        .unwrap();
    assert_eq!(model.dimensions().0, 6);
    assert_eq!(robot.movable().count(), 6);
    assert_eq!(model.reset(9).unwrap().joints.len(), 0);
    for _ in 0..24 {
        model.step().unwrap();
    }
    assert!(
        model
            .snapshot()
            .unwrap()
            .joints
            .values()
            .all(|n| n.is_finite())
    );
}

#[test]
#[ignore = "requires the configured MuJoCo 3.3.7 library"]
fn unsafe_dependencies_plugins_and_size_requests_are_refused() {
    let scratch = tempfile::tempdir().unwrap();
    let runtime = Runtime::open(&library(), scratch.path()).unwrap();
    let root = tempfile::tempdir().unwrap();
    for xml in [
        "<mujoco><include file=\"../outside.xml\"/></mujoco>",
        "<mujoco><asset><mesh file=\"https://example.com/mesh.stl\"/></asset></mujoco>",
        "<mujoco><extension><plugin plugin=\"anything\"/></extension></mujoco>",
        "<mujoco><size memory=\"1G\"/></mujoco>",
    ] {
        std::fs::write(root.path().join("model.xml"), xml).unwrap();
        assert!(runtime.load(root.path(), "model.xml", 0.01).is_err());
    }
    let mut pin = library();
    pin.sha256 = "0".repeat(64);
    assert!(Runtime::open(&pin, scratch.path()).is_err());
}

#[test]
#[ignore = "requires MuJoCo 3.3.7 and the pinned Franka scratch fixture"]
fn licensed_franka_fixture_imports_and_uses_an_explicit_initial_pose() {
    let root = PathBuf::from(
        std::env::var_os("ROBOT_LAB_FRANKA_ROOT").expect("Set ROBOT_LAB_FRANKA_ROOT"),
    );
    let pin: serde_json::Value =
        serde_json::from_slice(include_bytes!("fixtures/franka/source.json")).unwrap();
    for (name, expected) in pin["files"].as_object().unwrap() {
        let data = std::fs::read(root.join(name)).unwrap();
        assert_eq!(data.len() as u64, expected["bytes"].as_u64().unwrap());
        assert_eq!(
            robot_lab::store::digest(&data),
            expected["sha256"].as_str().unwrap()
        );
    }
    let scratch = tempfile::tempdir().unwrap();
    let runtime = Runtime::open(&library(), scratch.path()).unwrap();
    let mut model = runtime
        .load(&root, "panda_nohand.xml", 1.0 / 240.0)
        .unwrap();
    assert_eq!(model.dimensions().0, 7);
    let home = [0.0, 0.0, 0.0, -1.57079, 0.0, 1.57079, -0.7853];
    let mut targets = BTreeMap::new();
    for (index, value) in home.iter().enumerate() {
        let name = format!("joint{}", index + 1);
        model
            .control_joint(
                &name,
                Servo {
                    kp: 400.0,
                    kd: 40.0,
                    effort: if index < 4 { 87.0 } else { 12.0 },
                    speed: 2.0,
                },
            )
            .unwrap();
        targets.insert(name, *value);
    }
    model.initial_pose(&targets).unwrap();
    for _ in 0..240 {
        model.step().unwrap();
    }
    let state = model.snapshot().unwrap();
    assert!(state.limit_violations.is_empty());
    assert!(state.joints.values().all(|v| v.is_finite()));
    assert_eq!(model.reset(7).unwrap().joints, targets);
}

#[test]
#[ignore = "requires the configured MuJoCo 3.3.7 library"]
fn so101_table_scene_shares_contacts_and_preserves_the_imported_link_frames() {
    use glam::{DMat4, DQuat, DVec3};
    use robot_lab::document::{
        Camera, Geometry, Object, Pose, Reference, RobotInstance, Scene, Shape,
    };
    let scratch = tempfile::tempdir().unwrap();
    let runtime = Runtime::open(&library(), scratch.path()).unwrap();
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../robot-description/assets/so101");
    let (pin, robot) = robot_description::pin::load_pinned(&source).unwrap();
    let object = |id: &str, position, size, mass| Object {
        id: id.into(),
        geometry: Geometry {
            pose: Pose {
                position,
                ..Pose::default()
            },
            shape: Shape::Box { size },
            rgba: [0.8, 0.1, 0.1, 1.0],
        },
        mesh: None,
        collider: Some(Shape::Box { size }),
        mass,
        friction: 0.7,
    };
    let scene = Scene {
        name: "SO-101 table".into(),
        robots: vec![RobotInstance {
            id: "arm".into(),
            robot: Reference {
                document: "so101".into(),
                version: "a".repeat(64),
            },
            pose: Pose {
                position: [0.0, 0.75, 0.0],
                ..Pose::default()
            },
            joints: robot.movable().map(|j| (j.name.clone(), 0.0)).collect(),
        }],
        objects: vec![
            object("table", [0.0, 0.73, 0.0], [1.2, 0.04, 0.75], 0.0),
            object("cube", [0.22, 0.80, -0.1], [0.05; 3], 0.05),
        ],
        lights: vec![],
        cameras: vec![Camera {
            id: "front".into(),
            pose: Pose::default(),
            width: 64,
            height: 64,
            intrinsics: [50.0, 50.0, 32.0, 32.0],
            robot_link: None,
        }],
    };
    let mut model = runtime
        .arm_scene(
            &source,
            &pin.description,
            &scene,
            1.0 / 240.0,
            Servo {
                kp: 100.0,
                kd: 4.0,
                effort: 2.4,
                speed: 2.0,
            },
        )
        .unwrap();
    assert_eq!(model.dimensions().0, 13);
    let another = runtime
        .arm_scene(
            &source,
            &pin.description,
            &scene,
            1.0 / 240.0,
            Servo {
                kp: 100.0,
                kd: 4.0,
                effort: 2.4,
                speed: 2.0,
            },
        )
        .unwrap();
    assert_eq!(
        model.build(),
        another.build(),
        "private snapshot paths cannot change the backend identity"
    );
    drop(another);
    let frames = robot.forward(&robot.home()).unwrap();
    let base =
        DMat4::from_translation(DVec3::new(0.0, 0.75, 0.0)) * robot_description::z_up_to_y_up();
    for (link, frame) in robot.links().iter().zip(frames) {
        let expected = (base * frame).transform_point3(DVec3::ZERO);
        let actual = DVec3::from_array(model.body_pose(&link.name).unwrap().position);
        assert!(
            actual.distance(expected) < 1e-4,
            "{}: {actual} versus {expected}",
            link.name
        );
    }
    let mut touched_table = false;
    for _ in 0..240 {
        let snapshot = model.step().unwrap();
        touched_table |= snapshot.contacts.iter().any(|contact| {
            (contact.a == "cube" && contact.b == "table")
                || (contact.b == "cube" && contact.a == "table")
        });
        assert!(
            snapshot
                .joints
                .values()
                .all(|position| position.is_finite() && position.abs() < 0.1),
            "joint state {:?}; contacts {:?}",
            snapshot.joints,
            snapshot.contacts
        );
    }
    let snapshot = model.snapshot().unwrap();
    assert!(touched_table);
    assert!((snapshot.objects["cube"].pose.position[1] - 0.775).abs() < 0.005);
    let rotation = snapshot.objects["cube"].pose.rotation;
    assert!(
        DQuat::from_xyzw(rotation[0], rotation[1], rotation[2], rotation[3])
            .angle_between(DQuat::IDENTITY)
            < 0.01
    );
}

#[test]
#[ignore = "requires the configured MuJoCo 3.3.7 library"]
fn so101_jaw_pads_pick_up_the_table_cube_with_key_presses() {
    use robot_lab::document::{
        Camera, Geometry, Object, Pose, Reference, RobotInstance, Scene, Shape,
    };
    let scratch = tempfile::tempdir().unwrap();
    let runtime = Runtime::open(&library(), scratch.path()).unwrap();
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../robot-description/assets/so101");
    let (pin, robot) = robot_description::pin::load_pinned(&source).unwrap();
    let object = |id: &str, position, size, mass| Object {
        id: id.into(),
        geometry: Geometry {
            pose: Pose {
                position,
                ..Pose::default()
            },
            shape: Shape::Box { size },
            rgba: [0.8, 0.1, 0.1, 1.0],
        },
        mesh: None,
        collider: Some(Shape::Box { size }),
        mass,
        friction: 0.7,
    };
    // The Robot Lab import scene: the arm on a table with a 5 cm cube.
    let scene = Scene {
        name: "SO-101 table".into(),
        robots: vec![RobotInstance {
            id: "arm".into(),
            robot: Reference {
                document: "so101".into(),
                version: "a".repeat(64),
            },
            pose: Pose {
                position: [0.0, 0.75, 0.0],
                ..Pose::default()
            },
            joints: robot.movable().map(|j| (j.name.clone(), 0.0)).collect(),
        }],
        objects: vec![
            object("table", [0.0, 0.73, 0.0], [1.2, 0.04, 0.75], 0.0),
            object("cube", [0.22, 0.775, -0.1], [0.05; 3], 0.05),
        ],
        lights: vec![],
        cameras: vec![Camera {
            id: "front".into(),
            pose: Pose::default(),
            width: 64,
            height: 64,
            intrinsics: [50.0, 50.0, 32.0, 32.0],
            robot_link: None,
        }],
    };
    let mut model = runtime
        .arm_scene(
            &source,
            &pin.description,
            &scene,
            1.0 / 240.0,
            Servo {
                kp: 100.0,
                kd: 4.0,
                effort: 2.4,
                speed: 2.0,
            },
        )
        .unwrap();
    let mut target: BTreeMap<String, f64> =
        robot.movable().map(|j| (j.name.clone(), 0.0)).collect();
    let height = |model: &robot_sim::mujoco::Model| {
        model.snapshot().unwrap().objects["cube"].pose.position[1] - 0.775
    };
    // The player's keys: each press moves one joint target 0.05 rad, about
    // ten presses a second; a pause holds the targets. This is the grasp in
    // robot-workbench's README.
    for (joint, presses, pause) in [
        ("shoulder_lift", -3, 0.0),
        ("gripper", 20, 0.0),
        ("wrist_roll", 23, 0.0),
        ("wrist_flex", 27, 0.0),
        ("elbow_flex", 2, 0.0),
        ("shoulder_pan", -8, 0.5),
        ("shoulder_lift", 8, 0.5),
        ("gripper", -24, 2.5),
        ("shoulder_lift", -8, 0.0),
    ] {
        let mut hold = |target: &BTreeMap<String, f64>, seconds: f64| {
            for _ in 0..(seconds * 240.0_f64).round() as usize {
                model.apply(target).unwrap();
                model.step().unwrap();
            }
        };
        for _ in 0..i32::abs(presses) {
            let limit = robot
                .movable()
                .find(|j| j.name == joint)
                .and_then(|j| j.limit)
                .unwrap();
            let value = target[joint] + 0.05 * f64::from(presses.signum());
            target.insert(joint.into(), value.clamp(limit.lower, limit.upper));
            hold(&target, 0.1);
        }
        hold(&target, pause);
    }
    let lifted = height(&model);
    assert!(lifted >= 0.05, "the cube rose {lifted} m");
    for _ in 0..480 {
        model.apply(&target).unwrap();
        let snapshot = model.step().unwrap();
        assert!(snapshot.objects["cube"].held, "the cube slipped out");
    }
    let held = height(&model);
    assert!(held >= 0.05, "after two seconds the cube is {held} m up");
}
