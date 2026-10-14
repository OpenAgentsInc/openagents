//! One policy worker per controller. A timed-out result cannot enter a later
//! observation, and callers do not wait for an outstanding compute operation.
use crate::{
    Error, Result,
    act::Act,
    artifacts::{Artifact, Axis, Normalization, Pin, features, state_features},
    preprocess, require,
};
use candle_core::{Device, Tensor};
use robot_lab::{
    Store,
    document::{Build, Experiment, Robot, Scene, Sensor},
};
use robot_sim::runner::{ActionChunk, Controller, Observation, Snapshot};
use std::{
    collections::BTreeMap,
    sync::{Arc, mpsc},
    time::Duration,
};

struct Input {
    step: u32,
    seed: u64,
    state: Snapshot,
    sensors: BTreeMap<String, Sensor>,
}
trait Engine: Send + Sync {
    fn infer(&self, input: &Input) -> Result<ActionChunk>;
}
struct Native {
    model: Act,
    pin: Pin,
    normalization: Normalization,
    store: Store,
    max_steps: u32,
}
impl Engine for Native {
    fn infer(&self, input: &Input) -> Result<ActionChunk> {
        require(
            input.step < self.max_steps
                && input
                    .step
                    .is_multiple_of(self.pin.binding.control_every_steps),
            "Choose a policy observation on this experiment's control schedule.",
        )?;
        let raw = state_features(&self.pin.binding.state, &input.state.joints)?;
        let state = self.normalization.state.apply(&raw)?;
        let state = Tensor::from_vec(state, (1, self.pin.architecture.state_dim), &Device::Cpu)?;
        let environment = if let Some(normalization) = &self.normalization.environment {
            let values = self
                .pin
                .binding
                .environment
                .iter()
                .map(|feature| {
                    let object = input.state.objects.get(&feature.object).ok_or_else(|| {
                        Error::Invalid("A policy environment input is missing.".into())
                    })?;
                    let index = match feature.axis {
                        Axis::X => 0,
                        Axis::Y => 1,
                        Axis::Z => 2,
                    };
                    Ok(object.pose.position[index] as f32)
                })
                .collect::<Result<Vec<_>>>()?;
            Some(Tensor::from_vec(
                normalization.apply(&values)?,
                (1, self.pin.architecture.environment_dim),
                &Device::Cpu,
            )?)
        } else {
            None
        };
        let mut images = Vec::new();
        let time = f64::from(input.step) * self.pin.binding.timestep_seconds;
        for camera in &self.pin.binding.cameras {
            let sensor = input.sensors.get(&camera.id).ok_or_else(|| {
                Error::Invalid(format!(
                    "This policy needs a recorded image from camera {}.",
                    camera.id
                ))
            })?;
            require(
                sensor.asset.media_type == "image/png"
                    && sensor.timestamp_seconds.is_finite()
                    && sensor.timestamp_seconds <= time + 1e-9
                    && time - sensor.timestamp_seconds
                        <= f64::from(camera.max_age_steps) * self.pin.binding.timestep_seconds
                            + 1e-9,
                "A required policy camera is missing or too old.",
            )?;
            let bytes = self
                .store
                .asset_bytes(&sensor.asset, 16 << 20)
                .map_err(|e| Error::Invalid(e.to_string()))?;
            let values = preprocess::image(
                &bytes,
                camera.source,
                camera.target,
                &self.normalization.cameras[&camera.id],
            )?;
            images.push(Tensor::from_vec(
                values,
                (1, 3, camera.target[1] as usize, camera.target[0] as usize),
                &Device::Cpu,
            )?);
        }
        let predicted = self
            .model
            .forward(&state, environment.as_ref(), &images)?
            .to_vec3::<f32>()?;
        let last = self.max_steps.saturating_sub(2);
        require(
            input.step <= last,
            "This experiment has no remaining action steps.",
        )?;
        let count = self
            .pin
            .binding
            .action_steps
            .min(((last - input.step) / self.pin.binding.control_every_steps + 1) as usize);
        let action_features = features(&self.pin.binding.actions);
        let mut actions = Vec::with_capacity(count);
        for row in &predicted[0][..count] {
            let raw = self.normalization.action.undo(row)?;
            let values = raw.iter().map(|v| f64::from(*v)).collect::<Vec<_>>();
            actions.push(
                self.pin
                    .binding
                    .actions
                    .convert(&action_features, &values)
                    .map_err(|e| Error::Invalid(e.to_string()))?,
            );
        }
        let _ = input.seed;
        Ok(ActionChunk {
            observation_step: input.step,
            expires_step: input.step + (count as u32 - 1) * self.pin.binding.control_every_steps,
            actions,
        })
    }
}
struct Request {
    input: Input,
    reply: mpsc::SyncSender<Result<ActionChunk>>,
}
pub struct NativeController {
    build: Build,
    sender: Option<mpsc::SyncSender<Request>>,
    seed: Option<u64>,
    seeds: Vec<u64>,
}
impl NativeController {
    pub fn new(
        artifact: Artifact,
        store: Store,
        robot: &Robot,
        scene: &Scene,
        experiment: &Experiment,
    ) -> Result<Self> {
        artifact.pin.validate_scene(robot, scene, experiment)?;
        if let robot_lab::document::Controller::Policy {
            weights,
            normalization,
            ..
        } = &experiment.controller
        {
            require(
                weights.sha256 == artifact.pin.files[&artifact.pin.weights].sha256
                    && normalization.sha256
                        == artifact.pin.files[&artifact.pin.normalization].sha256,
                "Use the checkpoint and normalization saved with this experiment.",
            )?;
        } else {
            return Err(Error::Invalid(
                "Choose a learned-policy experiment for this controller.".into(),
            ));
        }
        require(
            weights_match(&artifact, experiment, scene),
            "Use the policy's exact robot, cameras, joints, and fixed instruction.",
        )?;
        let (model, pin, normalization, build) = artifact.model()?;
        let engine = Native {
            model,
            pin,
            normalization,
            store,
            max_steps: experiment.limits.max_steps,
        };
        Self::worker(Arc::new(engine), build, experiment.seeds.clone())
    }
    fn worker(engine: Arc<dyn Engine>, build: Build, seeds: Vec<u64>) -> Result<Self> {
        let (sender, receiver) = mpsc::sync_channel::<Request>(1);
        std::thread::Builder::new()
            .name("robot-act".into())
            .spawn(move || {
                while let Ok(request) = receiver.recv() {
                    let reply = engine.infer(&request.input);
                    let _ = request.reply.send(reply);
                }
            })?;
        Ok(Self {
            build,
            sender: Some(sender),
            seed: None,
            seeds,
        })
    }
}
impl Controller for NativeController {
    fn build(&self) -> Build {
        self.build.clone()
    }
    fn reset(&mut self, seed: u64) -> robot_sim::Result<()> {
        if self.sender.is_none() {
            return Err(robot_sim::Error::Invalid(
                "The policy stopped. Start a new run".into(),
            ));
        }
        if !self.seeds.contains(&seed) {
            return Err(robot_sim::Error::Invalid(
                "Choose a seed saved with this experiment".into(),
            ));
        }
        self.seed = Some(seed);
        Ok(())
    }
    fn infer(&mut self, observation: Observation<'_>) -> robot_sim::Result<ActionChunk> {
        let deadline =
            std::time::Instant::now() + Duration::from_millis(u64::from(observation.timeout_ms));
        if self.seed != Some(observation.seed) {
            return Err(robot_sim::Error::Invalid(
                "Reset this policy for the observation's seed".into(),
            ));
        }
        let (reply, receiver) = mpsc::sync_channel(1);
        let request = Request {
            input: Input {
                step: observation.step,
                seed: observation.seed,
                state: Snapshot {
                    joints: observation.state.joints.clone(),
                    objects: observation.state.objects.clone(),
                    ..Default::default()
                },
                sensors: observation.sensors.clone(),
            },
            reply,
        };
        let Some(sender) = &self.sender else {
            return Err(robot_sim::Error::Invalid(
                "Restart the policy after its previous call stopped".into(),
            ));
        };
        sender
            .try_send(request)
            .map_err(|_| robot_sim::Error::Invalid("The policy is busy or has stopped".into()))?;
        match receiver.recv_timeout(deadline.saturating_duration_since(std::time::Instant::now())) {
            Ok(Ok(chunk)) => Ok(chunk),
            Ok(Err(error)) => Err(robot_sim::Error::Invalid(error.to_string())),
            Err(_) => {
                self.sender = None;
                Err(robot_sim::Error::Invalid(
                    "The policy took too long to answer".into(),
                ))
            }
        }
    }
}

