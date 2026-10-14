//! Known poses, axes, scale, limits, and refused imports on small
//! hand-written fixtures. Analytic poses hold to [`TOLERANCE`] meters.

use std::f64::consts::FRAC_PI_2;
use std::path::{Path, PathBuf};

use glam::{DMat4, DVec3};
use robot_description::{Error, Geometry, JointKind, load, origin, parse, pin, z_up_to_y_up};

/// Analytic forward kinematics must match to a micrometer.
const TOLERANCE: f64 = 1e-6;

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn arm() -> robot_description::Robot {
    load(&fixtures().join("arm/arm.urdf")).unwrap()
}

fn at(frame: DMat4) -> DVec3 {
    frame.transform_point3(DVec3::ZERO)
}

fn close(a: DVec3, b: DVec3) {
    assert!(a.distance(b) < TOLERANCE, "{a} is not {b}");
}

/// base -> upper (pan about Z, 0.1 m up) -> lower (lift about the given
/// axis, 0.2 m up) -> tool (fixed, 0.3 m along X).
fn two_link(lift_axis: &str) -> String {
    format!(
        r#"<robot name="two_link">
  <link name="base"/><link name="upper"/><link name="lower"/><link name="tool"/>
  <joint name="pan" type="revolute">
    <parent link="base"/><child link="upper"/>
    <origin xyz="0 0 0.1"/><axis xyz="0 0 1"/>
    <limit lower="-2" upper="2" effort="1" velocity="1"/>
  </joint>
  <joint name="lift" type="revolute">
    <parent link="upper"/><child link="lower"/>
    <origin xyz="0 0 0.2"/><axis xyz="{lift_axis}"/>
    <limit lower="-1.6" upper="1.6" effort="1" velocity="1"/>
  </joint>
  <joint name="tool" type="fixed">
    <parent link="lower"/><child link="tool"/>
    <origin xyz="0.3 0 0"/>
  </joint>
</robot>"#
    )
}

fn robot(text: &str) -> robot_description::Robot {
    parse(text, &fixtures()).unwrap()
}

fn refused(text: &str) -> Error {
    parse(text, &fixtures()).unwrap_err()
}

#[test]
fn known_joint_vectors_place_the_tool() {
    let robot = robot(&two_link("0 1 0"));
    let tool = robot.link("tool").unwrap();
    assert_eq!(robot.movable().count(), 2);
    let place = |pose: &[f64]| at(robot.forward(pose).unwrap()[tool]);
    close(place(&[0.0, 0.0]), DVec3::new(0.3, 0.0, 0.3));
    // Pan turns the arm about Z: X goes to Y.
    close(place(&[FRAC_PI_2, 0.0]), DVec3::new(0.0, 0.3, 0.3));
    // Lift about +Y tips the tool's X reach down onto the base.
    close(place(&[0.0, FRAC_PI_2]), DVec3::new(0.0, 0.0, 0.0));
    close(place(&[FRAC_PI_2, FRAC_PI_2]), DVec3::new(0.0, 0.0, 0.0));
    // The tool frame turns with its link: its X axis points down.
    let frame = robot.forward(&[0.0, FRAC_PI_2]).unwrap()[tool];
    close(frame.transform_vector3(DVec3::X), DVec3::NEG_Z);
    // The base stays at the origin.
    close(
        at(robot.forward(&[1.0, 1.0]).unwrap()[robot.root()]),
        DVec3::ZERO,
    );
}

#[test]
fn a_reversed_axis_turns_the_other_way() {
    let robot = robot(&two_link("0 -1 0"));
    let tool = robot.link("tool").unwrap();
    close(
        at(robot.forward(&[0.0, FRAC_PI_2]).unwrap()[tool]),
        DVec3::new(0.0, 0.0, 0.6),
    );
}

#[test]
fn origins_rotate_roll_then_pitch_then_yaw_about_fixed_axes() {
    let turned = origin(
        DVec3::new(1.0, 0.0, 0.0),
        DVec3::new(FRAC_PI_2, 0.0, FRAC_PI_2),
    );
    close(turned.transform_vector3(DVec3::X), DVec3::Y);
    close(turned.transform_vector3(DVec3::Y), DVec3::Z);
    close(turned.transform_point3(DVec3::ZERO), DVec3::X);
}

