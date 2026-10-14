use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Barrier};

use robot_lab::document::*;
use robot_lab::store::{Note, Selection};
use robot_lab::{Document, Error, Operation, Reply, Store};
use tempfile::TempDir;

fn fixture() -> (TempDir, Store, Reference, Reference, Reference) {
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

fn episode(experiment: &Reference) -> Episode {
    Episode {
        experiment: experiment.clone(),
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
    }
}

#[test]
fn documents_round_trip_and_schema_names_are_checked() {
    let (_dir, store, robot, scene, experiment) = fixture();
    for reference in [robot, scene, experiment] {
        let value = store.get(&reference).unwrap().content;
        let encoded = serde_json::to_vec(&value).unwrap();
        assert_eq!(serde_json::from_slice::<Document>(&encoded).unwrap(), value);
        let mut json = serde_json::to_value(&value).unwrap();
        json["surprise"] = true.into();
        assert!(serde_json::from_value::<Document>(json).is_err());
    }
    assert!(serde_json::from_str::<Document>(r#"{"schema":"robot.v2","content":{}}"#).is_err());
}

#[test]
fn reopened_lab_keeps_selections_cards_notes_and_pinned_experiments() {
    let (dir, store, _, scene, experiment) = fixture();
    let recorded = store
        .save(
            "attempt",
            None,
            Document::Episode(episode(&experiment)),
            "Test",
            "Stopped attempt",
            4,
        )
        .unwrap();
    let selection = Selection {
        scene: scene.clone(),
        episode: Some(recorded.clone()),
        step: Some(1),
        comparison: None,
        camera: "front".into(),
    };
    store.select("chat-one", &selection).unwrap();
    store
        .add_note(Note {
            subject: recorded.clone(),
            episode_step: Some(1),
            author: "Test".into(),
            text: "Try a slower reach".into(),
            created_unix_ms: 5,
        })
        .unwrap();
    let Reply::Saved {
        reference: changed, ..
    } = store
        .execute(
            Operation::MoveObject {
                base: scene.clone(),
                object: "cube".into(),
                position: [0.4, 0.5, 0.0],
                author: "Test".into(),
                summary: "Cube moved 20 cm to the right".into(),
            },
            6,
        )
        .unwrap()
    else {
        panic!()
    };
    drop(store);
    let store = Store::open(dir.path(), "openagents").unwrap();
    assert_eq!(store.selection("chat-one").unwrap(), Some(selection));
    assert_eq!(store.latest("bench").unwrap(), changed);
    let Document::Scene(old_scene) = store.get(&scene).unwrap().content else {
        panic!()
    };
    assert_eq!(old_scene.objects[0].geometry.pose.position[0], 0.2);
    let Document::Experiment(inputs) = store.get(&experiment).unwrap().content else {
        panic!()
    };
    assert_eq!(inputs.scene, scene);
    let notes = store.notes(&recorded).unwrap();
    assert_eq!(notes[0].episode_step, Some(1));
    assert!(store.notes(&changed).unwrap().is_empty());
    assert!(
        matches!(store.execute(Operation::Inspect { episode: recorded, step: 1 }, 7).unwrap(), Reply::Inspected { step, .. } if step.index == 1)
    );
}

#[test]
fn concurrent_edits_cannot_overwrite_each_other() {
    let (_dir, store, _, scene, _) = fixture();
    let barrier = Arc::new(Barrier::new(2));
    let workers: Vec<_> = [0.3, 0.4]
        .into_iter()
        .map(|position| {
            let store = store.clone();
            let base = scene.clone();
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                barrier.wait();
                store.execute(
                    Operation::MoveObject {
                        base,
                        object: "cube".into(),
                        position: [position, 0.5, 0.0],
                        author: "Test".into(),
                        summary: "Moved cube".into(),
                    },
                    5,
                )
            })
        })
        .collect();
    let results: Vec<_> = workers
        .into_iter()
        .map(|worker| worker.join().unwrap())
        .collect();
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|r| matches!(r, Err(Error::Conflict)))
            .count(),
        1
    );
    assert_eq!(store.versions("bench").unwrap().len(), 2);
}

