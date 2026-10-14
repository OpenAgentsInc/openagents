//! Live, unsaved physics for the local player's Simulate toggle. One thread
//! owns the native model; requests use `record`'s pose and step commands with
//! the same servo profile, timestep, joint limits, and 240-step bound.
use crate::host::Host;
use crate::record::Command;
use robot_lab::Reference;
use robot_lab::document::Step;
use robot_sim::mujoco::{LibraryPin, Model, Runtime, Servo};
use robot_sim::runner::Backend;
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::sync::mpsc;

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum Request {
    Start { start: Reference },
    Command(Command),
}

type Job = (Request, mpsc::Sender<Result<Value, String>>);

/// The live scene: its model, robot limits, current targets, and step count.
struct Live {
    model: Model,
    robot: robot_lab::document::Robot,
    target: BTreeMap<String, f64>,
    index: u32,
}

#[derive(Clone)]
pub struct Sim {
    jobs: mpsc::Sender<Job>,
}

impl Sim {
    pub fn new(host: Host, pin: LibraryPin) -> Self {
        let (jobs, receive) = mpsc::channel::<Job>();
        std::thread::spawn(move || {
            let mut live: Option<Live> = None;
            while let Ok((request, reply)) = receive.recv() {
                let _ = reply.send(handle(&host, &pin, &mut live, request));
            }
        });
        Self { jobs }
    }
    pub fn run(&self, request: Request) -> Result<Value, String> {
        let (reply, answer) = mpsc::channel();
        self.jobs
            .send((request, reply))
            .map_err(|_| "The simulation stopped. Restart the lab")?;
        answer
            .recv()
            .map_err(|_| "The simulation stopped. Restart the lab")?
    }
}

fn handle(
    host: &Host,
    pin: &LibraryPin,
    live: &mut Option<Live>,
    request: Request,
) -> Result<Value, String> {
    match request {
        Request::Start { start } => {
            *live = None;
            let bundle = host.bundle(&start)?;
            if bundle.episode.is_some() {
                return Err("Choose a scene to simulate".into());
            }
            let checked = robot_sim::import::pinned_urdf(&host.store, &host.catalog)
                .map_err(|e| e.to_string())?;
            if bundle.robot != checked {
                return Err("Import the checked robot again before simulating".into());
            }
            let runtime = Runtime::open(pin, host.store.root()).map_err(|e| e.to_string())?;
            let model = runtime
                .arm_scene(
                    &host.catalog,
                    &bundle.robot.description,
                    &bundle.scene,
                    1. / 240.,
                    Servo {
                        kp: 100.,
                        kd: 4.,
                        effort: 2.4,
                        speed: 2.,
                    },
                )
                .map_err(|e| e.to_string())?;
            let state = model.snapshot().map_err(|e| e.to_string())?;
            let target = state.joints.clone();
            let next = Live {
                model,
                robot: bundle.robot,
                target,
                index: 0,
            };
            let value = step_value(&next, state);
            *live = Some(next);
            Ok(value)
        }
        Request::Command(Command::Finish) => {
            *live = None;
            Ok(json!({"finished":true}))
        }
        Request::Command(command) => {
            let live = live.as_mut().ok_or("Turn on Simulate first")?;
            let (joints, steps) = match command {
                Command::Pose { joints, steps } => (joints, steps),
                Command::Step { steps } => (BTreeMap::new(), steps),
                Command::Finish => unreachable!(),
            };
            if !(1..=240).contains(&steps) {
                return Err("Advance between 1 and 240 simulation steps".into());
            }
            let mut target = live.target.clone();
            target.extend(joints);
            live.robot
                .validate_pose(&target)
                .map_err(|e| e.to_string())?;
            live.target = target;
            let mut state = None;
            for _ in 0..steps {
                live.model.apply(&live.target).map_err(|e| e.to_string())?;
                state = Some(live.model.step().map_err(|e| e.to_string())?);
                live.index += 1;
            }
            Ok(step_value(live, state.unwrap_or_default()))
        }
    }
}

/// The same step document a recording saves, so the player draws it unchanged.
fn step_value(live: &Live, state: robot_sim::runner::Snapshot) -> Value {
    let step = Step {
        index: live.index,
        time_seconds: live.model.time(),
        joints: state.joints,
        actions: live.target.clone(),
        observation_step: None,
        objects: state.objects,
        contacts: state.contacts,
        sensors: BTreeMap::new(),
        inference_ms: None,
        limit_violations: state.limit_violations,
    };
    json!({ "step": step })
}
