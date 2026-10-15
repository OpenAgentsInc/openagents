use robot_lab::{Reference, Store};
use robot_workbench::host::{Host, definitions};
use robot_workbench::view::{Effect, Intent, State};
use serde_json::json;
use std::path::Path;

fn host() -> (tempfile::TempDir, Host) {
    let root = tempfile::tempdir().unwrap();
    let store = Store::open(root.path(), "project-one").unwrap();
    let catalog = Path::new(env!("CARGO_MANIFEST_DIR")).join("../robot-description/assets/so101");
    let host = Host::open(store, &catalog, "http://127.0.0.1:8794").unwrap();
    (root, host)
}
fn imported(host: &Host) -> Reference {
    let value=host.execute("robot_import",json!({"document":"table","source":"so101","revision":"a758567c3978dfeefe282ede0500085a48fe8f78"}),1).unwrap();
    serde_json::from_value(value["card"]["reference"].clone()).unwrap()
}

#[test]
fn typed_tool_walkthrough_keeps_versions_notes_comparisons_and_chat_selection() {
    let (root, host) = host();
    let original = imported(&host);
    let pose=host.execute("robot_pose",json!({"document":"table","base":original.version,"joint":"shoulder_pan","position":0.2}),2).unwrap();
    let posed: Reference = serde_json::from_value(pose["card"]["reference"].clone()).unwrap();
    let moved=host.execute("robot_move_object",json!({"document":"table","base":posed.version,"object":"cube","x":0.32,"y":0.775,"z":-0.1}),3).unwrap();
    let latest: Reference = serde_json::from_value(moved["card"]["reference"].clone()).unwrap();
    assert_ne!(original, latest);
    assert_eq!(
        host.bundle(&original).unwrap().scene.robots[0].joints["shoulder_pan"],
        0.0
    );
    assert_eq!(
        host.bundle(&latest).unwrap().scene.robots[0].joints["shoulder_pan"],
        0.2
    );
    assert_eq!(
        host.bundle(&latest).unwrap().scene.objects[1]
            .geometry
            .pose
            .position[0],
        0.32
    );
    let comparison = host
        .execute(
            "robot_compare",
            json!({"document":"table","a":original.version,"b":latest.version}),
            4,
        )
        .unwrap();
    assert_eq!(comparison["card"]["reference"]["version"], original.version);
    assert_eq!(comparison["card"]["comparison"]["version"], latest.version);
    host.execute(
        "robot_add_note",
        json!({"document":"table","version":latest.version,"text":"The cube is easier to reach"}),
        5,
    )
    .unwrap();
    host.execute(
        "robot_select",
        json!({"chat":"chat-one","document":"table","version":latest.version,"camera":"orbit"}),
        6,
    )
    .unwrap();
    let reopened = Host::open(
        Store::open(root.path(), "project-one").unwrap(),
        &host.catalog,
        &host.base,
    )
    .unwrap();
    let restored = reopened
        .execute("robot_restore", json!({"chat":"chat-one"}), 7)
        .unwrap();
    assert_eq!(restored["card"]["reference"]["version"], latest.version);
    assert_eq!(reopened.store.notes(&latest).unwrap().len(), 1);
    assert!(reopened.store.notes(&original).unwrap().is_empty());
    assert!(host.execute("robot_pose",json!({"document":"table","base":original.version,"joint":"shoulder_pan","position":0.3}),8).is_err());
    assert!(host.execute("robot_pose",json!({"document":"table","base":latest.version,"joint":"shoulder_pan","position":20.0}),9).is_err());
    assert!(host.execute("robot_pose",json!({"document":"table","base":latest.version,"joint":"shoulder_pan","position":0.3,"untrusted":true}),10).is_err());
    assert!(host.execute("lab_pose", json!({}), 11).is_err());
    let current = host.store.selection("chat-one").unwrap().unwrap();
    let base = current.sha256().unwrap();
    let mut another = current.clone();
    another.camera = "front".into();
    host.store
        .select_if_unchanged("chat-one", &another, Some(&base))
        .unwrap();
    assert!(
        host.store
            .select_if_unchanged("chat-one", &current, Some(&base))
            .is_err(),
        "a late callback cannot replace the newer selection"
    );
}

