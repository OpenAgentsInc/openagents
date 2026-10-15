//! Assembly as a plan run in checked stages: plan, simulate, check, act
//! (docs/research/aether-ai/README.md and
//! docs/spacecraft-design/autonomy-and-environment.md).
//!
//! - **Plan:** [`Plan`] is a typed document listing, for each part in
//!   precedence order, which arm places it and on what. The planner checks
//!   reach and clearance at every waypoint before the plan exists.
//! - **Simulate:** before a stage moves anything, the sequencer runs that
//!   stage on a copy of the factory, with the same physics, and refuses the
//!   stage if the copy hits a limit: reach, a joint's motor, a collision,
//!   the CMGs' momentum, or the factory's attitude box.
//! - **Act:** the real stage runs with the robot closing every fast loop
//!   itself: tracking, the latch envelope, the step back from a latch that
//!   will not seat, and resuming after a controller reset.
//! - **Check:** after the stage, the part must be latched, inside its pose
//!   tolerance, hold a proof load, and match the prediction; the factory
//!   must be inside its attitude box. A failed check makes safe; nothing
//!   pushes on.
//!
//! The ground supervises at the level of stages: the plan must be approved,
//! and no new stage starts while the ground link is down. The current stage
//! finishes, and the sequence resumes on reconnect.

use glam::DVec3;
use serde::{Deserialize, Serialize};

use crate::arm::{self, Pose};
use crate::factory::{CLEAR, DT, Factory, Holder};

/// The plan document's schema tag.
pub const PLAN_SCHEMA: &str = "assembly-plan.v1";
/// Latch capture envelope: position, m; attitude, rad; closing speed, m/s.
pub const LATCH_POSITION: f64 = 0.03;
pub const LATCH_ANGLE: f64 = 0.017;
pub const LATCH_SPEED: f64 = 0.01;
/// Latch attempts after the first before making safe.
pub const MAX_RETRIES: u32 = 2;
/// The factory's attitude box, rad.
pub const ATTITUDE_BOX: f64 = 0.035;
/// A joint further than this from its reference cannot follow it, rad.
pub const TRACKING_LIMIT: f64 = 0.05;
/// A motor at its effort limit this long cannot do the move, s.
pub const SATURATION_LIMIT: f64 = 3.0;
/// The proof load each latch must hold, N.
pub const PROOF_LOAD: f64 = 5_000.0;
/// Pose tolerance after latching: m and rad.
pub const POSE_TOLERANCE: (f64, f64) = (0.005, 0.0035);
/// Prediction steps per real step while a stage is being simulated.
pub const PREDICT_RATE: usize = 8;

/// One stage of the plan.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlannedStage {
    pub part: String,
    /// Arm index, from zero.
    pub arm: usize,
    /// The part it latches to, or `None` for the cradle.
    pub parent: Option<String>,
}

/// A typed, versioned assembly plan for one ship revision.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Plan {
    pub schema: String,
    pub ship: String,
    /// The ship document's content version this plan was made for.
    pub ship_version: String,
    pub stages: Vec<PlannedStage>,
}

/// A named failure: what stopped a stage, with the arm, joint, or bodies.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "failure", rename_all = "snake_case")]
pub enum Failure {
    OutOfReach {
        part: String,
        arm: usize,
        detail: String,
    },
    Collision {
        a: String,
        b: String,
    },
    MotorLimit {
        arm: usize,
        joint: String,
    },
    CmgSaturated,
    AttitudeOutOfBox {
        degrees: f64,
    },
    GraspMissed {
        part: String,
        detail: String,
    },
    LatchDidNotSeat {
        part: String,
        attempts: u32,
    },
    ProofLoad {
        part: String,
        detail: String,
    },
    CheckFailed {
        part: String,
        detail: String,
    },
}

impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OutOfReach { part, arm, detail } => {
                write!(f, "arm {} cannot reach {part}: {detail}", arm + 1)
            }
            Self::Collision { a, b } => write!(f, "collision: {a} would hit {b}"),
            Self::MotorLimit { arm, joint } => {
                write!(
                    f,
                    "motor limit: arm {} {joint} cannot follow its move",
                    arm + 1
                )
            }
            Self::CmgSaturated => write!(f, "the CMGs ran out of momentum"),
            Self::AttitudeOutOfBox { degrees } => {
                write!(
                    f,
                    "the factory turned {degrees:.2}° out of its attitude box"
                )
            }
            Self::GraspMissed { part, detail } => write!(f, "could not grasp {part}: {detail}"),
            Self::LatchDidNotSeat { part, attempts } => {
                write!(f, "{part}'s latch did not seat after {attempts} attempts")
            }
            Self::ProofLoad { part, detail } => write!(f, "{part} failed its proof load: {detail}"),
            Self::CheckFailed { part, detail } => write!(f, "{part} failed its check: {detail}"),
        }
    }
}

/// Faults injected into the real factory, never into the prediction.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Faults {
    /// A part whose latch fails to engage this many times.
    pub stuck_latch: Option<(String, u32)>,
    /// Ground link outages, (from, to) in seconds of factory time.
    pub link_loss: Vec<(f64, f64)>,
    /// Controller resets: (arm, time).
    pub resets: Vec<(usize, f64)>,
}

impl Faults {
    #[must_use]
    pub fn link_up(&self, t: f64) -> bool {
        !self.link_loss.iter().any(|&(a, b)| t >= a && t < b)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Phase {
    ToRack,
    Descend,
    Grasp,
    Lift,
    Transfer,
    Lower,
    Insert,
    Latch,
    StepBack,
    Release,
    Proof,
    Retreat,
    Home,
    Done,
}

impl Phase {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::ToRack => "to the rack",
            Self::Descend => "onto the fixture",
            Self::Grasp => "grasping",
            Self::Lift => "lifting",
            Self::Transfer => "carrying",
            Self::Lower => "lowering",
            Self::Insert => "inserting",
            Self::Latch => "latching",
            Self::StepBack => "stepping back",
            Self::Release => "releasing",
            Self::Proof => "proof load",
            Self::Retreat => "retreating",
            Self::Home => "parking",
            Self::Done => "done",
        }
    }
}

/// Joint targets for one stage, factory frame.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Waypoints {
    pub above_rack: Pose,
    /// Over the rack at the transit height.
    pub lift: Pose,
    /// Over the part's place at the transit height.
    pub carry: Pose,
    pub at_rack: Pose,
    pub above_target: Pose,
    pub at_target: Pose,
    pub home: Pose,
}

/// One stage in progress on one factory.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StageRun {
    pub stage: usize,
    /// Index into the factory's parts.
    pub part: usize,
    pub arm: usize,
    pub phase: Phase,
    pub phase_start: f64,
    started: bool,
    pub attempts: u32,
    epoch: u32,
    pub waypoints: Waypoints,
    proof_start: DVec3,
    ticks: u64,
}

/// Something worth telling the ground.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Event {
    pub time: f64,
    pub stage: Option<usize>,
    pub text: String,
}

/// Where the sequence stands.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Status {
    /// Waiting for the ground to approve the plan.
    AwaitingApproval,
    /// Simulating the next stage before committing it.
    Predicting,
    /// Running a stage on the real factory.
    Acting,
    /// At a hold point: no new stage starts.
    Holding(String),
    /// Every stage passed.
    Complete,
    /// Arms stopped, parts held, attitude held, waiting for the ground.
    Safe(Failure),
    /// The prediction found a limit; the stage never moved.
    Refused(Failure),
}

/// The predicted end of a stage.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Prediction {
    pub part_center: DVec3,
    pub attitude_error: f64,
    pub cmg_peak: f64,
    pub duration: f64,
}

/// A stage being simulated: the copy, its run, when it started, and the
/// peak CMG momentum so far.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StageSimulation {
    pub copy: Factory,
    pub run: StageRun,
    pub started: f64,
    pub peak: f64,
}

/// The sequencer: a plan, the real factory, and the stage being predicted
/// or run.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Assembly {
    pub factory: Factory,
    pub plan: Plan,
    pub faults: Faults,
    pub status: Status,
    /// The next stage to run (or the one running).
    pub stage: usize,
    pub run: Option<StageRun>,
    /// The copy being simulated for the next stage.
    pub predicting: Option<Box<StageSimulation>>,
    pub prediction: Option<Prediction>,
    pub events: Vec<Event>,
    /// Stage count passed; the first `passed` stages are checked.
    pub passed: usize,
    /// Peak CMG momentum in the current run, N m s.
    pub cmg_peak: f64,
    /// Prediction steps per real step while a stage is being simulated.
    pub predict_rate: usize,
}

