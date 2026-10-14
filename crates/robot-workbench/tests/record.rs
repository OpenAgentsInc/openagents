#![cfg(feature = "host")]
use robot_lab::document::{Camera, Pose, Scene, Step};
use robot_lab::{Document, Reference, Store};
use robot_workbench::{host::Host, record};
use serde_json::json;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

#[test]
fn recording_camera_preserves_principal_point_and_optical_axes() {
    let robot = robot_description::parse(
        "<robot name=\"one\"><link name=\"base\"/></robot>",
        Path::new("."),
    )
    .unwrap();
    let scene = Scene {
        name: "Fixture".into(),
        robots: vec![],
        objects: vec![],
        lights: vec![],
        cameras: vec![],
    };
    let step = Step {
        index: 0,
        time_seconds: 0.,
        joints: BTreeMap::new(),
        actions: BTreeMap::new(),
        observation_step: None,
        objects: BTreeMap::new(),
        contacts: vec![],
        sensors: BTreeMap::new(),
        inference_ms: None,
        limit_violations: vec![],
    };
    let camera = Camera {
        id: "front".into(),
        pose: Pose::default(),
        width: 320,
        height: 240,
        intrinsics: [200., 220., 120., 100.],
        robot_link: None,
    };
    let (matrix, eye) = record::camera_view(&robot, &scene, &step, &camera).unwrap();
    assert_eq!(eye, glam::Vec3::ZERO);
    let center = matrix.project_point3(glam::Vec3::Z);
    assert!((center.x - (2. * 120. / 320. - 1.)).abs() < 1e-6);
    assert!((center.y - (1. - 2. * 100. / 240.)).abs() < 1e-6);
    let right = matrix.project_point3(glam::Vec3::new(0.1, 0., 1.));
    let down = matrix.project_point3(glam::Vec3::new(0., 0.1, 1.));
    assert!(right.x > center.x && down.y < center.y);
    let mut mounted = camera;
    mounted.robot_link = Some(("arm".into(), "base".into()));
    assert!(record::camera_view(&robot, &scene, &step, &mounted).is_err());
}

#[test]
#[ignore = "requires the explicitly pinned native MuJoCo library and a capture GPU"]
fn native_manual_recording_saves_aligned_images_commands_and_interrupted_evidence() {
    let pin = robot_sim::mujoco::LibraryPin {
        path: PathBuf::from(std::env::var("ROBOT_LAB_MUJOCO_LIBRARY").unwrap()),
        sha256: std::env::var("ROBOT_LAB_MUJOCO_SHA256").unwrap(),
    };
    let root = tempfile::tempdir().unwrap();
    let store = Store::open(root.path(), "record-test").unwrap();
    let catalog = Path::new(env!("CARGO_MANIFEST_DIR")).join("../robot-description/assets/so101");
    let host = Host::open(store, &catalog, "http://127.0.0.1:8794").unwrap();
    let value=host.execute("robot_import",json!({"document":"table","source":"so101","revision":"a758567c3978dfeefe282ede0500085a48fe8f78"}),1).unwrap();
    let reference: Reference = serde_json::from_value(value["card"]["reference"].clone()).unwrap();
    let mut scene = host.bundle(&reference).unwrap().scene;
    scene.cameras[0].width = 160;
    scene.cameras[0].height = 100;
    scene.cameras[0].intrinsics = scene.cameras[0].intrinsics.map(|n| n / 4.);
    let subject = host
        .store
        .save(
            "table",
            Some(&reference),
            Document::Scene(scene),
            "Test",
            "Small recording camera",
            2,
        )
        .unwrap();
    let mut session =
        record::Session::start(host.clone(), subject.clone(), "manual-one", &pin, 7, 241).unwrap();
    assert!(
        session
            .pose(&BTreeMap::from([("shoulder_pan".into(), 100.)]))
            .is_err()
    );
    session
        .pose(&BTreeMap::from([("shoulder_pan".into(), 0.2)]))
        .unwrap();
    session.step(24).unwrap();
    let saved = session.finish().unwrap();
    let bundle = host.bundle(&saved).unwrap();
    let episode = bundle.episode.unwrap();
    assert_eq!(episode.steps.len(), 25);
    assert_eq!(episode.steps[1].observation_step, Some(0));
    assert_eq!(episode.steps[1].actions["shoulder_pan"], 0.2);
    assert!((episode.steps[24].joints["shoulder_pan"]).abs() > 0.001);
    for index in [0, 8, 16, 24] {
        let sensor = &episode.steps[index].sensors["front"];
        assert!((sensor.timestamp_seconds - index as f64 / 240.).abs() < 1e-10);
        let image = host.store.asset_bytes(&sensor.asset, 16 << 20).unwrap();
        assert_eq!(
            robot_data::export::rgb(&image, [160, 100]).unwrap().len(),
            160 * 100 * 3
        );
    }
    let destination = root.path().join("export");
    let manifest = record::export_episode(&host, &saved, &destination).unwrap();
    assert!(matches!(
        manifest.terminations[0],
        robot_data::export::Termination::Stopped
    ));
    let data = robot_data::dataset::Dataset::open(&destination).unwrap();
    assert_eq!(data.info.total_frames, 3);
    assert_eq!(data.images(0).unwrap()["observation.images.front"].len(), 3);
    let mut session =
        record::Session::start(host.clone(), subject, "manual-two", &pin, 8, 241).unwrap();
    session.step(8).unwrap();
    let interrupted = session.interrupt().unwrap();
    let manifest =
        record::export_episode(&host, &interrupted, &root.path().join("partial-export")).unwrap();
    assert!(matches!(
        manifest.terminations[0],
        robot_data::export::Termination::Interrupted
    ));
    assert_eq!(
        host.bundle(&interrupted)
            .unwrap()
            .episode
            .unwrap()
            .steps
            .len(),
        9
    );
}