#[test]
fn edits_validate_joint_limits_camera_links_and_object_dimensions() {
    let (_dir, store, _, scene, _) = fixture();
    for (joint, value) in [("shoulder", 1.01), ("unknown", 0.0), ("shoulder", f64::NAN)] {
        assert!(
            store
                .execute(
                    Operation::Pose {
                        base: scene.clone(),
                        robot: "arm".into(),
                        joints: BTreeMap::from([(joint.into(), value)]),
                        author: "Test".into(),
                        summary: "Moved joint".into()
                    },
                    4
                )
                .is_err()
        );
    }
    let Document::Scene(mut content) = store.get(&scene).unwrap().content else {
        panic!()
    };
    content.cameras[0].robot_link = Some(("arm".into(), "missing".into()));
    assert!(
        store
            .save(
                "bad",
                None,
                Document::Scene(content.clone()),
                "Test",
                "Bad scene",
                5
            )
            .is_err()
    );
    content.cameras[0].robot_link = None;
    content.objects[0].geometry.shape = Shape::Box {
        size: [-0.1, 0.2, 0.3],
    };
    assert!(
        store
            .save(
                "bad",
                None,
                Document::Scene(content),
                "Test",
                "Bad scene",
                5
            )
            .is_err()
    );
    assert_eq!(store.latest("bench").unwrap(), scene);
}

#[test]
fn robot_tree_axes_and_inertia_are_checked() {
    let (_dir, store, robot, _, _) = fixture();
    let Document::Robot(content) = store.get(&robot).unwrap().content else {
        panic!()
    };
    let mut bad = content.clone();
    bad.joints[0].axis = [0.0; 3];
    assert!(bad.validate().is_err());
    let mut bad = content.clone();
    bad.joints[0].child = "base".into();
    assert!(bad.validate().is_err());
    let mut bad = content.clone();
    bad.links[1].inertial.as_mut().unwrap().tensor = [1.0, 1.0, 3.0, 0.0, 0.0, 0.0];
    assert!(bad.validate().is_err());
    let mut bad = content.clone();
    bad.links[1].inertial.as_mut().unwrap().tensor = [1.0, 1.0, 1.0, 2.0, 0.0, 0.0];
    assert!(bad.validate().is_err());
    let mut marker = content.clone();
    marker.joints[0].kind = JointKind::Fixed;
    marker.joints[0].axis = [0.0; 3];
    marker.joints[0].limits = None;
    marker.links[1].inertial.as_mut().unwrap().mass = 1e-9;
    marker.links[1].inertial.as_mut().unwrap().tensor = [0.0; 6];
    assert!(marker.validate().is_ok());
    marker.links[1].visuals.push(Geometry {
        pose: Pose::default(),
        shape: Shape::Sphere { radius: 0.1 },
        rgba: [1.0; 4],
    });
    assert!(
        marker.validate().is_err(),
        "zero inertia is reserved for empty fixed markers"
    );
    let mut bad = content;
    bad.source
        .files
        .insert("../escape".into(), bad.source.files["arm.urdf"].clone());
    assert!(bad.validate().is_err());
}

#[test]
fn physical_limit_violations_are_saved_but_commands_stay_bounded() {
    let (_dir, store, _, _, experiment) = fixture();
    let mut observed = episode(&experiment);
    observed.steps[1].joints.insert("shoulder".into(), 1.01);
    observed.steps[1].limit_violations.push("shoulder".into());
    assert!(
        store
            .save(
                "observed",
                None,
                Document::Episode(observed.clone()),
                "Test",
                "Saved attempt",
                5
            )
            .is_ok()
    );
    observed.steps[1].limit_violations.clear();
    assert!(
        store
            .save(
                "unreported",
                None,
                Document::Episode(observed.clone()),
                "Test",
                "Saved attempt",
                6
            )
            .is_err()
    );
    observed.steps[1].limit_violations.push("shoulder".into());
    observed.steps[1].actions.insert("shoulder".into(), 1.01);
    assert!(
        store
            .save(
                "command",
                None,
                Document::Episode(observed),
                "Test",
                "Saved attempt",
                7
            )
            .is_err()
    );
}

