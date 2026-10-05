//! Validated character tuning over the existing world ability implementations.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AbilityTuning {
    pub cost: i32,
    pub cooldown: f32,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Character {
    pub key: String,
    pub health: i32,
    pub mana: i32,
    pub save_dc: i32,
    pub catalog: BTreeMap<u8, AbilityTuning>,
}
impl Default for Character {
    fn default() -> Self {
        Self {
            key: "chamber-wizard".into(),
            health: 200,
            mana: 20,
            save_dc: crate::spells::SPELL_SAVE_DC,
            catalog: crate::spells::CATALOG
                .iter()
                .map(|s| {
                    (
                        s.slot,
                        AbilityTuning {
                            cost: s.cost,
                            cooldown: s.cooldown,
                        },
                    )
                })
                .collect(),
        }
    }
}
impl Character {
    pub fn validate(&self) -> Result<(), String> {
        if self.key.is_empty()
            || self.key.len() > 64
            || !self
                .key
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
            || !(200..=600).contains(&self.health)
            || !(20..=60).contains(&self.mana)
            || !(5..=30).contains(&self.save_dc)
            || self.catalog.len() != crate::spells::CATALOG.len()
            || self.catalog.iter().any(|(slot, t)| {
                crate::spells::spell_in_slot(*slot).is_none()
                    || !(0..=20).contains(&t.cost)
                    || t.cost > self.mana
                    || !t.cooldown.is_finite()
                    || !(0. ..=600.).contains(&t.cooldown)
            })
        {
            return Err("Invalid character definition".into());
        }
        Ok(())
    }
}
