//! Deterministic control/sensor scheduling and bounded episode lifetimes.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use robot_lab::document::{
    Build, Contact, Episode, EpisodeEvent, EpisodeEventKind, EpisodeMode, Experiment, ObjectState,
    Outcome, Reference, Sensor, Step,
};
use serde::{Deserialize, Serialize};

use crate::{Result, require};

#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    pub joints: BTreeMap<String, f64>,
    pub velocities: BTreeMap<String, f64>,
    pub objects: BTreeMap<String, ObjectState>,
    pub contacts: Vec<Contact>,
    pub limit_violations: Vec<String>,
}

pub trait Backend {
    fn build(&self) -> Build;
    fn timestep(&self) -> f64;
    fn reset(&mut self, seed: u64) -> Result<Snapshot>;
    fn snapshot(&self) -> Result<Snapshot>;
    fn validate_action(&self, values: &BTreeMap<String, f64>) -> Result<()>;
    fn apply(&mut self, values: &BTreeMap<String, f64>) -> Result<()>;
    fn step(&mut self) -> Result<Snapshot>;
    fn sensors(&mut self, step: u32) -> Result<BTreeMap<String, Sensor>>;
}

pub struct Observation<'a> {
    pub step: u32,
    pub seed: u64,
    pub state: &'a Snapshot,
    pub sensors: &'a BTreeMap<String, Sensor>,
    pub timeout_ms: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActionChunk {
    pub observation_step: u32,
    /// The last simulation step on which any action in this chunk can apply.
    pub expires_step: u32,
    /// One action at each declared control interval, starting at the origin.
    pub actions: Vec<BTreeMap<String, f64>>,
}