impl Plan {
    /// Plan the factory's ship: stages in precedence order (each part after
    /// its parent), each with the arm the factory assigned, after checking
    /// reach at every waypoint and clearance where the part is placed.
    ///
    /// # Errors
    /// A part is out of reach or its place collides.
    pub fn new(factory: &Factory) -> Result<Self, Failure> {
        let mut stages = Vec::new();
        for (i, p) in factory.parts.iter().enumerate() {
            let part = &factory.ship.parts[p.index];
            waypoints(factory, i)?;
            stages.push(PlannedStage {
                part: part.id.clone(),
                arm: p.arm,
                parent: part.attach.parent.clone(),
            });
        }
        Ok(Self {
            schema: PLAN_SCHEMA.into(),
            ship: factory.ship.name.clone(),
            ship_version: factory.ship.version(),
            stages,
        })
    }

    /// Whether every part comes after the part it latches to.
    #[must_use]
    pub fn respects_precedence(&self) -> bool {
        self.stages
            .iter()
            .enumerate()
            .all(|(i, s)| match &s.parent {
                None => true,
                Some(parent) => self.stages[..i].iter().any(|t| &t.part == parent),
            })
    }
}

/// The stage's waypoints from inverse kinematics.
///
/// # Errors
/// A waypoint is out of reach.
pub fn waypoints(factory: &Factory, part: usize) -> Result<Waypoints, Failure> {
    let p = &factory.parts[part];
    let spec = &factory.arms[p.arm].spec;
    let id = &factory.ship.parts[p.index].id;
    let solve = |tool: DVec3, yaw: f64| {
        spec.solve(tool, yaw).map_err(|detail| Failure::OutOfReach {
            part: id.clone(),
            arm: p.arm,
            detail,
        })
    };
    let rack = factory.grapple(part, p.rack);
    let target = factory.grapple(part, p.design);
    // The tool height that carries the part's underside at the transit
    // height.
    let transit = crate::factory::TRANSIT + 2.0 * p.half.y;
    Ok(Waypoints {
        above_rack: solve(rack + DVec3::Y * CLEAR, p.rack_yaw)?,
        lift: solve(rack.with_y(transit), p.rack_yaw)?,
        carry: solve(target.with_y(transit), p.design_yaw)?,
        at_rack: solve(rack, p.rack_yaw)?,
        above_target: solve(target + DVec3::Y * CLEAR, p.design_yaw)?,
        at_target: solve(target, p.design_yaw)?,
        home: factory.arms[p.arm].home,
    })
}

impl StageRun {
    /// Start stage `stage` (placing factory part `part`).
    ///
    /// # Errors
    /// A waypoint is out of reach.
    pub fn new(factory: &Factory, stage: usize, part: usize) -> Result<Self, Failure> {
        let arm = factory.parts[part].arm;
        Ok(Self {
            stage,
            part,
            arm,
            phase: Phase::ToRack,
            phase_start: factory.time(),
            started: false,
            attempts: 0,
            epoch: factory.arms[arm].epoch,
            waypoints: waypoints(factory, part)?,
            proof_start: DVec3::ZERO,
            ticks: 0,
        })
    }

    fn name(&self, f: &Factory) -> String {
        f.ship.parts[f.parts[self.part].index].id.clone()
    }

    fn next(&mut self, f: &Factory, phase: Phase) {
        self.phase = phase;
        self.phase_start = f.time();
        self.started = false;
    }

    /// The motion phase's target and pace.
    fn motion(&self) -> Option<(Pose, f64)> {
        let w = &self.waypoints;
        Some(match self.phase {
            Phase::ToRack => (w.above_rack, 1.0),
            Phase::Lift => (w.lift, 0.6),
            Phase::Transfer => (w.carry, 1.0),
            Phase::Lower => (w.above_target, 0.6),
            Phase::Descend => (w.at_rack, 0.3),
            Phase::StepBack | Phase::Retreat => (w.above_target, 1.0),
            Phase::Insert => (w.at_target, 0.3),
            Phase::Home => (w.home, 1.0),
            _ => return None,
        })
    }