#[test]
fn a_prismatic_joint_slides_along_its_axis() {
    let robot = robot(
        r#"<robot name="slide"><link name="a"/><link name="b"/>
  <joint name="s" type="prismatic"><parent link="a"/><child link="b"/>
    <axis xyz="0 0 1"/><limit lower="0" upper="0.2" effort="1" velocity="1"/></joint></robot>"#,
    );
    close(
        at(robot.forward(&[0.15]).unwrap()[1]),
        DVec3::new(0.0, 0.0, 0.15),
    );
}

#[test]
fn z_up_becomes_y_up_once_at_the_base() {
    let to_y_up = z_up_to_y_up();
    close(to_y_up.transform_vector3(DVec3::Z), DVec3::Y);
    close(to_y_up.transform_vector3(DVec3::Y), DVec3::NEG_Z);
    close(to_y_up.transform_vector3(DVec3::X), DVec3::X);
    let robot = arm();
    let gripper = robot.link("gripper").unwrap();
    let frames = robot.forward(&robot.home()).unwrap();
    close(at(to_y_up * frames[gripper]), DVec3::new(0.0, 0.39, 0.0));
}

#[test]
fn the_desk_arm_loads_at_meter_scale() {
    let robot = arm();
    assert_eq!(robot.name(), "desk_arm");
    assert_eq!(robot.links().len(), 7);
    let names: Vec<&str> = robot.movable().map(|j| j.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "shoulder_pan",
            "shoulder_lift",
            "elbow_flex",
            "wrist_flex",
            "wrist_roll",
            "gripper"
        ]
    );
    assert!(robot.movable().all(|j| j.kind == JointKind::Revolute));
    // The base mesh is 80 mm wide in its file and 8 cm wide in the robot.
    let base = &robot.links()[robot.root()];
    let Geometry::Mesh { mesh, scale, file } = &base.visuals[0].geometry else {
        panic!("the base is not a mesh");
    };
    assert_eq!(file, "meshes/base.stl");
    assert_eq!(mesh.triangles.len(), 12);
    let width = f64::from(mesh.max[0] - mesh.min[0]) * scale.x;
    assert!((width - 0.08).abs() < TOLERANCE);
    assert_eq!(base.visuals[0].rgba, Some([0.95, 0.72, 0.12, 1.0]));
    // Upright at home; lifting the shoulder a quarter turn lays the arm
    // along +X at shoulder height.
    let gripper = robot.link("gripper").unwrap();
    close(
        at(robot.forward(&robot.home()).unwrap()[gripper]),
        DVec3::new(0.0, 0.0, 0.39),
    );
    let lifted = robot.named_pose(&[("shoulder_lift", FRAC_PI_2)]).unwrap();
    close(
        at(robot.forward(&lifted).unwrap()[gripper]),
        DVec3::new(0.29, 0.0, 0.1),
    );
}

#[test]
fn pinned_so101_loads_with_six_limited_joints_and_all_meshes() {
    let folder = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/so101");
    let (pin, robot) = pin::load_pinned(&folder).unwrap();
    assert_eq!(pin.revision, "a758567c3978dfeefe282ede0500085a48fe8f78");
    assert_eq!(robot.name(), "so101_new_calib");
    assert_eq!(robot.links().len(), 8);
    let joints: Vec<_> = robot.movable().collect();
    assert_eq!(joints.len(), 6);
    assert!(joints.iter().all(|joint| joint.kind == JointKind::Revolute));
    assert!(joints.iter().all(|joint| joint.limit.is_some()));
    let frames = robot.forward(&robot.home()).unwrap();
    let shoulder = robot.link("shoulder_link").unwrap();
    close(
        at(frames[shoulder]),
        DVec3::new(0.0388353, -8.97657e-9, 0.0624),
    );
    assert!(
        robot
            .links()
            .iter()
            .flat_map(|link| &link.visuals)
            .any(|shape| { matches!(shape.geometry, Geometry::Mesh { .. }) })
    );
}

