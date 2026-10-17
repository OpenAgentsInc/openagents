//! Robot Lab on the Labs platform (#11882): the kind's checks match the
//! local store's, and a local project moves onto a platform store with its
//! versions, notes, and files.

use std::collections::{BTreeMap, BTreeSet};

use lab_kit::{Change, LabKind, Locator, View, Want};
use robot_lab::document::*;
use robot_lab::kind::{Robot as Kind, SIMULATE, dependencies_checked, note_locator};
use robot_lab::store::Note;
use robot_lab::{Document, Operation, Store, migrate};
use serde_json::{Value, json};

struct Fixture {
    _dir: tempfile::TempDir,
    store: Store,
    scene: Reference,
}

fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path(), "openagents").unwrap();
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
    let episode = Episode {
        experiment,
        seed: 7,
        mode: EpisodeMode::Simulation,
        steps: (0..3)
            .map(|index| Step {
                index,
                time_seconds: f64::from(index) * 0.01,
                joints: BTreeMap::from([("shoulder".into(), 0.1 * f64::from(index))]),
                actions: BTreeMap::new(),
                observation_step: None,
                objects: BTreeMap::new(),
                contacts: vec![],
                sensors: BTreeMap::new(),
                inference_ms: None,
                limit_violations: vec![],
            })
            .collect(),
        outcome: Outcome::Stopped {
            reason: "Stopped by you".into(),
        },
        wall_seconds: 0.05,
        events: Vec::new(),
    };
    let episode = store
        .save(
            "run-7",
            None,
            Document::Episode(episode),
            "Test",
            "Ran seed 7",
            4,
        )
        .unwrap();
    store
        .add_note(Note {
            subject: episode,
            episode_step: Some(1),
            author: "Test".into(),
            text: "The cube slips here.".into(),
            created_unix_ms: 5,
        })
        .unwrap();
    Fixture {
        _dir: dir,
        store,
        scene,
    }
}

fn version(document: &str, number: u64, content: Value) -> lab_kit::Version {
    lab_kit::Version {
        document: document.into(),
        number,
        parent: number.checked_sub(1).filter(|p| *p > 0),
        author: "Test".into(),
        summary: String::new(),
        created_unix_ms: 0,
        schema: content["schema"].as_str().unwrap().into(),
        content_sha256: lab_kit::digest(&content),
        report: Kind::check(&content, &dependencies_checked(&content)).unwrap(),
        content,
        assets: vec![],
    }
}

#[test]
fn edits_and_refusals_match_the_local_store() {
    let f = fixture();
    let first = migrate::plan(&f.store).unwrap();
    let bench = first.iter().find(|d| d.id == "bench").unwrap();
    let base = version("bench", 1, bench.versions[0].content.clone());

    // The same pose through the local store and through the kind.
    let joints = BTreeMap::from([("shoulder".to_owned(), 0.5)]);
    f.store
        .execute(
            Operation::Pose {
                base: f.scene.clone(),
                robot: "arm".into(),
                joints: joints.clone(),
                author: "Test".into(),
                summary: "Raised the shoulder".into(),
            },
            6,
        )
        .unwrap();
    let pose = Change::Intent {
        intent: json!({"intent": "pose", "robot": "arm", "joints": joints}),
    };
    let unchecked = Kind::apply(&base, &pose, "Raised the shoulder", &Value::Null);
    assert_eq!(unchecked.unwrap_err().code, "dependencies_required");
    let proposed = {
        let Document::Scene(mut scene) = robot_lab::kind::document(&base.content).unwrap() else {
            panic!("a scene");
        };
        robot_lab::kind::pose(&mut scene, "arm", joints.clone()).unwrap();
        serde_json::to_value(Document::Scene(scene)).unwrap()
    };
    let draft = Kind::apply(
        &base,
        &pose,
        "Raised the shoulder",
        &dependencies_checked(&proposed),
    )
    .unwrap();
    let after = migrate::plan(&f.store).unwrap();
    let bench = after.iter().find(|d| d.id == "bench").unwrap();
    assert_eq!(bench.versions.len(), 2);
    assert_eq!(draft.content, bench.versions[1].content);
    assert_eq!(draft.report.facts["item"], "scene");

    // The same refusal sentences.
    let missing = f
        .store
        .execute(
            Operation::Pose {
                base: f.store.latest("bench").unwrap(),
                robot: "nobody".into(),
                joints: BTreeMap::new(),
                author: "Test".into(),
                summary: "x".into(),
            },
            7,
        )
        .unwrap_err()
        .to_string();
    let refused = Kind::apply(
        &base,
        &Change::Intent {
            intent: json!({"intent": "pose", "robot": "nobody", "joints": {}}),
        },
        "x",
        &Value::Null,
    )
    .unwrap_err();
    assert_eq!(refused.message, missing);
    let moved = Kind::apply(
        &base,
        &Change::Intent {
            intent: json!({"intent": "move_object", "object": "ghost", "position": [0, 0, 0]}),
        },
        "x",
        &Value::Null,
    )
    .unwrap_err();
    assert_eq!(moved.message, "Choose an object in this scene.");
    let robot = first.iter().find(|d| d.id == "test-arm").unwrap();
    let swapped = Kind::apply(
        &base,
        &Change::Intent {
            intent: json!({"intent": "replace", "content": robot.versions[0].content}),
        },
        "x",
        &Value::Null,
    )
    .unwrap_err();
    assert_eq!(swapped.message, "Keep the same item type when editing.");
    assert_eq!(
        Kind::check(&json!({"schema": "robot.v9"}), &Value::Null)
            .unwrap_err()
            .code,
        "document_format"
    );
}

