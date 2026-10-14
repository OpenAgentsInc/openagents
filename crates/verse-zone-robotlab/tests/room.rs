use robot_lab::{Document, Reference, Store, document::*};
use std::{collections::BTreeMap, path::Path, sync::Arc};
use verse_core::zone::Intent;
use verse_zone_robotlab::{Room, inspection_scene, local::Config};
fn fixture() -> (tempfile::TempDir, Store, Reference, Reference) {
    let root = tempfile::tempdir().unwrap();
    let store = Store::open(root.path(), "room-tests").unwrap();
    let catalog = Path::new(env!("CARGO_MANIFEST_DIR")).join("../robot-description/assets/so101");
    let doc = robot_sim::import::pinned_urdf(&store, &catalog).unwrap();
    let robot_ref = store
        .save(
            "arm",
            None,
            Document::Robot(doc.clone()),
            "Test",
            "Added arm",
            1,
        )
        .unwrap();
    let (_, robot) = robot_description::pin::load_pinned(&catalog).unwrap();
    let mut scene = inspection_scene(&robot, &doc.source.files[&doc.description].sha256);
    scene.robots[0].robot = robot_ref;
    let scene_ref = store
        .save(
            "table",
            None,
            Document::Scene(scene.clone()),
            "Test",
            "Added table",
            2,
        )
        .unwrap();
    let build = Build {
        name: "replay-fixture".into(),
        sha256: "a".repeat(64),
    };
    let experiment = Experiment {
        scene: scene_ref.clone(),
        task: CubeTask {
            version: build.clone(),
            object: "cube".into(),
            target_min: [0.3, 0.75, -0.15],
            target_max: [0.4, 0.85, -0.05],
            stable_seconds: 0.2,
            max_linear_speed: 0.01,
            max_angular_speed: 0.01,
        },
        controller: Controller::Manual {
            build: build.clone(),
            robot: "so101".into(),
            joints: scene.robots[0].joints.keys().cloned().collect(),
        },
        backend: build,
        timestep_seconds: 0.1,
        control_every_steps: 1,
        sensor_every_steps: 1,
        seeds: vec![7],
        limits: ResourceLimits {
            max_steps: 4,
            wall_seconds: 30,
            max_artifact_bytes: 1 << 20,
            inference_timeout_ms: 1000,
        },
    };
    let experiment_ref = store
        .save(
            "replay-settings",
            None,
            Document::Experiment(experiment),
            "Test",
            "Saved replay settings",
            3,
        )
        .unwrap();
    let steps = (0..4)
        .map(|index| {
            let mut joints = scene.robots[0].joints.clone();
            joints.insert("shoulder_pan".into(), f64::from(index) * 0.1);
            Step {
                index,
                time_seconds: f64::from(index) * 0.1,
                joints,
                actions: BTreeMap::new(),
                observation_step: None,
                objects: BTreeMap::from([(
                    "cube".into(),
                    ObjectState {
                        pose: scene.objects[1].geometry.pose.clone(),
                        linear_velocity: [0.; 3],
                        angular_velocity: [0.; 3],
                        held: false,
                    },
                )]),
                contacts: vec![],
                sensors: BTreeMap::new(),
                inference_ms: None,
                limit_violations: vec![],
            }
        })
        .collect();
    let episode = Episode {
        experiment: experiment_ref,
        seed: 7,
        mode: EpisodeMode::Demonstration,
        steps,
        outcome: Outcome::Complete,
        wall_seconds: 0.3,
        events: vec![],
    };
    let episode_ref = store
        .save(
            "motion",
            None,
            Document::Episode(episode),
            "Test",
            "Added original replay fixture",
            4,
        )
        .unwrap();
    (root, store, scene_ref, episode_ref)
}
#[test]
fn known_replay_poses_pause_step_speed_and_hidden_lifecycle_use_shared_data() {
    let (root, store, _, episode) = fixture();
    let mut room = Room::local(&Config {
        root: root.path().into(),
        project: "room-tests".into(),
        subject: Some(episode.clone()),
        control_url: None,
        watch_job: None,
    })
    .unwrap();
    assert_eq!(room.snapshot().subject, Some(episode));
    assert_eq!(room.snapshot().mode, "Recorded demonstration");
    let initial = room.dynamic().instances[0].records.clone();
    room.intent(Intent::Step).unwrap();
    assert_eq!(room.snapshot().frame, 1);
    assert_eq!(
        room.snapshot()
            .joints
            .iter()
            .find(|j| j.name == "shoulder_pan")
            .unwrap()
            .position,
        0.1
    );
    room.intent(Intent::KnobNext).unwrap();
    room.intent(Intent::Increase).unwrap();
    assert_eq!(room.snapshot().speed, 2.);
    room.intent(Intent::Pause).unwrap();
    room.tick(0.05);
    assert_eq!(room.snapshot().frame, 2);
    room.set_active(false);
    room.tick(1.);
    assert_eq!(room.snapshot().frame, 2);
    room.set_active(true);
    room.intent(Intent::Reset).unwrap();
    assert_eq!(room.snapshot().frame, 0);
    assert!(
        room.dynamic().instances[0]
            .records
            .iter()
            .zip(initial.iter())
            .all(|(a, b)| a.transform == b.transform)
    );
    assert!(
        !store.root().join("jobs").exists(),
        "replay cannot start an experiment"
    );
    assert!(oa_copy::violations(&room.caption(), &[]).is_empty());
}
#[test]
fn pose_edits_append_versions_and_a_previous_chat_reference_stays_readable() {
    let (root, store, scene, _) = fixture();
    let mut room = Room::local(&Config {
        root: root.path().into(),
        project: "room-tests".into(),
        subject: Some(scene.clone()),
        control_url: None,
        watch_job: None,
    })
    .unwrap();
    for _ in 0..3 {
        room.intent(Intent::KnobNext).unwrap();
    }
    let selected = room.snapshot().joints[0].name.clone();
    let before = room.snapshot().joints[0].position;
    room.intent(Intent::Increase).unwrap();
    let started = std::time::Instant::now();
    while room.snapshot().loading && started.elapsed() < std::time::Duration::from_secs(5) {
        room.tick(0.01);
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(
        room.snapshot().error.is_none(),
        "{:?}",
        room.snapshot().error
    );
    assert_ne!(room.snapshot().subject, Some(scene.clone()));
    assert!((room.snapshot().joints[0].position - before - 0.05).abs() < 1e-9);
    assert_eq!(
        store.view(&scene).unwrap().scene.robots[0].joints[&selected],
        before
    );
}
#[test]
fn the_default_room_has_checked_arms_blocks_cups_and_no_execution_control() {
    let room = Room::catalog().unwrap();
    let world = room.world_scene().unwrap();
    assert!(!world.mesh.lit.is_empty());
    assert!(world.mesh.faces.len() > 400);
    assert_eq!(world.blockers.len(), 2);
    assert!(
        !room
            .controls()
            .iter()
            .any(|(_, i)| matches!(i, Intent::Grab | Intent::Release))
    );
    let robot = Path::new(env!("CARGO_MANIFEST_DIR")).join("../robot-description/assets/so101");
    let (_, robot) = robot_description::pin::load_pinned(&robot).unwrap();
    assert!(Room::new(Arc::new(robot), room_scene()).is_ok());
}
fn room_scene() -> Scene {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../robot-description/assets/so101");
    let (pin, robot) = robot_description::pin::load_pinned(&root).unwrap();
    let sha = &pin
        .files
        .iter()
        .find(|p| p.path == pin.description)
        .unwrap()
        .sha256;
    inspection_scene(&robot, sha)
}