    /// Advance the stage by one step of `f`, before `f.step()`. Returns
    /// whether the stage is done.
    ///
    /// # Errors
    /// A named failure stopped the stage.
    pub fn step(
        &mut self,
        f: &mut Factory,
        faults: &mut Faults,
        events: &mut Vec<Event>,
    ) -> Result<bool, Failure> {
        let t = f.time();
        let arm = self.arm;
        let stage = self.stage;
        let mut say = |text: String| {
            events.push(Event {
                time: t,
                stage: Some(stage),
                text,
            });
        };
        self.ticks += 1;
        // A controller restart resumes this phase from its checkpoint: the
        // part is still in the grapple (it latches through power loss), so
        // the move is planned again from where the arm stopped.
        if f.arms[arm].epoch != self.epoch {
            self.epoch = f.arms[arm].epoch;
            if self.motion().is_some() {
                self.started = false;
            }
            say(format!(
                "arm {} controller reset while {}; resuming from the last check",
                arm + 1,
                self.phase.label()
            ));
        }
        self.limits(f)?;
        if let Some((target, pace)) = self.motion() {
            if !self.started {
                f.move_arm(arm, target, pace);
                self.started = true;
                return Ok(false);
            }
            let motion = f.arms[arm].motion.ok_or_else(|| Failure::MotorLimit {
                arm,
                joint: "controller".into(),
            })?;
            if !motion.done(t) {
                return Ok(false);
            }
            let here = f.measured(arm);
            let settled = here.iter().zip(&target).all(|(a, b)| (a - b).abs() < 0.002);
            if !settled {
                if t > motion.start + motion.duration + 20.0 {
                    let worst = (0..5)
                        .max_by(|&a, &b| {
                            (here[a] - target[a])
                                .abs()
                                .total_cmp(&(here[b] - target[b]).abs())
                        })
                        .unwrap_or(0);
                    return Err(Failure::MotorLimit {
                        arm,
                        joint: arm::JOINTS[worst].into(),
                    });
                }
                return Ok(false);
            }
            let next = match self.phase {
                Phase::ToRack => Phase::Descend,
                Phase::Descend => Phase::Grasp,
                Phase::Lift => Phase::Transfer,
                Phase::Transfer => Phase::Lower,
                Phase::Lower => Phase::Insert,
                Phase::Insert => Phase::Latch,
                Phase::StepBack => Phase::Insert,
                Phase::Retreat => Phase::Home,
                _ => Phase::Done,
            };
            self.next(f, next);
            return Ok(next == Phase::Done);
        }
        let elapsed = t - self.phase_start;
        match self.phase {
            Phase::Grasp => {
                if !self.started {
                    let part = self.part;
                    f.grasp(arm, part).map_err(|detail| Failure::GraspMissed {
                        part: self.name(f),
                        detail,
                    })?;
                    self.started = true;
                    say(format!("arm {} has {}", arm + 1, self.name(f)));
                } else if elapsed > 2.0 {
                    self.next(f, Phase::Lift);
                }
            }
            Phase::Latch => {
                if !self.started {
                    let (position, angle) = f.placement_error(self.part);
                    let speed = f.relative_speed(self.part);
                    let in_envelope =
                        position < LATCH_POSITION && angle < LATCH_ANGLE && speed < LATCH_SPEED;
                    let name = self.name(f);
                    let stuck = match &mut faults.stuck_latch {
                        Some((part, left)) if *part == name && *left > 0 => {
                            *left -= 1;
                            true
                        }
                        _ => false,
                    };
                    self.attempts += 1;
                    if in_envelope && !stuck {
                        f.latch(self.part).map_err(|detail| Failure::CheckFailed {
                            part: name.clone(),
                            detail,
                        })?;
                        self.started = true;
                        say(format!("{name} latched ({:.1} mm off)", position * 1e3));
                    } else {
                        let why = if stuck {
                            "the latch did not seat".to_owned()
                        } else {
                            format!(
                                "outside the capture envelope ({:.0} mm, {:.2}°, {:.0} mm/s)",
                                position * 1e3,
                                angle.to_degrees(),
                                speed * 1e3
                            )
                        };
                        if self.attempts > MAX_RETRIES {
                            return Err(Failure::LatchDidNotSeat {
                                part: name,
                                attempts: self.attempts,
                            });
                        }
                        say(format!("{name}: {why}; stepping back to try again"));
                        self.next(f, Phase::StepBack);
                    }
                } else if elapsed > 3.0 {
                    self.next(f, Phase::Release);
                }
            }
            Phase::Release => {
                if !self.started {
                    let part = self.part;
                    f.release(part).map_err(|detail| Failure::CheckFailed {
                        part: self.name(f),
                        detail,
                    })?;
                    self.started = true;
                } else if elapsed > 1.0 {
                    self.next(f, Phase::Proof);
                }
            }
            Phase::Proof => {
                // The arm pushes the latched part away from its own base
                // with a small known force; the reaction lands on the deck
                // along the same line, so the pair is internal.
                let body = f.parts[self.part].body;
                let base = f.to_world(f.arms[arm].spec.base);
                let at = f.world[body].pos;
                let push = (at - base).normalize() * PROOF_LOAD;
                if !self.started {
                    self.proof_start = f.to_factory(at);
                    self.started = true;
                }
                if elapsed < 2.0 {
                    f.world[body].apply_force(push);
                    let factory = f.body;
                    f.world[factory].apply_force_at(-push, base);
                } else if elapsed > 3.0 {
                    let moved = f.to_factory(at).distance(self.proof_start);
                    let saturated = f.latch_joint(self.part).is_none_or(|j| j.saturated);
                    if moved > 0.002 || saturated {
                        return Err(Failure::ProofLoad {
                            part: self.name(f),
                            detail: format!("moved {:.1} mm", moved * 1e3),
                        });
                    }
                    self.next(f, Phase::Retreat);
                }
            }
            _ => return Ok(true),
        }
        Ok(false)
    }

