//! Validated character tuning over the existing world ability implementations.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Trusted, data-only collision and character settings for a hosted scene.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Authored {
    pub character: Option<Character>,
    pub blockers: BTreeMap<u64, Bounds>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub navigation: Option<NavigationRegion>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NavigationRegion {
    pub min: [f64; 3],
    pub max: [f64; 3],
    pub cell: f64,
}
impl NavigationRegion {
    pub fn validate(&self) -> Result<(), String> {
        if !(0.5..=2.).contains(&self.cell)
            || !self.cell.is_finite()
            || (0..3).any(|axis| {
                !self.min[axis].is_finite()
                    || !self.max[axis].is_finite()
                    || self.min[axis].abs() > 1000.
                    || self.max[axis].abs() > 1000.
                    || self.min[axis] >= self.max[axis]
            })
            || ((self.max[0] - self.min[0]) / self.cell).ceil()
                * ((self.max[2] - self.min[2]) / self.cell).ceil()
                > 16384.
        {
            return Err("Author navigation within 1000 meters, with ordered bounds, 0.5..2 meter cells, and at most 16384 ground cells".into());
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bounds {
    pub min: [f64; 3],
    pub max: [f64; 3],
}
/// The existing host catalogs included in an authored generation's identity.
#[derive(Serialize)]
pub struct Gameplay<'a> {
    pub authored_combat_health: bool,
    pub rewards: &'a [crate::service::rewards::Policy],
    pub progression: &'a crate::service::progression::Config,
    pub items: &'a crate::service::items::Catalog,
    pub outfits: &'a crate::service::outfits::Catalog,
    pub equipment: &'a crate::service::equipment::Catalog,
}
pub fn bind_gameplay(content: [u8; 32], gameplay: &Gameplay<'_>) -> Result<[u8; 32], String> {
    use sha2::{Digest, Sha256};
    let mut digest = Sha256::new();
    digest.update(b"verse.authored.gameplay.v1\0");
    digest.update(content);
    digest.update(serde_json::to_vec(gameplay).map_err(|_| "Cannot encode authored catalogs")?);
    Ok(digest.finalize().into())
}
impl Authored {
    pub fn validate(&self) -> Result<(), String> {
        if let Some(region) = &self.navigation {
            region.validate()?;
        }
        if let Some(character) = &self.character {
            character.validate()?;
        }
        if self.blockers.len() > 128
            || self.blockers.iter().any(|(id, bounds)| {
                *id == 0
                    || *id > 1_000_000
                    || (0..3).any(|axis| {
                        !bounds.min[axis].is_finite()
                            || !bounds.max[axis].is_finite()
                            || bounds.min[axis].abs() > 1_000_000.
                            || bounds.max[axis].abs() > 1_000_000.
                            || bounds.min[axis] >= bounds.max[axis]
                    })
            })
        {
            return Err(
                "Authored blockers require 1..1000000 IDs and finite ordered bounds; limit 128"
                    .into(),
            );
        }
        Ok(())
    }
    pub fn apply(&self, game: &mut crate::play::Game) -> Result<(), String> {
        self.validate()?;
        if let Some(region) = &self.navigation {
            game.configure_authored_navigation(region, &self.blockers)?;
        }
        if let Some(character) = &self.character {
            game.configure_character(game.player_life(), character.clone())?;
        }
        for (id, bounds) in &self.blockers {
            game.set_navigation_blocker(
                physics::queries::Life {
                    instance: game.player_life().instance,
                    entity: 2_000_000 + *id,
                    generation: game.player_life().generation,
                },
                bounds.min.into(),
                bounds.max.into(),
            )?;
        }
        if let Some(encounter) = game.encounter.as_mut() {
            encounter.authored_settings = Some(self.clone());
        }
        Ok(())
    }
    pub fn bind_content(&self, content: [u8; 32]) -> Result<[u8; 32], String> {
        use sha2::{Digest, Sha256};
        self.validate()?;
        let mut digest = Sha256::new();
        digest.update(b"verse.authored.content.v1\0");
        digest.update(content);
        digest.update(serde_json::to_vec(self).map_err(|_| "Cannot encode authored settings")?);
        Ok(digest.finalize().into())
    }
}

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
