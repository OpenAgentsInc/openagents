//! Save the checked URDF and its notices without changing the source files.

use crate::Store;
use crate::document::*;
use crate::{Result, require};
use glam::{DMat3, DMat4, DQuat, DVec3, EulerRot};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

pub fn pinned_urdf(store: &Store, root: &Path) -> Result<Robot> {
    let (pin, robot) = robot_description::pin::load_pinned(root)
        .map_err(|e| crate::Error::Invalid(e.to_string()))?;
    let mut files = BTreeMap::new();
    for file in &pin.files {
        let asset = store.put_asset(
            std::fs::File::open(root.join(&file.path))?,
            if file.path.ends_with(".stl") {
                "model/stl"
            } else {
                "application/octet-stream"
            },
            robot_description::MAX_MESH_BYTES,
        )?;
        require(
            asset.sha256 == file.sha256,
            "The robot files changed during import.",
        )?;
        files.insert(file.path.clone(), asset);
    }
    let text = std::fs::read_to_string(root.join(&pin.description))?;
    let xml =
        roxmltree::Document::parse(&text).map_err(|e| crate::Error::Invalid(e.to_string()))?;
    let mut links = Vec::new();
    for link in robot.links() {
        let node = xml
            .descendants()
            .find(|n| n.has_tag_name("link") && n.attribute("name") == Some(&link.name))
            .unwrap();
        let inertial = if let Some(n) = node.children().find(|n| n.has_tag_name("inertial")) {
            let mass = n
                .children()
                .find(|n| n.has_tag_name("mass"))
                .ok_or_else(|| crate::Error::Invalid("Include the link's mass.".into()))?;
            let tensor = n
                .children()
                .find(|n| n.has_tag_name("inertia"))
                .ok_or_else(|| crate::Error::Invalid("Include the link's inertia.".into()))?;
            let origin = n.children().find(|n| n.has_tag_name("origin"));
            let center = vector(origin.and_then(|n| n.attribute("xyz")).unwrap_or("0 0 0"))?;
            let rpy = vector(origin.and_then(|n| n.attribute("rpy")).unwrap_or("0 0 0"))?;
            let [xx, yy, zz, xy, xz, yz] = ["ixx", "iyy", "izz", "ixy", "ixz", "iyz"]
                .map(|key| number(tensor.attribute(key)))
                .into_iter()
                .collect::<Result<Vec<_>>>()?
                .try_into()
                .unwrap();
            let matrix = DMat3::from_cols(
                DVec3::new(xx, xy, xz),
                DVec3::new(xy, yy, yz),
                DVec3::new(xz, yz, zz),
            );
            let rotation =
                DMat3::from_quat(DQuat::from_euler(EulerRot::ZYX, rpy[2], rpy[1], rpy[0]));
            let matrix = rotation * matrix * rotation.transpose();
            Some(Inertial {
                mass: number(mass.attribute("value"))?,
                center,
                tensor: [
                    matrix.x_axis.x,
                    matrix.y_axis.y,
                    matrix.z_axis.z,
                    matrix.y_axis.x,
                    matrix.z_axis.x,
                    matrix.z_axis.y,
                ],
            })
        } else {
            None
        };
        links.push(Link {
            name: link.name.clone(),
            inertial,
            visuals: link.visuals.iter().map(geometry).collect(),
            collisions: link.collisions.iter().map(geometry).collect(),
        });
    }
    let value = Robot {
        name: robot.name().into(),
        description: pin.description,
        source: Source {
            repository: pin.repository,
            revision: pin.revision,
            notices: pin.notices.iter().map(|name| files[name].clone()).collect(),
            files,
            license: pin.license,
            converter: "robot-urdf-document-v1".into(),
        },
        meters_per_unit: 1.0,
        source_axes: Axes::ZUp,
        links,
        joints: robot
            .joints()
            .iter()
            .map(|j| Joint {
                name: j.name.clone(),
                parent: robot.links()[j.parent].name.clone(),
                child: robot.links()[j.child].name.clone(),
                kind: match j.kind {
                    robot_description::JointKind::Fixed => JointKind::Fixed,
                    robot_description::JointKind::Revolute => JointKind::Revolute,
                    robot_description::JointKind::Continuous => JointKind::Continuous,
                    robot_description::JointKind::Prismatic => JointKind::Prismatic,
                },
                origin: pose(j.origin),
                axis: j.axis.to_array(),
                limits: j.limit.map(|l| Limits {
                    lower: l.lower,
                    upper: l.upper,
                    effort: l.effort,
                    velocity: l.velocity,
                }),
            })
            .collect(),
        capabilities: BTreeSet::from([
            Capability::Inspect,
            Capability::Replay,
            Capability::Simulate,
        ]),
    };
    value.validate()?;
    Ok(value)
}