    /// Limits watched every step.
    fn limits(&self, f: &Factory) -> Result<(), Failure> {
        let arm = self.arm;
        if f.cmg.saturated {
            return Err(Failure::CmgSaturated);
        }
        let attitude = f.attitude_error();
        if attitude > ATTITUDE_BOX {
            return Err(Failure::AttitudeOutOfBox {
                degrees: attitude.to_degrees(),
            });
        }
        for (i, &s) in f.arms[arm].saturated.iter().enumerate() {
            if s > SATURATION_LIMIT {
                return Err(Failure::MotorLimit {
                    arm,
                    joint: arm::JOINTS[i].into(),
                });
            }
        }
        if let Some(reference) = f.reference(arm) {
            let here = f.measured(arm);
            for i in 0..5 {
                if (here[i] - reference[i]).abs() > TRACKING_LIMIT {
                    return Err(Failure::MotorLimit {
                        arm,
                        joint: arm::JOINTS[i].into(),
                    });
                }
            }
        }
        if self.ticks % 10 == 0 {
            let parent = f.ship.parts[f.parts[self.part].index]
                .attach
                .parent
                .as_deref()
                .and_then(|id| f.parts.iter().position(|p| f.ship.parts[p.index].id == id));
            let ignore: Vec<usize> = parent.into_iter().collect();
            // The stage's own part may touch the tool throughout: on the
            // rack, in the grapple, and once latched.
            let pose = f.measured(arm);
            let own = Some((self.part, f.part_box(self.part)));
            if let Some(contact) = f.clearance(arm, &pose, own, &ignore) {
                return Err(Failure::Collision {
                    a: contact.a,
                    b: contact.b,
                });
            }
        }
        Ok(())
    }
}

impl Assembly {
    /// The sample ship's factory and plan, awaiting approval.
    #[must_use]
    pub fn sample() -> Self {
        let factory = Factory::sample();
        let plan = Plan::new(&factory).expect("the sample plan reaches every part");
        Self::new(factory, plan)
    }

    #[must_use]
    pub fn new(factory: Factory, plan: Plan) -> Self {
        Self {
            factory,
            plan,
            faults: Faults::default(),
            status: Status::AwaitingApproval,
            stage: 0,
            run: None,
            predicting: None,
            prediction: None,
            events: Vec::new(),
            passed: 0,
            cmg_peak: 0.0,
            predict_rate: PREDICT_RATE,
        }
    }