#[test]
fn missing_or_modified_assets_cannot_be_committed() {
    let (_dir, store, robot, _, _) = fixture();
    let content = store.get(&robot).unwrap().content;
    let asset = content.assets()[0];
    std::fs::write(store.root().join("assets").join(&asset.sha256), b"broken").unwrap();
    assert!(
        store
            .save("new-arm", None, content, "Test", "Added arm", 4)
            .is_err()
    );
    assert!(matches!(store.latest("new-arm"), Err(Error::Missing)));
}

#[test]
fn asset_limits_and_untrusted_paths_are_checked_before_reads() {
    let (_dir, store, _, _, _) = fixture();
    assert!(store.put_asset(&b"large"[..], "text/plain", 3).is_err());
    assert!(store.put_asset(&b""[..], "text/plain", 3).is_err());
    let asset = Asset {
        sha256: "../../project.json".into(),
        bytes: 1,
        media_type: "text/plain".into(),
    };
    assert!(store.asset_bytes(&asset, 100).is_err());
    assert!(
        store
            .get(&Reference {
                document: "../project".into(),
                version: "a".repeat(64)
            })
            .is_err()
    );
    assert!(store.selection("../chat").is_err());
}

#[test]
fn interrupted_writes_do_not_replace_heads_and_corruption_is_reported() {
    let (dir, store, _, scene, _) = fixture();
    std::fs::write(
        store.root().join("documents").join(".interrupted-save"),
        b"{partial",
    )
    .unwrap();
    std::fs::write(
        store
            .root()
            .join("versions")
            .join(format!("{}.json", "b".repeat(64))),
        b"{partial",
    )
    .unwrap();
    let store = Store::open(dir.path(), "openagents").unwrap();
    assert_eq!(store.latest("bench").unwrap(), scene);
    assert_eq!(store.list().unwrap().len(), 3);
    let path = store
        .root()
        .join("versions")
        .join(format!("{}.json", scene.version));
    let mut value: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    value["summary"] = "Changed behind the lab".into();
    std::fs::write(path, serde_json::to_vec(&value).unwrap()).unwrap();
    assert!(matches!(store.get(&scene), Err(Error::Corrupt)));
}

#[test]
fn project_root_is_explicit_private_and_outside_git() {
    let (dir, _, _, _, _) = fixture();
    assert!(Store::open(dir.path(), "different-project").is_err());
    assert!(Store::open("relative-lab", "project").is_err());
    let checkout = tempfile::tempdir().unwrap();
    std::fs::create_dir(checkout.path().join(".git")).unwrap();
    assert!(Store::open(checkout.path().join("private-lab"), "project").is_err());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(dir.path()).unwrap().permissions().mode() & 0o777,
            0o700
        );
    }
}

#[cfg(unix)]
#[test]
fn saved_file_symlinks_are_refused() {
    let (_dir, store, _, scene, _) = fixture();
    let path = store.root().join("documents").join("bench.json");
    std::fs::remove_file(&path).unwrap();
    std::os::unix::fs::symlink(store.root().join("project.json"), &path).unwrap();
    assert!(store.latest("bench").is_err());
    assert!(
        store
            .save(
                "bench",
                Some(&scene),
                Document::Scene(Scene {
                    name: "bad".into(),
                    robots: vec![],
                    objects: vec![],
                    lights: vec![],
                    cameras: vec![]
                }),
                "Test",
                "Bad",
                4
            )
            .is_err()
    );
}

#[test]
fn experiment_compatibility_rates_seeds_and_limits_are_checked() {
    let (_dir, store, _, _, experiment) = fixture();
    let Document::Experiment(content) = store.get(&experiment).unwrap().content else {
        panic!()
    };
    let mut bad = content.clone();
    bad.seeds = vec![7, 7];
    assert!(bad.validate().is_err());
    let mut bad = content.clone();
    bad.control_every_steps = 0;
    assert!(bad.validate().is_err());
    let mut bad = content.clone();
    bad.limits.wall_seconds = 0;
    assert!(bad.validate().is_err());
    let mut bad = content.clone();
    bad.scene.version = "f".repeat(64);
    assert!(
        store
            .validate_dependencies(&Document::Experiment(bad))
            .is_err()
    );
    let mut bad = content;
    if let Controller::Scripted { joints, .. } = &mut bad.controller {
        joints.push("unknown".into());
    }
    assert!(
        store
            .validate_dependencies(&Document::Experiment(bad))
            .is_err()
    );
}

