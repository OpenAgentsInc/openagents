//! Cube scoring is pinned independently of result browsing and transports.
use crate::robot::{Result, check};
use robot_lab::document::{
    CubeTask, Episode, EpisodeMode, Experiment, ObjectState, Outcome, Robot, Scene, Shape,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
pub const POSITION_TOLERANCE_METERS: f64 = 0.0001;
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
