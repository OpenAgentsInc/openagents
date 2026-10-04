//! Life-bound presentation extracted from the same authority as combat snapshots.
use super::wire::{ActorBinding, Life};
use crate::{play::Game, utilities::Area};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use verse_engine::{director::Actor, motion::Selection};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pose {
    pub actor: Actor,
    pub life: Life,
    pub teleport_stamp: Option<f32>,
    pub animation: Selection,
    pub animation_time: f32,
    pub visible: bool,
    pub health: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Effects {
    pub life: Life,
    pub position: [f32; 3],
    pub shield: i32,
    pub shield_until: f32,
    pub light: Option<[f32; 3]>,
    pub areas: Vec<Area>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Presentation {
    pub time: f32,
    pub actors: Vec<Pose>,
    pub effects: Vec<Effects>,
}
impl Presentation {
    pub(super) fn extract(game: &Game, bindings: &[ActorBinding]) -> Self {
        let lives: BTreeSet<_> = bindings
            .iter()
            .map(|b| verse_engine::core::LifeId::from(b.life))
            .collect();
        let teleports: std::collections::BTreeMap<_, _> = game
            .controlled_effects()
            .map(|(life, _, c)| (life, c.teleport_stamp()))
            .collect();
        let frame = game.frame();
        let actors = frame
            .actors
            .into_iter()
            .filter_map(|a| {
                let life = a.life.filter(|life| lives.contains(life))?;
                Some(Pose {
                    actor: a.actor,
                    life: life.into(),
                    teleport_stamp: teleports.get(&life).copied().flatten(),
                    animation: a.animation,
                    animation_time: a.animation_time,
                    visible: a.visible,
                    health: a.health,
                })
            })
            .collect();
        let effects = game
            .controlled_effects()
            .filter(|(life, _, _)| lives.contains(life))
            .map(|(life, position, c)| Effects {
                life: life.into(),
                position: position.to_array(),
                shield: c.shield,
                shield_until: c.shield_until,
                light: c.light.map(|p| p.to_array()),
                areas: c.areas.clone(),
            })
            .collect();
        Self {
            time: frame.time,
            actors,
            effects,
        }
    }
    pub fn validate(&self, instance: u64, bindings: &[ActorBinding]) -> Result<(), String> {
        let finite = |p: [f32; 3]| p.iter().all(|v| v.is_finite() && v.abs() <= 1_000_000.);
        let known: BTreeSet<_> = bindings
            .iter()
            .map(|b| verse_engine::core::LifeId::from(b.life))
            .collect();
        let mut poses = BTreeSet::new();
        let mut players = BTreeSet::new();
        if !self.time.is_finite()
            || self.time < 0.
            || self.actors.len() > 256
            || self.effects.len() > 64
        {
            return Err("Chamber presentation budget or clock refused".into());
        }
        for p in &self.actors {
            let life = verse_engine::core::LifeId::from(p.life);
            if p.life.instance != instance
                || p.actor.id != p.life.actor
                || !known.contains(&life)
                || !poses.insert(life)
                || !finite(p.actor.position.to_array())
                || !p.actor.yaw.is_finite()
                || !p.actor.scale.is_finite()
                || !(0.001..=100.).contains(&p.actor.scale)
                || !p.animation_time.is_finite()
                || p.animation_time < 0.
                || p.teleport_stamp
                    .is_some_and(|stamp| !stamp.is_finite() || stamp < 0.)
                || p.actor.name.len() > 256
                || p.actor.model.is_empty()
                || p.actor.model.len() > 128
            {
                return Err("Invalid chamber actor presentation".into());
            }
            if p.actor.model == "adventurer" {
                players.insert(life);
            }
        }
        if poses != known {
            return Err("Chamber presentation is missing actor lives".into());
        }
        let mut effects = BTreeSet::new();
        for e in &self.effects {
            let life = verse_engine::core::LifeId::from(e.life);
            if !players.contains(&life)
                || !effects.insert(life)
                || !finite(e.position)
                || !(0..=18).contains(&e.shield)
                || !e.shield_until.is_finite()
                || e.shield_until < 0.
                || e.light.is_some_and(|p| !finite(p))
                || e.areas.len() > 128
                || e.areas
                    .iter()
                    .any(|a| !finite(a.position.to_array()) || !a.until.is_finite() || a.until < 0.)
            {
                return Err("Invalid chamber player effects".into());
            }
        }
        if effects != players {
            return Err("Chamber presentation is missing player effects".into());
        }
        Ok(())
    }
}
