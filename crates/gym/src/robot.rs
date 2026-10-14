//! Robotics evidence is separate from decision-door questions and scores.
//! Every result is recomputed from the pinned physical episode and case.
use robot_lab::document::{
    Build, CubeTask, Episode, EpisodeMode, Experiment, ObjectState, Outcome, Robot, Scene, Shape,
};
use robot_lab::{Document, Reference, Store as Lab};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

pub const ROW_SCHEMA: &str = "openagents.gym.robot_row.v1";
pub const SUITE_SCHEMA: &str = "openagents.gym.robot_suite.v1";
pub const POSITION_TOLERANCE_METERS: f64 = 0.0001;
pub const TRIAL_FIELDS: &[&str] = &["suite_digest", "partition", "controller", "seed"];
pub type Result<T> = std::result::Result<T, String>;
fn check(valid: bool, message: &str) -> Result<()> {
    if valid { Ok(()) } else { Err(message.into()) }
}
pub fn digest(value: &impl Serialize) -> Result<String> {
    let bytes = serde_json::to_vec(value).map_err(|e| e.to_string())?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}
pub fn metric_build() -> Build {
    Build {
        name: "cube-placement-aabb-release-stable-v1".into(),
        sha256: format!("{:x}", Sha256::digest(include_bytes!("robot.rs"))),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Partition {
    Training,
    Validation,
    HeldOut,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Case {
    pub seed: u64,
    pub scene: Reference,
    pub backend: Build,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Suite {
    pub schema: String,
    pub name: String,
    pub scene_template: Reference,
    pub task: CubeTask,
    pub metric: Build,
    pub timestep_seconds: f64,
    pub control_every_steps: u32,
    pub sensor_every_steps: u32,
    pub max_steps: u32,
    pub cases: BTreeMap<Partition, Vec<Case>>,
    /// Seeds used to train or tune a named controller cannot be held out.
    pub exposure: BTreeMap<String, Vec<u64>>,
    pub digest: String,
}
impl Suite {
    pub fn compute_digest(&self) -> Result<String> {
        digest(&(
            &self.schema,
            &self.scene_template,
            &self.task,
            &self.metric,
            self.timestep_seconds,
            self.control_every_steps,
            self.sensor_every_steps,
            self.max_steps,
            &self.cases,
            &self.exposure,
        ))
    }
    pub fn validate(&self) -> Result<()> {
        check(
            self.schema == SUITE_SCHEMA
                && !self.name.is_empty()
                && self.name.len() <= 128
                && self.metric == metric_build()
                && self.digest == self.compute_digest()?,
            "Choose a checked robot suite with its saved scoring rule.",
        )?;
        self.scene_template.validate().map_err(|e| e.to_string())?;
        self.task.version.validate().map_err(|e| e.to_string())?;
        check(
            self.timestep_seconds.is_finite()
                && (1e-6..=1.).contains(&self.timestep_seconds)
                && (2..=100_000).contains(&self.max_steps)
                && self.control_every_steps > 0
                && self.control_every_steps < self.max_steps
                && self.sensor_every_steps > 0
                && self.sensor_every_steps < self.max_steps,
            "Use bounded robot-suite steps and sampling intervals.",
        )?;
        let mut seeds = BTreeSet::new();
        for cases in self.cases.values() {
            for case in cases {
                check(
                    seeds.insert(case.seed) && seeds.len() <= 1000,
                    "Keep training, validation, and held-out seeds separate.",
                )?;
                case.scene.validate().map_err(|e| e.to_string())?;
                case.backend.validate().map_err(|e| e.to_string())?;
            }
        }
        check(
            !seeds.is_empty()
                && self.exposure.len() <= 64
                && self.exposure.iter().all(|(controller, seeds)| {
                    controller.len() == 64
                        && controller.bytes().all(|b| b.is_ascii_hexdigit())
                        && seeds.len() <= 1000
                }),
            "Name bounded robot cases and declared training exposure.",
        )
    }
    pub fn case(&self, partition: Partition, seed: u64) -> Result<&Case> {
        self.cases
            .get(&partition)
            .and_then(|cases| cases.iter().find(|case| case.seed == seed))
            .ok_or("Choose a seed saved in this suite partition".into())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Metrics {
    pub success: bool,
    pub completion_seconds: Option<f64>,
    pub simulation_seconds: f64,
    pub collision_events: u64,
    pub robot_collision_events: u64,
    pub limit_violation_steps: u64,
    pub inference_calls: u64,
    pub inference_ms: Vec<f64>,
    pub failure: Option<String>,
}
fn inside(state: &ObjectState, size: [f64; 3], task: &CubeTask) -> bool {
    let rotation = glam::DQuat::from_array(state.pose.rotation);
    let center = glam::DVec3::from_array(state.pose.position);
    for x in [-0.5, 0.5] {
        for y in [-0.5, 0.5] {
            for z in [-0.5, 0.5] {
                let point =
                    center + rotation * glam::DVec3::new(size[0] * x, size[1] * y, size[2] * z);
                if (0..3).any(|i| {
                    point[i] < task.target_min[i] - POSITION_TOLERANCE_METERS
                        || point[i] > task.target_max[i] + POSITION_TOLERANCE_METERS
                }) {
                    return false;
                }
            }
        }
    }
    true
}
/// Success needs the whole oriented cube inside the target, no robot
/// contact, bounded speeds, and a continuous stable interval before expiry.
pub fn score(
    scene: &Scene,
    robot: &Robot,
    experiment: &Experiment,
    episode: &Episode,
) -> Result<Metrics> {
    scene.validate().map_err(|e| e.to_string())?;
    robot.validate().map_err(|e| e.to_string())?;
    experiment.validate().map_err(|e| e.to_string())?;
    episode.validate().map_err(|e| e.to_string())?;
    check(
        episode.mode == EpisodeMode::Simulation && !matches!(episode.outcome, Outcome::InProgress),
        "Finish a physics-controlled attempt before scoring it. Demonstrations cannot be scored.",
    )?;
    check(
        episode.steps.len() <= experiment.limits.max_steps as usize
            && experiment.seeds.contains(&episode.seed)
            && episode.steps.iter().all(|s| {
                (s.time_seconds - f64::from(s.index) * experiment.timestep_seconds).abs() <= 1e-9
            }),
        "Score the saved experiment's exact physical steps and seed.",
    )?;
    let task = &experiment.task;
    let object = scene
        .objects
        .iter()
        .find(|o| o.id == task.object)
        .ok_or("This task's cube is missing")?;
    let Some(Shape::Box { size }) = &object.collider else {
        return Err("Score cube placement with a box collision shape".into());
    };
    check(
        object.mass > 0.,
        "Use a moving physical cube for this task.",
    )?;
    let links: BTreeSet<_> = robot.links.iter().map(|l| l.name.as_str()).collect();
    let mut stable_since = None;
    let mut completion = None;
    let mut collision_events = 0;
    let mut robot_collision_events = 0;
    let mut previous = BTreeSet::new();
    let mut limits = 0;
    let mut inference_ms = Vec::new();
    for step in &episode.steps {
        let state = step
            .objects
            .get(&task.object)
            .ok_or("A recorded cube state is missing")?;
        let mut contacts = BTreeSet::new();
        let mut touching = false;
        for contact in &step.contacts {
            let (a, b) = if contact.a <= contact.b {
                (&contact.a, &contact.b)
            } else {
                (&contact.b, &contact.a)
            };
            contacts.insert((a.clone(), b.clone()));
            if (contact.a == task.object && links.contains(contact.b.as_str()))
                || (contact.b == task.object && links.contains(contact.a.as_str()))
            {
                touching = true;
            }
        }
        for (a, b) in contacts.difference(&previous) {
            collision_events += 1;
            if links.contains(a.as_str()) || links.contains(b.as_str()) {
                robot_collision_events += 1;
            }
        }
        previous = contacts;
        if !step.limit_violations.is_empty() {
            limits += 1;
        }
        if let Some(latency) = step.inference_ms {
            inference_ms.push(latency);
        }
        let stable = inside(state, *size, task)
            && !state.held
            && !touching
            && glam::DVec3::from_array(state.linear_velocity).length() <= task.max_linear_speed
            && glam::DVec3::from_array(state.angular_velocity).length() <= task.max_angular_speed;
        if stable {
            let since = *stable_since.get_or_insert(step.time_seconds);
            if step.time_seconds - since + 1e-9 >= task.stable_seconds && completion.is_none() {
                completion = Some(step.time_seconds);
            }
        } else {
            stable_since = None;
        }
    }
    let last = episode.steps.last().unwrap();
    let success = completion.is_some();
    let failure = if success {
        None
    } else {
        Some(match &episode.outcome {
            Outcome::Failed { reason } | Outcome::Stopped { reason } => reason.clone(),
            _ => "The cube didn't settle in the target".into(),
        })
    };
    Ok(Metrics {
        success,
        completion_seconds: completion,
        simulation_seconds: last.time_seconds,
        collision_events,
        robot_collision_events,
        limit_violation_steps: limits,
        inference_calls: inference_ms.len() as u64,
        inference_ms,
        failure,
    })
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Row {
    pub schema: String,
    pub suite_digest: String,
    pub suite_name: String,
    pub partition: Partition,
    pub controller: Build,
    pub seed: u64,
    pub scene: Reference,
    pub backend: Build,
    pub metric: Build,
    pub experiment: Reference,
    pub episode: Reference,
    pub metrics: Metrics,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_receipt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub receipt: Option<String>,
}
pub fn row(lab: &Lab, suite: &Suite, partition: Partition, subject: &Reference) -> Result<Row> {
    suite.validate()?;
    let Document::Episode(episode) = lab.get(subject).map_err(|e| e.to_string())?.content else {
        return Err("Choose a saved robot attempt".into());
    };
    let case = suite.case(partition, episode.seed)?;
    let Document::Experiment(experiment) = lab
        .get(&episode.experiment)
        .map_err(|e| e.to_string())?
        .content
    else {
        return Err("The attempt's experiment is missing".into());
    };
    check(
        experiment.scene == case.scene
            && experiment.backend == case.backend
            && experiment.task == suite.task
            && experiment.timestep_seconds == suite.timestep_seconds
            && experiment.control_every_steps == suite.control_every_steps
            && experiment.sensor_every_steps == suite.sensor_every_steps
            && experiment.limits.max_steps == suite.max_steps,
        "This attempt used different scene, simulator, task, or timing settings.",
    )?;
    let controller = match &experiment.controller {
        robot_lab::document::Controller::Manual { build, .. }
        | robot_lab::document::Controller::Scripted { build, .. }
        | robot_lab::document::Controller::Policy { build, .. } => build.clone(),
    };
    if partition == Partition::HeldOut {
        check(
            !suite
                .exposure
                .get(&controller.sha256)
                .is_some_and(|seeds| seeds.contains(&episode.seed)),
            "This controller trained or tuned on a held-out case. Use a separate test set.",
        )?;
    }
    let Document::Scene(scene) = lab.get(&case.scene).map_err(|e| e.to_string())?.content else {
        return Err("The attempt's scene is missing".into());
    };
    check(scene.robots.len() == 1, "Score a scene with one robot")?;
    let Document::Robot(robot) = lab
        .get(&scene.robots[0].robot)
        .map_err(|e| e.to_string())?
        .content
    else {
        return Err("The attempt's robot is missing".into());
    };
    let metrics = score(&scene, &robot, &experiment, &episode)?;
    Ok(Row {
        schema: ROW_SCHEMA.into(),
        suite_digest: suite.digest.clone(),
        suite_name: suite.name.clone(),
        partition,
        controller,
        seed: episode.seed,
        scene: case.scene.clone(),
        backend: case.backend.clone(),
        metric: suite.metric.clone(),
        experiment: episode.experiment,
        episode: subject.clone(),
        metrics,
        previous_receipt: None,
        receipt: None,
    })
}
pub fn verify_rows(lab: &Lab, suite: &Suite, path: &Path) -> Result<Vec<Row>> {
    let rows = crate::store::Store::at(path)
        .verified_rows()
        .map_err(|e| e.to_string())?;
    let mut output = Vec::new();
    for value in rows {
        check(
            value["schema"] == ROW_SCHEMA,
            "Choose a store of robotics results",
        )?;
        let stored: Row = serde_json::from_value(value).map_err(|e| e.to_string())?;
        check(
            stored.suite_digest == suite.digest,
            "Read results against the exact suite they used",
        )?;
        let computed = row(lab, suite, stored.partition, &stored.episode)?;
        check(
            stored.controller == computed.controller
                && stored.seed == computed.seed
                && stored.scene == computed.scene
                && stored.backend == computed.backend
                && stored.experiment == computed.experiment
                && stored.metric == computed.metric
                && stored.metrics == computed.metrics,
            "A recorded robot result disagrees with its saved physical episode.",
        )?;
        output.push(stored);
    }
    Ok(output)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Summary {
    pub controller: Build,
    pub partition: Partition,
    pub expected: u64,
    pub recorded: u64,
    pub successes: u64,
    pub failures: u64,
    pub missing_seeds: Vec<u64>,
    pub success_rate: Option<f64>,
    pub interval_95: Option<[f64; 2]>,
    pub collision_events: u64,
    pub limit_violation_steps: u64,
    pub mean_inference_ms: Option<f64>,
    pub attempts: Vec<Row>,
}
pub fn summary(
    suite: &Suite,
    partition: Partition,
    controller: &Build,
    rows: &[Row],
) -> Result<Summary> {
    suite.validate()?;
    let cases = suite
        .cases
        .get(&partition)
        .ok_or("Choose a partition in this robot suite")?;
    let selected = rows
        .iter()
        .filter(|r| {
            r.suite_digest == suite.digest
                && r.partition == partition
                && r.controller == *controller
        })
        .cloned()
        .collect::<Vec<_>>();
    let mut seeds = BTreeSet::new();
    for row in &selected {
        check(
            seeds.insert(row.seed) && cases.iter().any(|c| c.seed == row.seed),
            "Use each saved robot case once in a comparison",
        )?;
    }
    let missing = cases
        .iter()
        .filter(|c| !seeds.contains(&c.seed))
        .map(|c| c.seed)
        .collect::<Vec<_>>();
    let successes = selected.iter().filter(|r| r.metrics.success).count() as u64;
    let recorded = selected.len() as u64;
    let complete = missing.is_empty() && recorded > 0;
    let rate = complete.then(|| successes as f64 / recorded as f64);
    let interval = rate.map(|p| wilson(p, recorded));
    let latencies = selected
        .iter()
        .flat_map(|r| r.metrics.inference_ms.iter())
        .copied()
        .collect::<Vec<_>>();
    Ok(Summary {
        controller: controller.clone(),
        partition,
        expected: cases.len() as u64,
        recorded,
        successes,
        failures: recorded - successes,
        missing_seeds: missing,
        success_rate: rate,
        interval_95: interval,
        collision_events: selected.iter().map(|r| r.metrics.collision_events).sum(),
        limit_violation_steps: selected
            .iter()
            .map(|r| r.metrics.limit_violation_steps)
            .sum(),
        mean_inference_ms: (!latencies.is_empty())
            .then(|| latencies.iter().sum::<f64>() / latencies.len() as f64),
        attempts: selected,
    })
}
fn wilson(p: f64, n: u64) -> [f64; 2] {
    let n = n as f64;
    let z = 1.959963984540054;
    let center = (p + z * z / (2. * n)) / (1. + z * z / n);
    let radius = z * (p * (1. - p) / n + z * z / (4. * n * n)).sqrt() / (1. + z * z / n);
    [(center - radius).max(0.), (center + radius).min(1.)]
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Comparison {
    pub suite_digest: String,
    pub partition: Partition,
    pub left: Summary,
    pub right: Summary,
    pub paired_seeds: u64,
    pub paired_difference: Option<f64>,
    pub left_only_successes: u64,
    pub right_only_successes: u64,
    pub conclusion: String,
}
pub fn compare(
    suite: &Suite,
    partition: Partition,
    left: &Build,
    right: &Build,
    rows: &[Row],
) -> Result<Comparison> {
    check(left != right, "Choose two different controller builds")?;
    let a = summary(suite, partition, left, rows)?;
    let b = summary(suite, partition, right, rows)?;
    let mut paired = 0;
    let mut left_only = 0;
    let mut right_only = 0;
    for left in &a.attempts {
        if let Some(right) = b.attempts.iter().find(|r| r.seed == left.seed) {
            check(
                left.scene == right.scene
                    && left.backend == right.backend
                    && left.metric == right.metric,
                "Compare controllers on identical physical cases",
            )?;
            paired += 1;
            if left.metrics.success && !right.metrics.success {
                left_only += 1;
            }
            if right.metrics.success && !left.metrics.success {
                right_only += 1;
            }
        }
    }
    let complete = a.success_rate.is_some() && b.success_rate.is_some();
    let difference = complete.then(|| b.success_rate.unwrap() - a.success_rate.unwrap());
    let conclusion = if !complete {
        "Some attempts are missing"
    } else if left_only == 0 && right_only == 0 {
        "Both controllers had the same outcomes"
    } else {
        "Recorded outcomes differ; inspect the attempts and intervals"
    };
    Ok(Comparison {
        suite_digest: suite.digest.clone(),
        partition,
        left: a,
        right: b,
        paired_seeds: paired,
        paired_difference: difference,
        left_only_successes: left_only,
        right_only_successes: right_only,
        conclusion: conclusion.into(),
    })
}
