use anyhow::{Context, Result};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct EncounterSpec {
    pub version: String,
    #[serde(default)]
    pub pc_spawn: Option<String>,
    #[serde(default)]
    pub npcs: Vec<NpcSpec>,
    #[serde(default)]
    pub ai: Option<AiTuning>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NpcSpec {
    pub archetype: String, // "Wizard" | "Zombie" | "DeathKnight" | ...
    #[serde(default)]
    pub count: u32,
    #[serde(default)]
    pub unique: bool,
    pub spawn: String,
    #[serde(default)]
    pub faction: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AiTuning {
    #[serde(default)]
    pub aggro_radius_m: f32,
    #[serde(default)]
    pub attack_cooldown_s: f32,
}

pub fn load_encounter_for_zone(_packs_root: &std::path::Path, slug: &str) -> Result<EncounterSpec> {
    // Authoring path for v1 (can be switched to a baked copy later).
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data/zones")
        .join(slug)
        .join("encounter.json");
    let txt = std::fs::read_to_string(&path)
        .with_context(|| format!("read encounter: {}", path.display()))?;
    serde_json::from_str::<EncounterSpec>(&txt).context("parse encounter")
}