fn weights_match(artifact: &Artifact, experiment: &Experiment, scene: &Scene) -> bool {
    let robot_lab::document::Controller::Policy {
        weights,
        normalization,
        robot,
        joints,
        cameras,
        instruction,
        ..
    } = &experiment.controller
    else {
        return false;
    };
    weights.bytes == artifact.pin.files[&artifact.pin.weights].bytes
        && normalization.bytes == artifact.pin.files[&artifact.pin.normalization].bytes
        && robot == &scene.robots[0].id
        && joints.iter().collect::<std::collections::BTreeSet<_>>()
            == artifact
                .pin
                .binding
                .actions
                .joints
                .iter()
                .map(|j| &j.joint)
                .collect()
        && cameras.iter().collect::<std::collections::BTreeSet<_>>()
            == artifact.pin.binding.cameras.iter().map(|c| &c.id).collect()
        && instruction == &artifact.pin.binding.instruction
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Slow;
    impl Engine for Slow {
        fn infer(&self, input: &Input) -> Result<ActionChunk> {
            std::thread::sleep(Duration::from_millis(50));
            Ok(ActionChunk {
                observation_step: input.step,
                expires_step: input.step,
                actions: vec![BTreeMap::from([("shoulder".into(), 0.2)])],
            })
        }
    }
    #[test]
    fn a_timed_out_result_cannot_be_used_by_the_next_observation() {
        let mut controller = NativeController::worker(
            Arc::new(Slow),
            Build {
                name: "test-policy".into(),
                sha256: "a".repeat(64),
            },
            vec![7],
        )
        .unwrap();
        controller.reset(7).unwrap();
        let state = Snapshot::default();
        let sensors = BTreeMap::new();
        let start = std::time::Instant::now();
        assert!(
            controller
                .infer(Observation {
                    step: 0,
                    seed: 7,
                    state: &state,
                    sensors: &sensors,
                    timeout_ms: 5
                })
                .is_err()
        );
        assert!(start.elapsed() < Duration::from_millis(40));
        std::thread::sleep(Duration::from_millis(60));
        assert!(controller.reset(7).is_err());
        assert!(
            controller
                .infer(Observation {
                    step: 8,
                    seed: 7,
                    state: &state,
                    sensors: &sensors,
                    timeout_ms: 100
                })
                .is_err()
        );
    }
}
