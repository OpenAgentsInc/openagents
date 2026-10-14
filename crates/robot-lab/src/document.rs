//! Portable version-one robot, scene, experiment, and episode documents.

use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{Result, require};

pub const MAX_DOCUMENT_BYTES: usize = 16 << 20;
pub const MAX_STEPS: usize = 100_000;
pub const MAX_EPISODES: usize = 1_000;

/// An immutable saved document version, never a moving latest-version alias.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Reference {
    pub document: String,
    pub version: String,
}

impl Reference {
    pub fn validate(&self) -> Result<()> {
        identifier(&self.document)?;
        hash(&self.version)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "schema", content = "content", deny_unknown_fields)]
pub enum Document {
    #[serde(rename = "robot.v1")]
    Robot(Robot),
    #[serde(rename = "robot-scene.v1")]
    Scene(Scene),
    #[serde(rename = "robot-experiment.v1")]
    Experiment(Experiment),
    #[serde(rename = "robot-episode.v1")]
    Episode(Episode),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Asset {
    pub sha256: String,
    pub bytes: u64,
    pub media_type: String,
}

impl Asset {
    pub fn validate(&self) -> Result<()> {
        hash(&self.sha256)?;
        require(
            self.bytes > 0 && self.bytes <= 4 << 30,
            "Choose a file between 1 byte and 4 GB.",
        )?;
        text(&self.media_type, 128)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub repository: String,
    pub revision: String,
    /// Original relative file names and their exact saved bytes.
    pub files: BTreeMap<String, Asset>,
    pub license: String,
    pub notices: Vec<Asset>,
    pub converter: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Axes {
    YUp,
    ZUp,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Robot {
    pub name: String,
    pub source: Source,
    pub description: String,
    pub meters_per_unit: f64,
    pub source_axes: Axes,
    pub links: Vec<Link>,
    pub joints: Vec<Joint>,
    pub capabilities: BTreeSet<Capability>,
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    Inspect,
    Replay,
    Simulate,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Link {
    pub name: String,
    pub inertial: Option<Inertial>,
    pub visuals: Vec<Geometry>,
    pub collisions: Vec<Geometry>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Inertial {
    pub mass: f64,
    pub center: [f64; 3],
    /// Symmetric inertia: xx, yy, zz, xy, xz, yz, in kg m².
    pub tensor: [f64; 6],
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Pose {
    pub position: [f64; 3],
    /// Unit quaternion in xyzw order, in the containing coordinate frame.
    pub rotation: [f64; 4],
}

impl Default for Pose {
    fn default() -> Self {
        Self {
            position: [0.0; 3],
            rotation: [0.0, 0.0, 0.0, 1.0],
        }
    }
}

impl Pose {
    pub fn validate(&self) -> Result<()> {
        vector(&self.position, 1_000_000.0)?;
        require(
            self.rotation.iter().all(|n| n.is_finite())
                && (self.rotation.iter().map(|n| n * n).sum::<f64>() - 1.0).abs() < 1e-6,
            "Use a valid rotation.",
        )
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Geometry {
    pub pose: Pose,
    pub shape: Shape,
    pub rgba: [f64; 4],
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Shape {
    Box { size: [f64; 3] },
    Sphere { radius: f64 },
    Cylinder { radius: f64, length: f64 },
    Mesh { file: String, scale: [f64; 3] },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Joint {
    pub name: String,
    pub parent: String,
    pub child: String,
    pub kind: JointKind,
    pub origin: Pose,
    pub axis: [f64; 3],
    pub limits: Option<Limits>,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum JointKind {
    Fixed,
    Revolute,
    Continuous,
    Prismatic,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Limits {
    pub lower: f64,
    pub upper: f64,
    pub effort: f64,
    pub velocity: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Scene {
    pub name: String,
    pub robots: Vec<RobotInstance>,
    pub objects: Vec<Object>,
    pub lights: Vec<Light>,
    pub cameras: Vec<Camera>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RobotInstance {
    pub id: String,
    pub robot: Reference,
    /// Runtime Y-up meters; source conversion happens at import.
    pub pose: Pose,
    pub joints: BTreeMap<String, f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Object {
    pub id: String,
    pub geometry: Geometry,
    pub mesh: Option<Asset>,
    pub collider: Option<Shape>,
    /// Zero denotes a static object; positive mass denotes a dynamic body.
    pub mass: f64,
    pub friction: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Light {
    pub id: String,
    pub position: [f64; 3],
    pub rgb: [f64; 3],
    pub intensity: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Camera {
    pub id: String,
    pub pose: Pose,
    pub width: u32,
    pub height: u32,
    /// fx, fy, cx, cy, in pixels.
    pub intrinsics: [f64; 4],
    pub robot_link: Option<(String, String)>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Experiment {
    pub scene: Reference,
    pub task: CubeTask,
    pub controller: Controller,
    pub backend: Build,
    pub timestep_seconds: f64,
    pub control_every_steps: u32,
    pub sensor_every_steps: u32,
    pub seeds: Vec<u64>,
    pub limits: ResourceLimits,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Build {
    pub name: String,
    pub sha256: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Controller {
    Manual {
        build: Build,
        robot: String,
        joints: Vec<String>,
    },
    Scripted {
        build: Build,
        robot: String,
        joints: Vec<String>,
    },
    Policy {
        build: Build,
        weights: Asset,
        normalization: Asset,
        robot: String,
        joints: Vec<String>,
        cameras: Vec<String>,
        instruction: String,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CubeTask {
    pub version: Build,
    pub object: String,
    pub target_min: [f64; 3],
    pub target_max: [f64; 3],
    pub stable_seconds: f64,
    pub max_linear_speed: f64,
    pub max_angular_speed: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ResourceLimits {
    pub max_steps: u32,
    pub wall_seconds: u32,
    pub max_artifact_bytes: u64,
    pub inference_timeout_ms: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Episode {
    pub experiment: Reference,
    pub seed: u64,
    pub mode: EpisodeMode,
    pub steps: Vec<Step>,
    pub outcome: Outcome,
    pub wall_seconds: f64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub events: Vec<EpisodeEvent>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EpisodeEvent {
    pub step: u32,
    pub kind: EpisodeEventKind,
    pub reason: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EpisodeEventKind {
    ActionRejected,
    InferenceTimeout,
    BackendFailed,
    Stopped,
    Completed,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EpisodeMode {
    Demonstration,
    Simulation,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Outcome {
    InProgress,
    Complete,
    Success,
    Failed { reason: String },
    Stopped { reason: String },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Step {
    pub index: u32,
    pub time_seconds: f64,
    pub joints: BTreeMap<String, f64>,
    pub actions: BTreeMap<String, f64>,
    pub observation_step: Option<u32>,
    pub objects: BTreeMap<String, ObjectState>,
    pub contacts: Vec<Contact>,
    pub sensors: BTreeMap<String, Sensor>,
    pub inference_ms: Option<f64>,
    pub limit_violations: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ObjectState {
    pub pose: Pose,
    pub linear_velocity: [f64; 3],
    pub angular_velocity: [f64; 3],
    pub held: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Contact {
    pub a: String,
    pub b: String,
    pub position: [f64; 3],
    pub force: [f64; 3],
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Sensor {
    pub asset: Asset,
    pub timestamp_seconds: f64,
    pub frame: u64,
}

pub(crate) fn text(value: &str, max: usize) -> Result<()> {
    require(
        !value.trim().is_empty() && value.len() <= max && !value.chars().any(|c| c.is_control()),
        "Use a short name or description on one line.",
    )
}

pub(crate) fn identifier(value: &str) -> Result<()> {
    require(
        (1..=96).contains(&value.len())
            && value
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"_-.:".contains(&b))
            && value != "."
            && value != "..",
        "Use a name with letters, numbers, dots, dashes, or underscores.",
    )
}

pub(crate) fn hash(value: &str) -> Result<()> {
    require(
        value.len() == 64
            && value
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
        "Choose a file or version saved by this lab.",
    )
}

pub(crate) fn file_name(value: &str) -> Result<()> {
    require(
        value.len() <= 512
            && !value.is_empty()
            && !value.contains(['\\', ':', '\0'])
            && value
                .split('/')
                .all(|part| !part.is_empty() && part != "." && part != ".."),
        "Keep robot file paths inside the imported folder.",
    )
}

fn positive(value: f64, max: f64) -> Result<()> {
    require(
        value.is_finite() && value > 0.0 && value <= max,
        "Use a positive value within the allowed range.",
    )
}

fn nonnegative(value: f64, max: f64) -> Result<()> {
    require(
        value.is_finite() && value >= 0.0 && value <= max,
        "Use a value within the allowed range.",
    )
}

fn vector(values: &[f64], max: f64) -> Result<()> {
    require(
        values.iter().all(|v| v.is_finite() && v.abs() <= max),
        "Use finite values within the allowed range.",
    )
}

fn unique<'a>(values: impl IntoIterator<Item = &'a str>, max: usize) -> Result<()> {
    let mut names = BTreeSet::new();
    for value in values {
        identifier(value)?;
        require(
            names.insert(value) && names.len() <= max,
            "Each item needs a different name, within the item limit.",
        )?;
    }
    Ok(())
}

impl Shape {
    fn validate(&self) -> Result<()> {
        match self {
            Self::Box { size } => {
                for n in size {
                    positive(*n, 1_000.0)?;
                }
            }
            Self::Sphere { radius } => positive(*radius, 1_000.0)?,
            Self::Cylinder { radius, length } => {
                positive(*radius, 1_000.0)?;
                positive(*length, 1_000.0)?;
            }
            Self::Mesh { file, scale } => {
                file_name(file)?;
                for n in scale {
                    positive(*n, 1_000.0)?;
                }
            }
        }
        Ok(())
    }
}

impl Geometry {
    fn validate(&self) -> Result<()> {
        self.pose.validate()?;
        self.shape.validate()?;
        for n in self.rgba {
            nonnegative(n, 1.0)?;
        }
        Ok(())
    }
}

impl Inertial {
    fn validate(&self) -> Result<()> {
        positive(self.mass, 1e9)?;
        vector(&self.center, 1e6)?;
        vector(&self.tensor, 1e12)?;
        let [xx, yy, zz, xy, xz, yz] = self.tensor;
        // Positive definiteness and physical principal-moment triangle inequalities.
        let determinant =
            xx * yy * zz + 2.0 * xy * xz * yz - xx * yz * yz - yy * xz * xz - zz * xy * xy;
        require(
            xx > 0.0 && xx * yy > xy * xy && determinant > 0.0,
            "Use a positive robot inertia.",
        )?;
        let half_trace = (xx + yy + zz) * 0.5;
        let [a, b, c] = [half_trace - xx, half_trace - yy, half_trace - zz];
        require(
            a >= 0.0
                && b >= 0.0
                && c >= 0.0
                && a * b >= xy * xy
                && a * c >= xz * xz
                && b * c >= yz * yz
                && a * b * c - 2.0 * xy * xz * yz - a * yz * yz - b * xz * xz - c * xy * xy
                    >= -1e-12,
            "The robot inertia doesn't match a physical body.",
        )
    }
}

impl Robot {
    pub fn validate(&self) -> Result<()> {
        text(&self.name, 256)?;
        require(
            self.source.repository.starts_with("https://")
                && self.source.repository.len() <= 2_048
                && !self.source.repository.contains(['\n', '\r']),
            "Use an HTTPS source repository.",
        )?;
        require(
            (self.source.revision.len() == 40 || self.source.revision.len() == 64)
                && self.source.revision.bytes().all(|b| b.is_ascii_hexdigit()),
            "Choose an exact source commit.",
        )?;
        text(&self.source.license, 256)?;
        text(&self.source.converter, 256)?;
        file_name(&self.description)?;
        require(
            !self.source.files.is_empty()
                && self.source.files.len() <= 256
                && self.source.files.contains_key(&self.description),
            "Include the robot description and its files.",
        )?;
        for (path, asset) in &self.source.files {
            file_name(path)?;
            asset.validate()?;
        }
        require(
            !self.source.notices.is_empty() && self.source.notices.len() <= 32,
            "Include the robot's license notices.",
        )?;
        for notice in &self.source.notices {
            notice.validate()?;
        }
        positive(self.meters_per_unit, 1_000.0)?;
        require(!self.links.is_empty(), "Include at least one robot link.")?;
        unique(self.links.iter().map(|l| l.name.as_str()), 64)?;
        unique(self.joints.iter().map(|j| j.name.as_str()), 64)?;
        let links: BTreeSet<_> = self.links.iter().map(|l| l.name.as_str()).collect();
        let mut parents = BTreeMap::new();
        for link in &self.links {
            if let Some(inertia) = &link.inertial {
                let fixed_marker = link.visuals.is_empty()
                    && link.collisions.is_empty()
                    && self
                        .joints
                        .iter()
                        .any(|joint| joint.child == link.name && joint.kind == JointKind::Fixed)
                    && inertia.tensor == [0.0; 6]
                    && inertia.mass <= 1e-8;
                if fixed_marker {
                    // URDF tool frames can carry a negligible placeholder mass
                    // and zero inertia. Preserve it only on geometry-free,
                    // fixed frames; moving bodies still require physical inertia.
                    positive(inertia.mass, 1e-8)?;
                    vector(&inertia.center, 1e6)?;
                } else {
                    inertia.validate()?;
                }
            }
            require(
                link.visuals.len() <= 64 && link.collisions.len() <= 64,
                "Use fewer shapes per robot link.",
            )?;
            for geometry in link.visuals.iter().chain(&link.collisions) {
                geometry.validate()?;
                if let Shape::Mesh { file, .. } = &geometry.shape {
                    require(
                        self.source.files.contains_key(file),
                        "Include every referenced robot mesh.",
                    )?;
                }
            }
        }
        for joint in &self.joints {
            joint.origin.validate()?;
            require(
                links.contains(joint.parent.as_str())
                    && links.contains(joint.child.as_str())
                    && joint.parent != joint.child
                    && parents
                        .insert(joint.child.as_str(), joint.parent.as_str())
                        .is_none(),
                "Connect each robot link to one parent.",
            )?;
            vector(&joint.axis, 1.0)?;
            if joint.kind != JointKind::Fixed {
                require(
                    (joint.axis.iter().map(|n| n * n).sum::<f64>() - 1.0).abs() < 1e-6,
                    "Use a unit joint axis.",
                )?;
            }
            if let Some(limits) = joint.limits {
                vector(&[limits.lower, limits.upper], 1e6)?;
                require(
                    limits.lower < limits.upper,
                    "The joint's lower limit must be below its upper limit.",
                )?;
                positive(limits.effort, 1e9)?;
                positive(limits.velocity, 1e6)?;
            }
            require(
                !matches!(joint.kind, JointKind::Revolute | JointKind::Prismatic)
                    || joint.limits.is_some(),
                "Include the joint's position, effort, and speed limits.",
            )?;
        }
        require(
            self.joints.len() + 1 == self.links.len(),
            "Connect the robot as one link tree.",
        )?;
        for start in &links {
            let mut seen = BTreeSet::new();
            let mut at = *start;
            while let Some(parent) = parents.get(at) {
                require(seen.insert(at), "The robot's link tree contains a loop.")?;
                at = parent;
            }
        }
        require(
            self.capabilities.contains(&Capability::Inspect),
            "The robot must support inspection.",
        )
    }

    pub fn validate_pose(&self, values: &BTreeMap<String, f64>) -> Result<()> {
        self.validate_state(values)?;
        require(
            values.len() <= self.joints.len(),
            "Choose joints from this robot.",
        )?;
        for (name, value) in values {
            let joint = self
                .joints
                .iter()
                .find(|j| j.name == *name)
                .ok_or_else(|| {
                    crate::Error::Invalid(format!("This robot has no joint named {name}."))
                })?;
            require(
                joint.kind != JointKind::Fixed && value.is_finite(),
                "Choose a movable joint and a finite position.",
            )?;
            if let Some(limits) = joint.limits {
                require(
                    *value >= limits.lower && *value <= limits.upper,
                    "Keep the joint inside its position limits.",
                )?;
            }
        }
        Ok(())
    }

    /// Observed physics may exceed a soft limit. Preserve finite named values;
    /// callers must separately record the violated limits.
    pub fn validate_state(&self, values: &BTreeMap<String, f64>) -> Result<()> {
        require(
            values.len() <= self.joints.len(),
            "Choose joints from this robot.",
        )?;
        for (name, value) in values {
            require(
                value.is_finite()
                    && self
                        .joints
                        .iter()
                        .any(|j| j.name == *name && j.kind != JointKind::Fixed),
                "Record finite positions for this robot's movable joints.",
            )?;
        }
        Ok(())
    }
}

impl Scene {
    pub fn validate(&self) -> Result<()> {
        text(&self.name, 256)?;
        require(!self.robots.is_empty(), "Add a robot to this scene.")?;
        unique(
            self.robots
                .iter()
                .map(|r| r.id.as_str())
                .chain(self.objects.iter().map(|o| o.id.as_str())),
            256,
        )?;
        require(self.robots.len() <= 16, "Use at most 16 robots in a scene.")?;
        for robot in &self.robots {
            robot.robot.validate()?;
            robot.pose.validate()?;
            joint_values(&robot.joints)?;
        }
        for object in &self.objects {
            object.geometry.validate()?;
            nonnegative(object.mass, 1e9)?;
            nonnegative(object.friction, 10.0)?;
            if let Some(shape) = &object.collider {
                shape.validate()?;
            }
            if let Some(mesh) = &object.mesh {
                mesh.validate()?;
            }
            require(
                !matches!(object.geometry.shape, Shape::Mesh { .. }) || object.mesh.is_some(),
                "Include the object's mesh file.",
            )?;
            require(
                object.mass == 0.0 || object.collider.is_some(),
                "Give moving objects a collision shape.",
            )?;
        }
        unique(self.lights.iter().map(|l| l.id.as_str()), 32)?;
        for light in &self.lights {
            vector(&light.position, 1e6)?;
            for n in light.rgb {
                nonnegative(n, 1.0)?;
            }
            positive(light.intensity, 1e9)?;
        }
        require(!self.cameras.is_empty(), "Add a camera to this scene.")?;
        unique(self.cameras.iter().map(|c| c.id.as_str()), 16)?;
        require(
            self.cameras.iter().all(|camera| camera.id != "orbit"),
            "Choose a camera name other than orbit, which is the inspection view.",
        )?;
        for camera in &self.cameras {
            camera.pose.validate()?;
            require(
                (1..=8_192).contains(&camera.width)
                    && (1..=8_192).contains(&camera.height)
                    && u64::from(camera.width) * u64::from(camera.height) <= 16_777_216,
                "Use a camera with at most 16 million pixels.",
            )?;
            let [fx, fy, cx, cy] = camera.intrinsics;
            positive(fx, 1e6)?;
            positive(fy, 1e6)?;
            nonnegative(cx, f64::from(camera.width))?;
            nonnegative(cy, f64::from(camera.height))?;
            if let Some((robot, link)) = &camera.robot_link {
                identifier(robot)?;
                identifier(link)?;
                require(
                    self.robots.iter().any(|r| r.id == *robot),
                    "Attach the camera to a robot in this scene.",
                )?;
            }
        }
        Ok(())
    }
}

impl Build {
    pub fn validate(&self) -> Result<()> {
        text(&self.name, 256)?;
        hash(&self.sha256)
    }
}

impl Controller {
    pub fn robot(&self) -> &str {
        match self {
            Self::Manual { robot, .. }
            | Self::Scripted { robot, .. }
            | Self::Policy { robot, .. } => robot,
        }
    }
    pub fn joints(&self) -> &[String] {
        match self {
            Self::Manual { joints, .. }
            | Self::Scripted { joints, .. }
            | Self::Policy { joints, .. } => joints,
        }
    }
}

impl Experiment {
    pub fn validate(&self) -> Result<()> {
        self.scene.validate()?;
        self.backend.validate()?;
        self.task.version.validate()?;
        identifier(&self.task.object)?;
        vector(&self.task.target_min, 1e6)?;
        vector(&self.task.target_max, 1e6)?;
        require(
            self.task
                .target_min
                .iter()
                .zip(self.task.target_max)
                .all(|(a, b)| *a < b),
            "Give the target region a positive size.",
        )?;
        positive(self.task.stable_seconds, 86_400.0)?;
        nonnegative(self.task.max_linear_speed, 1e6)?;
        nonnegative(self.task.max_angular_speed, 1e6)?;
        positive(self.timestep_seconds, 1.0)?;
        require(
            self.timestep_seconds >= 1e-6,
            "Use a simulation step of at least one microsecond.",
        )?;
        require(
            self.control_every_steps > 0
                && self.sensor_every_steps > 0
                && self.control_every_steps <= self.limits.max_steps
                && self.sensor_every_steps <= self.limits.max_steps,
            "Choose control and sensor intervals inside the run length.",
        )?;
        require(
            !self.seeds.is_empty()
                && self.seeds.len() <= MAX_EPISODES
                && self.seeds.iter().collect::<BTreeSet<_>>().len() == self.seeds.len(),
            "Choose 1 to 1000 different episode seeds.",
        )?;
        require(
            self.limits.max_steps > 0
                && self.limits.max_steps as usize <= MAX_STEPS
                && (1..=86_400).contains(&self.limits.wall_seconds)
                && (1..=4 << 30).contains(&self.limits.max_artifact_bytes)
                && (1..=300_000).contains(&self.limits.inference_timeout_ms),
            "Set finite run, storage, and inference limits.",
        )?;
        identifier(self.controller.robot())?;
        require(
            !self.controller.joints().is_empty(),
            "Map the controller's joints.",
        )?;
        unique(self.controller.joints().iter().map(String::as_str), 64)?;
        match &self.controller {
            Controller::Manual { build, .. } | Controller::Scripted { build, .. } => {
                build.validate()?
            }
            Controller::Policy {
                build,
                weights,
                normalization,
                cameras,
                instruction,
                ..
            } => {
                build.validate()?;
                weights.validate()?;
                normalization.validate()?;
                unique(cameras.iter().map(String::as_str), 16)?;
                text(instruction, 4_096)?;
            }
        }
        Ok(())
    }
}

fn joint_values(values: &BTreeMap<String, f64>) -> Result<()> {
    unique(values.keys().map(String::as_str), 1_024)?;
    vector(&values.values().copied().collect::<Vec<_>>(), 1e9)
}

impl Episode {
    pub fn validate(&self) -> Result<()> {
        self.experiment.validate()?;
        require(
            !self.steps.is_empty() && self.steps.len() <= MAX_STEPS,
            "Save 1 to 100000 episode steps.",
        )?;
        nonnegative(self.wall_seconds, 86_400.0)?;
        require(
            self.events.len() <= 1_024,
            "Save at most 1024 episode notices.",
        )?;
        for event in &self.events {
            require(
                (event.step as usize) < self.steps.len(),
                "Attach episode notices to a saved step.",
            )?;
            text(&event.reason, 1_024)?;
        }
        if let Outcome::Failed { reason } | Outcome::Stopped { reason } = &self.outcome {
            text(reason, 1_024)?;
        }
        require(
            self.mode != EpisodeMode::Demonstration || self.outcome != Outcome::Success,
            "A demonstration cannot count as a simulation success.",
        )?;
        let mut last_time = None;
        for (i, step) in self.steps.iter().enumerate() {
            require(
                step.index as usize == i,
                "Keep episode steps in order without gaps.",
            )?;
            nonnegative(step.time_seconds, 1e9)?;
            require(
                last_time.is_none_or(|t| step.time_seconds > t),
                "Keep episode times in increasing order.",
            )?;
            last_time = Some(step.time_seconds);
            joint_values(&step.joints)?;
            joint_values(&step.actions)?;
            require(
                step.observation_step.is_none_or(|s| s <= step.index),
                "An action must use a current or earlier observation.",
            )?;
            if let Some(ms) = step.inference_ms {
                nonnegative(ms, 300_000.0)?;
            }
            unique(step.objects.keys().map(String::as_str), 256)?;
            for object in step.objects.values() {
                object.pose.validate()?;
                vector(&object.linear_velocity, 1e6)?;
                vector(&object.angular_velocity, 1e6)?;
            }
            require(step.contacts.len() <= 1_024, "Use fewer contacts per step.")?;
            for contact in &step.contacts {
                identifier(&contact.a)?;
                identifier(&contact.b)?;
                vector(&contact.position, 1e6)?;
                vector(&contact.force, 1e12)?;
            }
            unique(step.sensors.keys().map(String::as_str), 16)?;
            for sensor in step.sensors.values() {
                sensor.asset.validate()?;
                nonnegative(sensor.timestamp_seconds, 1e9)?;
            }
            unique(step.limit_violations.iter().map(String::as_str), 1_024)?;
        }
        Ok(())
    }
}

impl Document {
    pub fn validate(&self) -> Result<()> {
        match self {
            Self::Robot(r) => r.validate(),
            Self::Scene(s) => s.validate(),
            Self::Experiment(e) => e.validate(),
            Self::Episode(e) => e.validate(),
        }?;
        require(
            serde_json::to_vec(self)?.len() <= MAX_DOCUMENT_BYTES,
            "This lab item is too large. Save fewer steps or shapes.",
        )
    }

    pub fn kind(&self) -> &'static str {
        match self {
            Self::Robot(_) => "robot",
            Self::Scene(_) => "scene",
            Self::Experiment(_) => "experiment",
            Self::Episode(_) => "episode",
        }
    }

    pub fn references(&self) -> Vec<&Reference> {
        match self {
            Self::Robot(_) => vec![],
            Self::Scene(scene) => scene.robots.iter().map(|r| &r.robot).collect(),
            Self::Experiment(experiment) => vec![&experiment.scene],
            Self::Episode(episode) => vec![&episode.experiment],
        }
    }

    pub fn assets(&self) -> Vec<&Asset> {
        match self {
            Self::Robot(robot) => robot
                .source
                .files
                .values()
                .chain(&robot.source.notices)
                .collect(),
            Self::Scene(scene) => scene
                .objects
                .iter()
                .filter_map(|o| o.mesh.as_ref())
                .collect(),
            Self::Experiment(experiment) => match &experiment.controller {
                Controller::Policy {
                    weights,
                    normalization,
                    ..
                } => vec![weights, normalization],
                _ => vec![],
            },
            Self::Episode(episode) => episode
                .steps
                .iter()
                .flat_map(|s| s.sensors.values().map(|v| &v.asset))
                .collect(),
        }
    }
}