#[test]
fn hidden_views_invalidate_pending_mounts_and_old_cards_keep_their_subject() {
    let (_root, host) = host();
    let reference = imported(&host);
    let mut state = State::new(host.bundle(&reference).unwrap()).unwrap();
    state.lifecycle(true, true);
    let opening = state.generation();
    state.lifecycle(false, true);
    assert!(!state.mount(opening));
    assert!(!state.active());
    state.lifecycle(true, true);
    assert!(state.mount(state.generation()));
    state.pose_value("shoulder_pan", 0.2).unwrap();
    assert!(state.pose_value("shoulder_pan", 100.0).is_err());
    assert_eq!(state.bundle().subject, reference);
    assert_eq!(state.bundle().scene.robots[0].joints["shoulder_pan"], 0.0);
    assert_eq!(
        state.displayed_scene().robots[0].joints["shoulder_pan"],
        0.2
    );
    assert!(
        matches!(state.apply(Intent::ApplyPose,None).unwrap(),Effect::SavePose{base,..} if base==reference)
    );
    state.choose_link("shoulder_link").unwrap();
    assert_eq!(state.joint, "shoulder_pan");
    assert!(state.readout().contains("Limits"));
    assert!(state.choose_camera("missing").is_err());
    state.orbit.turn(5000.0, -5000.0);
    state.orbit.zoom(1e9);
    let (matrix, eye) = state.orbit.matrices(640, 400);
    assert!(matrix.is_finite() && eye.is_finite());
    state.close();
    assert!(!state.mounted());
    assert!(!state.active());
}

#[test]
fn native_controls_have_valid_accessible_copy_and_bound_callbacks() {
    let (_root, host) = host();
    let reference = imported(&host);
    let state = State::new(host.bundle(&reference).unwrap()).unwrap();
    let view = robot_workbench::native::card(&state, "lab-one", 1, "robot:one").unwrap();
    let encoded = view.to_json().unwrap();
    assert!(oa_copy::violations(&String::from_utf8(encoded).unwrap(), &[]).is_empty());
    assert!(
        view.activate(&rust_native::Activation {
            instance: "lab-one".into(),
            revision: 0,
            node: "orbit".into()
        })
        .is_err()
    );
    assert!(
        view.activate(&rust_native::Activation {
            instance: "lab-one".into(),
            revision: 1,
            node: "orbit".into()
        })
        .is_ok()
    );
    let tools = definitions();
    let declared = tools["tools"].as_array().unwrap();
    assert_eq!(declared.len(), 17);
    assert!(
        declared
            .iter()
            .all(|tool| tool["name"].as_str().unwrap().starts_with("robot_")
                && tool["parameters"]["additionalProperties"] == false)
    );
}

#[test]
fn in_memory_description_uses_verified_assets_without_filesystem_access() {
    let (_root, host) = host();
    let reference = imported(&host);
    let bundle = host.bundle(&reference).unwrap();
    let definition = &bundle.robot;
    let description = &definition.source.files[&definition.description];
    let bytes = host.store.asset_bytes(description, 1 << 20).unwrap();
    robot_workbench::view::verify_asset(description, &bytes).unwrap();
    assert!(robot_workbench::view::verify_asset(description, b"modified").is_err());
    let text = std::str::from_utf8(&bytes).unwrap();
    let robot = robot_description::parse_with_reader(text, &|name| {
        Ok(host
            .store
            .asset_bytes(&definition.source.files[name], 16 << 20)
            .unwrap())
    })
    .unwrap();
    assert_eq!(robot.movable().count(), 6);
    assert!(
        robot
            .forward(&robot.home())
            .unwrap()
            .iter()
            .all(|m| m.is_finite())
    );
}

#[test]
fn execution_tools_require_explicit_configuration_and_reject_unknown_actions() {
    let (_root, host) = host();
    let tools = host.definitions();
    assert!(
        !tools["tools"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t["name"] == "robot_run")
    );
    assert!(host.execute("robot_run",json!({"experiment":"experiment","version":"a".repeat(64),"seed":7,"job":"attempt","batch":true}),1).is_err());
    assert!(
        serde_json::from_value::<robot_workbench::runs::Action>(
            json!({"action":"shell","command":"anything"})
        )
        .is_err()
    );
}

