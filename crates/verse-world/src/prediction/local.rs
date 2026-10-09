//! Local input timing and presentation estimates for one authenticated connection.
#[path = "local_frames.rs"]
mod frames;
use crate::{
    Command, Intent,
    movement::{self, Baseline},
    play::Ability,
};
use glam::Vec3;
use physics::queries::{Filter, Life, SceneCache, SceneSnapshot};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use verse_engine::core::LifeId;

#[derive(Clone, Copy, Debug)]
pub struct Pose {
    pub life: LifeId,
    pub epoch: u64,
    pub position: Vec3,
    pub yaw: f32,
    pub axes: [f32; 2],
    pub airborne: bool,
    pub moving: bool,
    pub motion_time: f32,
}
/// Read-only timing evidence in 120 Hz physics steps; input count is bounded by history.
#[derive(Clone, Debug, serde::Serialize)]
pub struct Timing {
    recovery_blocks: u64,
    embedding_deferrals: u64,
    horizon_pauses: u64,
    separating_steps: u64,
    stale_crowd_steps: u64,
    last_embedding: Option<String>,
    render_floor: u64,
    deferred_steps: Vec<u64>,
    motor_history_steps: usize,
    last_reconciliation: Option<Reconciliation>,
    steps_per_second: u16,
    step: u64,
    simulated: u64,
    fraction: f64,
    baseline_step: Option<u64>,
    inputs: Vec<TimedInput>,
}
#[derive(Clone, Debug, serde::Serialize)]
struct TimedInput {
    token: u64,
    sequence: Option<u64>,
    superseded_by: Option<u64>,
    step: u64,
    intent: Intent<Ability>,
}
struct Input {
    token: u64,
    sequence: Option<u64>,
    superseded_by: Option<u64>,
    step: u64,
    intent: Intent<Ability>,
}
#[derive(Clone, Copy, Debug, serde::Serialize)]
struct Estimate {
    character: physics::character::Character,
    held: movement::Held,
    yaw: f32,
    policy: movement::Policy,
}
/// One reconciliation decision, retained without growing the input history.
#[derive(Clone, Copy, Debug, serde::Serialize)]
struct Reconciliation {
    previous_baseline: Option<Baseline>,
    completed_estimate: Option<Estimate>,
    fixed_geometry_matches: bool,
    blocking_geometry_matches: bool,
    dirty_before: bool,
    path: &'static str,
}
pub struct Local {
    recovery: movement::RecoveryObservations,
    embedding_deferrals: u64,
    horizon_pauses: u64,
    separating_steps: u64,
    stale_crowd_steps: u64,
    last_embedding: Option<String>,
    last_reconciliation: Option<Reconciliation>,
    collision: SceneCache,
    baseline: Option<Baseline>,
    world_credit: u64,
    deferred_steps: BTreeSet<u64>,
    estimates: BTreeMap<u64, Estimate>,
    inputs: VecDeque<Input>,
    token: u64,
    tick: u64,
    observation: u64,
    step: u64,
    simulated: u64,
    render_floor: u64,
    fraction: f64,
    character: Option<physics::character::Character>,
    yaw: f32,
    dirty: bool,
    moving: bool,
    motion_time: f32,
}
impl Local {
    /// Retain admitted observer geometry without advancing or rebinding movement.
    pub fn observe_animation_geometry(
        &mut self,
        geometry: &SceneSnapshot,
    ) -> Result<usize, String> {
        self.collision.update(geometry)
    }
    pub(crate) fn animation_scene(&self) -> &physics::queries::Scene {
        self.collision.scene()
    }
    pub fn new(instance: u64) -> Self {
        Self {
            recovery: Default::default(),
            embedding_deferrals: 0,
            horizon_pauses: 0,
            separating_steps: 0,
            stale_crowd_steps: 0,
            last_embedding: None,
            last_reconciliation: None,
            collision: SceneCache::new(instance),
            baseline: None,
            world_credit: 0,
            deferred_steps: BTreeSet::new(),
            estimates: BTreeMap::new(),
            inputs: VecDeque::new(),
            token: 0,
            tick: 0,
            observation: 0,
            step: 0,
            simulated: 0,
            render_floor: 0,
            fraction: 0.,
            character: None,
            yaw: 0.,
            dirty: false,
            moving: false,
            motion_time: 0.,
        }
    }
    pub fn clear(&mut self) {
        self.last_reconciliation = None;
        self.baseline = None;
        self.world_credit = 0;
        self.render_floor = 0;
        self.deferred_steps.clear();
        self.estimates.clear();
        self.inputs.clear();
        self.character = None;
        self.fraction = 0.;
        self.motion_time = 0.;
        self.moving = false;
    }
    /// Counts frames that waited at the correction horizon for authority.
    pub fn horizon_pauses(&self) -> u64 {
        self.horizon_pauses
    }
    pub fn context(&self) -> Option<(LifeId, u64)> {
        self.baseline.map(|b| (b.life, b.epoch))
    }
    /// Captures timing without advancing, correcting, or rebinding predicted input.
    pub fn timing(&self) -> Timing {
        Timing {
            recovery_blocks: self.recovery.blocks,
            embedding_deferrals: self.embedding_deferrals,
            horizon_pauses: self.horizon_pauses,
            separating_steps: self.separating_steps,
            stale_crowd_steps: self.stale_crowd_steps,
            last_embedding: self.last_embedding.clone(),
            render_floor: self.render_floor,
            deferred_steps: self.deferred_steps.iter().copied().collect(),
            motor_history_steps: self.estimates.len(),
            last_reconciliation: self.last_reconciliation,
            steps_per_second: 120,
            step: self.step,
            simulated: self.simulated,
            fraction: self.fraction,
            baseline_step: self.baseline.map(|b| b.physics_step),
            inputs: self
                .inputs
                .iter()
                .map(|i| TimedInput {
                    token: i.token,
                    sequence: i.sequence,
                    superseded_by: i.superseded_by,
                    step: i.step,
                    intent: i.intent.clone(),
                })
                .collect(),
        }
    }
    pub fn pending(&self) -> usize {
        self.inputs.len()
    }
    /// Separates the input clock from the last processed motor step.
    pub fn prediction_delay_steps(&self) -> u64 {
        self.step.saturating_sub(self.simulated)
    }
    pub fn embedding_deferrals(&self) -> u64 {
        self.embedding_deferrals
    }
    pub fn separating_steps(&self) -> u64 {
        self.separating_steps
    }
    /// Counts steps walked past actor capsules whose projected poses the
    /// predicted character was already embedded in.
    pub fn stale_crowd_steps(&self) -> u64 {
        self.stale_crowd_steps
    }
    pub fn last_embedding(&self) -> Option<&str> {
        self.last_embedding.as_deref()
    }
    pub fn observe(
        &mut self,
        baseline: Baseline,
        geometry: &SceneSnapshot,
        tick: u64,
        observation: u64,
    ) -> Result<(), String> {
        self.observe_inner(baseline, Some(geometry), tick, observation)
    }
    /// Applies verified completed travel using the most recent collision scene.
    /// Returns false for a superseded confirmation without changing prediction.
    pub fn observe_applied(
        &mut self,
        baseline: Baseline,
        tick: u64,
        observation: u64,
    ) -> Result<bool, String> {
        let Some(old) = self.baseline else {
            return Ok(false);
        };
        if baseline.life != old.life
            || baseline.epoch != old.epoch
            || baseline.profile != movement::Profile::Frames
        {
            return Ok(false);
        }
        if baseline.physics_step < old.physics_step
            || baseline.applied_sequence < old.applied_sequence
            || (baseline.physics_step == old.physics_step
                && baseline.applied_sequence == old.applied_sequence)
        {
            return Ok(false);
        }
        self.observe_inner(baseline, None, tick, observation)?;
        Ok(true)
    }
    /// Moves loose props to the committed poses a movement confirmation
    /// carries, before it replays (#10559). Their shapes stay as the last
    /// scene snapshot admitted them.
    ///
    /// # Errors
    ///
    /// Returns a message for an invalid pose.
    pub fn observe_dynamic_poses(
        &mut self,
        poses: &[crate::service::wire::ColliderPose],
    ) -> Result<usize, String> {
        let poses: Vec<_> = poses.iter().map(|p| (p.key, p.pose)).collect();
        self.collision.set_poses(&poses)
    }
    pub fn confirmed(&self) -> Option<Baseline> {
        self.baseline
    }
    fn movement_geometry_matches(
        &self,
        geometry: &SceneSnapshot,
        incoming: Option<Baseline>,
    ) -> bool {
        if self
            .baseline
            .is_none_or(|b| b.profile != movement::Profile::Frames)
        {
            return self.collision.blocking_geometry_matches(geometry);
        }
        if self.collision.blocking_geometry_matches(geometry) {
            return true;
        }
        let mut min = glam::DVec3::splat(f64::INFINITY);
        let mut max = glam::DVec3::splat(f64::NEG_INFINITY);
        let delta = incoming
            .and_then(|b| {
                self.estimates
                    .get(&b.physics_step)
                    .map(|e| b.character.feet - e.character.feet)
            })
            .unwrap_or(glam::DVec3::ZERO);
        let mut include = |feet: glam::DVec3| {
            min = min.min(feet).min(feet + delta);
            max = max.max(feet).max(feet + delta);
        };
        if let Some(baseline) = self.baseline {
            include(baseline.character.feet);
        }
        if let Some(character) = self.character {
            include(character.feet);
        }
        if let Some(baseline) = incoming {
            include(baseline.character.feet);
        }
        for estimate in self.estimates.values() {
            include(estimate.character.feet);
        }
        let settings = physics::character::Settings::default();
        let margin = settings.radius + settings.step_height + settings.ground_snap + 1e-5;
        self.collision.blocking_geometry_matches_in(
            geometry,
            min - glam::DVec3::splat(margin),
            max + glam::DVec3::splat(margin) + glam::DVec3::Y * settings.height,
        )
    }
    fn observe_inner(
        &mut self,
        baseline: Baseline,
        geometry: Option<&SceneSnapshot>,
        tick: u64,
        observation: u64,
    ) -> Result<(), String> {
        baseline.validate()?;
        if let Some(old) = self.baseline {
            let rejection = if observation <= self.observation {
                Some("observation order")
            } else if tick < self.tick {
                Some("authority tick")
            } else if baseline.life.instance != old.life.instance
                || baseline.life.actor != old.life.actor
            {
                Some("actor identity")
            } else if baseline.life.generation < old.life.generation {
                Some("life generation")
            } else if baseline.epoch < old.epoch {
                Some("control epoch")
            } else if baseline.epoch == old.epoch {
                if baseline.physics_step < old.physics_step {
                    Some("confirmed physics step")
                } else if baseline.applied_sequence < old.applied_sequence {
                    Some("applied sequence")
                } else if baseline.life != old.life {
                    Some("life changed without an epoch")
                } else if baseline.profile != old.profile {
                    Some("profile changed without an epoch")
                } else if baseline.profile == movement::Profile::Frames
                    && baseline.physics_step > self.step
                {
                    Some("confirmation ahead of local simulation")
                } else if baseline.profile == movement::Profile::Frames
                    && baseline.world_step < old.world_step
                {
                    Some("world step")
                } else {
                    None
                }
            } else {
                None
            };
            if let Some(reason) = rejection {
                return Err(format!(
                    "Local prediction observation regressed: {reason}; observation {observation}/{}, tick {tick}/{}, epoch {}/{}, confirmed step {}/{}, local step {}, world step {}/{}, sequence {}/{}",
                    self.observation,
                    self.tick,
                    baseline.epoch,
                    old.epoch,
                    baseline.physics_step,
                    old.physics_step,
                    self.step,
                    baseline.world_step,
                    old.world_step,
                    baseline.applied_sequence,
                    old.applied_sequence,
                ));
            }
        }
        let reset = self.context() != Some((baseline.life, baseline.epoch));
        let blocking_geometry_matches =
            geometry.is_none_or(|scene| self.movement_geometry_matches(scene, Some(baseline)));
        let mut reconciliation = Reconciliation {
            previous_baseline: self.baseline,
            completed_estimate: self.estimates.get(&baseline.physics_step).copied(),
            fixed_geometry_matches: geometry
                .is_none_or(|geometry| self.collision.fixed_geometry_matches(geometry)),
            blocking_geometry_matches,
            dirty_before: self.dirty,
            path: "replay",
        };
        // An unchanged authority motor cannot rewrite processed input when a
        // corpse or another fixed blocker appears. New geometry applies to the
        // next integration; a changed motor still reconciles confirmed travel.
        let same_travel = baseline.profile == movement::Profile::Frames
            && self.baseline.is_some_and(|mut previous| {
                previous.world_step = baseline.world_step;
                previous == baseline
            });
        // Compare completed travel at its own physics step. Capsule poses from
        // another observation cannot retroactively replace a pending path that
        // this estimate already processed. Other motor state still requires replay.
        let mut translation = None;
        let mut constrained = None;
        if !reset
            && !same_travel
            && !self.dirty
            && baseline.profile == movement::Profile::Frames
            && blocking_geometry_matches
        {
            if let (Some(reference), Some(current)) =
                (self.estimates.get(&baseline.physics_step), self.character)
            {
                let delta = baseline.character.feet - reference.character.feet;
                let mut expected = reference.character;
                expected.feet = baseline.character.feet;
                if expected == baseline.character
                    && expected.support.is_some()
                    && delta.y.abs() <= 1e-8
                    && reference.policy == baseline.policy
                    && reference.held.axes(baseline.physics_step)
                        == baseline.held.axes(baseline.physics_step)
                    && reference.yaw == baseline.yaw
                {
                    let mut filter = Filter::blocking(baseline.life.instance);
                    filter.ignore = Some(Life {
                        instance: baseline.life.instance,
                        entity: baseline.life.actor,
                        generation: baseline.life.generation,
                    });
                    let fixed_clear_at = |feet: glam::DVec3| -> Result<bool, String> {
                        let overlap = self.collision.scene().overlap(
                            physics::character::Settings::default().capsule(feet + delta),
                            filter,
                        )?;
                        if overlap.truncated {
                            return Err("Prediction reconciliation query budget exceeded".into());
                        }
                        Ok(overlap.hits.iter().all(|hit| {
                            hit.penetration <= 1e-5 || self.collision.is_capsule(hit.collider)
                        }))
                    };
                    let mut fixed_clear = delta == glam::DVec3::ZERO;
                    if !fixed_clear {
                        fixed_clear = fixed_clear_at(current.feet)?;
                        if fixed_clear {
                            // A clear current pose does not guarantee that shifting
                            // earlier wall contacts preserves valid collision history.
                            for (_, estimate) in self.estimates.range((
                                std::ops::Bound::Excluded(baseline.physics_step),
                                std::ops::Bound::Unbounded,
                            )) {
                                if !fixed_clear_at(estimate.character.feet)? {
                                    fixed_clear = false;
                                    break;
                                }
                            }
                        }
                    }
                    if fixed_clear {
                        translation = Some(delta);
                    } else {
                        // Later actor capsules affect future integration. Resolve a
                        // completed confirmation against the unchanged fixed scene,
                        // preserving the collision-constrained path already processed.
                        let mut fixed = self.collision.scene().clone();
                        let capsules: Vec<_> = fixed.capsule_keys().collect();
                        for key in capsules {
                            fixed.remove_capsule(key);
                        }
                        let settings = physics::character::Settings::default();
                        let clear = |feet: glam::DVec3| -> Result<bool, String> {
                            let overlap = fixed.overlap(settings.capsule(feet), filter)?;
                            if overlap.truncated {
                                return Err(
                                    "Prediction reconciliation query budget exceeded".into()
                                );
                            }
                            Ok(overlap.hits.iter().all(|hit| hit.penetration <= 1e-5))
                        };
                        // Independent translations can put adjacent samples on opposite
                        // sides of a corner. Follow shifted targets in time order from
                        // the confirmed anchor, bounded by already processed travel.
                        let mut original = reference.character.feet;
                        let mut corrected = baseline.character.feet;
                        let mut history = Vec::new();
                        let mut valid = clear(original)? && clear(corrected)?;
                        for (step, estimate) in self.estimates.range((
                            std::ops::Bound::Excluded(baseline.physics_step),
                            std::ops::Bound::Unbounded,
                        )) {
                            if !valid {
                                break;
                            }
                            let feet = estimate.character.feet;
                            if !clear(feet)? {
                                history.push((*step, None));
                                continue;
                            }
                            let next = physics::character::slide(
                                &fixed,
                                filter,
                                settings,
                                corrected,
                                (feet + delta - corrected)
                                    .clamp_length_max((feet - original).length()),
                                true,
                            )?;
                            if next.y != feet.y || !clear(next)? {
                                valid = false;
                                break;
                            }
                            history.push((*step, Some(next)));
                            original = feet;
                            corrected = next;
                        }
                        if valid && clear(current.feet)? {
                            let feet = physics::character::slide(
                                &fixed,
                                filter,
                                settings,
                                corrected,
                                (current.feet + delta - corrected)
                                    .clamp_length_max((current.feet - original).length()),
                                true,
                            )?;
                            if feet.y == current.feet.y && clear(feet)? {
                                constrained = Some((feet, history));
                            }
                        }
                    }
                }
            }
        }
        let shift = if reset || baseline.physics_step <= self.step {
            0
        } else {
            self.inputs
                .iter()
                .find(|i| {
                    i.sequence
                        .is_none_or(|sequence| sequence > baseline.applied_sequence)
                })
                .map_or(0, |i| baseline.physics_step.saturating_sub(i.step))
        };
        let step = self
            .step
            .checked_add(shift)
            .ok_or("Local prediction clock exhausted")?;
        let simulated = self
            .simulated
            .checked_add(shift)
            .ok_or("Local prediction clock exhausted")?;
        if let Some(geometry) = geometry {
            self.collision.update(geometry)?;
        }
        if reset {
            self.deferred_steps.clear();
            self.estimates.clear();
            self.inputs.clear();
            // Entry requires a stationary grounded pose. Align only this fresh
            // interval timeline to verified simulated time, preserving the old
            // character pose and neutral input throughout the gap.
            self.step = if baseline.profile == movement::Profile::Frames
                && baseline.applied_sequence == 0
                && baseline.character.support.is_some()
                && baseline.held.axes(baseline.physics_step) == [0.; 2]
            {
                baseline.world_step
            } else {
                baseline.physics_step
            };
            self.simulated = self.step;
            self.render_floor = self.step;
            self.fraction = 0.;
            self.motion_time = 0.;
            self.moving = false;
        } else {
            self.deferred_steps
                .retain(|step| *step >= baseline.physics_step);
            self.estimates
                .retain(|step, _| *step >= baseline.physics_step);
            self.inputs
                .retain(|i| i.sequence.is_none_or(|s| s > baseline.applied_sequence));
            // Rebase pending intervals only when authority overtakes the local clock.
            // Late snapshots must not add elapsed time or renew pending movement holds.
            for input in &mut self.inputs {
                input.step += shift;
            }
            self.step = step.max(baseline.physics_step);
            self.simulated = simulated;
        }
        // New geometry cannot invent movement during already replayed time.
        // A changed authority motor state still requires reconciliation below.
        if let Some(delta) = translation {
            self.character
                .as_mut()
                .expect("Verified prediction character")
                .feet += delta;
            for (step, estimate) in &mut self.estimates {
                if *step > baseline.physics_step {
                    estimate.character.feet += delta;
                }
            }
        } else if let Some((feet, history)) = &constrained {
            self.character
                .as_mut()
                .expect("Verified prediction character")
                .feet = *feet;
            // An obstructed historical sample cannot invalidate the separately
            // checked current pose. Remove it so later confirmations cannot use
            // that sample as a translation reference.
            for (step, corrected) in history {
                if let Some(corrected) = corrected {
                    if let Some(estimate) = self.estimates.get_mut(step) {
                        estimate.character.feet = *corrected;
                    }
                } else {
                    self.estimates.remove(step);
                }
            }
        } else if !same_travel {
            self.character = Some(baseline.character);
            self.yaw = baseline.yaw;
            self.estimates.clear();
        }
        if baseline.profile == movement::Profile::Frames {
            self.estimates.insert(
                baseline.physics_step,
                Estimate {
                    character: baseline.character,
                    held: baseline.held,
                    yaw: baseline.yaw,
                    policy: baseline.policy,
                },
            );
        }
        self.world_credit = if reset {
            baseline.world_step
        } else {
            self.world_credit.max(baseline.world_step)
        };
        self.baseline = Some(baseline);
        reconciliation.path = if reset {
            "control_reset"
        } else if same_travel {
            "same_travel"
        } else if translation.is_some() {
            "translation"
        } else if constrained.is_some() {
            "constrained_translation"
        } else {
            "replay"
        };
        self.last_reconciliation = Some(reconciliation);
        self.tick = tick;
        self.observation = observation;
        if translation.is_some() || constrained.is_some() {
            self.dirty = false;
        } else if !same_travel {
            self.dirty = !reset || self.step == baseline.physics_step;
        }
        Ok(())
    }
    pub fn observation(&self) -> u64 {
        self.observation
    }
    pub fn contains(&self, token: u64) -> bool {
        self.inputs.iter().any(|i| i.token == token)
    }
    /// Refreshes collision while the server withholds a pending movement baseline.
    pub fn update_geometry(
        &mut self,
        geometry: &SceneSnapshot,
        tick: u64,
        observation: u64,
    ) -> Result<(), String> {
        if self.baseline.is_none() || tick < self.tick || observation <= self.observation {
            return Err("Prediction collision observation is inactive or regressed".into());
        }
        let replay = self
            .baseline
            .is_some_and(|b| b.profile != movement::Profile::Frames)
            || !self.movement_geometry_matches(geometry, None);
        self.collision.update(geometry)?;
        self.tick = tick;
        self.observation = observation;
        self.dirty |= replay;
        Ok(())
    }
    pub fn queue(&mut self, token: u64, intent: Intent<Ability>) -> Result<(), String> {
        if self.baseline.is_none() || token == 0 || token <= self.token {
            return Err("Local prediction input context or budget is invalid".into());
        }
        match &intent {
            Intent::Move { axes, yaw } => {
                movement::walk(*axes, *yaw)?;
                if axes.iter().any(|v| v.abs() > 1.) {
                    return Err("Predicted axes exceed input bounds".into());
                }
            }
            Intent::Jump => {}
            _ => return Err("Only movement inputs can be predicted".into()),
        }
        // Consecutive untransmitted direction samples at the same instant
        // replace each other. No bound interval or intervening jump changes.
        if self.movement_profile() == Some(movement::Profile::Frames)
            && matches!(intent, Intent::Move { .. })
            && self.inputs.back().is_some_and(|last| {
                last.step == self.step
                    && last.sequence.is_none()
                    && last.superseded_by.is_none()
                    && matches!(last.intent, Intent::Move { .. })
                    && !self
                        .inputs
                        .iter()
                        .any(|input| input.superseded_by == Some(last.token))
            })
        {
            let last = self.inputs.back_mut().unwrap();
            last.token = token;
            last.intent = intent;
            self.token = token;
            return Ok(());
        }
        if self.inputs.len() >= super::CAPACITY {
            return Err("Local prediction input context or budget is invalid".into());
        }
        self.token = token;
        self.inputs.push_back(Input {
            token,
            sequence: None,
            superseded_by: None,
            step: self.step,
            intent,
        });
        // Interval input starts at the current clock boundary. It changes only
        // future integration, never travel already processed against older geometry.
        if self
            .baseline
            .is_some_and(|b| b.profile != movement::Profile::Frames)
        {
            self.dirty = true;
        }
        Ok(())
    }
    /// Retains local motion until the newer unsent movement is acknowledged.
    pub fn supersede(&mut self, token: u64, replacement: u64) -> Result<(), String> {
        let old = self
            .inputs
            .iter()
            .position(|i| i.token == token)
            .ok_or("Unknown superseded input")?;
        let new = self
            .inputs
            .iter()
            .find(|i| i.token == replacement)
            .ok_or("Unknown replacement input")?;
        if replacement <= token
            || self.inputs[old].sequence.is_some()
            || self.inputs[old].superseded_by.is_some()
            || new.sequence.is_some()
            || !matches!(self.inputs[old].intent, Intent::Move { .. })
            || !matches!(new.intent, Intent::Move { .. })
        {
            return Err("Invalid unsent movement replacement".into());
        }
        self.inputs[old].superseded_by = Some(replacement);
        Ok(())
    }
    fn depends_on(&self, token: u64, target: u64) -> bool {
        let mut current = token;
        for _ in 0..super::CAPACITY {
            if current == target {
                return true;
            }
            let Some(next) = self
                .inputs
                .iter()
                .find(|i| i.token == current)
                .and_then(|i| i.superseded_by)
            else {
                return false;
            };
            current = next;
        }
        false
    }
    pub fn bind(&mut self, token: u64, command: &Command<Ability>) -> Result<(), String> {
        let baseline = self.baseline.ok_or("Local prediction is inactive")?;
        let index = self
            .inputs
            .iter()
            .position(|i| i.token == token)
            .ok_or("Unknown predicted input token")?;
        let input = &self.inputs[index];
        if command.actor != baseline.life
            || command.epoch != baseline.epoch
            || command.intent != input.intent
            || command.sequence <= baseline.applied_sequence
            || input.sequence.is_some()
            || input.superseded_by.is_some()
            || self.inputs.iter().any(|other| {
                other.sequence.is_some_and(|sequence| {
                    (other.token < token && sequence >= command.sequence)
                        || (other.token > token && sequence <= command.sequence)
                })
            })
        {
            return Err("Predicted input binding does not match transmitted control".into());
        }
        let associated: Vec<_> = self
            .inputs
            .iter()
            .filter(|i| self.depends_on(i.token, token))
            .map(|i| i.token)
            .collect();
        for input in &mut self.inputs {
            if associated.contains(&input.token) {
                input.sequence = Some(command.sequence);
            }
        }
        Ok(())
    }
    pub fn reject(&mut self, token: u64) {
        let retired: Vec<_> = self
            .inputs
            .iter()
            .filter(|i| self.depends_on(i.token, token))
            .map(|i| i.token)
            .collect();
        self.inputs.retain(|i| !retired.contains(&i.token));
        self.dirty = true;
    }
    fn outward_crowd_step(
        &self,
        character: &physics::character::Character,
        filter: Filter,
        overlap: &physics::queries::Results,
        delta: glam::DVec3,
        jump: bool,
    ) -> Result<Option<glam::DVec3>, String> {
        let Some(support) = character.support else {
            return Ok(None);
        };
        if jump
            || character.vertical_speed != 0.
            || character.gravity.is_some()
            || character.external != glam::DVec3::ZERO
            || delta.y != 0.
            || delta.length_squared() <= 1e-16
            || self.collision.is_capsule(support)
            || character.support_pose() != self.collision.scene().pose(support)
        {
            return Ok(None);
        }
        let contacts: Vec<_> = overlap
            .hits
            .iter()
            .filter(|hit| hit.penetration > 1e-5)
            .collect();
        if contacts.is_empty()
            || contacts.iter().any(|hit| {
                !self.collision.is_capsule(hit.collider) || delta.dot(hit.normal) < -1e-12
            })
        {
            return Ok(None);
        }
        // A straight translation out of every convex contact plane cannot
        // increase these existing capsule overlaps. Sweep all other colliders;
        // do not invent a depenetration push or a new supported height.
        let mut remaining = self.collision.scene().clone();
        for hit in &contacts {
            remaining.remove_capsule(hit.collider);
        }
        let settings = physics::character::Settings::default();
        let sweep = remaining.sweep(settings.capsule(character.feet), delta, filter)?;
        if sweep.truncated {
            return Err("Prediction separating sweep query budget exceeded".into());
        }
        if sweep
            .hits
            .iter()
            .any(|hit| hit.fraction < 1. && delta.dot(hit.normal) < -1e-12)
        {
            return Ok(None);
        }
        let feet = character.feet + delta;
        let endpoint = self
            .collision
            .scene()
            .overlap(settings.capsule(feet), filter)?;
        if endpoint.truncated {
            return Err("Prediction separating overlap query budget exceeded".into());
        }
        if endpoint.hits.iter().any(|hit| {
            hit.penetration
                > contacts
                    .iter()
                    .find(|old| old.collider == hit.collider)
                    .map_or(1e-5, |old| old.penetration + 1e-8)
        }) {
            return Ok(None);
        }
        let ground = remaining.sweep(settings.capsule(feet), -glam::DVec3::Y * 2e-5, filter)?;
        if ground.truncated {
            return Err("Prediction separating support query budget exceeded".into());
        }
        Ok(ground
            .hits
            .iter()
            .any(|hit| hit.collider == support && hit.surface_normal.y >= settings.slope_cos)
            .then_some(feet))
    }
    /// A predicted character embedded only in other actors' capsules is ahead
    /// of those projected poses: authority admitted this character's state
    /// without that overlap at its own time, so the overlap is evidence that
    /// the projection is stale. Walk the shared solver past exactly those
    /// capsules. Every other collider still blocks, an actor support is never
    /// discarded, and no depenetration push is invented.
    fn stale_crowd_step(
        &self,
        character: &physics::character::Character,
        filter: Filter,
        overlap: &physics::queries::Results,
        velocity: glam::DVec3,
        jump: bool,
    ) -> Result<Option<physics::character::Character>, String> {
        let contacts: Vec<_> = overlap
            .hits
            .iter()
            .filter(|hit| hit.penetration > 1e-5)
            .collect();
        if contacts.is_empty()
            || contacts.iter().any(|hit| {
                !self.collision.is_capsule(hit.collider) || character.support == Some(hit.collider)
            })
        {
            return Ok(None);
        }
        let mut remaining = self.collision.scene().clone();
        for hit in &contacts {
            remaining.remove_capsule(hit.collider);
        }
        let mut next = *character;
        let travel =
            movement::advance(&mut next, &remaining, filter, velocity, jump, 1, 1. / 120.)?;
        Ok((travel.recovery.blocks == 0).then_some(next))
    }
    pub fn advance(&mut self, seconds: f64) -> Result<(), String> {
        if !seconds.is_finite() || !(0. ..=0.1).contains(&seconds) {
            return Err("Invalid local prediction frame interval".into());
        }
        let Some(baseline) = self.baseline else {
            return Ok(());
        };
        self.fraction += seconds * 120.;
        let steps = self.fraction.floor() as u64;
        self.fraction -= steps as f64;
        // Wait at the bounded horizon without discarding already submitted time.
        // Excess elapsed time cannot renew movement or accumulate a catch-up burst.
        let room = u64::from(super::MAX_PENDING_STEPS)
            .saturating_sub(self.step.saturating_sub(baseline.physics_step));
        let admitted = if baseline.profile == movement::Profile::Frames {
            steps.min(room)
        } else {
            if steps > room {
                self.clear();
                return Err("Local prediction exceeded its correction horizon".into());
            }
            steps
        };
        if admitted < steps {
            self.horizon_pauses = self.horizon_pauses.saturating_add(1);
            self.fraction = 0.;
        }
        self.step = self
            .step
            .checked_add(admitted)
            .ok_or("Local prediction clock exhausted")?;
        let mut filter = Filter::blocking(baseline.life.instance);
        filter.ignore = Some(Life {
            instance: baseline.life.instance,
            entity: baseline.life.actor,
            generation: baseline.life.generation,
        });
        // Replay the existing input timeline without granting elapsed time from
        // an acknowledgment. Embedded projected geometry is handled below.
        let target = self.step;
        let from = if self.dirty {
            if baseline.profile == movement::Profile::Frames {
                baseline.physics_step.max(self.render_floor)
            } else {
                baseline.physics_step
            }
        } else {
            self.simulated
        };
        let mut character = if self.dirty {
            baseline.character
        } else {
            self.character.unwrap_or(baseline.character)
        };
        for step in from..target {
            // An earlier estimate held these exact steps against projected
            // obstruction. A later baseline confirms travel only through its
            // own clock; it cannot make the held pending time move retroactively.
            let previous = character.feet;
            let mut held = baseline.held;
            let mut yaw = baseline.yaw;
            let mut jump = false;
            for input in &self.inputs {
                let at = input.step.max(baseline.physics_step);
                if at > step {
                    break;
                }
                match input.intent {
                    Intent::Move {
                        axes,
                        yaw: input_yaw,
                    } => {
                        held.refresh(axes, input.step)?;
                        yaw = input_yaw;
                    }
                    Intent::Jump => jump |= at == step,
                    _ => {}
                }
            }
            if baseline.profile == movement::Profile::Frames && self.deferred_steps.contains(&step)
            {
                self.estimates.insert(
                    step + 1,
                    Estimate {
                        character,
                        held,
                        yaw,
                        policy: baseline.policy,
                    },
                );
                self.moving = false;
                continue;
            }
            let walk = movement::walk(held.axes(step), yaw)?;
            let velocity = baseline.policy.velocity(held.axes(step), yaw)?;
            let mut separating = false;
            // A projected crowd can embed a newer confirmed character in an
            // older collider pose. The estimate separates outward or walks
            // past embedded actor capsules with the shared solver; any other
            // embedding holds its pose, since authority chooses that recovery
            // exit and a local estimate never invents the displacement.
            if baseline.profile == movement::Profile::Frames {
                let overlap = self.collision.scene().overlap(
                    physics::character::Settings::default().capsule(character.feet),
                    filter,
                )?;
                if overlap.truncated {
                    return Err("Prediction overlap query budget exceeded".into());
                }
                if let Some(hit) = overlap
                    .hits
                    .iter()
                    .filter(|hit| hit.penetration > 1e-5)
                    .max_by(|a, b| a.penetration.total_cmp(&b.penetration))
                {
                    if let Some(feet) = self.outward_crowd_step(
                        &character,
                        filter,
                        &overlap,
                        velocity / 120.,
                        jump,
                    )? {
                        character.feet = feet;
                        separating = true;
                        self.separating_steps = self.separating_steps.saturating_add(1);
                    } else if let Some(next) = self.stale_crowd_step(
                        &character,
                        filter,
                        &overlap,
                        velocity,
                        jump && baseline.policy.jump_allowed,
                    )? {
                        character = next;
                        separating = true;
                        self.stale_crowd_steps = self.stale_crowd_steps.saturating_add(1);
                    } else {
                        self.deferred_steps.extend(step..target);
                        self.embedding_deferrals = self.embedding_deferrals.saturating_add(1);
                        self.last_embedding = Some(format!(
                            "Prediction holds actor {:?} at {:?}: contact {:?}, penetration {}, normal {:?}, observed tick {}, replay step {}",
                            baseline.life,
                            character.feet,
                            hit.collider,
                            hit.penetration,
                            hit.normal,
                            self.tick,
                            step,
                        ));
                        self.estimates.insert(
                            step + 1,
                            Estimate {
                                character,
                                held,
                                yaw,
                                policy: baseline.policy,
                            },
                        );
                        self.moving = false;
                        continue;
                    }
                }
            }
            if !separating {
                let travel = movement::advance(
                    &mut character,
                    self.collision.scene(),
                    filter,
                    velocity,
                    jump && baseline.policy.jump_allowed,
                    1,
                    1. / 120.,
                )?;
                if travel.recovery.blocks > 0 {
                    if baseline.profile == movement::Profile::Frames {
                        self.deferred_steps.extend(step..target);
                    }
                    self.recovery.blocks =
                        self.recovery.blocks.saturating_add(travel.recovery.blocks);
                    self.recovery.last_diagnostic = travel.recovery.last_diagnostic;
                    self.moving = false;
                    if baseline.profile == movement::Profile::Frames {
                        self.estimates.insert(
                            step + 1,
                            Estimate {
                                character,
                                held,
                                yaw,
                                policy: baseline.policy,
                            },
                        );
                        continue;
                    }
                    break;
                }
            }
            if baseline.profile == movement::Profile::Frames {
                self.estimates.insert(
                    step + 1,
                    Estimate {
                        character,
                        held,
                        yaw,
                        policy: baseline.policy,
                    },
                );
            }
            let distance = character.feet.distance(previous) as f32;
            self.moving = distance > 0.00001;
            if step >= self.simulated {
                self.motion_time += distance / walk.speed;
            }
        }
        self.yaw = self
            .inputs
            .iter()
            .rev()
            .find_map(|i| match i.intent {
                Intent::Move { yaw, .. } => Some(yaw),
                _ => None,
            })
            .unwrap_or(baseline.yaw);
        self.character = Some(character);
        self.simulated = target;
        self.dirty = false;
        Ok(())
    }
    pub fn pose(&self) -> Option<Pose> {
        let baseline = self.baseline?;
        let mut held = baseline.held;
        for input in &self.inputs {
            if input.step > self.step {
                break;
            }
            if let Intent::Move { axes, .. } = input.intent {
                held.refresh(axes, input.step).ok()?;
            }
        }
        Some(Pose {
            axes: if baseline.policy.walking_scale == 0. {
                [0.; 2]
            } else {
                held.axes(self.step)
            },
            airborne: self
                .character
                .unwrap_or(baseline.character)
                .support
                .is_none(),
            moving: self.moving,
            motion_time: self.motion_time,
            life: baseline.life,
            epoch: baseline.epoch,
            position: self.character.unwrap_or(baseline.character).feet.as_vec3(),
            yaw: self.yaw,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use physics::queries::{ColliderKey, Mesh, MeshCollider, Scene, Usage};
    fn setup() -> (Local, Baseline, SceneSnapshot) {
        let mut scene = Scene::default();
        scene
            .insert(MeshCollider {
                key: ColliderKey {
                    life: Life {
                        instance: 7,
                        entity: 0,
                        generation: 0,
                    },
                    shape: 0,
                },
                layers: 1,
                usage: Usage::Blocking,
                mesh: Mesh::from_box(
                    glam::DVec3::new(-20., -1., -20.),
                    glam::DVec3::new(20., 0., 20.),
                )
                .unwrap(),
            })
            .unwrap();
        let baseline = Baseline {
            profile: Default::default(),
            world_step: 0,
            life: LifeId {
                instance: 7,
                actor: 14,
                generation: 0,
            },
            epoch: 1,
            applied_sequence: 0,
            physics_step: 0,
            held: Default::default(),
            policy: Default::default(),
            character: physics::character::Character::new(glam::DVec3::ZERO),
            yaw: 0.,
        };
        let source = scene.snapshot(7).unwrap();
        let mut local = Local::new(7);
        local.observe(baseline, &source, 1, 1).unwrap();
        (local, baseline, source)
    }
    fn movement() -> Intent<Ability> {
        Intent::Move {
            axes: [1., 0.],
            yaw: 0.,
        }
    }
    fn command(b: Baseline, sequence: u64, intent: Intent<Ability>) -> Command<Ability> {
        Command {
            actor: b.life,
            epoch: b.epoch,
            sequence,
            tick: 1,
            intent,
        }
    }
    #[test]
    fn correction_horizon_preserves_submitted_time_until_confirmation() {
        let (mut local, mut baseline, geometry) = setup();
        baseline.profile = movement::Profile::Frames;
        baseline.epoch += 1;
        local.observe(baseline, &geometry, 2, 2).unwrap();
        local.queue(1, movement()).unwrap();
        for _ in 0..21 {
            local.advance(0.1).unwrap();
        }
        local
            .queue(
                2,
                Intent::Move {
                    axes: [0.; 2],
                    yaw: 0.,
                },
            )
            .unwrap();
        local.advance(4. / 120.).unwrap();
        let end = local.physics_step();
        assert_eq!(end, u64::from(super::super::MAX_PENDING_STEPS));
        local
            .grant_world_credit(baseline.life, baseline.epoch, end)
            .unwrap();
        for start in (0..end).step_by(4) {
            let mut frame = local.movement_frame(start, 4).unwrap();
            frame.sequence = start / 4 + 1;
            local.bind_movement_frame(&frame).unwrap();
        }
        let character = local.character.unwrap();
        let before = local.pose().unwrap().position;
        for _ in 0..5 {
            local.advance(0.1).unwrap();
            assert_eq!(local.context(), Some((baseline.life, baseline.epoch)));
            assert_eq!(local.physics_step(), end);
            assert_eq!(local.pose().unwrap().position, before);
            assert_eq!(local.pending(), 2);
        }
        baseline.physics_step = end;
        baseline.world_step = end + 12;
        baseline.applied_sequence = end / 4;
        baseline.character = character;
        baseline.held.refresh([0.; 2], end - 4).unwrap();
        assert!(local.observe_applied(baseline, 3, 3).unwrap());
        local.advance(0.).unwrap();
        assert_eq!(local.physics_step(), end);
        assert_eq!(local.pending(), 0);
        local.advance(4. / 120.).unwrap();
        assert_eq!(local.physics_step(), end + 4);
        assert_eq!(local.pose().unwrap().position, before);
    }

    #[test]
    fn paused_clock_movement_samples_preserve_bound_history_jump_and_latest_direction() {
        let (mut local, mut baseline, geometry) = setup();
        baseline.profile = movement::Profile::Frames;
        baseline.epoch += 1;
        local.observe(baseline, &geometry, 2, 2).unwrap();
        local.queue(1, movement()).unwrap();
        for _ in 0..21 {
            local.advance(0.1).unwrap();
        }
        local
            .queue(
                2,
                Intent::Move {
                    axes: [0.; 2],
                    yaw: 0.,
                },
            )
            .unwrap();
        local.advance(4. / 120.).unwrap();
        let end = local.physics_step();
        local
            .grant_world_credit(baseline.life, baseline.epoch, end)
            .unwrap();
        for start in (0..end).step_by(4) {
            let mut frame = local.movement_frame(start, 4).unwrap();
            frame.sequence = start / 4 + 1;
            local.bind_movement_frame(&frame).unwrap();
        }
        let character = local.character.unwrap();
        for token in 3..=1002 {
            local.advance(0.1).unwrap();
            local.queue(token, movement()).unwrap();
            assert_eq!(local.physics_step(), end);
            assert_eq!(local.pending(), 3);
            assert_eq!(local.context(), Some((baseline.life, baseline.epoch)));
        }
        // A jump separates movement samples even at the paused boundary.
        local.queue(1003, Intent::Jump).unwrap();
        local.queue(1004, movement()).unwrap();
        local
            .queue(
                1005,
                Intent::Move {
                    axes: [0., 1.],
                    yaw: 0.,
                },
            )
            .unwrap();
        assert_eq!(local.pending(), 5);
        assert!(local.contains(1003) && local.contains(1005));
        assert!(!local.contains(1004));
        assert_eq!(
            local.movement_frame(end - 4, 4).unwrap().segments[0].axes,
            [0.; 2]
        );
        baseline.physics_step = end;
        baseline.world_step = end + 12;
        baseline.applied_sequence = end / 4;
        baseline.character = character;
        baseline.held.refresh([0.; 2], end - 4).unwrap();
        local.observe_applied(baseline, 3, 3).unwrap();
        assert_eq!(local.pending(), 3);
        local.advance(4. / 120.).unwrap();
        let next = local.movement_frame(end, 4).unwrap();
        assert!(next.segments[0].jump);
        assert_eq!(next.segments[0].axes, [0., 1.]);
        assert_eq!(local.physics_step(), end + 4);
    }
    #[test]
    fn partial_interval_confirmation_replays_only_unconsumed_steps() {
        let (mut local, mut baseline, geometry) = setup();
        baseline.profile = movement::Profile::Frames;
        baseline.epoch += 1;
        baseline.world_step = 12;
        local.observe(baseline, &geometry, 2, 2).unwrap();
        local.queue(1, movement()).unwrap();
        local.advance(4. / 120.).unwrap();
        let partial = local.character.unwrap();
        local.advance(8. / 120.).unwrap();
        let predicted = local.character.unwrap();
        let mut frame = local.movement_frame(0, 12).unwrap();
        frame.sequence = 1;
        local.bind_movement_frame(&frame).unwrap();
        baseline.physics_step = 4;
        baseline.character = partial;
        baseline.held.refresh([1., 0.], 0).unwrap();
        assert!(local.observe_applied(baseline, 3, 3).unwrap());
        local.advance(0.).unwrap();
        assert_eq!(local.physics_step(), 12);
        assert!(local.contains(1));
        assert_eq!(
            serde_json::to_vec(&local.character.unwrap()).unwrap(),
            serde_json::to_vec(&predicted).unwrap()
        );
        baseline.physics_step = 12;
        baseline.applied_sequence = 1;
        baseline.character = predicted;
        assert!(local.observe_applied(baseline, 4, 4).unwrap());
        local.advance(0.).unwrap();
        assert!(!local.contains(1));
        assert_eq!(
            serde_json::to_vec(&local.character.unwrap()).unwrap(),
            serde_json::to_vec(&predicted).unwrap()
        );
    }

    #[test]
    fn verified_credit_recovers_stalled_prediction_in_bounded_batches() {
        let (mut local, mut baseline, source) = setup();
        baseline.profile = movement::Profile::Frames;
        baseline.epoch += 1;
        baseline.world_step = 4;
        local.observe(baseline, &source, 2, 2).unwrap();
        local
            .queue(
                1,
                Intent::Move {
                    axes: [0., 1.],
                    yaw: 0.,
                },
            )
            .unwrap();
        local.advance(0.1).unwrap();
        local
            .grant_world_credit(baseline.life, baseline.epoch, 40)
            .unwrap();
        for expected in [24, 36, 40, 40] {
            local.recover_world_credit().unwrap();
            assert_eq!(local.physics_step(), expected);
            assert!(local.contains(1));
        }
        let frame = local.movement_frame(28, 12).unwrap();
        assert_eq!(frame.segments[0].axes, [0., 1.]);
        assert_eq!(local.movement_frame_limit(), Some(40));
        assert!(
            local
                .grant_world_credit(baseline.life, baseline.epoch + 1, 100)
                .is_err()
        );
        local.recover_world_credit().unwrap();
        assert_eq!(local.physics_step(), 40);
    }

    #[test]
    fn interval_transmission_waits_for_verified_credit_while_prediction_advances() {
        let (mut local, mut baseline, source) = setup();
        baseline.profile = movement::Profile::Frames;
        baseline.epoch += 1;
        baseline.world_step = 4;
        local.observe(baseline, &source, 2, 2).unwrap();
        local.advance(12. / 120.).unwrap();
        local.advance(12. / 120.).unwrap();
        assert_eq!(local.physics_step(), 24);
        assert_eq!(local.movement_frame_limit(), Some(16));
        // Repeated observations during a storage pause grant no wall-time credit.
        local.observe(baseline, &source, 3, 3).unwrap();
        local.advance(12. / 120.).unwrap();
        assert_eq!(local.physics_step(), 36);
        assert_eq!(local.movement_frame_limit(), Some(16));
        baseline.world_step = 20;
        local.observe(baseline, &source, 4, 4).unwrap();
        assert_eq!(local.movement_frame_limit(), Some(32));
        assert_eq!(local.physics_step(), 36);
        let before = local.pose().unwrap().position;
        local
            .movement_credit(baseline.life, baseline.epoch, 24)
            .unwrap();
        assert_eq!(local.movement_frame_limit(), Some(36));
        assert_eq!(local.physics_step(), 36);
        assert_eq!(local.pose().unwrap().position, before);
        assert!(
            local
                .movement_credit(baseline.life, baseline.epoch, 23)
                .is_err()
        );
        assert!(
            local
                .movement_credit(baseline.life, baseline.epoch + 1, 28)
                .is_err()
        );
        let mut foreign = baseline.life;
        foreign.generation += 1;
        assert!(local.movement_credit(foreign, baseline.epoch, 28).is_err());
        assert!(
            local
                .movement_credit(baseline.life, baseline.epoch, u64::MAX)
                .is_err()
        );
        local.advance(12. / 120.).unwrap();
        local
            .movement_credit(baseline.life, baseline.epoch, 24)
            .unwrap();
        assert_eq!(local.physics_step(), 48);
        assert_eq!(local.movement_frame_limit(), Some(36));
        // An older body baseline cannot revoke newer verified header credit.
        local.observe(baseline, &source, 5, 5).unwrap();
        assert_eq!(local.movement_frame_limit(), Some(36));
    }

    #[test]
    fn embedded_prediction_waits_for_authority_without_losing_input_or_query_errors() {
        use physics::queries::{ColliderKey, GeometrySnapshot, Pose, ShapeSnapshot, Usage};
        let (mut local, mut baseline, mut source) = setup();
        baseline.profile = movement::Profile::Frames;
        baseline.epoch += 1;
        let clean = source.clone();
        let settings = physics::character::Settings::default();
        let capsule = settings.capsule(glam::DVec3::new(-0.1, 0., 0.));
        let obstacle = ShapeSnapshot {
            key: ColliderKey {
                life: Life {
                    instance: 7,
                    entity: 216,
                    generation: 0,
                },
                shape: 0,
            },
            layers: 1,
            usage: Usage::Blocking,
            pose: Pose::default(),
            // Static embedding: only actor capsules are treated as stale
            // projections that a predicted character may walk past.
            geometry: GeometrySnapshot::Box {
                min: capsule.a - glam::DVec3::new(0.35, 0., 0.25),
                max: capsule.a + glam::DVec3::new(-0.1, 1., 0.25),
            },
        };
        source.colliders.push(obstacle.clone());
        local.observe(baseline, &source, 2, 2).unwrap();
        local.queue(1, movement()).unwrap();
        local.advance(4. / 120.).unwrap();
        assert_eq!(local.pose().unwrap().position, Vec3::ZERO);
        assert_eq!(local.physics_step(), 4);
        assert_eq!(local.pending(), 1);
        assert_eq!(local.embedding_deferrals(), 1);
        assert!(local.last_embedding().unwrap().contains("entity: 216"));
        // The same contact remains in authority's scene and can recover there.
        let mut authoritative = baseline.character;
        let travel = movement::advance(
            &mut authoritative,
            local.collision.scene(),
            Filter::blocking(7),
            glam::DVec3::X,
            false,
            1,
            1. / 120.,
        )
        .unwrap();
        assert_eq!(travel.recovery.blocks, 0);
        assert!(authoritative.feet.distance(baseline.character.feet) > 0.1);
        local.observe(baseline, &clean, 3, 3).unwrap();
        local.advance(0.).unwrap();
        assert_eq!(local.pose().unwrap().position, Vec3::ZERO);
        local.advance(4. / 120.).unwrap();
        assert!(local.pose().unwrap().position.x > 0.2);
        assert_eq!(local.pending(), 1);
        // A truncated query is an error, rather than an uncertain crowd hold.
        for entity in 216..296 {
            let mut shape = obstacle.clone();
            shape.key.life.entity = entity;
            source.colliders.push(shape);
        }
        source.colliders.remove(1);
        local.update_geometry(&source, 4, 4).unwrap();
        assert!(
            local
                .advance(1. / 120.)
                .unwrap_err()
                .contains("query budget")
        );
    }

    #[test]
    fn outward_projected_crowd_motion_preserves_support_contacts_and_input_time() {
        use physics::queries::{ColliderKey, GeometrySnapshot, Pose, ShapeSnapshot, Usage};
        for case in 0..7 {
            let (mut local, mut baseline, mut source) = setup();
            local.advance(1. / 120.).unwrap();
            baseline.character = local.character.unwrap();
            baseline.profile = movement::Profile::Frames;
            baseline.epoch += 1;
            // The retained actor-230 contact: penetration 0.1453 m and an
            // outward normal with positive z while input walks toward +z.
            let normal = glam::DVec3::new(0.8549089799662449, 0., 0.5187780218678065);
            let settings = physics::character::Settings::default();
            let mut add_capsule = |entity, feet| {
                let capsule = settings.capsule(feet);
                source.colliders.push(ShapeSnapshot {
                    key: ColliderKey {
                        life: Life {
                            instance: 7,
                            entity,
                            generation: 0,
                        },
                        shape: 0,
                    },
                    layers: 1,
                    usage: Usage::Blocking,
                    pose: Pose::default(),
                    geometry: GeometrySnapshot::Capsule {
                        a: capsule.a,
                        b: capsule.b,
                        radius: capsule.radius,
                    },
                });
            };
            add_capsule(230, -normal * (0.7 - 0.14529274757222588));
            if case == 6 {
                add_capsule(231, glam::DVec3::Z * 0.5);
            }
            if case == 2 {
                source.colliders.push(ShapeSnapshot {
                    key: ColliderKey {
                        life: Life {
                            instance: 7,
                            entity: 0,
                            generation: 0,
                        },
                        shape: 1,
                    },
                    layers: 1,
                    usage: Usage::Blocking,
                    pose: Pose::default(),
                    geometry: GeometrySnapshot::Box {
                        min: glam::DVec3::new(-2., 0., 0.4),
                        max: glam::DVec3::new(2., 4., 0.5),
                    },
                });
            }
            if case == 3 {
                source.colliders[0].geometry = GeometrySnapshot::Box {
                    min: glam::DVec3::new(-20., -1., -20.),
                    max: glam::DVec3::new(20., 0., 0.),
                };
            }
            if case == 4 {
                baseline.character.external.x = 2.;
            }
            local.observe(baseline, &source, 2, 2).unwrap();
            local
                .queue(
                    1,
                    Intent::Move {
                        axes: [0., if case == 1 { -1. } else { 1. }],
                        yaw: std::f32::consts::PI,
                    },
                )
                .unwrap();
            if case == 5 {
                local.queue(2, Intent::Jump).unwrap();
            }
            local.advance(4. / 120.).unwrap();
            let position = local.pose().unwrap().position;
            if case == 0 {
                assert!(
                    (position.z - 0.21336).abs() < 0.0001,
                    "Outward travel was frozen: {position:?}"
                );
                assert!(
                    position.x.abs() < 1e-5,
                    "Prediction invented a recovery push"
                );
                assert_eq!(local.embedding_deferrals(), 0);
                let filter = Filter::blocking(7);
                let before = local
                    .collision
                    .scene()
                    .overlap(settings.capsule(baseline.character.feet), filter)
                    .unwrap();
                let after = local
                    .collision
                    .scene()
                    .overlap(settings.capsule(position.as_dvec3()), filter)
                    .unwrap();
                let depth = |hits: &physics::queries::Results| {
                    hits.hits
                        .iter()
                        .find(|h| h.collider.life.entity == 230)
                        .map_or(0., |h| h.penetration)
                };
                assert!(depth(&after) < depth(&before));
                assert_eq!(local.character.unwrap().support, baseline.character.support);
            } else {
                // Inward, walled, unsupported, shoved, jumping, and doubly
                // embedded estimates walk past the stale actor projections
                // with the shared solver instead of freezing pending time.
                assert_ne!(position, Vec3::ZERO, "Case {case} froze stale crowd travel");
                assert_eq!(local.embedding_deferrals(), 0, "Case {case}");
                assert!(local.stale_crowd_steps() > 0, "Case {case}");
                if case == 1 {
                    assert!(
                        position.z < -0.13 && position.x.abs() < 1e-5,
                        "{position:?}"
                    );
                }
                if case == 2 {
                    // Static geometry still blocks the walk past the crowd.
                    assert!(position.z <= 0.4 - 0.35 + 1e-4, "{position:?}");
                }
            }
            assert_eq!(local.physics_step(), 4);
            assert_eq!(local.timing().simulated, 4);
            assert_eq!(local.pending(), if case == 5 { 2 } else { 1 });
            assert_eq!(
                local.collision.scene().capsule_keys().count(),
                if case == 6 { 2 } else { 1 }
            );
        }
    }
    #[test]
    fn stale_actor_projection_does_not_freeze_confirmed_tangential_travel() {
        use physics::queries::{ColliderKey, GeometrySnapshot, Pose, ShapeSnapshot, Usage};
        // Retained 600-second battle trace (player 10, actor 236): walking
        // +z while embedded 0.2783 m in actor 235's projected capsule with
        // normal (0.999, 0, -0.0446). Authority walked seven free steps; the
        // held estimate became a 0.3734 m confirmation correction.
        for mixed in [false, true] {
            let (mut local, mut baseline, mut source) = setup();
            local.advance(1. / 120.).unwrap();
            baseline.character = local.character.unwrap();
            baseline.profile = movement::Profile::Frames;
            baseline.epoch += 1;
            let authority_scene = source.clone();
            let normal = glam::DVec3::new(0.999002932290504, 0., -0.04464461081670551);
            let settings = physics::character::Settings::default();
            let capsule = settings.capsule(-normal * (0.7 - 0.2783030413576324));
            source.colliders.push(ShapeSnapshot {
                key: ColliderKey {
                    life: Life {
                        instance: 7,
                        entity: 235,
                        generation: 0,
                    },
                    shape: 0,
                },
                layers: 1,
                usage: Usage::Blocking,
                pose: Pose::default(),
                geometry: GeometrySnapshot::Capsule {
                    a: capsule.a,
                    b: capsule.b,
                    radius: capsule.radius,
                },
            });
            if mixed {
                // A simultaneous static embedding is not crowd staleness.
                source.colliders.push(ShapeSnapshot {
                    key: ColliderKey {
                        life: Life {
                            instance: 7,
                            entity: 0,
                            generation: 0,
                        },
                        shape: 1,
                    },
                    layers: 1,
                    usage: Usage::Blocking,
                    pose: Pose::default(),
                    geometry: GeometrySnapshot::Box {
                        min: glam::DVec3::new(0.3, 0.2, -0.2),
                        max: glam::DVec3::new(0.6, 1.2, 0.2),
                    },
                });
            }
            local.observe(baseline, &source, 2, 2).unwrap();
            let intent = Intent::Move {
                axes: [0., 1.],
                yaw: std::f32::consts::PI,
            };
            local.queue(1, intent).unwrap();
            local.advance(7. / 120.).unwrap();
            let predicted = local.pose().unwrap().position;
            assert_eq!(local.physics_step(), 7);
            if mixed {
                assert_eq!(predicted, Vec3::ZERO);
                assert_eq!(local.embedding_deferrals(), 1);
                assert_eq!(local.stale_crowd_steps(), 0);
                continue;
            }
            assert_eq!(local.embedding_deferrals(), 0);
            // The first step leaves the inward-facing contact plane; later
            // steps are ordinary outward separation.
            assert_eq!(local.stale_crowd_steps(), 1);
            assert!(local.separating_steps() > 0);
            assert!(predicted.x.abs() < 1e-5, "Prediction invented a push");
            // Authority applies the same seven steps with the shared solver
            // against its own scene, where the actor is not at that pose.
            let authority = authority_scene.compile(7).unwrap();
            let mut authoritative = baseline.character;
            let travel = movement::advance(
                &mut authoritative,
                &authority,
                Filter::blocking(7),
                baseline
                    .policy
                    .velocity([0., 1.], std::f32::consts::PI)
                    .unwrap(),
                false,
                7,
                1. / 120.,
            )
            .unwrap();
            assert_eq!(travel.recovery.blocks, 0);
            assert!((authoritative.feet.z - 0.37338).abs() < 1e-4);
            assert!(
                predicted.as_dvec3().distance(authoritative.feet) < 1e-5,
                "{predicted:?} vs {:?}",
                authoritative.feet
            );
            // The acknowledged baseline (still beside the stale projection)
            // confirms the estimate instead of correcting it.
            baseline.physics_step = 7;
            baseline.world_step = 7;
            baseline.character = authoritative;
            baseline.held.refresh([0., 1.], 1).unwrap();
            baseline.yaw = std::f32::consts::PI;
            local.observe(baseline, &source, 3, 3).unwrap();
            local.advance(0.).unwrap();
            let confirmed = local.pose().unwrap().position;
            assert!(confirmed.distance(predicted) < 1e-5, "{confirmed:?}");
        }
    }
    #[test]
    fn constrained_confirmation_preserves_continuous_travel_around_a_wall_corner() {
        use physics::queries::{ColliderKey, GeometrySnapshot, Pose, ShapeSnapshot, Usage};
        let (mut local, mut baseline, mut source) = setup();
        baseline.profile = movement::Profile::Frames;
        baseline.epoch += 1;
        baseline.character.feet = glam::DVec3::new(4., 0., 3.7);
        source.colliders.push(ShapeSnapshot {
            key: ColliderKey {
                life: Life {
                    instance: 7,
                    entity: 0,
                    generation: 0,
                },
                shape: 1,
            },
            layers: 1,
            usage: Usage::Blocking,
            pose: Pose::default(),
            geometry: GeometrySnapshot::Box {
                min: glam::DVec3::new(6., 0., -2.),
                max: glam::DVec3::new(6.1, 4., 2.),
            },
        });
        local.observe(baseline, &source, 2, 2).unwrap();
        local
            .queue(
                1,
                Intent::Move {
                    axes: [0., 1.],
                    yaw: 0.,
                },
            )
            .unwrap();
        for _ in 0..3 {
            local.advance(0.1).unwrap();
        }
        let reference = local.estimates[&12];
        baseline.physics_step = 12;
        baseline.world_step = local.physics_step();
        baseline.character = reference.character;
        baseline.character.feet.x += 2.;
        baseline.held = reference.held;
        baseline.yaw = reference.yaw;
        baseline.policy = reference.policy;
        local.observe(baseline, &source, 3, 3).unwrap();
        assert_eq!(
            local.last_reconciliation.as_ref().unwrap().path,
            "constrained_translation"
        );
        let mut previous = (baseline.physics_step, baseline.character.feet);
        for (step, estimate) in local.estimates.range(12..) {
            let allowed = 6.4008 * (*step - previous.0) as f64 / 120. + 0.0001;
            assert!(
                (estimate.character.feet - previous.1).length() <= allowed,
                "Correction jumped between steps {} and {}: {:?} -> {:?}",
                previous.0,
                step,
                previous.1,
                estimate.character.feet
            );
            previous = (*step, estimate.character.feet);
        }
        let pose = local.pose().unwrap();
        assert!((pose.position.x - 6.).abs() < 0.0001);
        assert!(pose.position.z >= 2.35 - 0.0001);
    }

    #[test]
    fn wall_constrained_confirmation_preserves_time_processed_before_later_crowd_geometry() {
        use physics::queries::{ColliderKey, GeometrySnapshot, Pose, ShapeSnapshot, Usage};
        let (mut local, mut baseline, mut source) = setup();
        baseline.profile = movement::Profile::Frames;
        baseline.epoch += 1;
        source.colliders.push(ShapeSnapshot {
            key: ColliderKey {
                life: Life {
                    instance: 7,
                    entity: 0,
                    generation: 0,
                },
                shape: 1,
            },
            layers: 1,
            usage: Usage::Blocking,
            pose: Pose::default(),
            geometry: GeometrySnapshot::Box {
                min: glam::DVec3::new(6., 0., -2.),
                max: glam::DVec3::new(6.1, 4., 2.),
            },
        });
        local.observe(baseline, &source, 2, 2).unwrap();
        for token in 1..=9 {
            local.queue(token, movement()).unwrap();
            local.advance(0.1).unwrap();
        }
        assert!((local.pose().unwrap().position.x - 5.64999).abs() < 0.0001);
        movement::advance(
            &mut baseline.character,
            local.collision.scene(),
            Filter::blocking(7),
            glam::DVec3::X * 6.4008,
            false,
            12,
            1. / 120.,
        )
        .unwrap();
        let capsule =
            physics::character::Settings::default().capsule(glam::DVec3::new(0.7, 0., 0.));
        source.colliders.push(ShapeSnapshot {
            key: ColliderKey {
                life: Life {
                    instance: 7,
                    entity: 216,
                    generation: 0,
                },
                shape: 0,
            },
            layers: 1,
            usage: Usage::Blocking,
            pose: Pose::default(),
            geometry: GeometrySnapshot::Capsule {
                a: capsule.a,
                b: capsule.b,
                radius: capsule.radius,
            },
        });
        local.update_geometry(&source, 3, 3).unwrap();
        baseline.physics_step = 12;
        baseline.world_step = 108;
        baseline.character.feet.x = 0.7;
        baseline.held.refresh([1., 0.], 0).unwrap();
        local.observe(baseline, &source, 4, 4).unwrap();
        local.advance(0.).unwrap();
        assert!(
            (local.pose().unwrap().position.x - 5.64999).abs() < 0.0001,
            "A later capsule erased processed pending travel: {:?}",
            local.pose().unwrap().position
        );
        assert_eq!(local.physics_step(), 108);
        assert_eq!(local.pending(), 9);
        assert!(local.estimates.len() <= super::super::MAX_PENDING_STEPS as usize + 1);
        local.advance(4. / 120.).unwrap();
        assert!((local.pose().unwrap().position.x - 5.64999).abs() < 0.0001);
    }

    #[test]
    fn invalid_historical_wall_pose_does_not_discard_a_valid_current_correction() {
        use physics::queries::{ColliderKey, GeometrySnapshot, Pose, ShapeSnapshot, Usage};
        let (mut local, mut baseline, mut source) = setup();
        baseline.profile = movement::Profile::Frames;
        baseline.epoch += 1;
        source.colliders.push(ShapeSnapshot {
            key: ColliderKey {
                life: Life {
                    instance: 7,
                    entity: 0,
                    generation: 0,
                },
                shape: 1,
            },
            layers: 1,
            usage: Usage::Blocking,
            pose: Pose::default(),
            geometry: GeometrySnapshot::Box {
                min: glam::DVec3::new(6., 0., -2.),
                max: glam::DVec3::new(6.1, 4., 2.),
            },
        });
        local.observe(baseline, &source, 2, 2).unwrap();
        for token in 1..=9 {
            local.queue(token, movement()).unwrap();
            local.advance(0.1).unwrap();
        }
        assert!((local.pose().unwrap().position.x - 5.64999).abs() < 0.0001);
        movement::advance(
            &mut baseline.character,
            local.collision.scene(),
            Filter::blocking(7),
            glam::DVec3::X * 6.4008,
            false,
            12,
            1. / 120.,
        )
        .unwrap();
        let capsule =
            physics::character::Settings::default().capsule(glam::DVec3::new(0.7, 0., 0.));
        source.colliders.push(ShapeSnapshot {
            key: ColliderKey {
                life: Life {
                    instance: 7,
                    entity: 216,
                    generation: 0,
                },
                shape: 0,
            },
            layers: 1,
            usage: Usage::Blocking,
            pose: Pose::default(),
            geometry: GeometrySnapshot::Capsule {
                a: capsule.a,
                b: capsule.b,
                radius: capsule.radius,
            },
        });
        local.update_geometry(&source, 3, 3).unwrap();
        // A retained sample can become obstructed after a crowd correction.
        // It must not erase the independently validated current wall constraint.
        local.estimates.get_mut(&24).unwrap().character.feet.x = 6.05;
        baseline.physics_step = 12;
        baseline.world_step = 108;
        baseline.character.feet.x = 0.7;
        baseline.held.refresh([1., 0.], 0).unwrap();
        local.observe(baseline, &source, 4, 4).unwrap();
        assert_eq!(
            local.last_reconciliation.as_ref().unwrap().path,
            "constrained_translation"
        );
        assert!(!local.estimates.contains_key(&24));
        local.advance(0.).unwrap();
        assert!(
            (local.pose().unwrap().position.x - 5.64999).abs() < 0.0001,
            "A later capsule erased processed pending travel: {:?}",
            local.pose().unwrap().position
        );
        assert_eq!(local.physics_step(), 108);
        assert_eq!(local.pending(), 9);
        assert!(local.estimates.len() <= super::super::MAX_PENDING_STEPS as usize + 1);
        local.advance(4. / 120.).unwrap();
        assert!((local.pose().unwrap().position.x - 5.64999).abs() < 0.0001);
    }

    #[test]
    fn motor_history_preserves_fixed_walls_external_motion_and_policy_changes() {
        use physics::queries::{ColliderKey, GeometrySnapshot, Pose, ShapeSnapshot, Usage};
        for case in 0..3 {
            let (mut local, mut baseline, mut source) = setup();
            baseline.profile = movement::Profile::Frames;
            baseline.epoch += 1;
            if case == 0 {
                source.colliders.push(ShapeSnapshot {
                    key: ColliderKey {
                        life: Life {
                            instance: 7,
                            entity: 0,
                            generation: 0,
                        },
                        shape: 1,
                    },
                    layers: 1,
                    usage: Usage::Blocking,
                    pose: Pose::default(),
                    geometry: GeometrySnapshot::Box {
                        min: glam::DVec3::new(2., 0., -2.),
                        max: glam::DVec3::new(2.1, 4., 2.),
                    },
                });
            }
            local.observe(baseline, &source, 2, 2).unwrap();
            local.queue(1, movement()).unwrap();
            for _ in 0..3 {
                local.advance(0.1).unwrap();
            }
            movement::advance(
                &mut baseline.character,
                local.collision.scene(),
                Filter::blocking(7),
                glam::DVec3::X * 6.4008,
                false,
                12,
                1. / 120.,
            )
            .unwrap();
            baseline.physics_step = 12;
            baseline.world_step = 36;
            baseline.held.refresh([1., 0.], 0).unwrap();
            let expected = match case {
                0 => {
                    baseline.character.feet.x += 0.1;
                    1.64999
                }
                1 => {
                    baseline.character.external.x = 2.;
                    2.04524
                }
                _ => {
                    baseline.policy.walking_scale = 0.5;
                    1.28016
                }
            };
            local.observe(baseline, &source, 3, 3).unwrap();
            assert_eq!(
                local.dirty,
                case != 0,
                "Only changed motor state requires replay"
            );
            local.advance(0.).unwrap();
            assert!(
                (local.pose().unwrap().position.x - expected).abs() < 0.0001,
                "Case {case}: {:?}",
                local.pose().unwrap().position
            );
            assert_eq!(local.physics_step(), 36);
            assert_eq!(local.pending(), 1);
            assert!(local.estimates.len() <= super::super::MAX_PENDING_STEPS as usize + 1);
            local.clear();
            assert!(local.estimates.is_empty());
        }
    }

    #[test]
    fn unchanged_confirmation_applies_new_fixed_blockers_only_to_future_travel() {
        use physics::queries::{ColliderKey, GeometrySnapshot, Pose, ShapeSnapshot};
        let (mut local, mut baseline, mut geometry) = setup();
        movement::advance(
            &mut baseline.character,
            local.collision.scene(),
            Filter::blocking(7),
            glam::DVec3::ZERO,
            false,
            1,
            1. / 120.,
        )
        .unwrap();
        baseline.profile = movement::Profile::Frames;
        baseline.epoch += 1;
        local.observe(baseline, &geometry, 2, 2).unwrap();
        local.queue(1, movement()).unwrap();
        local.advance(0.1).unwrap();
        local.advance(0.1).unwrap();
        let before = local.character.unwrap();
        let step = local.physics_step();
        // A corpse appears across the path already processed. Another new
        // blocker lies ahead, so retaining history must still update collision.
        for (entity, min_x, max_x) in [
            (216, 0.4, before.feet.x - 0.45),
            (217, before.feet.x + 0.45, before.feet.x + 1.0),
        ] {
            geometry.colliders.push(ShapeSnapshot {
                key: ColliderKey {
                    life: Life {
                        instance: 7,
                        entity,
                        generation: 0,
                    },
                    shape: 0,
                },
                layers: 1,
                usage: Usage::Blocking,
                pose: Pose::default(),
                geometry: GeometrySnapshot::Box {
                    min: glam::DVec3::new(min_x, 0., -0.5),
                    max: glam::DVec3::new(max_x, 2., 0.5),
                },
            });
        }
        baseline.world_step = step;
        local.observe(baseline, &geometry, 3, 3).unwrap();
        local.advance(0.).unwrap();
        assert_eq!(
            local.character.unwrap(),
            before,
            "Unchanged confirmation cannot replay historical travel through a new blocker"
        );
        assert_eq!(local.physics_step(), step);
        assert_eq!(local.pending(), 1);
        local.advance(4. / 120.).unwrap();
        let after = local.character.unwrap();
        assert!(after.feet.x > before.feet.x + 0.05);
        assert!(
            after.feet.x < before.feet.x + 0.11,
            "Future travel must stop at the new blocker: {:?}",
            after.feet
        );
        assert_eq!(local.physics_step(), step + 4);
    }

    #[test]
    fn fresh_interval_input_cannot_replay_processed_travel_against_a_later_crowd_pose() {
        use physics::queries::{GeometrySnapshot, Pose, ShapeSnapshot};
        let (mut local, mut baseline, mut geometry) = setup();
        baseline.profile = movement::Profile::Frames;
        baseline.epoch += 1;
        movement::advance(
            &mut baseline.character,
            local.collision.scene(),
            Filter::blocking(7),
            glam::DVec3::ZERO,
            false,
            1,
            1. / 120.,
        )
        .unwrap();
        baseline.character.external = glam::DVec3::new(0.1988262287, 0., 0.0657269547);
        local.observe(baseline, &geometry, 2, 2).unwrap();
        local.queue(1, movement()).unwrap();
        for _ in 0..6 {
            local.advance(0.1).unwrap();
        }
        local.advance(2. / 120.).unwrap();
        let before = local.character.unwrap();
        let capsule = physics::character::Settings::default().capsule(baseline.character.feet);
        geometry.colliders.push(ShapeSnapshot {
            key: ColliderKey {
                life: Life {
                    instance: 7,
                    entity: 216,
                    generation: 0,
                },
                shape: 0,
            },
            layers: 1,
            usage: Usage::Blocking,
            pose: Pose::default(),
            geometry: GeometrySnapshot::Capsule {
                a: capsule.a,
                b: capsule.b,
                radius: capsule.radius,
            },
        });
        local.update_geometry(&geometry, 3, 3).unwrap();
        local
            .queue(
                2,
                Intent::Move {
                    axes: [0., 1.],
                    yaw: 0.,
                },
            )
            .unwrap();
        baseline.world_step = local.physics_step();
        local.observe(baseline, &geometry, 4, 4).unwrap();
        local.advance(0.).unwrap();
        assert_eq!(
            local.character.unwrap(),
            before,
            "Fresh boundary input cannot rewrite historical travel"
        );
        assert_eq!(local.physics_step(), 74);
        assert_eq!(local.timing().simulated, 74);
        assert_eq!(local.pending(), 2);
        assert!(local.deferred_steps.is_empty());
        local.advance(4. / 120.).unwrap();
        let after = local.character.unwrap();
        assert!((after.feet.x - before.feet.x).abs() < 1e-8);
        assert!((after.feet.z - before.feet.z + 6.4008 * 4. / 120.).abs() < 1e-7);
        assert_eq!(local.physics_step(), 78);
        assert!(
            local
                .collision
                .is_capsule(geometry.colliders.last().unwrap().key)
        );
    }

    #[test]
    fn selection_only_changes_preserve_completed_pending_motion() {
        use physics::queries::{ColliderKey, GeometrySnapshot, Pose, ShapeSnapshot, Usage};
        for change in 0..3 {
            let (mut local, mut baseline, mut source) = setup();
            let selection = ShapeSnapshot {
                key: ColliderKey {
                    life: Life {
                        instance: 7,
                        entity: 300,
                        generation: 0,
                    },
                    shape: 0,
                },
                layers: 1,
                usage: Usage::Selection,
                pose: Pose::default(),
                geometry: GeometrySnapshot::Box {
                    min: glam::DVec3::new(100., 0., 100.),
                    max: glam::DVec3::new(101., 1., 101.),
                },
            };
            source.colliders.push(selection.clone());
            baseline.profile = movement::Profile::Frames;
            baseline.epoch += 1;
            local.observe(baseline, &source, 2, 2).unwrap();
            local.queue(1, movement()).unwrap();
            for _ in 0..3 {
                local.advance(0.1).unwrap();
            }
            let before = local.pose().unwrap().position;
            let completed = local.estimates[&12];
            baseline.physics_step = 12;
            baseline.world_step = 36;
            baseline.character = completed.character;
            baseline.held = completed.held;
            baseline.policy = completed.policy;
            baseline.yaw = completed.yaw;
            local.observe(baseline, &source, 3, 3).unwrap();
            local.advance(0.).unwrap();
            let capsule =
                physics::character::Settings::default().capsule(glam::DVec3::new(1.3, 0., 0.));
            source.colliders.push(ShapeSnapshot {
                key: ColliderKey {
                    life: Life {
                        instance: 7,
                        entity: 216,
                        generation: 0,
                    },
                    shape: 0,
                },
                layers: 1,
                usage: Usage::Blocking,
                pose: Pose::default(),
                geometry: GeometrySnapshot::Capsule {
                    a: capsule.a,
                    b: capsule.b,
                    radius: capsule.radius,
                },
            });
            match change {
                0 => {
                    let mut added = selection.clone();
                    added.key.life.entity += 1;
                    source.colliders.push(added);
                }
                1 => {
                    source
                        .colliders
                        .iter_mut()
                        .find(|shape| shape.key == selection.key)
                        .unwrap()
                        .pose
                        .position
                        .x += 1.;
                }
                _ => source.colliders.retain(|shape| shape.key != selection.key),
            }
            baseline.physics_step = 12;
            baseline.world_step = 36;
            baseline.character = completed.character;
            baseline.held = completed.held;
            baseline.policy = completed.policy;
            baseline.yaw = completed.yaw;
            local.observe(baseline, &source, 4, 4).unwrap();
            local.advance(0.).unwrap();
            assert!(
                local.pose().unwrap().position.distance(before) < 1e-7,
                "Selection change {change} rewound completed pending movement: {before:?} -> {:?}",
                local.pose().unwrap().position
            );
            assert_eq!(local.physics_step(), 36);
            assert_eq!(local.timing().simulated, 36);
            assert_eq!(local.pending(), 1);
        }
    }

    #[test]
    fn distant_blocking_prop_changes_preserve_completed_pending_motion() {
        use physics::queries::{ColliderKey, GeometrySnapshot, Pose, ShapeSnapshot, Usage};
        for change in 0..3 {
            let (mut local, mut baseline, mut source) = setup();
            let selection = ShapeSnapshot {
                key: ColliderKey {
                    life: Life {
                        instance: 7,
                        entity: 300,
                        generation: 0,
                    },
                    shape: 0,
                },
                layers: 1,
                usage: Usage::Blocking,
                pose: Pose::default(),
                geometry: GeometrySnapshot::Box {
                    min: glam::DVec3::new(100., 0., 100.),
                    max: glam::DVec3::new(101., 1., 101.),
                },
            };
            source.colliders.push(selection.clone());
            baseline.profile = movement::Profile::Frames;
            baseline.epoch += 1;
            local.observe(baseline, &source, 2, 2).unwrap();
            local.queue(1, movement()).unwrap();
            for _ in 0..3 {
                local.advance(0.1).unwrap();
            }
            let before = local.pose().unwrap().position;
            let completed = local.estimates[&12];
            baseline.physics_step = 12;
            baseline.world_step = 36;
            baseline.character = completed.character;
            baseline.held = completed.held;
            baseline.policy = completed.policy;
            baseline.yaw = completed.yaw;
            local.observe(baseline, &source, 3, 3).unwrap();
            local.advance(0.).unwrap();
            let capsule =
                physics::character::Settings::default().capsule(glam::DVec3::new(1.3, 0., 0.));
            source.colliders.push(ShapeSnapshot {
                key: ColliderKey {
                    life: Life {
                        instance: 7,
                        entity: 216,
                        generation: 0,
                    },
                    shape: 0,
                },
                layers: 1,
                usage: Usage::Blocking,
                pose: Pose::default(),
                geometry: GeometrySnapshot::Capsule {
                    a: capsule.a,
                    b: capsule.b,
                    radius: capsule.radius,
                },
            });
            match change {
                0 => {
                    let mut added = selection.clone();
                    added.key.life.entity += 1;
                    source.colliders.push(added);
                }
                1 => {
                    source
                        .colliders
                        .iter_mut()
                        .find(|shape| shape.key == selection.key)
                        .unwrap()
                        .pose
                        .position
                        .x += 1.;
                }
                _ => source.colliders.retain(|shape| shape.key != selection.key),
            }
            baseline.physics_step = 12;
            baseline.world_step = 36;
            baseline.character = completed.character;
            baseline.held = completed.held;
            baseline.policy = completed.policy;
            baseline.yaw = completed.yaw;
            local.observe(baseline, &source, 4, 4).unwrap();
            local.advance(0.).unwrap();
            assert!(
                local.pose().unwrap().position.distance(before) < 1e-7,
                "Distant blocker change {change} rewound completed pending movement: {before:?} -> {:?}",
                local.pose().unwrap().position
            );
            assert_eq!(local.physics_step(), 36);
            assert_eq!(local.timing().simulated, 36);
            assert_eq!(local.pending(), 1);
        }
    }

    #[test]
    fn free_current_correction_keeps_historical_wall_contacts_valid() {
        use physics::queries::{ColliderKey, GeometrySnapshot, Pose, ShapeSnapshot, Usage};
        let (mut local, mut baseline, mut source) = setup();
        baseline.profile = movement::Profile::Frames;
        baseline.epoch += 1;
        baseline.character.feet.x = 4.;
        source.colliders.push(ShapeSnapshot {
            key: ColliderKey {
                life: Life {
                    instance: 7,
                    entity: 0,
                    generation: 0,
                },
                shape: 1,
            },
            layers: 1,
            usage: Usage::Blocking,
            pose: Pose::default(),
            geometry: GeometrySnapshot::Box {
                min: glam::DVec3::new(6., 0., -2.),
                max: glam::DVec3::new(6.1, 4., 2.),
            },
        });
        local.observe(baseline, &source, 2, 2).unwrap();
        local.queue(1, movement()).unwrap();
        for _ in 0..3 {
            local.advance(0.1).unwrap();
        }
        assert!(local.pose().unwrap().position.x > 5.64);
        local
            .queue(
                2,
                Intent::Move {
                    axes: [-1., 0.],
                    yaw: 0.,
                },
            )
            .unwrap();
        local.advance(0.1).unwrap();
        local.advance(0.1).unwrap();
        let before = local.pose().unwrap().position;
        let completed = local.estimates[&12];
        baseline.physics_step = 12;
        baseline.world_step = local.physics_step();
        baseline.character = completed.character;
        baseline.character.feet.x += 0.1;
        baseline.held = completed.held;
        baseline.policy = completed.policy;
        baseline.yaw = completed.yaw;
        local.observe(baseline, &source, 3, 3).unwrap();
        local.advance(0.).unwrap();
        assert!((local.pose().unwrap().position.x - before.x - 0.1).abs() < 1e-6);
        let settings = physics::character::Settings::default();
        let mut previous = baseline.character.feet;
        for (step, estimate) in &local.estimates {
            assert!(
                estimate.character.feet.distance(previous) <= 6.4008 / 120. + 1e-5,
                "Correction introduced a discontinuous historical step {step}: {previous:?} -> {:?}",
                estimate.character.feet,
            );
            previous = estimate.character.feet;
            let overlaps = local
                .collision
                .scene()
                .overlap(
                    settings.capsule(estimate.character.feet),
                    Filter::blocking(7),
                )
                .unwrap();
            assert!(!overlaps.truncated);
            assert!(
                overlaps.hits.iter().all(|hit| hit.penetration <= 1e-5),
                "Correction embedded historical step {step} in a fixed wall: {:?}",
                estimate.character.feet
            );
        }
        assert_eq!(local.physics_step(), 60);
        assert_eq!(local.pending(), 2);
    }
    #[test]
    fn confirmed_time_reconciliation_does_not_replay_through_later_crowd_geometry() {
        use physics::queries::{ColliderKey, GeometrySnapshot, Pose, ShapeSnapshot, Usage};
        let (mut local, mut baseline, mut source) = setup();
        baseline.profile = movement::Profile::Frames;
        baseline.epoch += 1;
        local.observe(baseline, &source, 2, 2).unwrap();
        local.queue(1, movement()).unwrap();
        for _ in 0..3 {
            local.advance(0.1).unwrap();
        }
        let before = local.pose().unwrap().position;
        assert!((before.x - 1.92024).abs() < 0.0001);
        movement::advance(
            &mut baseline.character,
            local.collision.scene(),
            Filter::blocking(7),
            glam::DVec3::X * 6.4008,
            false,
            12,
            1. / 120.,
        )
        .unwrap();
        // This crowd pose is newer than the completed baseline below. It affects
        // future steps, and does not erase the already estimated pending path.
        let capsule =
            physics::character::Settings::default().capsule(glam::DVec3::new(0.64, 0., 0.));
        source.colliders.push(ShapeSnapshot {
            key: ColliderKey {
                life: Life {
                    instance: 7,
                    entity: 216,
                    generation: 0,
                },
                shape: 0,
            },
            layers: 1,
            usage: Usage::Blocking,
            pose: Pose::default(),
            geometry: GeometrySnapshot::Capsule {
                a: capsule.a,
                b: capsule.b,
                radius: capsule.radius,
            },
        });
        local.update_geometry(&source, 3, 3).unwrap();
        baseline.physics_step = 12;
        baseline.world_step = 36;
        baseline.character.feet.x = 0.7;
        baseline.held.refresh([1., 0.], 0).unwrap();
        local.observe(baseline, &source, 4, 4).unwrap();
        local.advance(0.).unwrap();
        assert!(
            (local.pose().unwrap().position.x - 1.98016).abs() < 0.0001,
            "{}",
            local.pose().unwrap().position.x
        );
        assert_eq!(local.physics_step(), 36);
        assert_eq!(local.pending(), 1);
        local.advance(4. / 120.).unwrap();
        assert!((local.pose().unwrap().position.x - 2.19352).abs() < 0.0001);
    }

    #[test]
    fn confirmed_exit_cannot_replay_previously_deferred_crowd_time() {
        use physics::queries::{ColliderKey, GeometrySnapshot, Pose, ShapeSnapshot, Usage};
        let (mut local, mut baseline, mut source) = setup();
        baseline.profile = movement::Profile::Frames;
        baseline.epoch += 1;
        let capsule =
            physics::character::Settings::default().capsule(glam::DVec3::new(-0.1, 0., 0.));
        source.colliders.push(ShapeSnapshot {
            key: ColliderKey {
                life: Life {
                    instance: 7,
                    entity: 216,
                    generation: 0,
                },
                shape: 0,
            },
            layers: 1,
            usage: Usage::Blocking,
            pose: Pose::default(),
            // Static embedding: only actor capsules are treated as stale
            // projections that a predicted character may walk past.
            geometry: GeometrySnapshot::Box {
                min: capsule.a - glam::DVec3::new(0.35, 0., 0.25),
                max: capsule.a + glam::DVec3::new(-0.1, 1., 0.25),
            },
        });
        local.observe(baseline, &source, 2, 2).unwrap();
        local.queue(1, movement()).unwrap();
        for _ in 0..3 {
            local.advance(0.1).unwrap();
        }
        assert_eq!(local.pose().unwrap().position, Vec3::ZERO);
        // A completed authority baseline exits the old projected overlap.
        // The estimate already held the remaining input time; an acknowledgment
        // cannot turn that old time into new travel through the frozen crowd.
        baseline.physics_step = 12;
        baseline.world_step = 36;
        baseline.character.feet.x = 0.75;
        local.observe(baseline, &source, 3, 3).unwrap();
        local.advance(0.).unwrap();
        assert_eq!(local.pose().unwrap().position.x, 0.75);
        assert_eq!(local.physics_step(), 36);
        assert_eq!(local.pending(), 1);
        assert_eq!(local.deferred_steps.len(), 24);
        assert!(local.deferred_steps.len() <= super::super::MAX_PENDING_STEPS as usize);
        local.advance(4. / 120.).unwrap();
        assert!((local.pose().unwrap().position.x - 0.96336).abs() < 0.0001);
        assert_eq!(local.physics_step(), 40);
        assert_eq!(local.pending(), 1);
        local.clear();
        assert!(local.deferred_steps.is_empty());
    }

    #[test]
    fn geometry_refresh_applies_to_future_steps_without_replaying_past_blocked_input() {
        use physics::queries::{ColliderKey, GeometrySnapshot, Pose, ShapeSnapshot, Usage};
        let (mut local, mut baseline, mut geometry) = setup();
        baseline.profile = movement::Profile::Frames;
        baseline.epoch += 1;
        let capsule =
            physics::character::Settings::default().capsule(glam::DVec3::new(1.2, 0., 0.));
        geometry.colliders.push(ShapeSnapshot {
            key: ColliderKey {
                life: Life {
                    instance: 7,
                    entity: 216,
                    generation: 0,
                },
                shape: 0,
            },
            layers: 1,
            usage: Usage::Blocking,
            pose: Pose::default(),
            geometry: GeometrySnapshot::Capsule {
                a: capsule.a,
                b: capsule.b,
                radius: capsule.radius,
            },
        });
        local.observe(baseline, &geometry, 2, 2).unwrap();
        local.queue(1, movement()).unwrap();
        local.advance(0.1).unwrap();
        let before = local.pose().unwrap().position;
        assert!((before.x - 0.5).abs() < 0.001);
        geometry.colliders.last_mut().unwrap().pose.position.x = 5.;
        baseline.world_step = 12;
        local.observe(baseline, &geometry, 3, 3).unwrap();
        local.advance(0.).unwrap();
        assert_eq!(local.pose().unwrap().position, before);
        assert_eq!(local.physics_step(), 12);
        local.advance(4. / 120.).unwrap();
        assert!((local.pose().unwrap().position.x - before.x - 0.21336).abs() < 0.001);
        assert_eq!(local.pending(), 1);
        // Withheld baselines update collision under the same future-only rule.
        geometry.colliders.last_mut().unwrap().pose.position.x = 10.;
        let before = local.pose().unwrap().position;
        local.update_geometry(&geometry, 4, 4).unwrap();
        local.advance(0.).unwrap();
        assert_eq!(local.pose().unwrap().position, before);
    }

    #[test]
    fn completed_confirmation_replays_existing_time_without_granting_a_new_interval() {
        use physics::queries::{ColliderKey, GeometrySnapshot, Pose, ShapeSnapshot, Usage};
        let (mut local, mut baseline, mut geometry) = setup();
        let capsule =
            physics::character::Settings::default().capsule(glam::DVec3::new(0., 0., 1.1));
        geometry.colliders.push(ShapeSnapshot {
            key: ColliderKey {
                life: Life {
                    instance: 7,
                    entity: 216,
                    generation: 0,
                },
                shape: 0,
            },
            layers: 1,
            usage: Usage::Blocking,
            pose: Pose::default(),
            geometry: GeometrySnapshot::Capsule {
                a: capsule.a,
                b: capsule.b,
                radius: capsule.radius,
            },
        });
        baseline.profile = movement::Profile::Frames;
        baseline.epoch += 1;
        local.observe(baseline, &geometry, 2, 2).unwrap();
        local.queue(1, movement()).unwrap();
        local.advance(0.1).unwrap();
        let mut first = local.movement_frame(0, 12).unwrap();
        first.sequence = 1;
        local.bind_movement_frame(&first).unwrap();
        let confirmed_character = local.character.unwrap();
        local.advance(0.1).unwrap();
        assert_eq!(local.physics_step(), 24);
        assert_eq!(local.timing().simulated, 24);
        assert!(
            local
                .character
                .unwrap()
                .feet
                .distance(confirmed_character.feet)
                > 0.6
        );
        assert_eq!(local.prediction_delay_steps(), 0);
        local
            .movement_credit(baseline.life, baseline.epoch, 24)
            .unwrap();
        let next = local.movement_frame(12, 12).unwrap();
        assert_eq!(next.start, 12);
        assert_eq!(next.steps, 12);
        baseline.character = confirmed_character;
        baseline.physics_step = 12;
        baseline.world_step = 24;
        baseline.applied_sequence = 1;
        baseline.held.refresh([1., 0.], 0).unwrap();
        let before = local.pose().unwrap().position;
        local.observe_applied(baseline, 3, 3).unwrap();
        local.advance(0.).unwrap();
        assert!(local.pose().unwrap().position.distance(before) < 0.00001);
        assert_eq!(local.physics_step(), 24);
        assert_eq!(local.timing().simulated, 24);
        local.advance(2. / 120.).unwrap();
        assert_eq!(local.timing().simulated, 26);
        assert!(local.pose().unwrap().position.distance(before) < 0.11);
        assert_eq!(local.prediction_delay_steps(), 0);
    }

    #[test]
    fn grounded_interval_bootstrap_aligns_only_verified_time_and_keeps_the_gap_neutral() {
        let (mut local, mut baseline, geometry) = setup();
        // Obtain an actual supported character before entering the interval profile.
        local.advance(1. / 120.).unwrap();
        baseline.character = local.character.unwrap();
        assert!(baseline.character.support.is_some());
        baseline.profile = movement::Profile::Frames;
        baseline.epoch += 1;
        baseline.world_step = 8;
        local.observe(baseline, &geometry, 2, 2).unwrap();
        assert_eq!(local.physics_step(), 8);
        assert_eq!(local.character.unwrap().feet, baseline.character.feet);
        local.queue(1, movement()).unwrap();
        local.advance(4. / 120.).unwrap();
        let frame = local.movement_frame(0, 12).unwrap();
        assert_eq!(frame.segments[0].axes, [0.; 2]);
        assert_eq!(frame.segments[1].offset, 8);
        assert_eq!(frame.segments[1].axes, [1., 0.]);
        // Ordinary credit changes permission only; it cannot repeat bootstrap.
        local
            .movement_credit(baseline.life, baseline.epoch, 20)
            .unwrap();
        assert_eq!(local.physics_step(), 12);
        assert!(local.pose().unwrap().position.x < 0.25);
    }

    #[test]
    fn applied_confirmation_retires_inputs_without_minting_time_or_reverting_to_old_travel() {
        let (mut local, mut baseline, geometry) = setup();
        baseline.profile = movement::Profile::Frames;
        baseline.epoch += 1;
        local.observe(baseline, &geometry, 2, 2).unwrap();
        local.queue(1, movement()).unwrap();
        local.advance(4. / 120.).unwrap();
        baseline.character = local.character.unwrap();
        baseline.held.refresh([1., 0.], 0).unwrap();
        baseline.physics_step = 4;
        baseline.world_step = 4;
        baseline.applied_sequence = 1;
        let mut frame = local.movement_frame(0, 4).unwrap();
        frame.sequence = 1;
        local.bind_movement_frame(&frame).unwrap();
        local.advance(8. / 120.).unwrap();
        let before = local.pose().unwrap();
        let time = local.timing();
        assert!(local.observe_applied(baseline, 3, 3).unwrap());
        local.advance(0.).unwrap();
        assert_eq!(local.pending(), 0);
        assert_eq!(local.timing().step, time.step);
        assert_eq!(local.timing().simulated, time.simulated);
        assert!(local.pose().unwrap().position.distance(before.position) < 0.00001);
        let mut stale = baseline;
        stale.applied_sequence = 0;
        assert!(!local.observe_applied(stale, 4, 4).unwrap());
        stale.epoch += 1;
        assert!(!local.observe_applied(stale, 4, 4).unwrap());
        assert_eq!(local.observation(), 3);
        let mut future = baseline;
        future.applied_sequence += 1;
        future.physics_step = local.physics_step() + 1;
        future.world_step = future.physics_step;
        assert!(local.observe_applied(future, 4, 4).is_err());
    }

    #[test]
    fn delayed_baselines_do_not_extend_the_clock_or_renew_unacknowledged_holds() {
        let (mut local, mut baseline, geometry) = setup();
        local.queue(1, movement()).unwrap();
        local.bind(1, &command(baseline, 1, movement())).unwrap();
        local.advance(0.1).unwrap();
        for observation in 2..=18 {
            baseline.physics_step += 4;
            let before = local.timing();
            local
                .observe(baseline, &geometry, observation, observation)
                .unwrap();
            let after = local.timing();
            assert_eq!(after.step, before.step);
            assert_eq!(after.simulated, before.simulated);
            assert_eq!(after.inputs[0].step, 0);
            local.advance(4. / 120.).unwrap();
        }
        assert_eq!(local.timing().step, 80);
        assert_eq!(local.pending(), 1);
        assert_eq!(local.pose().unwrap().axes, [0., 0.]);
        // The late acknowledgment retires the original hold without granting its travel.
        baseline.applied_sequence = 1;
        baseline.physics_step = 72;
        local.observe(baseline, &geometry, 19, 19).unwrap();
        local.advance(0.).unwrap();
        assert_eq!(local.pending(), 0);
        assert_eq!(local.timing().step, 80);
        assert_eq!(local.pose().unwrap().position, Vec3::ZERO);
    }

    #[test]
    fn overtaken_pending_intervals_preserve_move_jump_stop_and_expire() {
        let (mut local, mut baseline, geometry) = setup();
        let stop = Intent::Move {
            axes: [0., 0.],
            yaw: 0.,
        };
        local.queue(1, stop.clone()).unwrap();
        local.bind(1, &command(baseline, 1, stop.clone())).unwrap();
        local.advance(4. / 120.).unwrap();
        local.queue(2, movement()).unwrap();
        local.bind(2, &command(baseline, 2, movement())).unwrap();
        local.advance(4. / 120.).unwrap();
        local.queue(3, Intent::Jump).unwrap();
        local.bind(3, &command(baseline, 3, Intent::Jump)).unwrap();
        local.advance(4. / 120.).unwrap();
        local.queue(4, stop.clone()).unwrap();
        local.advance(4. / 120.).unwrap();
        let before = local.pose().unwrap();
        assert!(before.airborne && before.position.x > 0.4);
        baseline.physics_step = 36;
        baseline.applied_sequence = 1;
        local.observe(baseline, &geometry, 2, 2).unwrap();
        local.advance(0.).unwrap();
        let after = local.pose().unwrap();
        assert!(before.position.distance(after.position) < 0.00001);
        assert!((before.motion_time - after.motion_time).abs() < 0.00001);
        assert_eq!(local.timing().step, 48);
        assert_eq!(
            local
                .timing()
                .inputs
                .iter()
                .map(|i| i.step)
                .collect::<Vec<_>>(),
            vec![36, 40, 44]
        );
        assert_eq!(local.pending(), 3);
        // A later acknowledgment retires the move and jump, without replaying either.
        baseline.physics_step = 48;
        baseline.applied_sequence = 3;
        baseline.character = local.character.unwrap();
        local.observe(baseline, &geometry, 3, 3).unwrap();
        local.advance(0.).unwrap();
        assert_eq!(local.pending(), 1);
        assert_eq!(local.timing().inputs[0].token, 4);
        assert_eq!(local.timing().step, 48);
        assert_eq!(
            serde_json::to_vec(&local.character.unwrap()).unwrap(),
            serde_json::to_vec(&baseline.character).unwrap()
        );
        // Gravity advances only when another frame elapses, not on acknowledgment.
        local.advance(4. / 120.).unwrap();
        let mut expected = baseline.character;
        movement::advance(
            &mut expected,
            local.collision.scene(),
            Filter::blocking(7),
            glam::DVec3::ZERO,
            false,
            4,
            1. / 120.,
        )
        .unwrap();
        assert_eq!(
            serde_json::to_vec(&local.character.unwrap()).unwrap(),
            serde_json::to_vec(&expected).unwrap()
        );
        for _ in 0..22 {
            if local.advance(0.1).is_err() {
                break;
            }
        }
        assert!(local.context().is_none());
    }

    #[test]
    fn overtaken_unacknowledged_hold_keeps_its_age_and_expires() {
        let (mut local, mut baseline, geometry) = setup();
        local.queue(1, movement()).unwrap();
        local.advance(0.1).unwrap();
        baseline.physics_step = 60;
        local.observe(baseline, &geometry, 2, 2).unwrap();
        local.advance(0.).unwrap();
        assert_eq!(local.timing().step, 72);
        assert_eq!(local.timing().inputs[0].step, 60);
        for _ in 0..5 {
            local.advance(0.1).unwrap();
        }
        assert_eq!(local.pose().unwrap().axes, [0., 0.]);
        let stopped = local.pose().unwrap().position;
        local.advance(0.1).unwrap();
        assert_eq!(local.pose().unwrap().position, stopped);
        baseline.epoch += 1;
        baseline.physics_step = 84;
        local.observe(baseline, &geometry, 3, 3).unwrap();
        local.advance(0.).unwrap();
        assert_eq!(local.pending(), 0);
        assert_eq!(local.timing().step, 84);
        assert_eq!(local.pose().unwrap().position, glam::Vec3::ZERO);
    }

    #[test]
    fn timing_distinguishes_local_steps_input_binding_and_authoritative_ack() {
        let (mut local, mut baseline, geometry) = setup();
        local.queue(1, movement()).unwrap();
        local.advance(0.1).unwrap();
        local.queue(2, Intent::Jump).unwrap();
        local.bind(1, &command(baseline, 1, movement())).unwrap();
        let before = local.pose().unwrap().position;
        let timing = local.timing();
        assert_eq!(timing.steps_per_second, 120);
        assert_eq!(timing.step, 12);
        assert_eq!(timing.simulated, 12);
        assert_eq!(timing.baseline_step, Some(0));
        assert_eq!(timing.inputs[0].step, 0);
        assert_eq!(timing.inputs[0].sequence, Some(1));
        assert_eq!(timing.inputs[1].step, 12);
        assert_eq!(timing.inputs[1].sequence, None);
        assert_eq!(local.pose().unwrap().position, before);
        baseline.physics_step = 8;
        baseline.applied_sequence = 1;
        local.observe(baseline, &geometry, 2, 2).unwrap();
        let timing = local.timing();
        assert_eq!(timing.baseline_step, Some(8));
        assert_eq!(timing.step, 12);
        assert_eq!(timing.inputs.len(), 1);
        assert_eq!(timing.inputs[0].token, 2);
        let serialized = serde_json::to_value(&timing).unwrap();
        assert_eq!(serialized["steps_per_second"], 120);
        local.clear();
        assert!(local.timing().inputs.is_empty());
        assert_eq!(local.timing().baseline_step, None);
    }
    #[test]
    fn chained_unsent_replacements_preserve_motion_and_retire_with_the_real_ack() {
        let (mut local, mut baseline, geometry) = setup();
        let stop = Intent::Move {
            axes: [0., 0.],
            yaw: 0.,
        };
        local.queue(1, movement()).unwrap();
        local.advance(0.1).unwrap();
        local.queue(2, stop.clone()).unwrap();
        local.advance(0.1).unwrap();
        let before = local.pose().unwrap().position;
        assert!(before.x > 0.6);
        local.supersede(1, 2).unwrap();
        local.advance(0.).unwrap();
        assert_eq!(local.pose().unwrap().position, before);
        local.queue(3, stop.clone()).unwrap();
        local.supersede(2, 3).unwrap();
        local.advance(0.).unwrap();
        assert_eq!(local.pose().unwrap().position, before);
        assert!(local.bind(1, &command(baseline, 1, movement())).is_err());
        local.bind(3, &command(baseline, 1, stop)).unwrap();
        assert!(local.inputs.iter().all(|i| i.sequence == Some(1)));
        local.queue(4, Intent::Jump).unwrap();
        local.bind(4, &command(baseline, 2, Intent::Jump)).unwrap();
        // Authority only received the stop; it never grants the earlier local travel.
        baseline.physics_step = 24;
        baseline.applied_sequence = 1;
        local.observe(baseline, &geometry, 2, 2).unwrap();
        local.advance(0.).unwrap();
        assert_eq!(local.pending(), 1);
        assert_eq!(local.pose().unwrap().position, glam::Vec3::ZERO);
        local.reject(4);
        assert_eq!(local.pending(), 0);
    }
    #[test]
    fn refused_replacement_cancels_its_entire_bounded_local_chain() {
        let (mut local, baseline, _) = setup();
        local.queue(1, movement()).unwrap();
        local.advance(0.1).unwrap();
        local.queue(2, movement()).unwrap();
        local.supersede(1, 2).unwrap();
        assert!(local.supersede(2, 1).is_err());
        assert!(local.supersede(1, 2).is_err());
        assert!(local.supersede(2, 99).is_err());
        local.queue(3, Intent::Jump).unwrap();
        assert!(local.supersede(2, 3).is_err());
        local.bind(2, &command(baseline, 1, movement())).unwrap();
        local.reject(2);
        assert_eq!(local.pending(), 1);
        assert!(local.contains(3));
        local.clear();
        assert!(local.supersede(1, 2).is_err());
        assert_eq!(local.pending(), 0);
    }
    #[test]
    fn input_moves_before_binding_and_acknowledgment_then_uses_authoritative_hold() {
        let (mut local, mut baseline, geometry) = setup();
        local.queue(1, movement()).unwrap();
        local.advance(1. / 30.).unwrap();
        assert!((local.pose().unwrap().position.x - 6.4008 / 30.).abs() < 0.0001);
        assert!(local.pose().unwrap().moving);
        assert!((local.pose().unwrap().motion_time - 1. / 30.).abs() < 0.0001);
        local.bind(1, &command(baseline, 1, movement())).unwrap();
        baseline.character = local.character.unwrap();
        baseline.physics_step = 4;
        baseline.applied_sequence = 1;
        baseline.held.refresh([1., 0.], 0).unwrap();
        local.observe(baseline, &geometry, 2, 2).unwrap();
        local.advance(0.).unwrap();
        assert_eq!(local.pending(), 0);
        assert_eq!(
            serde_json::to_vec(&local.character.unwrap()).unwrap(),
            serde_json::to_vec(&baseline.character).unwrap()
        );
        local.advance(1. / 30.).unwrap();
        assert!((local.pose().unwrap().position.x - 6.4008 * 2. / 30.).abs() < 0.0001);
        let before = local.pose().unwrap().position;
        assert!(local.observe(baseline, &geometry, 1, 1).is_err());
        assert_eq!(local.pose().unwrap().position, before);
    }
    #[test]
    fn accepted_jump_is_not_repeated_and_rejected_movement_is_removed() {
        let (mut local, mut baseline, geometry) = setup();
        local.queue(1, Intent::Jump).unwrap();
        local.advance(1. / 30.).unwrap();
        assert!(local.pose().unwrap().position.y > 0.);
        local.bind(1, &command(baseline, 1, Intent::Jump)).unwrap();
        baseline.character = local.character.unwrap();
        baseline.physics_step = 4;
        baseline.applied_sequence = 1;
        local.observe(baseline, &geometry, 2, 2).unwrap();
        local.advance(1. / 120.).unwrap();
        let mut expected = baseline.character;
        movement::advance(
            &mut expected,
            local.collision.scene(),
            Filter::blocking(7),
            glam::DVec3::ZERO,
            false,
            1,
            1. / 120.,
        )
        .unwrap();
        assert_eq!(
            serde_json::to_vec(&local.character.unwrap()).unwrap(),
            serde_json::to_vec(&expected).unwrap()
        );
        local.queue(2, movement()).unwrap();
        local.advance(1. / 30.).unwrap();
        assert!(local.pose().unwrap().position.x > 0.);
        local.reject(2);
        local.advance(0.).unwrap();
        assert_eq!(local.pose().unwrap().position.x, 0.);
        assert!(local.bind(99, &command(baseline, 2, movement())).is_err());
    }
    #[test]
    fn pending_geometry_corrects_unacknowledged_travel_and_retires_removed_walls() {
        use physics::queries::{GeometrySnapshot, Pose, ShapeSnapshot};
        for profile in [movement::Profile::Arrival, movement::Profile::Frames] {
            let (mut local, mut baseline, mut geometry) = setup();
            baseline.profile = profile;
            baseline.epoch += 1;
            local.observe(baseline, &geometry, 1, 2).unwrap();
            local.queue(1, movement()).unwrap();
            for _ in 0..5 {
                local.advance(0.1).unwrap();
            }
            assert!(local.pose().unwrap().position.x > 3.);
            geometry.colliders.push(ShapeSnapshot {
                key: ColliderKey {
                    life: Life {
                        instance: 7,
                        entity: 99,
                        generation: 1,
                    },
                    shape: 0,
                },
                layers: 1,
                usage: Usage::Blocking,
                pose: Pose::default(),
                geometry: GeometrySnapshot::Box {
                    min: glam::DVec3::new(1., 0., -5.),
                    max: glam::DVec3::new(1.01, 3., 5.),
                },
            });
            local.update_geometry(&geometry, 2, 3).unwrap();
            local.advance(0.).unwrap();
            assert!(local.pose().unwrap().position.x < 0.71);
            assert_eq!(local.pending(), 1);
            let stopped = local.pose().unwrap().position;
            assert!(local.update_geometry(&geometry, 1, 4).is_err());
            assert!(local.update_geometry(&geometry, 2, 3).is_err());
            let mut foreign = geometry.clone();
            foreign.instance = 8;
            assert!(local.update_geometry(&foreign, 3, 4).is_err());
            assert_eq!(local.pose().unwrap().position, stopped);
            assert!(local.observe(baseline, &geometry, 2, 3).is_err());
            geometry.colliders.pop();
            local.update_geometry(&geometry, 3, 4).unwrap();
            local.advance(0.).unwrap();
            assert!(local.pose().unwrap().position.x > 3.);
            assert_eq!(local.observation(), 4);
        }
    }
    #[test]
    fn movement_policy_preserves_slowing_and_control_routing() {
        let (mut local, mut baseline, geometry) = setup();
        baseline.policy.walking_scale = 0.5;
        local.observe(baseline, &geometry, 2, 2).unwrap();
        local.queue(1, movement()).unwrap();
        local.advance(1. / 30.).unwrap();
        assert!((local.pose().unwrap().position.x - 6.4008 / 60.).abs() < 0.0001);
        local.clear();
        baseline.policy.walking_scale = 0.;
        baseline.policy.jump_allowed = false;
        local.observe(baseline, &geometry, 3, 3).unwrap();
        local.queue(2, movement()).unwrap();
        local.queue(3, Intent::Jump).unwrap();
        local.advance(1. / 30.).unwrap();
        assert_eq!(local.pose().unwrap().position, Vec3::ZERO);
        assert_eq!(local.pose().unwrap().axes, [0.; 2]);
        local.bind(2, &command(baseline, 2, movement())).unwrap();
        assert!(local.bind(3, &command(baseline, 2, Intent::Jump)).is_err());
        assert!(local.bind(2, &command(baseline, 2, movement())).is_err());
    }
    #[test]
    fn lifecycle_clear_and_bounded_history_never_replay_old_inputs() {
        let (mut local, mut baseline, geometry) = setup();
        for token in 1..=super::super::CAPACITY as u64 {
            local.queue(token, movement()).unwrap();
        }
        assert!(local.queue(65, movement()).is_err());
        assert_eq!(local.pending(), super::super::CAPACITY);
        baseline.epoch += 1;
        baseline.life.generation += 1;
        local.observe(baseline, &geometry, 2, 2).unwrap();
        local.advance(0.).unwrap();
        assert_eq!(local.pending(), 0);
        assert_eq!(local.pose().unwrap().position, Vec3::ZERO);
        local.clear();
        assert!(local.pose().is_none());
        assert!(local.queue(66, movement()).is_err());
        local.observe(baseline, &geometry, 3, 3).unwrap();
        for _ in 0..21 {
            local.advance(0.1).unwrap();
        }
        assert!(local.advance(0.1).is_err());
        assert!(local.pose().is_none());
    }

    #[test]
    fn a_corpse_support_never_carries_by_the_living_capsule_pose() {
        use physics::queries::{ColliderKey, GeometrySnapshot, Pose, ShapeSnapshot, Usage};
        // Retained 600-second battle (run 1, player 15, actor 241): the applied
        // confirmation stood the player 0.24 m up on actor 240's corpse, whose
        // box has an identity pose. The client's scene still held that life's
        // living capsule, posed at the actor. Replaying from the baseline then
        // carried the player by the actor's whole position (3.47 m, no input).
        let (mut local, mut baseline, source) = setup();
        let dead = Life {
            instance: 7,
            entity: 240,
            generation: 1,
        };
        let actor_feet = glam::DVec3::new(-0.42, 0., 3.47);
        let feet = glam::DVec3::new(-0.157, 0., 4.139);
        // The client's last geometry: the actor alive, its capsule at its body.
        let mut client = source.clone();
        let settings = physics::character::Settings::default();
        client.colliders.push(ShapeSnapshot {
            key: ColliderKey {
                life: dead,
                shape: 0,
            },
            layers: 2,
            usage: Usage::Blocking,
            pose: Pose {
                position: actor_feet + glam::DVec3::Y * 0.9,
                rotation: glam::DQuat::IDENTITY,
            },
            geometry: GeometrySnapshot::Capsule {
                a: glam::DVec3::Y * (0.35 - 0.9),
                b: glam::DVec3::Y * (0.9 - 0.35),
                radius: 0.35,
            },
        });
        // Authority: the actor died, and its corpse is a navigation blocker.
        let mut blockers = physics::walkable::Blockers::new(7);
        blockers
            .upsert(
                dead,
                actor_feet + glam::DVec3::new(-0.35, 0., -0.8),
                actor_feet + glam::DVec3::new(0.35, 0.24, 0.8),
            )
            .unwrap();
        let mut authority = source.compile(7).unwrap();
        for collider in blockers.colliders().unwrap() {
            authority.insert(collider).unwrap();
        }
        let mut on_ground = physics::character::Character::new(feet);
        on_ground
            .step(
                &client.compile(7).unwrap(),
                Filter::blocking(7),
                settings,
                glam::DVec3::ZERO,
                false,
                1. / 120.,
            )
            .unwrap();
        assert!(on_ground.support.is_some_and(|s| s.life.entity == 0));
        baseline.profile = movement::Profile::Frames;
        baseline.epoch += 1;
        baseline.character = on_ground;
        local.observe(baseline, &client, 2, 2).unwrap();
        local.advance(4. / 120.).unwrap();
        let mut on_corpse = physics::character::Character::new(feet + glam::DVec3::Y * 0.24);
        on_corpse
            .step(
                &authority,
                Filter::blocking(7),
                settings,
                glam::DVec3::ZERO,
                false,
                1. / 120.,
            )
            .unwrap();
        assert_eq!(on_corpse.support.map(|s| s.life), Some(dead));
        let mut confirmed = baseline;
        confirmed.character = on_corpse;
        confirmed.physics_step = 4;
        confirmed.world_step = 4;
        confirmed.applied_sequence = 1;
        assert!(local.observe_applied(confirmed, 3, 3).unwrap());
        local.advance(1. / 120.).unwrap();
        let moved = local.pose().unwrap().position.as_dvec3() - on_corpse.feet;
        assert!(
            glam::DVec2::new(moved.x, moved.z).length() < 0.01,
            "Prediction carried the player {moved:?} with no input"
        );
    }

    #[test]
    fn confirmed_loose_prop_poses_replace_a_stale_snapshot_pose() {
        use physics::queries::{ColliderKey, GeometrySnapshot, Pose, ShapeSnapshot, Usage};
        // Retained 600-second battle (run 2, player 1): the loose ritual
        // crate stood in the client's last snapshot where the authority had
        // already pushed it away, and the prediction climbed it while the
        // authority walked the ground.
        let crate_key = ColliderKey {
            life: Life {
                instance: 7,
                entity: crate::spells::PROP_ENTITY_BASE,
                generation: 0,
            },
            shape: 0,
        };
        let walk = |refresh: bool| {
            let (mut local, mut baseline, mut source) = setup();
            // Find the walking direction, then stand a 1.2 m box 1.5 m along it.
            let mut probe = Local::new(7);
            probe.observe(baseline, &source, 1, 1).unwrap();
            probe.queue(1, movement()).unwrap();
            probe.advance(0.1).unwrap();
            let direction = probe.pose().unwrap().position.as_dvec3().normalize();
            source.colliders.push(ShapeSnapshot {
                key: crate_key,
                layers: 1,
                usage: Usage::Blocking,
                pose: Pose {
                    position: direction * 1.5 + glam::DVec3::Y * 0.6,
                    rotation: glam::DQuat::IDENTITY,
                },
                geometry: GeometrySnapshot::Box {
                    min: glam::DVec3::splat(-0.6),
                    max: glam::DVec3::splat(0.6),
                },
            });
            baseline.epoch += 1;
            baseline.profile = movement::Profile::Frames;
            local.observe(baseline, &source, 2, 2).unwrap();
            if refresh {
                let moved = local
                    .observe_dynamic_poses(&[crate::service::wire::ColliderPose {
                        key: crate_key,
                        pose: Pose {
                            position: glam::DVec3::new(15., 0.6, 15.),
                            rotation: glam::DQuat::IDENTITY,
                        },
                    }])
                    .unwrap();
                assert_eq!(moved, 1);
            }
            local.queue(1, movement()).unwrap();
            for _ in 0..6 {
                local.advance(0.1).unwrap();
            }
            local.pose().unwrap().position
        };
        let stale = walk(false);
        let fresh = walk(true);
        assert!(fresh.y.abs() < 1e-4, "{fresh:?}");
        assert!(fresh.length() > 1.5, "{fresh:?}");
        assert!(
            stale.y > 0.3 || stale.length() < 1.0,
            "the crate never mattered: {stale:?}"
        );
    }
}