    /// The ground approves the plan.
    pub fn approve(&mut self) {
        if self.status == Status::AwaitingApproval {
            self.status = Status::Predicting;
            self.say(
                None,
                format!("plan approved: {} stages", self.plan.stages.len()),
            );
        }
    }

    fn say(&mut self, stage: Option<usize>, text: String) {
        self.events.push(Event {
            time: self.factory.time(),
            stage,
            text,
        });
    }

    #[must_use]
    pub fn finished(&self) -> bool {
        matches!(
            self.status,
            Status::Complete | Status::Safe(_) | Status::Refused(_)
        )
    }

    /// Make safe: stop every arm where it is. Parts stay in their
    /// grapples, on their racks, or latched; the CMGs keep holding
    /// attitude.
    fn make_safe(&mut self, failure: Failure) {
        for arm in 0..self.factory.arms.len() {
            self.factory.hold_arm(arm);
        }
        self.run = None;
        self.say(Some(self.stage), format!("made safe: {failure}"));
        self.status = Status::Safe(failure);
    }

    /// One real step: the sequencer decides, then the factory moves.
    pub fn step(&mut self) {
        let t = self.factory.time();
        // Faults that strike the real factory now.
        let resets: Vec<usize> = self
            .faults
            .resets
            .iter()
            .filter(|&&(_, at)| at > t - DT / 2.0 && at <= t + DT / 2.0)
            .map(|&(arm, _)| arm)
            .collect();
        for arm in resets {
            self.factory.reset_controller(arm);
        }
        match self.status.clone() {
            Status::Predicting => self.predict(),
            Status::Holding(_) => {
                if self.faults.link_up(t) {
                    self.say(None, "ground link back; resuming".into());
                    self.status = Status::Predicting;
                }
            }
            Status::Acting => self.act(),
            _ => {}
        }
        self.factory.step();
        self.cmg_peak = self.cmg_peak.max(self.factory.cmg.h.length());
    }

    fn predict(&mut self) {
        if self.stage >= self.plan.stages.len() {
            self.status = Status::Complete;
            self.say(None, "assembly complete".into());
            return;
        }
        let t = self.factory.time();
        if self.predicting.is_none() {
            // A hold point: no stage starts without the ground.
            if !self.faults.link_up(t) {
                self.say(
                    Some(self.stage),
                    "ground link lost; holding before the next stage".into(),
                );
                self.status = Status::Holding("ground link lost".into());
                return;
            }
            let part = self.stage;
            // The model is the factory as it is now; it knows nothing of
            // the faults to come.
            let copy = self.factory.clone();
            match StageRun::new(&copy, self.stage, part) {
                Ok(run) => {
                    self.predicting = Some(Box::new(StageSimulation {
                        copy,
                        run,
                        started: t,
                        peak: 0.0,
                    }));
                }
                Err(failure) => {
                    self.status = Status::Refused(failure);
                    return;
                }
            }
        }
        let Some(boxed) = self.predicting.as_mut() else {
            return;
        };
        let StageSimulation {
            copy,
            run,
            started,
            peak,
        } = &mut **boxed;
        let mut none = Faults::default();
        let mut scratch = Vec::new();
        for _ in 0..self.predict_rate.max(1) {
            match run.step(copy, &mut none, &mut scratch) {
                Ok(false) => {
                    copy.step();
                    *peak = peak.max(copy.cmg.h.length());
                }
                Ok(true) => {
                    let prediction = Prediction {
                        part_center: copy.to_factory(copy.world[copy.parts[run.part].body].pos),
                        attitude_error: copy.attitude_error(),
                        cmg_peak: *peak,
                        duration: copy.time() - *started,
                    };
                    self.prediction = Some(prediction);
                    self.predicting = None;
                    if !self.faults.link_up(self.factory.time()) {
                        self.say(
                            Some(self.stage),
                            "ground link lost; holding before the next stage".into(),
                        );
                        self.status = Status::Holding("ground link lost".into());
                        return;
                    }
                    let stage = self.stage;
                    let part = self.plan.stages[stage].part.clone();
                    self.say(
                        Some(stage),
                        format!(
                            "simulated {part}: {:.0} s, peak CMG {:.0} kN m s; committing",
                            prediction.duration,
                            prediction.cmg_peak / 1e3
                        ),
                    );
                    match StageRun::new(&self.factory, stage, stage) {
                        Ok(run) => {
                            self.run = Some(run);
                            self.status = Status::Acting;
                            self.cmg_peak = 0.0;
                        }
                        Err(failure) => self.status = Status::Refused(failure),
                    }
                    return;
                }
                Err(failure) => {
                    self.predicting = None;
                    self.say(
                        Some(self.stage),
                        format!("simulation refused the stage: {failure}"),
                    );
                    self.status = Status::Refused(failure);
                    return;
                }
            }
        }
    }

