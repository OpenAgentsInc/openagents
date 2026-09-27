//! Zone authoring schema and loader (Phase 1).
//!
//! Scope
//! - Minimal manifest describing a persistent, named Zone with terrain and weather defaults.
//! - JSON lives under `data/zones/<slug>/manifest.json`.
//! - Client uses it to set up terrain generation and sky parameters.
//!
//! Extending
//! - Add spawn tables, connectors, biome layers, and snapshot references.
//! - Introduce server/runtime delta logs in a separate module when needed.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// The world plane a zone belongs to.
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "PascalCase")]
pub enum ZonePlane {
    #[default]
    Material,
    Feywild,
    Shadowfell,
    Other(String),
}

/// Terrain generation parameters for the client-side prototype.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TerrainSpec {
    /// Grid dimension N (vertices per side). Use odd numbers like 129 (128 quads).
    pub size: u32,
    /// Half-extent in world meters (terrain spans [-extent, +extent] on X and Z).
    pub extent: f32,
    /// Seed for deterministic generation.
    pub seed: u32,
}

/// Vegetation placement parameters (prototype).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VegetationSpec {
    /// Number of trees to scatter on gentle slopes near the player area.
    pub tree_count: u32,
    /// Seed for deterministic scatter.
    pub tree_seed: u32,
}

/// Simple weather defaults affecting the sky model.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct WeatherSpec {
    /// 1..10; higher = hazier.
    pub turbidity: f32,
    /// Approximate ground albedo (RGB).
    pub ground_albedo: [f32; 3],
}

/// Authoring manifest for a Zone (Phase 1 subset).
///
/// Time‑of‑day (TOD)
/// - `start_time_frac` lets designers choose an initial TOD for the scene without code changes.
///   The value is a fraction in `[0..1]` where `0.5` is noon and `~0.0/1.0` lies at midnight.
/// - `start_paused` starts the sky paused so the scene doesn’t advance time until the user
///   toggles it (Space). Useful for fixed‑time lookdev shots (e.g., a night demo).
/// - `start_time_scale` controls how quickly TOD progresses when not paused.
///
/// The renderer (`render_wgpu::gfx`) reads these fields after loading the manifest and
/// applies them to `SkyStateCPU` before the first frame.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ZoneManifest {
    pub zone_id: u32,
    pub slug: String,
    pub display_name: String,
    #[serde(default)]
    pub plane: ZonePlane,
    pub terrain: TerrainSpec,
    #[serde(default)]
    pub weather: Option<WeatherSpec>,
    #[serde(default)]
    pub vegetation: Option<VegetationSpec>,
    /// Initial time‑of‑day fraction in `[0..1]` (0.5 = noon; ~0.0/1.0 = midnight)
    #[serde(default)]
    pub start_time_frac: Option<f32>,
    /// Start the sky paused (TOD does not advance until unpaused)
    #[serde(default)]
    pub start_paused: Option<bool>,
    /// Initial time scale multiplier for TOD progression
    #[serde(default)]
    pub start_time_scale: Option<f32>,
    /// Allow player casting in this zone (controls input gating)
    #[serde(default)]
    pub allow_casting: Option<bool>,
    /// Show player HUD bars/hotbar in this zone
    #[serde(default)]
    pub show_player_hud: Option<bool>,
    /// Optional worldsmithing policy block (authoring caps and allowed kinds)
    #[serde(default)]
    pub worldsmithing: Option<WorldsmithingPolicy>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct WorldsmithingPolicy {
    #[serde(default)]
    pub enabled: Option<bool>,
    #[serde(default)]
    pub kinds: Vec<String>,
    #[serde(default)]
    pub caps: Option<WorldsmithingCaps>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct WorldsmithingCaps {
    #[serde(default)]
    pub trees: Option<u32>,
    #[serde(default)]
    pub place_per_second: Option<u32>,
}

/// Load a Zone manifest from `data/zones/<slug>/manifest.json`.
pub fn load_zone_manifest(slug: &str) -> Result<ZoneManifest> {
    use crate::loader::read_json;
    let rel = format!("zones/{}/manifest.json", slug);
    let txt = read_json(&rel).with_context(|| format!("read zone manifest: {}", rel))?;
    let z: ZoneManifest = serde_json::from_str(&txt).context("parse zone manifest json")?;
    if z.slug != slug {
        log::warn!(
            "zone slug mismatch: manifest='{}' path='{}' (using manifest)",
            z.slug,
            slug
        );
    }
    Ok(z)
}
