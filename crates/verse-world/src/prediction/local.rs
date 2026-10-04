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
struct Input {
    token: u64,
    sequence: Option<u64>,
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
            step: self.step,
            intent,
        });
        self.dirty = true;
        Ok(())
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
            || self.inputs.iter().any(|other| {
                other.sequence.is_some_and(|sequence| {
                    (other.token < token && sequence >= command.sequence)
                        || (other.token > token && sequence <= command.sequence)
                })
            })
        {
            return Err("Predicted input binding does not match transmitted control".into());
        }
        self.inputs[index].sequence = Some(command.sequence);
        Ok(())
    }
    pub fn reject(&mut self, token: u64) {
        self.inputs.retain(|i| i.token != token);
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
