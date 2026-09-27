//! Projectile specifications used to parameterize server-side projectile spawns.

use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Clone, Deserialize)]
pub struct ProjectileSpec {
    pub speed_mps: f32,
    pub radius_m: f32,
    pub damage: i32,
    pub life_s: f32,
    #[serde(default = "default_arming_delay_s")]
    pub arming_delay_s: f32,
    /// Whether this projectile should carve destructible proxies (server authority).
    #[serde(default)]
    pub carves_destructibles: bool,
    /// Carve radius in meters (object-space after transform); separate from AoE radius.
    #[serde(default)]
    pub carve_radius_m: f32,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ProjectileSpecDb {
    /// Map from action name (e.g., "AtWillLMB", "EncounterQ") to spec
    pub actions: HashMap<String, ProjectileSpec>,
}

impl ProjectileSpecDb {
    /// Read the pinned source configuration without a host filesystem.
    pub fn load_default() -> Result<Self> {
        toml::from_str(include_str!("../../../../data/config/projectiles.toml"))
            .context("parse embedded projectiles TOML")
    }
}

fn default_arming_delay_s() -> f32 {
    0.08
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn defaults_present() {
        let db = ProjectileSpecDb::load_default().expect("load");
        assert!(db.actions.contains_key("AtWillLMB"));
        assert!(db.actions.contains_key("EncounterQ"));
    }
}