#[test]
fn native_run_controls_step_reset_and_stop_preserve_both_attempts() {
    let Some(library) = std::env::var_os("ROBOT_LAB_MUJOCO_LIBRARY") else {
        return;
    };
    let (_root, mut host) = host();
    let scene = imported(&host);
    let config = robot_workbench::experiments::Config {
        library: robot_sim::mujoco::LibraryPin {
            path: library.into(),
            sha256: std::env::var("ROBOT_LAB_MUJOCO_SHA256").unwrap(),
        },
        policy: None,
    };
    let home = host.bundle(&scene).unwrap().scene.robots[0].joints.clone();
    let experiment = robot_workbench::experiments::scripted_experiment(
        &host,
        &scene,
        vec![7, 8],
        241,
        vec![home],
        1,
        &config,
        "experiment",
    )
    .unwrap();
    host.runs = Some(std::sync::Arc::new(robot_workbench::runs::Runs::new(
        config,
    )));
    let runs = host.runs.as_ref().unwrap();
    let started = runs
        .start(host.clone(), experiment, 7, "controls-one".into(), false)
        .unwrap();
    assert_eq!(started["step"], 0);
    let stepped = runs
        .control("controls-one", robot_workbench::runs::Action::Step)
        .unwrap();
    assert_eq!(stepped["step"], 1);
    let reset = runs
        .control(
            "controls-one",
            robot_workbench::runs::Action::Reset {
                next_job: "controls-two".into(),
                seed: 8,
            },
        )
        .unwrap();
    assert_eq!(reset["step"], 0);
    let first = robot_workbench::runs::Runs::inspect(&host, "controls-one").unwrap();
    assert!(!first["episode"].is_null());
    let stopped = runs
        .control("controls-one", robot_workbench::runs::Action::Stop)
        .unwrap();
    assert!(!stopped["episode"].is_null());
    let first: Reference = serde_json::from_value(first["episode"].clone()).unwrap();
    let second: Reference = serde_json::from_value(stopped["episode"].clone()).unwrap();
    assert_eq!(host.bundle(&first).unwrap().episode.unwrap().steps.len(), 2);
    assert_eq!(
        host.bundle(&second).unwrap().episode.unwrap().steps.len(),
        1
    );
}

#[test]
fn phone_preview_keeps_accessible_readouts_and_hides_unavailable_controls() {
    use robot_workbench::device::{self, Card};
    let (_root, host) = host();
    let reference = imported(&host);
    let state = State::new(host.bundle(&reference).unwrap()).unwrap();
    let render = |operate| {
        device::card(
            Card {
                previous: None,
                next: None,
                sensor_resource: None,
                state: Some(&state),
                resource: Some("image:robot-fixture"),
                jobs: &[],
                documents: &[],
                experiments: &[(reference.clone(), 7)],
                operate,
                controllable: false,
                execution: true,
                playing: false,
                busy: false,
                job: None,
                status: "",
                saved: None,
                error: None,
            },
            "phone-fixture",
            1,
        )
        .unwrap()
    };
    let readonly = render(false);
    let text = serde_json::to_string(readonly.view()).unwrap();
    assert!(oa_copy::violations(&text, &[]).is_empty());
    assert!(text.contains("shoulder_pan"));
    assert!(text.contains("Version 1"));
    assert!(!text.contains("Run batch"));
    assert!(!text.contains("Orbit"));
    assert!(
        readonly
            .activate(&rust_native::Activation {
                instance: "phone-fixture".into(),
                revision: 0,
                node: "close".into()
            })
            .is_err()
    );
    assert!(
        readonly
            .activate(&rust_native::Activation {
                instance: "phone-fixture".into(),
                revision: 1,
                node: "close".into()
            })
            .is_ok()
    );
    assert!(
        serde_json::to_string(render(true).view())
            .unwrap()
            .contains("Run batch")
    );
}