#[test]
fn the_plan_orders_documents_and_rewrites_references() {
    let f = fixture();
    let plan = migrate::plan(&f.store).unwrap();
    let ids: Vec<_> = plan.iter().map(|d| d.id.as_str()).collect();
    assert_eq!(ids, ["test-arm", "bench", "experiment", "run-7"]);
    let arm = &plan[0].versions[0];
    assert_eq!(plan[0].title, "Test arm");
    assert_eq!(arm.files.len(), 2);
    assert_eq!(
        plan[1].versions[0].content["content"]["robots"][0]["robot"]["version"],
        lab_kit::digest(&arm.content)
    );
    assert_eq!(
        plan[2].versions[0].content["content"]["scene"]["version"],
        lab_kit::digest(&plan[1].versions[0].content)
    );
    assert_eq!(plan[3].notes.len(), 1);
    assert_eq!(plan[3].notes[0].locator, Locator::Step { step: 1 });

    let episode = &plan[3].versions[0].content;
    assert_eq!(
        note_locator(episode, Some(2)).unwrap(),
        Locator::Step { step: 2 }
    );
    assert_eq!(note_locator(episode, None).unwrap(), Locator::None);
    assert_eq!(
        note_locator(episode, Some(3)).unwrap_err().message,
        "Choose a step from this saved episode."
    );
    assert!(note_locator(&arm.content, Some(0)).is_err());

    let run = version("run-7", 1, episode.clone());
    assert_eq!(Kind::card(&run, &[], &[]).view, View::Report);
    assert_eq!(Kind::jobs_for(&run, &Want::Card).len(), 1);
    let experiment = version("experiment", 1, plan[2].versions[0].content.clone());
    assert!(Kind::jobs_for(&experiment, &Want::Card).is_empty());
    let simulate = Kind::jobs_for(
        &experiment,
        &Want::Job {
            job_type: SIMULATE.into(),
            params: Value::Null,
        },
    );
    assert_eq!(simulate[0].requirements.tools, ["robot-sim"]);
    assert!(
        Kind::manifest()
            .tools
            .iter()
            .all(|t| t.name.starts_with("robot_") && !t.name.contains("control"))
    );
}

#[cfg(feature = "platform")]
#[tokio::test(flavor = "current_thread")]
async fn a_local_project_moves_onto_a_platform_store() {
    use lab_store::LabStore;
    let f = fixture();
    let plan = migrate::plan(&f.store).unwrap();
    let root = tempfile::tempdir().unwrap();
    let backend = std::sync::Arc::new(lab_store::backend::FsBackend::open(root.path()).unwrap());
    let target = lab_store::Store::new(
        backend,
        &lab_kit::SpaceRef::Personal {
            account: "test".into(),
        },
    )
    .unwrap();
    let saved = migrate::import(
        &plan,
        &f.store,
        &target,
        lab_kit::AssetClass::NoTraining,
        1 << 20,
    )
    .await
    .unwrap();
    for (document, versions) in plan.iter().zip(&saved) {
        assert_eq!(versions.len(), document.versions.len());
        for (local, platform) in document.versions.iter().zip(versions) {
            assert_eq!(platform.content, local.content);
            assert_eq!(platform.content_sha256, lab_kit::digest(&local.content));
            assert_eq!(platform.summary, local.summary);
            let read = target.version(&document.id, platform.number).await.unwrap();
            assert_eq!(read.content, local.content);
        }
    }
    let files = target
        .version_artifacts(&saved[0][0].reference())
        .await
        .unwrap();
    assert_eq!(files.len(), 2);
    assert!(files.iter().all(|a| a.no_training));
    let notes = target.notes("run-7", 0, 10).await.unwrap().items;
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].locator, Locator::Step { step: 1 });
    assert_eq!(
        target.document("bench").await.unwrap().title,
        "Arm and cube"
    );
}
