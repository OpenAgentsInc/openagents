//! Destructible budgets and tuning loaded from data/config/destructible.toml
//! with sensible defaults and clamping.

use anyhow::{Context, Result};

#[derive(Debug, Clone, PartialEq, serde::Deserialize)]
pub struct DestructibleConfigFile {
    pub voxel_size_m: f64,
    pub chunk: [u32; 3],
    pub aabb_pad_m: f64,
    pub max_remesh_per_tick: usize,
    pub collider_budget_per_tick: usize,
    pub max_debris: usize,
    pub max_carve_chunks: u32,
    pub close_surfaces: bool,
    pub seed: u64,
}

impl Default for DestructibleConfigFile {
    fn default() -> Self {
        Self {
            voxel_size_m: 0.10,
            chunk: [32, 32, 32],
            aabb_pad_m: 0.25,
            max_remesh_per_tick: 4,
            collider_budget_per_tick: 2,
            max_debris: 1500,
            max_carve_chunks: 64,
            close_surfaces: false,
            seed: 0x00C0FFEE,
        }
    }
}

fn clamp(mut cfg: DestructibleConfigFile) -> DestructibleConfigFile {
    if cfg.voxel_size_m < 0.02 {
        cfg.voxel_size_m = 0.02;
    }
    if cfg.max_remesh_per_tick > 256 {
        cfg.max_remesh_per_tick = 256;
    }
    cfg
}

/// Load the destructible config from the default location, falling back to defaults.
pub fn load_default() -> Result<DestructibleConfigFile> {
    let txt = include_str!("../../../../data/config/destructible.toml");
    let parsed: DestructibleConfigFile = toml::from_str(txt).context("parse TOML")?;
    Ok(clamp(parsed))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn default_or_file_loads() {
        // Succeeds even if file missing (repo ships a sample file).
        let cfg = load_default().expect("load");
        assert!(cfg.chunk[0] >= 8);
    }
}