#[test]
fn remote_lab_pages_checked_views_and_keeps_pose_versions_separate() {
    use robot_lab::remote::{Answer, CHUNK_BYTES, Request, decode};
    let (root, host) = host();
    let reference = imported(&host);
    let server = robot_workbench::server::Server::new(host, root.path());
    let Answer::Data { value, .. } = server.remote(Request::List { offset: 0 }).unwrap() else {
        panic!()
    };
    assert_eq!(value["execution"], false);
    assert!(
        value["documents"]
            .as_array()
            .unwrap()
            .iter()
            .any(|doc| doc["reference"]["version"] == reference.version)
    );
    let mut bytes = Vec::new();
    loop {
        let offset = bytes.len() as u64;
        let Answer::Chunk {
            size, bytes: part, ..
        } = server
            .remote(Request::View {
                reference: reference.clone(),
                offset,
            })
            .unwrap()
        else {
            panic!()
        };
        bytes.extend(decode(&part).unwrap());
        if bytes.len() as u64 == size {
            break;
        }
        assert_eq!(bytes.len() % CHUNK_BYTES, 0);
    }
    let before: robot_lab::view::Bundle = serde_json::from_slice(&bytes).unwrap();
    before.validate().unwrap();
    assert_eq!(before.subject, reference);
    let command = Request::Pose {
        reference: reference.clone(),
        joint: "shoulder_pan".into(),
        position: "0.2".into(),
    };
    let Answer::Data { value, .. } = server.remote(command.clone()).unwrap() else {
        panic!()
    };
    let updated: Reference = serde_json::from_value(value["card"]["reference"].clone()).unwrap();
    assert_ne!(updated, reference);
    assert!(server.remote(command).is_err());
    assert_eq!(
        server.host.bundle(&reference).unwrap().scene.robots[0].joints["shoulder_pan"],
        0.0
    );
    assert_eq!(
        server.host.bundle(&updated).unwrap().scene.robots[0].joints["shoulder_pan"],
        0.2
    );
    assert!(
        server
            .remote(Request::Run {
                experiment: reference,
                seed: 7,
                job: "forbidden".into(),
                batch: true
            })
            .is_err()
    );
}

#[test]
fn remote_simulation_keeps_partial_evidence_and_reads_exact_saved_preview() {
    use robot_lab::remote::{Answer, Control, Request};
    let Some(library) = std::env::var_os("ROBOT_LAB_MUJOCO_LIBRARY") else {
        return;
    };
    let (root, mut host) = host();
    let scene = imported(&host);
    let config = robot_workbench::experiments::Config {
        library: robot_sim::mujoco::LibraryPin {
            path: library.into(),
            sha256: std::env::var("ROBOT_LAB_MUJOCO_SHA256").unwrap(),
        },
        policy: None,
    };
    let pose = host.bundle(&scene).unwrap().scene.robots[0].joints.clone();
    let experiment = robot_workbench::experiments::scripted_experiment(
        &host,
        &scene,
        vec![7],
        241,
        vec![pose],
        1,
        &config,
        "remote-experiment",
    )
    .unwrap();
    host.runs = Some(std::sync::Arc::new(robot_workbench::runs::Runs::new(
        config,
    )));
    let server = robot_workbench::server::Server::new(host, root.path());
    let run = Request::Run {
        experiment,
        seed: 7,
        job: "remote-one".into(),
        batch: false,
    };
    assert!(server.remote(run).is_ok());
    let control = |job: &str, command| Request::Control {
        job: job.into(),
        command,
    };
    assert!(server.remote(control("remote-one", Control::Step)).is_ok());
    let Answer::Data { value, .. } = server
        .remote(Request::Status {
            job: "remote-one".into(),
        })
        .unwrap()
    else {
        panic!()
    };
    assert_eq!(value["latest"]["index"], 1);
    assert!(
        server
            .remote(control(
                "remote-one",
                Control::Reset {
                    next_job: "remote-reset".into(),
                    seed: 7
                }
            ))
            .is_ok()
    );
    // Reconnecting with the new durable job name still reaches the current worker.
    let Answer::Data { value, .. } = server
        .remote(control("remote-reset", Control::Stop))
        .unwrap()
    else {
        panic!()
    };
    let reference: Reference = serde_json::from_value(value["episode"].clone()).unwrap();
    let episode = server.host.bundle(&reference).unwrap().episode.unwrap();
    assert_eq!(episode.steps.len(), 1);
    let old = robot_sim::jobs::Book::open(server.host.store.clone())
        .unwrap()
        .get("remote-one")
        .unwrap();
    assert!(old.episode.is_some());
    assert_eq!(old.recording.unwrap().episode.steps.len(), 2);
    let Answer::Data { value, .. } = server
        .remote(Request::Preview {
            reference,
            frame: 0,
        })
        .unwrap()
    else {
        panic!()
    };
    let asset: robot_lab::document::Asset = serde_json::from_value(value).unwrap();
    assert_eq!(asset.media_type, "image/png");
    assert!(
        server
            .remote(Request::Asset {
                sha256: asset.sha256,
                offset: 0
            })
            .is_ok()
    );
}

