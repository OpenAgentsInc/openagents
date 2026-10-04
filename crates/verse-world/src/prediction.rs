//! Bounded capsule replay over authoritative applied movement baselines.
use crate::movement::{self, Baseline};
use glam::DVec3;
use physics::{
    character::Character,
    queries::{Filter, Scene},
};
use std::collections::VecDeque;
use verse_engine::core::LifeId;

pub const CAPACITY: usize = 64;
pub const MAX_PENDING_STEPS: u32 = 256;
/// One locally simulated movement interval, bound to its transmitted command.
#[derive(Clone, Copy, Debug)]
pub struct Frame {
    pub life: LifeId,
    pub epoch: u64,
    pub sequence: u64,
    /// Effective walking velocity after shared movement modifiers, m/s.
    pub velocity: DVec3,
    pub yaw: f32,
    pub jump: bool,
    pub steps: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Correction {
    Replayed,
    Duplicate,
    Reset,
}
/// Owns presentation estimates only; never submits commands or changes authority.
pub struct History {
    baseline: Baseline,
    tick: u64,
    observation: u64,
    encoded: Vec<u8>,
    pending: VecDeque<Frame>,
    character: Character,
    yaw: f32,
    last_sequence: u64,
}
impl History {
    pub fn new(baseline: Baseline, tick: u64, observation: u64) -> Result<Self, String> {
        baseline.validate()?;
        Ok(Self {
            encoded: serde_json::to_vec(&baseline).map_err(|e| e.to_string())?,
            tick,
            observation,
            character: baseline.character,
            yaw: baseline.yaw,
            last_sequence: baseline.applied_sequence,
            baseline,
            pending: VecDeque::new(),
        })
    }
    fn check_filter(life: LifeId, filter: Filter) -> Result<(), String> {
        if filter.instance != life.instance
            || filter.ignore.is_some_and(|ignored| {
                ignored.instance != life.instance
                    || ignored.entity != life.actor
                    || ignored.generation != life.generation
            })
        {
            return Err("Prediction collision filter life mismatch".into());
        }
        Ok(())
    }
    pub fn character(&self) -> &Character {
        &self.character
    }
    pub fn yaw(&self) -> f32 {
        self.yaw
    }
    pub fn pending(&self) -> usize {
        self.pending.len()
    }
    pub fn predict(&mut self, frame: Frame, scene: &Scene, filter: Filter) -> Result<(), String> {
        Self::check_filter(frame.life, filter)?;
        if frame.life != self.baseline.life || frame.epoch != self.baseline.epoch {
            return Err("Predicted input control mismatch".into());
        }
        if frame.sequence <= self.last_sequence
            || !frame.yaw.is_finite()
            || frame.steps == 0
            || frame.steps > 12
            || !frame.velocity.is_finite()
            || frame.velocity.length() > 100.
        {
            return Err("Invalid predicted movement interval".into());
        }
        if self.pending.len() >= CAPACITY
            || self.pending.iter().map(|f| f.steps).sum::<u32>() + frame.steps > MAX_PENDING_STEPS
        {
            return Err("Movement prediction history is full".into());
        }
        let mut next = self.character;
        movement::advance(
            &mut next,
            scene,
            filter,
            frame.velocity,
            frame.jump,
            frame.steps,
            1. / 120.,
        )?;
        self.character = next;
        self.yaw = frame.yaw;
        self.last_sequence = frame.sequence;
        self.pending.push_back(frame);
        Ok(())
    }
    /// `observation` increases per snapshot delivery within one connection.
    /// Recreate history on disconnect, content replacement, or ownership change.
    pub fn reconcile(
        &mut self,
        baseline: Baseline,
        tick: u64,
        observation: u64,
        scene: &Scene,
        filter: Filter,
    ) -> Result<Correction, String> {
        baseline.validate()?;
        Self::check_filter(baseline.life, filter)?;
        if baseline.life.instance != self.baseline.life.instance
            || baseline.life.actor != self.baseline.life.actor
            || baseline.life.generation < self.baseline.life.generation
            || baseline.epoch < self.baseline.epoch
            || (baseline.physics_step < self.baseline.physics_step
                && baseline.epoch == self.baseline.epoch)
            || tick < self.tick
            || observation < self.observation
        {
            return Err("Movement correction context regressed".into());
        }
        if baseline.life.generation != self.baseline.life.generation
            && baseline.epoch == self.baseline.epoch
        {
            return Err("Movement life changed without a new control epoch".into());
        }
        let encoded = serde_json::to_vec(&baseline).map_err(|e| e.to_string())?;
        let reset = baseline.life != self.baseline.life || baseline.epoch != self.baseline.epoch;
        if observation == self.observation {
            if tick != self.tick || encoded != self.encoded {
                return Err("Conflicting movement baseline in one observation".into());
            }
            return Ok(Correction::Duplicate);
        }
        if !reset && baseline.applied_sequence < self.baseline.applied_sequence {
            return Err("Applied movement sequence regressed".into());
        }
        let pending: VecDeque<_> = if reset {
            VecDeque::new()
        } else {
            self.pending
                .iter()
                .copied()
                .filter(|f| f.sequence > baseline.applied_sequence)
                .collect()
        };
        let mut next = baseline.character;
        let mut yaw = baseline.yaw;
        for frame in &pending {
            movement::advance(
                &mut next,
                scene,
                filter,
                frame.velocity,
                frame.jump,
                frame.steps,
                1. / 120.,
            )?;
            yaw = frame.yaw;
        }
        self.character = next;
        self.yaw = yaw;
        self.last_sequence = pending
            .back()
            .map_or(baseline.applied_sequence, |f| f.sequence);
        self.pending = pending;
        self.baseline = baseline;
        self.tick = tick;
        self.observation = observation;
        self.encoded = encoded;
        Ok(if reset {
            Correction::Reset
        } else {
            Correction::Replayed
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use physics::queries::{ColliderKey, Life, Mesh, MeshCollider, Usage};
    fn geometry() -> Scene {
        let mut s = Scene::default();
        s.insert(MeshCollider {
            key: ColliderKey {
                life: Life {
                    instance: 1,
                    entity: 0,
                    generation: 0,
                },
                shape: 0,
            },
            layers: 1,
            usage: Usage::Blocking,
            mesh: Mesh::from_box(DVec3::new(-20., -1., -20.), DVec3::new(20., 0., 20.)).unwrap(),
        })
        .unwrap();
        s
    }
    fn baseline() -> Baseline {
        Baseline {
            life: LifeId {
                instance: 1,
                actor: 14,
                generation: 0,
            },
            epoch: 3,
            applied_sequence: 0,
            physics_step: 0,
            held: Default::default(),
            character: Character::new(DVec3::ZERO),
            yaw: 0.,
        }
    }
    fn frame(sequence: u64) -> Frame {
        Frame {
            life: baseline().life,
            epoch: 3,
            sequence,
            velocity: DVec3::X * 6.4008,
            yaw: 0.,
            jump: false,
            steps: 4,
        }
    }
    #[test]
    fn physics_watermarks_cannot_regress_without_a_control_reset() {
        let s = geometry();
        let mut initial = baseline();
        initial.physics_step = 10;
        let mut h = History::new(initial, 3, 1).unwrap();
        let mut stale = initial;
        stale.physics_step = 9;
        assert!(h.reconcile(stale, 4, 2, &s, Filter::blocking(1)).is_err());
        let mut reset = initial;
        reset.epoch += 1;
        reset.physics_step = 0;
        assert_eq!(
            h.reconcile(reset, 4, 2, &s, Filter::blocking(1)).unwrap(),
            Correction::Reset
        );
    }
    #[test]
    fn delayed_ack_discards_only_applied_inputs_and_replays_exact_motor_state() {
        let s = geometry();
        let f = Filter::blocking(1);
        let mut h = History::new(baseline(), 0, 0).unwrap();
        for seq in 1..=3 {
            h.predict(frame(seq), &s, f).unwrap();
        }
        let expected = serde_json::to_vec(h.character()).unwrap();
        let mut applied = baseline();
        movement::advance(
            &mut applied.character,
            &s,
            f,
            frame(1).velocity,
            false,
            4,
            1. / 120.,
        )
        .unwrap();
        applied.applied_sequence = 1;
        assert_eq!(
            h.reconcile(applied, 1, 1, &s, f).unwrap(),
            Correction::Replayed
        );
        assert_eq!(h.pending(), 2);
        assert_eq!(serde_json::to_vec(h.character()).unwrap(), expected);
        assert_eq!(
            h.reconcile(applied, 1, 1, &s, f).unwrap(),
            Correction::Duplicate
        );
        assert!(h.reconcile(baseline(), 0, 0, &s, f).is_err());
        assert_eq!(serde_json::to_vec(h.character()).unwrap(), expected);
    }
    #[test]
    fn new_life_and_epoch_retire_old_input_and_conflicting_acks_are_atomic() {
        let s = geometry();
        let f = Filter::blocking(1);
        let mut h = History::new(baseline(), 0, 0).unwrap();
        h.predict(frame(1), &s, f).unwrap();
        let mut changed = baseline();
        changed.character.feet.x = 2.;
        assert!(h.reconcile(changed, 0, 0, &s, f).is_err());
        changed.life.generation += 1;
        changed.epoch += 1;
        assert_eq!(
            h.reconcile(changed, 1, 1, &s, f).unwrap(),
            Correction::Reset
        );
        assert_eq!(h.pending(), 0);
        assert_eq!(h.character().feet.x, 2.);
        assert!(h.predict(frame(2), &s, f).is_err());
        assert!(h.reconcile(baseline(), 2, 2, &s, f).is_err());
    }
    #[test]
    fn newer_observation_can_correct_external_motion_at_the_same_server_tick() {
        let s = geometry();
        let f = Filter::blocking(1);
        let mut h = History::new(baseline(), 0, 1).unwrap();
        h.predict(frame(1), &s, f).unwrap();
        let mut moved = baseline();
        moved.character.feet.x = 2.;
        assert_eq!(
            h.reconcile(moved, 0, 2, &s, f).unwrap(),
            Correction::Replayed
        );
        assert!(h.character().feet.x > 2.);
        assert!(h.reconcile(baseline(), 0, 1, &s, f).is_err());
        assert_eq!(h.pending(), 1);
    }
    #[test]
    fn overflow_and_duplicate_input_preserve_the_prediction() {
        let s = geometry();
        let f = Filter::blocking(1);
        let mut h = History::new(baseline(), 0, 0).unwrap();
        for seq in 1..=CAPACITY as u64 {
            h.predict(frame(seq), &s, f).unwrap();
        }
        let before = serde_json::to_vec(h.character()).unwrap();
        assert!(h.predict(frame(CAPACITY as u64 + 1), &s, f).is_err());
        assert!(h.predict(frame(1), &s, f).is_err());
        assert_eq!(h.pending(), CAPACITY);
        assert_eq!(serde_json::to_vec(h.character()).unwrap(), before);
    }
    #[test]
    fn acknowledged_jump_is_not_replayed_and_external_motor_state_survives() {
        let s = geometry();
        let f = Filter::blocking(1);
        let mut grounded = baseline();
        movement::advance(
            &mut grounded.character,
            &s,
            f,
            DVec3::ZERO,
            false,
            4,
            1. / 120.,
        )
        .unwrap();
        grounded.character.external = DVec3::X * 5.;
        let mut h = History::new(grounded, 0, 0).unwrap();
        let mut jump = frame(1);
        jump.jump = true;
        h.predict(jump, &s, f).unwrap();
        h.predict(frame(2), &s, f).unwrap();
        let expected = serde_json::to_vec(h.character()).unwrap();
        movement::advance(
            &mut grounded.character,
            &s,
            f,
            jump.velocity,
            true,
            4,
            1. / 120.,
        )
        .unwrap();
        grounded.applied_sequence = 1;
        h.reconcile(grounded, 1, 1, &s, f).unwrap();
        assert_eq!(h.pending(), 1);
        assert_eq!(serde_json::to_vec(h.character()).unwrap(), expected);
        assert!(h.character().feet.y > 0. && h.character().external.x > 0.);
    }
    #[test]
    fn substep_budget_and_foreign_collision_context_are_refused_atomically() {
        let s = geometry();
        let f = Filter::blocking(1);
        let mut h = History::new(baseline(), 0, 0).unwrap();
        let mut long = frame(1);
        long.steps = 12;
        for seq in 1..=21 {
            long.sequence = seq;
            h.predict(long, &s, f).unwrap();
        }
        let before = serde_json::to_vec(h.character()).unwrap();
        long.sequence = 22;
        assert!(h.predict(long, &s, f).is_err());
        assert_eq!(serde_json::to_vec(h.character()).unwrap(), before);
        assert!(
            h.reconcile(baseline(), 1, 1, &s, Filter::blocking(2))
                .is_err()
        );
        assert_eq!(h.pending(), 21);
    }
    #[test]
    fn coalesced_authority_inputs_correct_the_estimate_without_duplicate_travel() {
        let s = geometry();
        let f = Filter::blocking(1);
        let mut h = History::new(baseline(), 0, 0).unwrap();
        for seq in 1..=3 {
            h.predict(frame(seq), &s, f).unwrap();
        }
        let mut applied = baseline();
        movement::advance(
            &mut applied.character,
            &s,
            f,
            frame(2).velocity,
            false,
            4,
            1. / 120.,
        )
        .unwrap();
        applied.applied_sequence = 2;
        let mut expected = applied.character;
        movement::advance(&mut expected, &s, f, frame(3).velocity, false, 4, 1. / 120.).unwrap();
        h.reconcile(applied, 1, 1, &s, f).unwrap();
        assert_eq!(h.pending(), 1);
        assert_eq!(
            serde_json::to_vec(h.character()).unwrap(),
            serde_json::to_vec(&expected).unwrap()
        );
    }
    #[test]
    fn changed_collision_replays_pending_input_against_current_geometry() {
        let mut s = geometry();
        let f = Filter::blocking(1);
        let mut h = History::new(baseline(), 0, 0).unwrap();
        for seq in 1..=8 {
            h.predict(frame(seq), &s, f).unwrap();
        }
        assert!(h.character().feet.x > 1.);
        s.insert(MeshCollider {
            key: ColliderKey {
                life: Life {
                    instance: 1,
                    entity: 0,
                    generation: 0,
                },
                shape: 1,
            },
            layers: 1,
            usage: Usage::Blocking,
            mesh: Mesh::from_box(DVec3::new(1., 0., -5.), DVec3::new(1.01, 5., 5.)).unwrap(),
        })
        .unwrap();
        h.reconcile(baseline(), 1, 1, &s, f).unwrap();
        assert!(h.character().feet.x < 0.651);
        assert_eq!(h.pending(), 8);
    }
}
