//! Read-only player-owned resource, cooldown, and cast presentation.
use crate::{
    play::{Ability, Casting, Game},
    rules::{Player, Snapshot},
    utilities::Controls,
};
use serde::{Deserialize, Serialize};
use verse_engine::core::LifeId;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Slot {
    pub ability: Ability,
    pub ready: bool,
    pub remaining: f32,
    pub duration: f32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Own {
    pub life: LifeId,
    pub time: f32,
    pub resources: Player,
    pub slots: Vec<Slot>,
    pub casting: Option<Casting>,
}
impl Own {
    pub(crate) fn extract(
        game: &Game,
        life: LifeId,
        snapshot: Snapshot,
        controls: &Controls,
        bow_ready: f32,
        casting: Option<&Casting>,
    ) -> Self {
        let slots = Ability::ALL
            .into_iter()
            .map(|ability| {
                let (ready, remaining, duration) = if let Some(spell) = ability.spell() {
                    let gate = snapshot.abilities.iter().find(|a| a.id == spell).unwrap();
                    (
                        gate.ready,
                        gate.cooldown_remaining,
                        game.cooldown_duration(spell),
                    )
                } else if let Some(utility) = ability.utility() {
                    let remaining = controls.cooldown(utility, game.time);
                    (
                        remaining == 0. && snapshot.player.mana >= utility.cost(),
                        remaining,
                        utility.cooldown(),
                    )
                } else {
                    (game.time >= bow_ready, (bow_ready - game.time).max(0.), 1.)
                };
                Slot {
                    ability,
                    ready: ready && snapshot.player.hp > 0 && casting.is_none(),
                    remaining,
                    duration,
                }
            })
            .collect();
        Self {
            life,
            time: game.time,
            resources: snapshot.player,
            slots,
            casting: casting.cloned(),
        }
    }
    pub fn validate(&self, instance: u64) -> Result<(), String> {
        let p = &self.resources;
        if self.life.instance != instance
            || !self.time.is_finite()
            || self.time < 0.
            || !(1..=1_000_000).contains(&p.max_hp)
            || !(0..=p.max_hp).contains(&p.hp)
            || !(1..=1_000_000).contains(&p.max_mana)
            || !(0..=p.max_mana).contains(&p.mana)
            || self.slots.len() != Ability::ALL.len()
            || self.slots.iter().zip(Ability::ALL).any(|(s, a)| {
                s.ability != a
                    || !s.remaining.is_finite()
                    || !(0.0..=600.).contains(&s.remaining)
                    || !s.duration.is_finite()
                    || !(0.001..=600.).contains(&s.duration)
                    || (s.ready && (p.hp == 0 || self.casting.is_some() || s.remaining > 0.))
            })
        {
            return Err("Invalid owned HUD resources or slots".into());
        }
        if self.casting.as_ref().is_some_and(|c| {
            c.target_life.instance != instance
                || !c.aim.is_finite()
                || !c.started.is_finite()
                || c.started < 0.
                || c.started > self.time
                || !c.ends.is_finite()
                || c.ends < c.started
                || c.ends < self.time
                || (!Ability::ALL.contains(&c.ability) && c.ability.catalog().is_none())
        }) {
            return Err("Invalid owned HUD cast progress".into());
        }
        Ok(())
    }
}
