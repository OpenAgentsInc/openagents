#![cfg(feature = "host")]
use robot_lab::{Reference, Store};
use robot_workbench::{
    host::Host,
    sim::{Request, Sim},
};
use serde_json::json;
use std::path::{Path, PathBuf};

#[test]
fn live_physics_requests_use_the_recording_commands() {
    let start: Request =
        serde_json::from_value(json!({"start":{"document":"table","version":"a"}})).unwrap();
    assert!(matches!(start, Request::Start { .. }));
    let pose: Request =
        serde_json::from_value(json!({"command":"pose","joints":{"shoulder_pan":0.2},"steps":8}))
            .unwrap();
    assert!(matches!(pose, Request::Command(_)));
    assert!(serde_json::from_value::<Request>(json!({"command":"step","steps":24})).is_ok());
    assert!(serde_json::from_value::<Request>(json!({"command":"run"})).is_err());
}

#[test]
#[ignore = "requires the explicitly pinned native MuJoCo library"]
fn live_physics_moves_the_arm_within_limits_and_step_bounds() {
    let pin = robot_sim::mujoco::LibraryPin {
        path: PathBuf::from(std::env::var("ROBOT_LAB_MUJOCO_LIBRARY").unwrap()),
        sha256: std::env::var("ROBOT_LAB_MUJOCO_SHA256").unwrap(),
    };
    let root = tempfile::tempdir().unwrap();
    let store = Store::open(root.path(), "sim-test").unwrap();
    let catalog = Path::new(env!("CARGO_MANIFEST_DIR")).join("../robot-description/assets/so101");
    let host = Host::open(store, &catalog, "http://127.0.0.1:8794").unwrap();
    let value=host.execute("robot_import",json!({"document":"table","source":"so101","revision":"a758567c3978dfeefe282ede0500085a48fe8f78"}),1).unwrap();
    let reference: Reference = serde_json::from_value(value["card"]["reference"].clone()).unwrap();
    let sim = Sim::new(host, pin);
    let request = |value| serde_json::from_value::<Request>(value).unwrap();
    assert!(
        sim.run(request(json!({"command":"step","steps":1})))
            .is_err()
    );
    let first = sim.run(request(json!({ "start": reference }))).unwrap();
    assert_eq!(first["step"]["index"], 0);
    assert!(first["step"]["objects"]["cube"].is_object());
    assert!(
        sim.run(request(json!({"command":"step","steps":241})))
            .is_err()
    );
    assert!(
        sim.run(request(
            json!({"command":"pose","joints":{"shoulder_pan":9.0},"steps":8})
        ))
        .is_err()
    );
    let moved = sim
        .run(request(
            json!({"command":"pose","joints":{"shoulder_pan":0.5},"steps":240}),
        ))
        .unwrap();
    assert_eq!(moved["step"]["index"], 240);
    let pan = moved["step"]["joints"]["shoulder_pan"].as_f64().unwrap();
    assert!((pan - 0.5).abs() < 0.02, "{pan}");
    sim.run(request(json!({"command":"finish"}))).unwrap();
    assert!(
        sim.run(request(json!({"command":"step","steps":1})))
            .is_err()
    );
}
