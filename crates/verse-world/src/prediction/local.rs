//! Local input timing and presentation estimates for one authenticated connection.
use crate::{
    Command, Intent,
    movement::{self, Baseline},
    play::Ability,
};
use glam::Vec3;
use physics::queries::{Filter, Life, SceneCache, SceneSnapshot};
use std::collections::VecDeque;
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
#[derive(serde::Serialize)]
pub struct Timing {
    steps_per_second: u16,
    step: u64,
    simulated: u64,
    fraction: f64,
    baseline_step: Option<u64>,
    inputs: Vec<TimedInput>,
}
#[derive(serde::Serialize)]
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
pub struct Local {
    collision: SceneCache,
    baseline: Option<Baseline>,
    inputs: VecDeque<Input>,
    token: u64,
    tick: u64,
    observation: u64,
    step: u64,
    simulated: u64,
    fraction: f64,
    character: Option<physics::character::Character>,
    yaw: f32,
    dirty: bool,
    moving: bool,
    motion_time: f32,
}
impl Local {
    pub fn new(instance: u64) -> Self {
        Self {
            collision: SceneCache::new(instance),
            baseline: None,
            inputs: VecDeque::new(),
            token: 0,
            tick: 0,
            observation: 0,
            step: 0,
            simulated: 0,
            fraction: 0.,
            character: None,
            yaw: 0.,
            dirty: false,
            moving: false,
            motion_time: 0.,
        }
    }
    pub fn clear(&mut self) {
        self.baseline = None;
        self.inputs.clear();
        self.character = None;
        self.fraction = 0.;
        self.motion_time = 0.;
        self.moving = false;
    }
    pub fn context(&self) -> Option<(LifeId, u64)> {
        self.baseline.map(|b| (b.life, b.epoch))
    }
    /// Captures timing without advancing, correcting, or rebinding predicted input.
    pub fn timing(&self) -> Timing {
        Timing {
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
    pub fn observe(
        &mut self,
        baseline: Baseline,
        geometry: &SceneSnapshot,
        tick: u64,
        observation: u64,
    ) -> Result<(), String> {
        baseline.validate()?;
        if let Some(old) = self.baseline {
            if observation <= self.observation
                || tick < self.tick
                || baseline.life.instance != old.life.instance
                || baseline.life.actor != old.life.actor
                || baseline.life.generation < old.life.generation
                || baseline.epoch < old.epoch
                || (baseline.epoch == old.epoch
                    && (baseline.physics_step < old.physics_step
                        || baseline.applied_sequence < old.applied_sequence
                        || baseline.life != old.life))
            {
                return Err("Local prediction observation regressed".into());
            }
        }
        self.collision.update(geometry)?;
        if self.context() != Some((baseline.life, baseline.epoch)) {
            self.inputs.clear();
            self.step = baseline.physics_step;
            self.fraction = 0.;
            self.motion_time = 0.;
            self.moving = false;
        } else {
            self.step = self.step.max(baseline.physics_step);
            self.inputs
                .retain(|i| i.sequence.is_none_or(|s| s > baseline.applied_sequence));
        }
        self.character = Some(baseline.character);
        self.yaw = baseline.yaw;
        self.baseline = Some(baseline);
        self.tick = tick;
        self.observation = observation;
        self.dirty = true;
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
        self.collision.update(geometry)?;
        self.tick = tick;
        self.observation = observation;
        self.dirty = true;
        Ok(())
    }
    pub fn queue(&mut self, token: u64, intent: Intent<Ability>) -> Result<(), String> {
        if self.baseline.is_none()
            || token == 0
            || token <= self.token
            || self.inputs.len() >= super::CAPACITY
        {
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
        self.token = token;
        self.inputs.push_back(Input {
            token,
            sequence: None,
            superseded_by: None,
            step: self.step,
            intent,
        });
        self.dirty = true;
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
        self.step = self
            .step
            .checked_add(steps)
            .ok_or("Local prediction clock exhausted")?;
        if self.step.saturating_sub(baseline.physics_step) > u64::from(super::MAX_PENDING_STEPS) {
            self.clear();
            return Err("Local prediction exceeded its correction horizon".into());
        }
        let from = if self.dirty {
            baseline.physics_step
        } else {
            self.simulated
        };
        let mut character = if self.dirty {
            baseline.character
        } else {
            self.character.unwrap_or(baseline.character)
        };
        let mut filter = Filter::blocking(baseline.life.instance);
        filter.ignore = Some(Life {
            instance: baseline.life.instance,
            entity: baseline.life.actor,
            generation: baseline.life.generation,
        });
        for step in from..self.step {
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
            let walk = movement::walk(held.axes(step), yaw)?;
            let velocity = baseline.policy.velocity(held.axes(step), yaw)?;
            movement::advance(
                &mut character,
                self.collision.scene(),
                filter,
                velocity,
                jump && baseline.policy.jump_allowed,
                1,
                1. / 120.,
            )?;
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
        self.simulated = self.step;
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
        let (mut local, baseline, mut geometry) = setup();
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
        local.update_geometry(&geometry, 2, 2).unwrap();
        local.advance(0.).unwrap();
        assert!(local.pose().unwrap().position.x < 0.71);
        assert_eq!(local.pending(), 1);
        let stopped = local.pose().unwrap().position;
        assert!(local.update_geometry(&geometry, 1, 3).is_err());
        assert!(local.update_geometry(&geometry, 2, 2).is_err());
        let mut foreign = geometry.clone();
        foreign.instance = 8;
        assert!(local.update_geometry(&foreign, 3, 3).is_err());
        assert_eq!(local.pose().unwrap().position, stopped);
        assert!(local.observe(baseline, &geometry, 2, 2).is_err());
        geometry.colliders.pop();
        local.update_geometry(&geometry, 3, 3).unwrap();
        local.advance(0.).unwrap();
        assert!(local.pose().unwrap().position.x > 3.);
        assert_eq!(local.observation(), 3);
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
}