fn number(value: Option<&str>) -> Result<f64> {
    value
        .and_then(|v| v.parse::<f64>().ok())
        .filter(|v| v.is_finite())
        .ok_or_else(|| crate::Error::Invalid("Use finite mass and inertia values.".into()))
}
fn vector(value: &str) -> Result<[f64; 3]> {
    let values = value
        .split_whitespace()
        .map(|v| number(Some(v)))
        .collect::<Result<Vec<_>>>()?;
    values
        .try_into()
        .map_err(|_| crate::Error::Invalid("Use three values for a position or rotation.".into()))
}
fn pose(matrix: DMat4) -> Pose {
    let (_, rotation, position) = matrix.to_scale_rotation_translation();
    Pose {
        position: position.to_array(),
        rotation: rotation.to_array(),
    }
}
fn geometry(value: &robot_description::Shape) -> Geometry {
    Geometry {
        pose: pose(value.origin),
        rgba: value.rgba.unwrap_or([0.7, 0.7, 0.7, 1.0]).map(f64::from),
        shape: match &value.geometry {
            robot_description::Geometry::Box { size } => Shape::Box {
                size: size.to_array(),
            },
            robot_description::Geometry::Sphere { radius } => Shape::Sphere { radius: *radius },
            robot_description::Geometry::Cylinder { radius, length } => Shape::Cylinder {
                radius: *radius,
                length: *length,
            },
            robot_description::Geometry::Mesh { file, scale, .. } => Shape::Mesh {
                file: file.clone(),
                scale: scale.to_array(),
            },
        },
    }
}

pub fn floor_scene(name: &str, reference: Reference, joints: BTreeMap<String, f64>) -> Scene {
    let eye = glam::DVec3::new(1.9, 1.3, 1.9);
    let forward = (glam::DVec3::new(0.0, 0.4, 0.0) - eye).normalize();
    let right = forward.cross(glam::DVec3::Y).normalize();
    let down = forward.cross(right);
    let rotation = glam::DQuat::from_mat3(&glam::DMat3::from_cols(right, down, forward));
    let size = [6.0, 0.04, 6.0];
    Scene {
        name: name.chars().take(120).collect(),
        robots: vec![RobotInstance {
            id: "robot".into(),
            robot: reference,
            pose: Pose::default(),
            joints,
        }],
        objects: vec![Object {
            id: "floor".into(),
            geometry: Geometry {
                pose: Pose {
                    position: [0.0, -0.02, 0.0],
                    ..Pose::default()
                },
                shape: Shape::Box { size },
                rgba: [0.55, 0.53, 0.5, 1.0],
            },
            mesh: None,
            collider: Some(Shape::Box { size }),
            mass: 0.0,
            friction: 0.8,
        }],
        lights: vec![],
        cameras: vec![Camera {
            id: "front".into(),
            pose: Pose {
                position: eye.to_array(),
                rotation: rotation.to_array(),
            },
            width: 640,
            height: 400,
            intrinsics: [812.0, 812.0, 320.0, 200.0],
            robot_link: None,
        }],
    }
}
