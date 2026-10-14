//! Robot descriptions for Robot Lab (`docs/lab/README.md`): a checked URDF
//! subset, its STL meshes, the source pin that names where they came from,
//! and forward kinematics. No renderer, chat, or network dependency.
//!
//! [`load`] reads one URDF file. Its meshes must be relative paths under the
//! file's own folder; absolute paths, URLs (`package://`), `..`, and links
//! that lead outside the folder are refused, as are Xacro macros,
//! transmissions, mimic joints, closed chains, floating and planar joints,
//! and inputs over the bounds below. Transmission tags are accepted as
//! actuator metadata but do not affect inspection or imply motor support.
//! Lengths are meters and angles radians, as URDF defines them; values that
//! look like millimeters or degrees are refused rather than guessed at.
//!
//! [`Robot::forward`] places every link in the robot's own frame, which is
//! the URDF's Z-up frame. A Y-up renderer applies [`z_up_to_y_up`] once at
//! the robot's base, never per link.
//!
//! [`pin::verify`] checks a vendored description against its `source.json`:
//! the upstream repository, an immutable revision, every file's SHA-256, and
//! the license and notices that travel with it.

mod error;
pub mod pin;
pub mod stl;
mod urdf;

use std::sync::Arc;

use glam::{DMat4, DVec3};

pub use error::Error;
pub use stl::TriangleMesh;
pub use urdf::{load, parse};

/// The largest description file read, in bytes.
pub const MAX_DESCRIPTION_BYTES: u64 = 1 << 20;
/// The most XML nodes a description may hold.
pub const MAX_XML_NODES: u32 = 50_000;
/// The most links a robot may have.
pub const MAX_LINKS: usize = 64;
/// The most joints a robot may have.
pub const MAX_JOINTS: usize = 64;
/// The most distinct mesh files a robot may reference.
pub const MAX_MESHES: usize = 64;
/// The largest mesh file read, in bytes.
pub const MAX_MESH_BYTES: u64 = 16 << 20;
/// The most mesh bytes one robot may read in total.
pub const MAX_TOTAL_MESH_BYTES: u64 = 64 << 20;
/// The farthest a joint or shape origin may sit from its parent, in meters.
/// A larger offset almost always means millimeters.
pub const MAX_OFFSET_METERS: f64 = 5.0;
/// The widest a scaled mesh may be, in meters.
pub const MAX_MESH_EXTENT_METERS: f64 = 2.0;
/// How far a joint axis may be from unit length.
pub const AXIS_TOLERANCE: f64 = 1e-3;

/// A checked robot: links, joints, and loaded meshes.
#[derive(Clone, Debug)]
pub struct Robot {
    name: String,
    links: Vec<Link>,
    joints: Vec<Joint>,
    root: usize,
    /// Joint indices with every parent link placed before its child.
    order: Vec<usize>,
    /// Indices of the joints a pose sets, in declaration order.
    movable: Vec<usize>,
}

/// One rigid link.
#[derive(Clone, Debug)]
pub struct Link {
    pub name: String,
    /// What the link looks like, in the link's frame.
    pub visuals: Vec<Shape>,
    /// Its contact geometry, in the link's frame. Kept for later
    /// simulation; inspection does not use it.
    pub collisions: Vec<Shape>,
}

/// A placed piece of geometry.
#[derive(Clone, Debug)]
pub struct Shape {
    /// Where the geometry sits in its link's frame.
    pub origin: DMat4,
    pub geometry: Geometry,
    /// RGBA from 0 to 1 as the URDF material gives it, when it has a color.
    pub rgba: Option<[f32; 4]>,
}

/// URDF geometry.
#[derive(Clone, Debug)]
pub enum Geometry {
    /// A box with these full side lengths, centered on its origin.
    Box {
        size: DVec3,
    },
    /// A cylinder along its origin's Z axis, centered on it.
    Cylinder {
        radius: f64,
        length: f64,
    },
    Sphere {
        radius: f64,
    },
    /// An STL mesh scaled from its file's units into meters.
    Mesh {
        /// The path as the description wrote it.
        file: String,
        scale: DVec3,
        mesh: Arc<TriangleMesh>,
    },
}

/// One joint between a parent and a child link.
#[derive(Clone, Debug)]
pub struct Joint {
    pub name: String,
    pub kind: JointKind,
    /// Index into [`Robot::links`].
    pub parent: usize,
    /// Index into [`Robot::links`].
    pub child: usize,
    /// The child frame at a zero joint value, in the parent's frame.
    pub origin: DMat4,
    /// Unit axis in the joint frame.
    pub axis: DVec3,
    pub limit: Option<Limit>,
}

/// Supported joint kinds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JointKind {
    Fixed,
    /// Turns about its axis between limits, in radians.
    Revolute,
    /// Turns about its axis without limits.
    Continuous,
    /// Slides along its axis between limits, in meters.
    Prismatic,
}

