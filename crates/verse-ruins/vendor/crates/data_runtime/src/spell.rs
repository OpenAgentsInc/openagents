//! Spell schema (SRD-derived). Keep minimal; loaders will fill from JSON.

use crate::ids::Id;
use serde::Deserialize;

#[derive(Debug, Clone)]
pub struct Spell {
    pub id: Id,
    pub name: String,
    pub level: u8,
    pub school: String,
}

impl Spell {
    pub fn is_cantrip(&self) -> bool {
        self.level == 0
    }
}

// A more detailed, data-driven spell spec used by the simulator and tools.
// This maps closely to docs/design/spells/fire_bolt.md and data/spells/*.json.
#[derive(Debug, Clone, Deserialize)]
pub struct SpellSpec {
    pub id: String,
    pub name: String,
    pub version: Option<String>,
    pub source: Option<String>,
    pub school: String,
    pub level: u8,
    #[serde(default)]
    pub classes: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,

    // Cast/queue/cd
    pub cast_time_s: f32,
    pub gcd_s: f32,
    pub cooldown_s: f32,
    pub resource_cost: Option<serde_json::Value>,
    pub can_move_while_casting: bool,

    // Targeting/LoS
    pub targeting: String,
    pub requires_line_of_sight: bool,
    pub range_ft: u32,
    pub minimum_range_ft: u32,
    pub firing_arc_deg: u32,

    // Attack/damage
    pub attack: Option<AttackSpec>,
    pub damage: Option<DamageSpec>,

    // Projectile
    #[serde(default)]
    pub projectile: Option<ProjectileSpec>,

    // Secondary/latency/policy (optional)
    #[serde(default)]
    pub secondary: Option<serde_json::Value>,
    #[serde(default)]
    pub latency: Option<serde_json::Value>,
    #[serde(default)]
    pub events: Vec<String>,
    #[serde(default)]
    pub metrics: Option<serde_json::Value>,
    #[serde(default)]
    pub policy: Option<serde_json::Value>,
    #[serde(default)]
    pub save: Option<SaveSpec>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AttackSpec {
    #[serde(rename = "type")]
    pub kind: String,
    pub rng_stream: Option<String>,
    pub crit_rule: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DamageSpec {
    #[serde(rename = "type")]
    pub damage_type: String,
    #[serde(default)]
    pub add_spell_mod_to_damage: bool,
    #[serde(default)]
    pub dice_by_level_band: Option<std::collections::HashMap<String, String>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProjectileSpec {
    pub enabled: bool,
    pub speed_mps: f32,
    pub radius_m: f32,
    pub gravity: f32,
    pub collide_with: Vec<String>,
    pub spawn_offset_m: SpawnOffset,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SpawnOffset {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SaveSpec {
    pub kind: String, // e.g., "dex", "wis"
    #[serde(default)]
    pub dc: Option<i32>, // if None, use caster's spell_save_dc
    #[serde(default)]
    pub on_fail: Option<OnFail>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OnFail {
    #[serde(default)]
    pub apply_condition: Option<String>, // e.g., "prone"
    #[serde(default)]
    pub duration_ms: Option<u32>,
}