#[test]
fn partial_episodes_keep_exact_step_notes_and_reject_invalid_timing() {
    let (_dir, store, _, _, experiment) = fixture();
    let content = episode(&experiment);
    let reference = store
        .save(
            "attempt",
            None,
            Document::Episode(content.clone()),
            "Test",
            "Stopped attempt",
            4,
        )
        .unwrap();
    assert!(
        store
            .add_note(Note {
                subject: reference,
                episode_step: Some(3),
                author: "Test".into(),
                text: "Out of range".into(),
                created_unix_ms: 5
            })
            .is_err()
    );
    let mut bad = content.clone();
    bad.steps[1].time_seconds = 0.0;
    assert!(bad.validate().is_err());
    let mut bad = content.clone();
    bad.steps[1].time_seconds = 0.011;
    assert!(
        store
            .validate_dependencies(&Document::Episode(bad))
            .is_err()
    );
    let mut bad = content.clone();
    bad.steps[0].observation_step = Some(1);
    assert!(bad.validate().is_err());
    let mut bad = content.clone();
    bad.seed = 10;
    assert!(
        store
            .validate_dependencies(&Document::Episode(bad))
            .is_err()
    );
    let mut bad = content;
    bad.mode = EpisodeMode::Demonstration;
    bad.outcome = Outcome::Success;
    assert!(bad.validate().is_err());
}

#[test]
fn old_versions_can_be_compared_and_cannot_be_used_as_an_edit_base() {
    let (_dir, store, _, scene, _) = fixture();
    let operation = Operation::Pose {
        base: scene.clone(),
        robot: "arm".into(),
        joints: BTreeMap::from([("shoulder".into(), 0.5)]),
        author: "Test".into(),
        summary: "Moved shoulder".into(),
    };
    let Reply::Saved { reference, .. } = store.execute(operation.clone(), 4).unwrap() else {
        panic!()
    };
    assert!(matches!(store.execute(operation, 4), Err(Error::Conflict)));
    assert!(
        matches!(store.execute(Operation::Compare { a: scene, b: reference }, 5).unwrap(), Reply::Compared { left, right, .. } if left.number == 1 && right.number == 2)
    );
}

#[test]
fn visible_error_copy_is_plain() {
    let (_dir, store, _, _, _) = fixture();
    for error in [
        Error::Conflict,
        Error::Missing,
        Error::Corrupt,
        store.selection("../chat").unwrap_err(),
        store.put_asset(&b"large"[..], "text/plain", 1).unwrap_err(),
    ] {
        assert!(
            oa_copy::violations(&error.to_string(), &[]).is_empty(),
            "{error}"
        );
    }
}

#[test]
fn saved_hashes_survive_full_precision_pose_round_trips() {
    let (_dir, store, _, scene, _) = fixture();
    let mut reference = scene;
    for position in [
        0.12345678901234568,
        0.30000000000000004,
        0.8455124082255701,
        f64::MIN_POSITIVE,
        -0.0,
    ] {
        let Reply::Saved {
            reference: next, ..
        } = store
            .execute(
                Operation::MoveObject {
                    base: reference,
                    object: "cube".into(),
                    position: [position, 0.5, 0.0],
                    author: "Test".into(),
                    summary: "Moved cube".into(),
                },
                4,
            )
            .unwrap()
        else {
            panic!()
        };
        let Document::Scene(saved) = store.get(&next).unwrap().content else {
            panic!()
        };
        assert_eq!(
            saved.objects[0].geometry.pose.position[0].to_bits(),
            position.to_bits()
        );
        reference = next;
    }
}
