//! A policy import pins architecture, normalization, mappings, and source
//! evidence together. Loading it never fetches or executes repository code.
use crate::{
    Error, Result,
    act::{Act, Config},
    preprocess::Normalize,
    require,
};
use robot_data::{
    dataset::FilePin,
    mapping::{Mapping, Unit},
};
use robot_lab::document::{Build, Experiment, JointKind, Robot, Scene};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::File,
    io::Read,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pin {
    pub schema: String,
    pub family: String,
    pub source: Source,
    pub architecture: Config,
    pub binding: Binding,
    pub weights: String,
    pub normalization: String,
    pub files: BTreeMap<String, FilePin>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub repository: String,
    pub revision: String,
    pub license: String,
    pub purpose: String,
    pub training_evidence: serde_json::Value,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Binding {
    pub robot_type: String,
    pub robot_description_sha256: String,
    pub task: Build,
    pub task_definition_sha256: String,
    pub instruction: String,
    pub state: Mapping,
    pub actions: Mapping,
    pub environment: Vec<Environment>,
    pub cameras: Vec<Camera>,
    pub timestep_seconds: f64,
    pub control_every_steps: u32,
    pub action_steps: usize,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Environment {
    pub object: String,
    pub axis: Axis,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Axis {
    X,
    Y,
    Z,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Camera {
    pub id: String,
    pub source: [u32; 2],
    pub target: [u32; 2],
    pub definition_sha256: String,
    pub max_age_steps: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Normalization {
    pub state: Normalize,
    pub environment: Option<Normalize>,
    pub action: Normalize,
    pub cameras: BTreeMap<String, Normalize>,
}

impl Pin {
    pub fn validate(&self) -> Result<()> {
        self.architecture.validate()?;
        require(
            self.schema == "robot-policy.v1"
                && self.family == "act"
                && self.files.len() <= 16
                && self.files.contains_key(&self.weights)
                && self.files.contains_key(&self.normalization),
            "Choose a supported ACT checkpoint with all its saved files.",
        )?;
        self.source.validate()?;
        self.binding.validate(
            self.architecture.state_dim,
            self.architecture.action_dim,
            self.architecture.environment_dim,
            self.architecture.cameras,
            self.architecture.chunk_size,
        )?;
        let mut total = 0_u64;
        for (name, pin) in &self.files {
            file_name(name)?;
            total = total.saturating_add(pin.bytes);
            require(
                pin.sha256.len() == 64
                    && pin.sha256.bytes().all(|b| b.is_ascii_hexdigit())
                    && pin.bytes > 0
                    && pin.bytes <= 256 << 20
                    && total <= 300 << 20,
                "Use bounded policy files with recorded hashes.",
            )?;
        }
        Ok(())
    }
    pub fn validate_scene(
        &self,
        robot: &Robot,
        scene: &Scene,
        experiment: &Experiment,
    ) -> Result<()> {
        self.validate()?;
        self.binding.validate_scene(robot, scene, experiment)
    }
}
impl Binding {
    pub fn validate(
        &self,
        state_dim: usize,
        action_dim: usize,
        environment_dim: usize,
        camera_count: usize,
        chunk_size: usize,
    ) -> Result<()> {
        require(
            (1..=64).contains(&state_dim)
                && (1..=64).contains(&action_dim)
                && environment_dim <= 64
                && camera_count <= 4
                && (1..=128).contains(&chunk_size),
            "Use bounded policy input and action shapes",
        )?;
        let binding = self;
        let state = features(&binding.state);
        let actions = features(&binding.actions);
        binding
            .state
            .validate(&binding.robot_type, &state)
            .map_err(|e| Error::Invalid(e.to_string()))?;
        binding
            .actions
            .validate(&binding.robot_type, &actions)
            .map_err(|e| Error::Invalid(e.to_string()))?;
        require(
            binding.robot_description_sha256.len() == 64
                && binding
                    .robot_description_sha256
                    .bytes()
                    .all(|b| b.is_ascii_hexdigit())
                && binding.state.robot_version == binding.robot_description_sha256
                && binding.actions.robot_version == binding.robot_description_sha256
                && state.len() == state_dim
                && actions.len() == action_dim
                && binding.environment.len() == environment_dim
                && binding.cameras.len() == camera_count,
            "Map the checkpoint to its exact robot, input shapes, and actions.",
        )?;
        require(
            (1..=chunk_size).contains(&binding.action_steps)
                && binding.timestep_seconds.is_finite()
                && (1e-6..=1.).contains(&binding.timestep_seconds)
                && (1..=10000).contains(&binding.control_every_steps),
            "Choose bounded action chunks and the trained control interval.",
        )?;
        binding
            .task
            .validate()
            .map_err(|e| Error::Invalid(e.to_string()))?;
        let mut cameras = BTreeSet::new();
        require(
            binding.task_definition_sha256.len() == 64
                && binding
                    .task_definition_sha256
                    .bytes()
                    .all(|b| b.is_ascii_hexdigit())
                && !binding.instruction.is_empty()
                && binding.instruction.len() <= 4096
                && !binding.instruction.chars().any(|c| c.is_control()),
            "Pin the policy's exact task and fixed instruction.",
        )?;
        for camera in &binding.cameras {
            require(
                !camera.id.is_empty()
                    && camera.id.len() <= 96
                    && camera
                        .id
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
                    && cameras.insert(&camera.id)
                    && camera.source.iter().all(|n| (1..=2048).contains(n))
                    && camera.target.iter().all(|n| (16..=640).contains(n))
                    && camera.definition_sha256.len() == 64
                    && camera
                        .definition_sha256
                        .bytes()
                        .all(|b| b.is_ascii_hexdigit())
                    && camera.max_age_steps <= 10000,
                "Name each required policy camera and its calibrated dimensions.",
            )?;
        }
        for feature in &binding.environment {
            require(
                !feature.object.is_empty()
                    && feature.object.len() <= 96
                    && feature
                        .object
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b)),
                "Map environment inputs to named scene objects.",
            )?;
        }
        Ok(())
    }
    pub fn validate_scene(
        &self,
        robot: &Robot,
        scene: &Scene,
        experiment: &Experiment,
    ) -> Result<()> {
        robot
            .validate()
            .map_err(|e| Error::Invalid(e.to_string()))?;
        scene
            .validate()
            .map_err(|e| Error::Invalid(e.to_string()))?;
        experiment
            .validate()
            .map_err(|e| Error::Invalid(e.to_string()))?;
        let description = robot
            .source
            .files
            .get(&robot.description)
            .ok_or_else(|| Error::Invalid("The robot is missing its description.".into()))?;
        require(
            description.sha256 == self.robot_description_sha256
                && scene.robots.len() == 1
                && experiment.task.version == self.task
                && digest(&serde_json::to_vec(&experiment.task)?) == self.task_definition_sha256
                && (experiment.timestep_seconds - self.timestep_seconds).abs() < 1e-12
                && experiment.control_every_steps == self.control_every_steps,
            "Use the robot, task, and control timing declared by this policy.",
        )?;
        for mapping in [&self.state, &self.actions] {
            let expected: Vec<_> = robot
                .joints
                .iter()
                .filter(|j| j.kind != JointKind::Fixed)
                .collect();
            require(
                mapping.joints.len() == expected.len(),
                "Map every movable robot joint for this policy.",
            )?;
            for joint in &mapping.joints {
                let robot_joint =
                    expected
                        .iter()
                        .find(|j| j.name == joint.joint)
                        .ok_or_else(|| {
                            Error::Invalid(
                                "The policy names a joint this robot doesn't have.".into(),
                            )
                        })?;
                let limits = robot_joint.limits.ok_or_else(|| {
                    Error::Invalid("Use bounded joints for policy control.".into())
                })?;
                require(
                    (joint.lower - limits.lower).abs() < 1e-9
                        && (joint.upper - limits.upper).abs() < 1e-9
                        && match joint.unit {
                            Unit::Meters => robot_joint.kind == JointKind::Prismatic,
                            Unit::Degrees | Unit::Radians => {
                                robot_joint.kind != JointKind::Prismatic
                            }
                            Unit::Normalized { .. } => true,
                        },
                    "Use this robot's exact joint limits and explicit action units.",
                )?;
            }
        }
        for camera in &self.cameras {
            let actual = scene
                .cameras
                .iter()
                .find(|c| c.id == camera.id)
                .ok_or_else(|| {
                    Error::Invalid(format!("This policy needs camera {}.", camera.id))
                })?;
            require(
                [actual.width, actual.height] == camera.source
                    && digest(&serde_json::to_vec(actual)?) == camera.definition_sha256,
                "Use the camera placement and calibration declared by this policy.",
            )?;
        }
        require(
            self.environment
                .iter()
                .all(|f| scene.objects.iter().any(|o| o.id == f.object)),
            "This policy needs its mapped scene objects.",
        )
    }
}

impl Normalization {
    pub fn validate(&self, pin: &Pin) -> Result<()> {
        self.validate_binding(
            &pin.binding,
            pin.architecture.state_dim,
            pin.architecture.action_dim,
            pin.architecture.environment_dim,
        )
    }
    pub fn validate_binding(
        &self,
        binding: &Binding,
        state_dim: usize,
        action_dim: usize,
        environment_dim: usize,
    ) -> Result<()> {
        self.state.validate(state_dim)?;
        self.action.validate(action_dim)?;
        match (&self.environment, environment_dim) {
            (None, 0) => (),
            (Some(norm), n) if n > 0 => norm.validate(n)?,
            _ => {
                return Err(Error::Invalid(
                    "Use the policy's declared environment normalization.".into(),
                ));
            }
        }
        require(
            self.cameras.len() == binding.cameras.len(),
            "Save normalization for every policy camera.",
        )?;
        for camera in &binding.cameras {
            self.cameras
                .get(&camera.id)
                .ok_or_else(|| {
                    Error::Invalid("A policy camera is missing its normalization.".into())
                })?
                .validate(3)?;
        }
        Ok(())
    }
}
pub struct Artifact {
    pub root: PathBuf,
    pub pin: Pin,
    pub normalization: Normalization,
    pub build: Build,
    weights: Vec<u8>,
}
impl Artifact {
    pub fn open(root: &Path, expected_sha256: &str) -> Result<Self> {
        require(
            root.is_absolute(),
            "Choose an absolute folder for private policy weights.",
        )?;
        let root = std::fs::canonicalize(root)?;
        require(
            !root.ancestors().any(|p| p.join(".git").exists()),
            "Keep policy weights outside the Git checkout.",
        )?;
        let bytes = read(&root, "policy.json", 2 << 20)?;
        require(
            digest(&bytes) == expected_sha256,
            "This policy changed. Review its new checkpoint before running it.",
        )?;
        let pin: Pin = serde_json::from_slice(&bytes)?;
        pin.validate()?;
        let mut weights = None;
        let mut normalization = None;
        for (name, expected) in &pin.files {
            let data = read(&root, name, expected.bytes as usize)?;
            require(
                data.len() as u64 == expected.bytes && digest(&data) == expected.sha256,
                "A policy file changed. Restore the pinned checkpoint.",
            )?;
            if name == &pin.weights {
                weights = Some(data);
            } else if name == &pin.normalization {
                require(data.len() <= 1 << 20, "Use bounded policy normalization.")?;
                normalization = Some(serde_json::from_slice::<Normalization>(&data)?);
            }
        }
        let normalization = normalization
            .ok_or_else(|| Error::Invalid("This policy is missing its normalization.".into()))?;
        normalization.validate(&pin)?;
        let build = Build {
            name: "robot-act-candle-v1".into(),
            sha256: digest(&serde_json::to_vec(&(
                digest(&bytes),
                digest(include_bytes!("act.rs")),
                digest(include_bytes!("preprocess.rs")),
                digest(include_bytes!("artifacts.rs")),
                digest(include_bytes!("controller.rs")),
                "candle-0.11-f32-cpu",
            ))?),
        };
        Ok(Self {
            root,
            pin,
            normalization,
            build,
            weights: weights
                .ok_or_else(|| Error::Invalid("This policy is missing its weights.".into()))?,
        })
    }
    pub fn model(self) -> Result<(Act, Pin, Normalization, Build)> {
        let vb = candle_nn::VarBuilder::from_buffered_safetensors(
            self.weights,
            candle_core::DType::F32,
            &candle_core::Device::Cpu,
        )?;
        let model = Act::load(self.pin.architecture.clone(), vb.pp("model"))?;
        Ok((model, self.pin, self.normalization, self.build))
    }
}
pub fn features(mapping: &Mapping) -> Vec<String> {
    mapping.joints.iter().map(|j| j.feature.clone()).collect()
}
pub fn state_features(mapping: &Mapping, joints: &BTreeMap<String, f64>) -> Result<Vec<f32>> {
    mapping
        .validate(&mapping.robot_type, &features(mapping))
        .map_err(|e| Error::Invalid(e.to_string()))?;
    mapping
        .joints
        .iter()
        .map(|joint| {
            let physical = *joints
                .get(&joint.joint)
                .ok_or_else(|| Error::Invalid("A policy input joint is missing.".into()))?;
            require(physical.is_finite(), "Use finite observed joint positions.")?;
            let value = (physical - joint.offset) / joint.direction;
            let value = match joint.unit {
                Unit::Radians | Unit::Meters => value,
                Unit::Degrees => value.to_degrees(),
                Unit::Normalized {
                    source_min,
                    source_max,
                    target_min,
                    target_max,
                } => {
                    let value = source_min
                        + (value - target_min) / (target_max - target_min)
                            * (source_max - source_min);
                    require(
                        value >= source_min && value <= source_max,
                        "An observed joint exceeds this checkpoint's calibration range.",
                    )?;
                    value
                }
            };
            require(
                (value as f32).is_finite(),
                "A policy input exceeds its numeric range.",
            )?;
            Ok(value as f32)
        })
        .collect()
}
pub fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
pub(crate) fn file_name(name: &str) -> Result<()> {
    require(
        !name.is_empty()
            && name.len() <= 512
            && !name.contains(['\\', ':', '\0'])
            && name
                .split('/')
                .all(|part| !part.is_empty() && part != "." && part != ".."),
        "Keep policy files inside the checkpoint folder.",
    )
}
pub(crate) fn read(root: &Path, name: &str, limit: usize) -> Result<Vec<u8>> {
    file_name(name)?;
    let path = root.join(name);
    require(
        std::fs::symlink_metadata(&path)?.is_file()
            && std::fs::canonicalize(&path)?.starts_with(root),
        "Use ordinary files saved inside the checkpoint folder.",
    )?;
    let mut bytes = Vec::new();
    File::open(path)?
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)?;
    require(bytes.len() <= limit, "This policy file is too large.")?;
    Ok(bytes)
}

impl Source {
    pub fn validate(&self) -> Result<()> {
        require(
            self.repository.starts_with("https://")
                && self.repository.len() <= 2048
                && self.revision.len() == 40
                && self.revision.bytes().all(|b| b.is_ascii_hexdigit())
                && !self.license.is_empty()
                && self.license.len() <= 128
                && !self.purpose.is_empty()
                && self.purpose.len() <= 512
                && serde_json::to_vec(&self.training_evidence)?.len() <= 1 << 20,
            "Pin the policy's source, license, and qualification evidence.",
        )
    }
}