#[test]
fn poses_stay_within_the_declared_limits() {
    let robot = arm();
    let mut pose = robot.home();
    pose[0] = 1.92;
    assert!(robot.forward(&pose).is_ok(), "a limit itself is allowed");
    pose[0] = 1.920_001;
    assert_eq!(
        robot.forward(&pose).unwrap_err(),
        Error::Limit {
            joint: "shoulder_pan".into(),
            value: 1.920_001,
            lower: -1.92,
            upper: 1.92
        }
    );
    pose[0] = f64::NAN;
    assert!(
        robot
            .forward(&pose)
            .unwrap_err()
            .to_string()
            .contains("not a finite number")
    );
    assert_eq!(
        robot.forward(&[0.0]).unwrap_err(),
        Error::JointCount {
            expected: 6,
            got: 1
        }
    );
    let error = robot.named_pose(&[("elbow", 0.1)]).unwrap_err().to_string();
    assert!(error.contains("no movable joint is named elbow"), "{error}");
    assert!(robot.named_pose(&[("elbow_flex", -2.0)]).is_err());
    // A continuous joint has no limits.
    let spin = parse(
        r#"<robot name="spin"><link name="a"/><link name="b"/>
  <joint name="s" type="continuous"><parent link="a"/><child link="b"/><axis xyz="0 0 1"/></joint></robot>"#,
        &fixtures(),
    )
    .unwrap();
    assert!(spin.forward(&[100.0]).is_ok());
}

fn says(error: Error, words: &str) {
    let text = error.to_string();
    assert!(text.contains(words), "{text:?} does not say {words:?}");
}