/// A joint's declared range and ratings.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Limit {
    pub lower: f64,
    pub upper: f64,
    /// Newton-meters or newtons.
    pub effort: f64,
    /// Radians or meters per second.
    pub velocity: f64,
}

impl Robot {
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Every link, in declaration order.
    #[must_use]
    pub fn links(&self) -> &[Link] {
        &self.links
    }

    /// Every joint, in declaration order.
    #[must_use]
    pub fn joints(&self) -> &[Joint] {
        &self.joints
    }

    /// The link no joint moves, the robot's base.
    #[must_use]
    pub fn root(&self) -> usize {
        self.root
    }

    /// The index of the link named `name`.
    #[must_use]
    pub fn link(&self, name: &str) -> Option<usize> {
        self.links.iter().position(|link| link.name == name)
    }

    /// The joints a pose sets, in declaration order: every joint that is
    /// not fixed.
    pub fn movable(&self) -> impl Iterator<Item = &Joint> {
        self.movable.iter().map(|&i| &self.joints[i])
    }

    /// The rest pose: every movable joint at zero, or at its nearest limit
    /// when zero is outside them.
    #[must_use]
    pub fn home(&self) -> Vec<f64> {
        self.movable()
            .map(|joint| joint.limit.map_or(0.0, |l| 0.0f64.clamp(l.lower, l.upper)))
            .collect()
    }

    /// The home pose with the named joints set; refused when a name is not
    /// a movable joint or a value is outside its limits.
    pub fn named_pose(&self, values: &[(&str, f64)]) -> Result<Vec<f64>, Error> {
        let mut pose = self.home();
        for &(name, value) in values {
            let index = self
                .movable()
                .position(|joint| joint.name == name)
                .ok_or_else(|| {
                    let names: Vec<&str> = self.movable().map(|j| j.name.as_str()).collect();
                    Error::Pose(format!(
                        "no movable joint is named {name}; the joints are {}",
                        names.join(", ")
                    ))
                })?;
            pose[index] = value;
        }
        self.check(&pose)?;
        Ok(pose)
    }

    /// Whether `pose` has one finite value per movable joint, each within
    /// its joint's limits (the limits themselves included).
    pub fn check(&self, pose: &[f64]) -> Result<(), Error> {
        if pose.len() != self.movable.len() {
            return Err(Error::JointCount {
                expected: self.movable.len(),
                got: pose.len(),
            });
        }
        for (joint, &value) in self.movable().zip(pose) {
            if !value.is_finite() {
                return Err(Error::Pose(format!(
                    "joint {} is given {value}, not a finite number",
                    joint.name
                )));
            }
            if let Some(limit) = joint.limit
                && !(limit.lower..=limit.upper).contains(&value)
            {
                return Err(Error::Limit {
                    joint: joint.name.clone(),
                    value,
                    lower: limit.lower,
                    upper: limit.upper,
                });
            }
        }
        Ok(())
    }

    /// Each link's frame in the robot's base frame for `pose` (one value
    /// per [`Robot::movable`] joint), indexed like [`Robot::links`].
    /// A pose outside the limits is refused, not clamped.
    pub fn forward(&self, pose: &[f64]) -> Result<Vec<DMat4>, Error> {
        self.check(pose)?;
        let mut value = vec![0.0; self.joints.len()];
        for (&joint, &q) in self.movable.iter().zip(pose) {
            value[joint] = q;
        }
        let mut frames = vec![DMat4::IDENTITY; self.links.len()];
        for &index in &self.order {
            let joint = &self.joints[index];
            let q = value[index];
            let motion = match joint.kind {
                JointKind::Fixed => DMat4::IDENTITY,
                JointKind::Revolute | JointKind::Continuous => {
                    DMat4::from_axis_angle(joint.axis, q)
                }
                JointKind::Prismatic => DMat4::from_translation(joint.axis * q),
            };
            frames[joint.child] = frames[joint.parent] * joint.origin * motion;
        }
        Ok(frames)
    }
}

/// The rotation from a Z-up robot frame (URDF, and Verse asset packs) into
/// the Y-up runtime frame: Z becomes Y and Y becomes -Z. Apply it once, at
/// the robot's base.
#[must_use]
pub fn z_up_to_y_up() -> DMat4 {
    DMat4::from_rotation_x(-std::f64::consts::FRAC_PI_2)
}

/// A URDF origin: translate by `xyz` meters, then rotate by roll about X,
/// pitch about Y, and yaw about Z, all about the parent's fixed axes.
#[must_use]
pub fn origin(xyz: DVec3, rpy: DVec3) -> DMat4 {
    DMat4::from_translation(xyz)
        * DMat4::from_rotation_z(rpy.z)
        * DMat4::from_rotation_y(rpy.y)
        * DMat4::from_rotation_x(rpy.x)
}