    fn act(&mut self) {
        let Some(run) = self.run.as_mut() else {
            self.status = Status::Predicting;
            return;
        };
        let mut events = Vec::new();
        let result = run.step(&mut self.factory, &mut self.faults, &mut events);
        self.events.append(&mut events);
        match result {
            Ok(false) => {}
            Ok(true) => match self.check() {
                Ok(()) => {
                    let part = self.plan.stages[self.stage].part.clone();
                    self.say(Some(self.stage), format!("{part} checked"));
                    self.passed += 1;
                    self.stage += 1;
                    self.run = None;
                    self.status = Status::Predicting;
                }
                Err(failure) => self.make_safe(failure),
            },
            Err(failure) => self.make_safe(failure),
        }
    }

    /// The check after a stage: latched, in tolerance, inside the attitude
    /// box, matching the prediction, and nothing unrestrained.
    fn check(&self) -> Result<(), Failure> {
        let part = self.stage;
        let f = &self.factory;
        let name = self.plan.stages[part].part.clone();
        let fail = |detail: String| Failure::CheckFailed {
            part: name.clone(),
            detail,
        };
        if !matches!(f.parts[part].holder, Holder::Latched(_)) {
            return Err(fail("not latched".into()));
        }
        let (position, angle) = f.placement_error(part);
        if position > POSE_TOLERANCE.0 || angle > POSE_TOLERANCE.1 {
            return Err(fail(format!(
                "{:.1} mm and {:.2}° from its place",
                position * 1e3,
                angle.to_degrees()
            )));
        }
        if f.attitude_error() > ATTITUDE_BOX {
            return Err(Failure::AttitudeOutOfBox {
                degrees: f.attitude_error().to_degrees(),
            });
        }
        if !f.all_restrained() {
            return Err(fail("something is unrestrained".into()));
        }
        if let Some(p) = self.prediction {
            let here = f.to_factory(f.world[f.parts[part].body].pos);
            if here.distance(p.part_center) > 0.01 {
                return Err(fail(format!(
                    "ended {:.0} mm from where the simulation put it",
                    here.distance(p.part_center) * 1e3
                )));
            }
        }
        Ok(())
    }

    /// Run until finished or `seconds` of factory time pass.
    pub fn run(&mut self, seconds: f64) {
        let end = self.factory.time() + seconds;
        while !self.finished() && self.factory.time() < end {
            self.step();
        }
    }

    /// A one-line status for the HUD.
    #[must_use]
    pub fn caption(&self) -> String {
        let n = self.plan.stages.len();
        match &self.status {
            Status::AwaitingApproval => "Awaiting plan approval".into(),
            Status::Predicting => match self.plan.stages.get(self.stage) {
                Some(s) => format!("Stage {}/{n}: simulating {}", self.stage + 1, s.part),
                None => "Assembly complete".into(),
            },
            Status::Acting => {
                let s = &self.plan.stages[self.stage];
                let phase = self.run.as_ref().map_or("", |r| r.phase.label());
                format!(
                    "Stage {}/{n}: arm {} {} {}",
                    self.stage + 1,
                    s.arm + 1,
                    phase,
                    s.part
                )
            }
            Status::Holding(why) => format!("Holding: {why}"),
            Status::Complete => "Assembly complete".into(),
            Status::Safe(f) => format!("Safe: {f}"),
            Status::Refused(f) => format!("Refused: {f}"),
        }
    }
}