#[test]
fn unsupported_features_are_refused_by_name() {
    let joint = |inner: &str, kind: &str| {
        format!(
            r#"<robot name="r"><link name="a"/><link name="b"/>
  <joint name="j" type="{kind}"><parent link="a"/><child link="b"/>{inner}</joint></robot>"#
        )
    };
    let limit = r#"<limit lower="-1" upper="1" effort="1" velocity="1"/>"#;
    says(
        refused(&joint(
            &format!(r#"{limit}<mimic joint="other"/>"#),
            "revolute",
        )),
        "a mimic joint (following joint other) is not supported",
    );
    says(
        refused(&joint("", "floating")),
        "a floating joint is not supported",
    );
    says(
        refused(&joint("", "planar")),
        "a planar joint is not supported",
    );
    says(refused(&joint("", "ball")), "unknown joint type \"ball\"");
    assert_eq!(
        robot(r#"<robot name="r"><link name="a"/><transmission name="t"/></robot>"#).name(),
        "r"
    );
    says(
        refused(r#"<robot name="r"><link name="a"><origin xyz="1 0 0"/></link></robot>"#),
        "non-identity <origin> directly in a link",
    );
    says(
        refused(r#"<robot name="r"><link name="a"/><gazebo/></robot>"#),
        "<gazebo> (simulator settings) is not supported",
    );
    says(
        refused(
            r#"<robot name="r" xmlns:xacro="http://www.ros.org/wiki/xacro"><xacro:macro name="m"/></robot>"#,
        ),
        "Xacro",
    );
    says(
        refused(r#"<mujoco model="r"><worldbody/></mujoco>"#),
        "the top element is <mujoco>",
    );
    says(refused("<robot name="), "not well-formed XML");
    let shape = |geometry: &str| {
        format!(
            r#"<robot name="r"><link name="a"><visual><geometry>{geometry}</geometry></visual></link></robot>"#
        )
    };
    says(
        refused(&shape(r#"<mesh filename="meshes/base.dae"/>"#)),
        "mesh format \"dae\" (only STL is read) is not supported",
    );
    says(
        refused(&shape(r#"<capsule radius="1" length="1"/>"#)),
        "<capsule> geometry",
    );
}

#[test]
fn malformed_trees_are_refused() {
    let tree = |joints: &str| {
        format!(
            r#"<robot name="r"><link name="a"/><link name="b"/><link name="c"/>{joints}</robot>"#
        )
    };
    let fixed = |name: &str, parent: &str, child: &str| {
        format!(
            r#"<joint name="{name}" type="fixed"><parent link="{parent}"/><child link="{child}"/></joint>"#
        )
    };
    says(
        refused(&tree(&format!(
            "{}{}",
            fixed("ab", "a", "b"),
            fixed("cb", "c", "b")
        ))),
        "a closed kinematic chain (link b is the child of joints ab and cb)",
    );
    says(
        refused(&tree(&fixed("ab", "a", "b"))),
        "links a, c have no parent joint",
    );
    says(
        refused(&tree(&format!(
            "{}{}{}",
            fixed("ab", "a", "b"),
            fixed("bc", "b", "c"),
            fixed("ca", "c", "a")
        ))),
        "the joints form a loop",
    );
    says(
        refused(&format!(
            r#"<robot name="r"><link name="a"/><link name="b"/><link name="c"/>{}{}</robot>"#,
            fixed("bc", "b", "c"),
            fixed("cb", "c", "b")
        )),
        "links b, c are not connected to base link a",
    );
    says(
        refused(&tree(&fixed("ax", "a", "x"))),
        "joint ax names child link x, which the description does not define",
    );
    says(
        refused(r#"<robot name="r"><link name="a"/><link name="a"/></robot>"#),
        "two links are named a",
    );
    says(refused(r#"<robot name="r"/>"#), "the robot has no links");
}

#[test]
fn values_in_the_wrong_unit_or_domain_are_refused() {
    let joint = |inner: &str| {
        format!(
            r#"<robot name="r"><link name="a"/><link name="b"/>
  <joint name="j" type="revolute"><parent link="a"/><child link="b"/>{inner}</joint></robot>"#
        )
    };
    let limit = r#"<limit lower="-1" upper="1" effort="1" velocity="1"/>"#;
    says(
        refused(&joint(
            r#"<limit lower="-110" upper="110" effort="1" velocity="1"/>"#,
        )),
        "URDF joint limits are radians, not degrees",
    );
    says(
        refused(&joint(&format!(r#"<origin xyz="0 0 62.4"/>{limit}"#))),
        "62.4 m from its parent; URDF lengths are meters",
    );
    says(
        refused(&joint(&format!(r#"<axis xyz="0 0 2"/>{limit}"#))),
        "not a unit vector",
    );
    says(
        refused(&joint(r#"<axis xyz="0 0 1"/>"#)),
        "no <limit lower upper>",
    );
    says(
        refused(&joint(
            r#"<limit lower="1" upper="-1" effort="1" velocity="1"/>"#,
        )),
        "leave no range of motion",
    );
    says(
        refused(&joint(&format!(r#"<origin xyz="0 0 nan"/>{limit}"#))),
        "not finite",
    );
    says(
        refused(&joint(&format!(r#"<origin xyz="0 0"/>{limit}"#))),
        "it needs 3 numbers",
    );
    // The base mesh is millimeters: without its scale it spans 80 m.
    says(
        parse(
            r#"<robot name="r"><link name="a"><visual><geometry><mesh filename="meshes/base.stl"/></geometry></visual></link></robot>"#,
            &fixtures().join("arm"),
        )
        .unwrap_err(),
        "spans 80.0 m; URDF lengths are meters",
    );
}

#[test]
fn mesh_paths_must_stay_in_the_description_folder() {
    let mesh = |file: &str| {
        format!(
            r#"<robot name="r"><link name="a"><visual><geometry><mesh filename="{file}" scale="0.001 0.001 0.001"/></geometry></visual></link></robot>"#
        )
    };
    let folder = fixtures().join("arm");
    let refused = |file: &str| parse(&mesh(file), &folder).unwrap_err();
    assert!(parse(&mesh("meshes/base.stl"), &folder).is_ok());
    assert!(parse(&mesh("./meshes/base.stl"), &folder).is_ok());
    says(refused("../arm/meshes/base.stl"), "climbs out with ..");
    says(
        refused("meshes/../../arm/meshes/base.stl"),
        "climbs out with ..",
    );
    says(refused("/etc/passwd.stl"), "is absolute");
    says(refused("C:/meshes/base.stl"), "is absolute");
    says(refused("package://so101/meshes/base.stl"), "is a URL");
    says(refused("meshes\\base.stl"), "backslashes");
    says(
        refused("meshes/missing.stl"),
        "does not exist under the description's folder",
    );
}

#[cfg(unix)]
#[test]
fn a_link_out_of_the_folder_is_refused() {
    let outside = tempfile::tempdir().unwrap();
    std::fs::copy(
        fixtures().join("arm/meshes/base.stl"),
        outside.path().join("base.stl"),
    )
    .unwrap();
    let folder = tempfile::tempdir().unwrap();
    std::os::unix::fs::symlink(
        outside.path().join("base.stl"),
        folder.path().join("base.stl"),
    )
    .unwrap();
    let error = parse(
        r#"<robot name="r"><link name="a"><visual><geometry><mesh filename="base.stl" scale="0.001 0.001 0.001"/></geometry></visual></link></robot>"#,
        folder.path(),
    )
    .unwrap_err();
    says(
        error,
        "leads outside the description's folder through a link",
    );
}

#[test]
fn oversized_inputs_are_refused() {
    let big = format!(
        r#"<robot name="r"><link name="a"/><!--{}--></robot>"#,
        "x".repeat(robot_description::MAX_DESCRIPTION_BYTES as usize)
    );
    assert!(matches!(refused(&big), Error::TooLarge { .. }));
    let links: String = (0..=robot_description::MAX_LINKS)
        .map(|i| format!(r#"<link name="l{i}"/>"#))
        .collect();
    let error = refused(&format!(r#"<robot name="r">{links}</robot>"#));
    says(error, "the number of links is 65, over the limit of 64");
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("big.urdf");
    std::fs::write(&file, big).unwrap();
    assert!(matches!(load(&file).unwrap_err(), Error::TooLarge { .. }));
    let xacro = dir.path().join("arm.urdf.xacro");
    std::fs::write(&xacro, "<robot name=\"r\"/>").unwrap();
    says(load(&xacro).unwrap_err(), "Xacro");
    says(
        load(&dir.path().join("arm.xml")).unwrap_err(),
        "is not a .urdf file",
    );
}

/// A pinned copy of the desk arm in a scratch folder.
fn pinned(revision: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::create_dir(root.join("meshes")).unwrap();
    for file in ["arm.urdf", "meshes/base.stl"] {
        std::fs::copy(fixtures().join("arm").join(file), root.join(file)).unwrap();
    }
    std::fs::write(root.join("LICENSE"), "Apache License 2.0 (fixture)\n").unwrap();
    let files: Vec<_> = ["arm.urdf", "meshes/base.stl", "LICENSE"]
        .iter()
        .map(|path| {
            serde_json::json!({
                "path": path,
                "sha256": pin::sha256_hex(&std::fs::read(root.join(path)).unwrap()),
            })
        })
        .collect();
    let source = serde_json::json!({
        "repository": "https://example.com/desk-arm",
        "revision": revision,
        "source_path": "description",
        "license": "Apache-2.0",
        "description": "arm.urdf",
        "notices": ["LICENSE"],
        "files": files,
    });
    std::fs::write(root.join(pin::PIN_FILE), source.to_string()).unwrap();
    dir
}

#[test]
fn a_pinned_folder_matches_its_hashes_and_notices() {
    let revision = "0123456789abcdef0123456789abcdef01234567";
    let dir = pinned(revision);
    let (source, robot) = pin::load_pinned(dir.path()).unwrap();
    assert_eq!(source.revision, revision);
    assert_eq!(source.notices, ["LICENSE"]);
    assert_eq!(robot.links().len(), 7);

    std::fs::write(dir.path().join("LICENSE"), "changed\n").unwrap();
    says(pin::verify(dir.path()).unwrap_err(), "LICENSE has SHA-256");

    let dir = pinned(revision);
    std::fs::write(dir.path().join("setup.sh"), "#!/bin/sh\n").unwrap();
    says(
        pin::verify(dir.path()).unwrap_err(),
        "setup.sh is in the folder but not in the pin",
    );

    says(
        pin::verify(pinned("main").path()).unwrap_err(),
        "not a 40-digit commit hash",
    );
}
