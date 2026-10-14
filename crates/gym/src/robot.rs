//! Robotics evidence is separate from decision-door questions and scores.
//! Every result is recomputed from the pinned physical episode and case.
use robot_lab::document::{Build, CubeTask};
use robot_lab::{Document, Reference, Store as Lab};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

pub const ROW_SCHEMA: &str = "openagents.gym.robot_row.v1";
pub const SUITE_SCHEMA: &str = "openagents.gym.robot_suite.v1";
pub use crate::robot_metric::{Metrics, POSITION_TOLERANCE_METERS, score};
pub const TRIAL_FIELDS: &[&str] = &["suite_digest", "partition", "controller", "seed"];
pub type Result<T> = std::result::Result<T, String>;
pub(crate) fn check(valid: bool, message: &str) -> Result<()> {
    if valid { Ok(()) } else { Err(message.into()) }
}
pub fn digest(value: &impl Serialize) -> Result<String> {
    let bytes = serde_json::to_vec(value).map_err(|e| e.to_string())?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}
pub fn metric_build() -> Build {
    Build {
        name: "cube-placement-aabb-release-stable-v1".into(),
        sha256: format!("{:x}", Sha256::digest(include_bytes!("robot_metric.rs"))),
    }
}

fn supported_metric(build: &Build) -> bool {
    // The first build used this identical scoring function inside robot.rs.
    // Keep its saved evidence readable when browsing or transports change.
    build.name == metric_build().name
        && (build.sha256 == metric_build().sha256
            || build.sha256 == "3da2a71a0700c560cca5d866367ea09fcb90f542497c02845de1d80da2812570")
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
                && supported_metric(&self.metric)
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
        | robot_lab::document::Controller::Service { build, .. }
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
