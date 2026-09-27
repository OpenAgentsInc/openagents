//! Unique NPC/Boss configuration loader (e.g., Nivita, Lady of Undertide).
//!
//! Parses `data/config/nivita.toml` into a structured config used to seed ECS
//! components on spawn. Keep this crate free of ECS dependencies; convert into
//! ECS types in the caller as needed.

use anyhow::{Context, Result};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, Default)]
pub struct NivitaCfg {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub kind: String, // e.g., "boss"
    #[serde(default)]
    pub team: Option<String>, // e.g., "enemy_raid"
    pub level: u8,
    pub hp_range: (i32, i32),
    #[serde(default)]
    pub speed_mps: Option<f32>,
    #[serde(default)]
    pub radius_m: Option<f32>,
    #[serde(default)]
    pub height_m: Option<f32>,

    #[serde(default)]
    pub abilities: AbilitiesCfg,
    #[serde(default)]
    pub defenses: DefensesCfg,
    /// Optional explicit save overrides; if omitted, compute from abilities + proficiency.
    pub saves: Option<SavesCfg>,

    #[serde(default)]
    pub legendary: LegendaryCfg,
    #[serde(default)]
    pub spellbook: SpellbookCfg,
    #[serde(default)]
    pub legendary_actions: Vec<LegendaryActionCfg>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct AbilitiesCfg {
    pub str: i8,
    pub dex: i8,
    pub con: i8,
    pub int: i8,
    pub wis: i8,
    pub cha: i8,
    #[serde(default)]
    pub proficiency: i8,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct DefensesCfg {
    pub ac: u8,
    #[serde(default)]
    pub resistances: Vec<String>,
    #[serde(default)]
    pub immunities: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct SavesCfg {
    pub str: i8,
    pub dex: i8,
    pub con: i8,
    pub int: i8,
    pub wis: i8,
    pub cha: i8,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct LegendaryCfg {
    #[serde(default)]
    pub resist_per_day: u8,
    #[serde(default)]
    pub resets: Option<String>, // "long_rest" | "per_encounter"
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct SpellbookCfg {
    #[serde(default)]
    pub cantrips: Vec<String>,
    #[serde(default)]
    pub level_1_3: Vec<String>,
    #[serde(default)]
    pub level_4_5: Vec<String>,
    #[serde(default)]
    pub signature: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LegendaryActionCfg {
    pub id: String,
    pub cost: u8,
}

/// Load the default Nivita config from `data/config/nivita.toml`.
pub fn load_nivita() -> Result<NivitaCfg> {
    let txt = include_str!("../../../../data/config/nivita.toml");
    let mut cfg: NivitaCfg = toml::from_str(txt).context("parse nivita.toml")?;
    if cfg.kind.is_empty() {
        cfg.kind = "boss".into();
    }
    Ok(cfg)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn load_nivita_toml() {
        // Requires the repo data/ checked out; this runs in CI and dev.
        let cfg = load_nivita().expect("nivita");
        assert!(cfg.name.to_lowercase().contains("nivita"));
        assert!(cfg.hp_range.0 > 0 && cfg.hp_range.1 >= cfg.hp_range.0);
        assert!(cfg.defenses.ac >= 10);
    }
    // Converters moved to ecs_core::parse; see that crate's tests.
}
