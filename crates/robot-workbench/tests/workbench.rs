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
    assert_eq!(declared.len(), 13);
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