/// A CAD Lab version's URDF export (as `cad_export` writes it: a pinned
/// folder with a 64-digit document digest) imports as a robot on a floor,
/// keeping each link's mass and inertia and its joints' limits.
#[test]
fn a_cad_lab_urdf_imports_with_its_mass_and_joints() {
    use sha2::{Digest, Sha256};
    let (root, mut host) = host();
    let cad = root.path().join("cad");
    let folder = cad.join("cart/v3/out/urdf");
    std::fs::create_dir_all(&folder).unwrap();
    let urdf = r#"<?xml version="1.0"?>
<robot name="cart">
  <link name="base_link"/>
  <link name="body">
    <inertial><origin xyz="0 0 0.1" rpy="0 0 0"/><mass value="4.5"/>
      <inertia ixx="0.02" ixy="0" ixz="0.001" iyy="0.03" iyz="0" izz="0.04"/></inertial>
    <visual><geometry><box size="0.4 0.3 0.1"/></geometry></visual>
  </link>
  <link name="lid">
    <inertial><origin xyz="0.05 0 0.01" rpy="0 0 0"/><mass value="0.25"/>
      <inertia ixx="0.001" ixy="0" ixz="0" iyy="0.002" iyz="0" izz="0.003"/></inertial>
  </link>
  <joint name="body_fixed" type="fixed"><parent link="base_link"/><child link="body"/><origin xyz="0 0 0.1" rpy="0 0 0"/></joint>
  <joint name="hinge" type="revolute"><parent link="body"/><child link="lid"/><origin xyz="-0.2 0 0.05" rpy="0 0 0"/>
    <axis xyz="0 -1 0"/><limit lower="0" upper="1.9" effort="100" velocity="2"/></joint>
</robot>
"#;
    let files = [
        ("cart.urdf", urdf.as_bytes()),
        ("NOTICE", b"CAD Lab cart v3\n".as_slice()),
    ];
    let mut pinned = Vec::new();
    for (path, bytes) in files {
        std::fs::write(folder.join(path), bytes).unwrap();
        let hash: String = Sha256::digest(bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        pinned.push(json!({"path": path, "sha256": hash}));
    }
    let revision = "ab".repeat(32);
    std::fs::write(
        folder.join("source.json"),
        json!({"repository": "https://github.com/OpenAgentsInc/openagents", "revision": revision,
               "source_path": "CAD Lab cart v3", "license": "LicenseRef-CAD-Lab-design",
               "description": "cart.urdf", "notices": ["NOTICE"], "files": pinned})
        .to_string(),
    )
    .unwrap();
    host.cad = Some(cad);
    let wrong = host.execute(
        "robot_import",
        json!({"document": "cart", "source": "cad:cart/v3", "revision": "0".repeat(64)}),
        1,
    );
    assert!(wrong.unwrap_err().contains(&revision));
    assert!(
        host.execute(
            "robot_import",
            json!({"document": "cart", "source": "cad:../x/v3", "revision": revision}),
            1
        )
        .is_err()
    );
    let value = host
        .execute(
            "robot_import",
            json!({"document": "cart", "source": "cad:cart/v3", "revision": revision}),
            1,
        )
        .unwrap();
    assert!(
        value["summary"].as_str().unwrap().contains("4.75 kg"),
        "{value}"
    );
    let reference: Reference = serde_json::from_value(value["card"]["reference"].clone()).unwrap();
    let bundle = host.bundle(&reference).unwrap();
    assert_eq!(bundle.scene.objects[0].id, "floor");
    let robot = &bundle.robot;
    let body = robot.links.iter().find(|l| l.name == "body").unwrap();
    let inertial = body.inertial.as_ref().unwrap();
    assert_eq!(inertial.mass, 4.5);
    assert_eq!(inertial.center, [0.0, 0.0, 0.1]);
    assert_eq!(inertial.tensor, [0.02, 0.03, 0.04, 0.0, 0.001, 0.0]);
    let hinge = robot.joints.iter().find(|j| j.name == "hinge").unwrap();
    assert_eq!(hinge.kind, robot_lab::document::JointKind::Revolute);
    assert_eq!(hinge.limits.unwrap().upper, 1.9);
    assert_eq!(bundle.scene.robots[0].joints["hinge"], 0.0);
}