pub trait Controller {
    fn build(&self) -> Build;
    fn reset(&mut self, seed: u64) -> Result<()>;
    /// Native policy clients must enforce `timeout_ms` before returning.
    /// Simulation time stays still during this call.
    fn infer(&mut self, observation: Observation<'_>) -> Result<ActionChunk>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Interactive,
    Batch,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum Event {
    ActionRejected { step: u32, reason: String },
    InferenceTimeout { step: u32, elapsed_ms: f64 },
    Stopped { step: u32 },
    Completed { step: u32 },
    BackendFailed { step: u32, reason: String },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Recording {
    pub schema: String,
    pub episode: Episode,
    pub events: Vec<Event>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Progress {
    Paused,
    Advanced,
    Finished,
}

pub struct Runner<B, C> {
    backend: B,
    controller: C,
    experiment: Experiment,
    mode: Mode,
    paused: bool,
    finished: bool,
    stop: Arc<AtomicBool>,
    started: Instant,
    state: Snapshot,
    sensors: BTreeMap<String, Sensor>,
    chunk: Option<ActionChunk>,
    recording: Recording,
    artifact_sizes: BTreeMap<String, u64>,
    recorded_bytes: usize,
}

impl<B: Backend, C: Controller> Runner<B, C> {
    pub fn new(
        mut backend: B,
        mut controller: C,
        reference: Reference,
        experiment: Experiment,
        seed: u64,
        mode: Mode,
    ) -> Result<Self> {
        reference.validate()?;
        experiment.validate()?;
        require(
            experiment.seeds.contains(&seed),
            "Choose a seed from this experiment.",
        )?;
        require(
            backend.build() == experiment.backend
                && (backend.timestep() - experiment.timestep_seconds).abs() <= 1e-12,
            "Use the simulator and step size saved with this experiment.",
        )?;
        let expected = match &experiment.controller {
            robot_lab::document::Controller::Manual { build, .. }
            | robot_lab::document::Controller::Scripted { build, .. }
            | robot_lab::document::Controller::Policy { build, .. } => build,
        };
        require(
            &controller.build() == expected,
            "Use the controller saved with this experiment.",
        )?;
        controller.reset(seed)?;
        let state = backend.reset(seed)?;
        let sensors = backend.sensors(0)?;
        let initial = recorded_step(
            0,
            experiment.timestep_seconds,
            &state,
            &sensors,
            BTreeMap::new(),
            None,
            None,
        );
        let (recorded_bytes, artifact_sizes) = initial_size(&initial, &experiment)?;
        let recording = Recording {
            schema: "robot-run-recording.v1".into(),
            episode: Episode {
                experiment: reference,
                seed,
                mode: EpisodeMode::Simulation,
                steps: vec![initial],
                outcome: Outcome::InProgress,
                wall_seconds: 0.0,
                events: Vec::new(),
            },
            events: Vec::new(),
        };
        recording.episode.validate()?;
        Ok(Self {
            backend,
            controller,
            experiment,
            mode,
            paused: mode == Mode::Interactive,
            finished: false,
            stop: Arc::new(AtomicBool::new(false)),
            started: Instant::now(),
            state,
            sensors,
            chunk: None,
            recording,
            artifact_sizes,
            recorded_bytes,
        })
    }

    pub fn pause(&mut self) {
        self.paused = true;
    }
    pub fn resume(&mut self) {
        if !self.finished {
            self.paused = false;
        }
    }
    pub fn close_view(&mut self) {
        if self.mode == Mode::Interactive {
            self.pause();
        }
    }
    pub fn stop_handle(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.stop)
    }
    pub fn recording(&self) -> &Recording {
        &self.recording
    }
    pub fn backend(&self) -> &B {
        &self.backend
    }
    pub fn mode(&self) -> Mode {
        self.mode
    }
    pub fn paused(&self) -> bool {
        self.paused
    }
    pub fn experiment(&self) -> &Experiment {
        &self.experiment
    }
    pub fn finished(&self) -> bool {
        self.finished
    }

    /// Tick a running view or batch; explicit single-step ignores pause.
    pub fn tick(&mut self) -> Result<Progress> {
        self.advance(false)
    }
    pub fn single_step(&mut self) -> Result<Progress> {
        self.advance(true)
    }

    pub fn stop(&mut self) {
        if !self.finished {
            self.stop.store(true, Ordering::Release);
            let step = self.recording.episode.steps.last().unwrap().index;
            self.recording.events.push(Event::Stopped { step });
            self.finish(Outcome::Stopped {
                reason: "Stopped by you".into(),
            });
        }
    }

    pub fn reset(&mut self, seed: u64) -> Result<Recording> {
        require(
            self.experiment.seeds.contains(&seed),
            "Choose a seed from this experiment.",
        )?;
        if !self.finished {
            self.stop();
        }
        let previous = self.recording.clone();
        self.controller.reset(seed)?;
        let state = self.backend.reset(seed)?;
        let sensors = self.backend.sensors(0)?;
        let initial = recorded_step(
            0,
            self.experiment.timestep_seconds,
            &state,
            &sensors,
            BTreeMap::new(),
            None,
            None,
        );
        let (recorded_bytes, artifact_sizes) = initial_size(&initial, &self.experiment)?;
        let mut episode = self.recording.episode.clone();
        episode.seed = seed;
        episode.steps = vec![initial];
        episode.outcome = Outcome::InProgress;
        episode.wall_seconds = 0.0;
        episode.events.clear();
        episode.validate()?;
        self.state = state;
        self.sensors = sensors;
        self.recording.episode = episode;
        self.recording.events.clear();
        self.artifact_sizes = artifact_sizes;
        self.recorded_bytes = recorded_bytes;
        self.chunk = None;
        self.finished = false;
        self.paused = self.mode == Mode::Interactive;
        self.stop.store(false, Ordering::Release);
        self.started = Instant::now();
        Ok(previous)
    }

    fn advance(&mut self, explicit: bool) -> Result<Progress> {
        if self.finished {
            return Ok(Progress::Finished);
        }
        if self.stop.load(Ordering::Acquire) {
            self.stop();
            return Ok(Progress::Finished);
        }
        if self.paused && !explicit {
            return Ok(Progress::Paused);
        }
        if self.started.elapsed().as_secs_f64() >= f64::from(self.experiment.limits.wall_seconds) {
            self.finish(Outcome::Failed {
                reason: "The run reached its time limit".into(),
            });
            return Ok(Progress::Finished);
        }
        let current = self.recording.episode.steps.last().unwrap().index;
        if self.recording.episode.steps.len() >= self.experiment.limits.max_steps as usize {
            self.recording
                .events
                .push(Event::Completed { step: current });
            self.finish(Outcome::Complete);
            return Ok(Progress::Finished);
        }
        let mut actions = BTreeMap::new();
        let mut observation_step = None;
        let mut inference_ms = None;
        if current.is_multiple_of(self.experiment.control_every_steps) {
            let ready = self.chunk.as_ref().is_some_and(|chunk| {
                current >= chunk.observation_step
                    && current <= chunk.expires_step
                    && ((current - chunk.observation_step) / self.experiment.control_every_steps)
                        < chunk.actions.len() as u32
            });
            if !ready {
                let began = Instant::now();
                let inferred = self.controller.infer(Observation {
                    step: current,
                    seed: self.recording.episode.seed,
                    state: &self.state,
                    sensors: &self.sensors,
                    timeout_ms: self.experiment.limits.inference_timeout_ms,
                });
                let elapsed = began.elapsed().as_secs_f64() * 1_000.0;
                inference_ms = Some(elapsed);
                if elapsed > f64::from(self.experiment.limits.inference_timeout_ms) {
                    self.recording
                        .episode
                        .steps
                        .last_mut()
                        .unwrap()
                        .inference_ms = Some(elapsed);
                    self.recording.events.push(Event::InferenceTimeout {
                        step: current,
                        elapsed_ms: elapsed,
                    });
                    self.finish(Outcome::Failed {
                        reason: "The controller took too long to answer".into(),
                    });
                    return Ok(Progress::Finished);
                }
                match inferred {
                    Ok(chunk) => {
                        if let Err(error) = self.validate_chunk(&chunk, current) {
                            self.recording
                                .episode
                                .steps
                                .last_mut()
                                .unwrap()
                                .inference_ms = Some(elapsed);
                            self.reject(current, error.to_string());
                            return Ok(Progress::Finished);
                        }
                        self.chunk = Some(chunk);
                    }
                    Err(error) => {
                        self.recording
                            .episode
                            .steps
                            .last_mut()
                            .unwrap()
                            .inference_ms = Some(elapsed);
                        self.reject(current, error.to_string());
                        return Ok(Progress::Finished);
                    }
                }
            }
            let chunk = self.chunk.as_ref().unwrap();
            let index =
                ((current - chunk.observation_step) / self.experiment.control_every_steps) as usize;
            actions = chunk.actions[index].clone();
            observation_step = Some(chunk.observation_step);
            if let Err(error) = self.backend.validate_action(&actions) {
                self.reject(current, error.to_string());
                return Ok(Progress::Finished);
            }
            if self.stop.load(Ordering::Acquire) {
                self.stop();
                return Ok(Progress::Finished);
            }
            if let Err(error) = self.backend.apply(&actions) {
                self.backend_failed(current, error.to_string());
                return Ok(Progress::Finished);
            }
        }
        if self.started.elapsed().as_secs_f64() >= f64::from(self.experiment.limits.wall_seconds) {
            self.finish(Outcome::Failed {
                reason: "The run reached its time limit".into(),
            });
            return Ok(Progress::Finished);
        }
        if self.stop.load(Ordering::Acquire) {
            self.stop();
            return Ok(Progress::Finished);
        }
        self.state = match self.backend.step() {
            Ok(state) => state,
            Err(error) => {
                self.backend_failed(current, error.to_string());
                return Ok(Progress::Finished);
            }
        };
        let next = current + 1;
        let mut sampled = if next.is_multiple_of(self.experiment.sensor_every_steps) {
            match self.backend.sensors(next) {
                Ok(sensors) => sensors,
                Err(error) => {
                    self.backend_failed(current, error.to_string());
                    return Ok(Progress::Finished);
                }
            }
        } else {
            BTreeMap::new()
        };
        let mut artifacts = self.artifact_sizes.clone();
        for sensor in sampled.values() {
            artifacts.insert(sensor.asset.sha256.clone(), sensor.asset.bytes);
        }
        let total = artifacts
            .values()
            .try_fold(0_u64, |sum, value| sum.checked_add(*value));
        let camera_limit =
            total.is_none_or(|bytes| bytes > self.experiment.limits.max_artifact_bytes);
        if camera_limit {
            sampled.clear();
        }
        if next.is_multiple_of(self.experiment.sensor_every_steps) {
            self.sensors = sampled.clone();
        }
        let recorded = recorded_step(
            next,
            self.experiment.timestep_seconds,
            &self.state,
            &sampled,
            actions,
            observation_step,
            inference_ms,
        );
        let bytes = serde_json::to_vec(&recorded)?.len();
        if self.recorded_bytes.saturating_add(bytes)
            > robot_lab::document::MAX_DOCUMENT_BYTES - (2 << 20)
        {
            self.finish(Outcome::Failed {
                reason: "The run reached its saved motion size limit".into(),
            });
            return Ok(Progress::Finished);
        }
        self.recorded_bytes += bytes;
        self.recording.episode.steps.push(recorded);
        self.recording.episode.wall_seconds = self.started.elapsed().as_secs_f64();
        // Stop without referring to camera files that exceed the saved budget.
        // The completed physical state remains available for replay.
        if camera_limit {
            self.finish(Outcome::Failed {
                reason: "The run reached its camera storage limit".into(),
            });
            return Ok(Progress::Finished);
        }
        self.artifact_sizes = artifacts;
        Ok(Progress::Advanced)
    }

    fn validate_chunk(&self, chunk: &ActionChunk, step: u32) -> Result<()> {
        require(
            chunk.observation_step == step
                && chunk.expires_step >= step
                && chunk.expires_step < self.experiment.limits.max_steps
                && !chunk.actions.is_empty()
                && chunk.actions.len() <= 1_024,
            "The controller returned an old or invalid action chunk.",
        )?;
        for actions in &chunk.actions {
            require(
                actions.len() == self.experiment.controller.joints().len()
                    && self
                        .experiment
                        .controller
                        .joints()
                        .iter()
                        .all(|joint| actions.contains_key(joint))
                    && actions.values().all(|n| n.is_finite()),
                "Map every controller action to a finite robot joint position.",
            )?;
            self.backend.validate_action(actions)?;
        }
        Ok(())
    }

    fn reject(&mut self, step: u32, reason: String) {
        let reason = short_reason(reason);
        self.recording.events.push(Event::ActionRejected {
            step,
            reason: reason.clone(),
        });
        self.finish(Outcome::Failed { reason });
    }

    fn backend_failed(&mut self, step: u32, reason: String) {
        let reason = short_reason(reason);
        self.recording.events.push(Event::BackendFailed {
            step,
            reason: reason.clone(),
        });
        self.finish(Outcome::Failed { reason });
    }

    fn finish(&mut self, outcome: Outcome) {
        self.finished = true;
        self.paused = true;
        self.recording.episode.outcome = outcome;
        self.recording.episode.wall_seconds = self.started.elapsed().as_secs_f64();
        self.recording.episode.events = self
            .recording
            .events
            .iter()
            .map(|event| {
                let (step, kind, reason) = match event {
                    Event::ActionRejected { step, reason } => {
                        (*step, EpisodeEventKind::ActionRejected, reason.clone())
                    }
                    Event::InferenceTimeout { step, .. } => (
                        *step,
                        EpisodeEventKind::InferenceTimeout,
                        "The controller took too long to answer".into(),
                    ),
                    Event::BackendFailed { step, reason } => {
                        (*step, EpisodeEventKind::BackendFailed, reason.clone())
                    }
                    Event::Stopped { step } => {
                        (*step, EpisodeEventKind::Stopped, "Stopped by you".into())
                    }
                    Event::Completed { step } => {
                        (*step, EpisodeEventKind::Completed, "Run finished".into())
                    }
                };
                EpisodeEvent { step, kind, reason }
            })
            .collect();
    }

    pub fn save(
        &self,
        store: &robot_lab::Store,
        document: &str,
        author: &str,
        now_unix_ms: u64,
    ) -> Result<Reference> {
        Ok(store.save(
            document,
            None,
            robot_lab::Document::Episode(self.recording.episode.clone()),
            author,
            "Saved robot attempt",
            now_unix_ms,
        )?)
    }
}

fn initial_size(step: &Step, experiment: &Experiment) -> Result<(usize, BTreeMap<String, u64>)> {
    let bytes = serde_json::to_vec(step)?.len();
    let assets: BTreeMap<_, _> = step
        .sensors
        .values()
        .map(|s| (s.asset.sha256.clone(), s.asset.bytes))
        .collect();
    let total = assets
        .values()
        .try_fold(0_u64, |sum, value| sum.checked_add(*value));
    require(
        bytes <= robot_lab::document::MAX_DOCUMENT_BYTES - (2 << 20)
            && total.is_some_and(|size| size <= experiment.limits.max_artifact_bytes),
        "The initial observation exceeds this experiment's storage limit.",
    )?;
    Ok((bytes, assets))
}

fn short_reason(reason: String) -> String {
    let mut value = reason.split_whitespace().collect::<Vec<_>>().join(" ");
    if value.is_empty() {
        value = "The run couldn't continue".into();
    }
    let mut end = value.len().min(1_024);
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    value.truncate(end);
    value
}

fn recorded_step(
    index: u32,
    timestep: f64,
    state: &Snapshot,
    sensors: &BTreeMap<String, Sensor>,
    actions: BTreeMap<String, f64>,
    observation_step: Option<u32>,
    inference_ms: Option<f64>,
) -> Step {
    Step {
        index,
        time_seconds: f64::from(index) * timestep,
        joints: state.joints.clone(),
        actions,
        observation_step,
        objects: state.objects.clone(),
        contacts: state.contacts.clone(),
        sensors: sensors.clone(),
        inference_ms,
        limit_violations: state.limit_violations.clone(),
    }
}

/// State-only bounded baseline: an authored sequence of joint targets.
/// A sequence is pinned by its serialized content hash in `build`.
pub struct Scripted {
    build: Build,
    frames: Vec<BTreeMap<String, f64>>,
    hold_control_intervals: u32,
    at: usize,
    held: u32,
}

impl Scripted {
    pub fn new(frames: Vec<BTreeMap<String, f64>>, hold_control_intervals: u32) -> Result<Self> {
        require(
            !frames.is_empty() && frames.len() <= 1_024 && hold_control_intervals > 0,
            "Use a bounded scripted joint sequence.",
        )?;
        let names: Vec<_> = frames[0].keys().collect();
        require(
            !names.is_empty()
                && names.len() <= 64
                && frames.iter().all(|frame| {
                    frame.keys().collect::<Vec<_>>() == names
                        && frame.values().all(|n| n.is_finite())
                }),
            "Use the same finite named joints in every scripted frame.",
        )?;
        let bytes = serde_json::to_vec(&(&frames, hold_control_intervals))?;
        let build = Build {
            name: "robot-scripted-v1".into(),
            sha256: robot_lab::store::digest(&bytes),
        };
        Ok(Self {
            build,
            frames,
            hold_control_intervals,
            at: 0,
            held: 0,
        })
    }
}

impl Controller for Scripted {
    fn build(&self) -> Build {
        self.build.clone()
    }
    fn reset(&mut self, _: u64) -> Result<()> {
        self.at = 0;
        self.held = 0;
        Ok(())
    }
    fn infer(&mut self, observation: Observation<'_>) -> Result<ActionChunk> {
        let actions = self.frames[self.at].clone();
        self.held += 1;
        if self.held >= self.hold_control_intervals {
            self.at = (self.at + 1).min(self.frames.len() - 1);
            self.held = 0;
        }
        Ok(ActionChunk {
            observation_step: observation.step,
            expires_step: observation.step,
            actions: vec![actions],
        })
    }
}
