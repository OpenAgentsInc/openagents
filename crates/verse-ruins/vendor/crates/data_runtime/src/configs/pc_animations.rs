//! PC animation clip names: exact strings loaded from data/config/pc_animations.toml
//! with optional env overrides.

use anyhow::{Context, Result};
use serde::Deserialize;
#[cfg(not(target_arch = "wasm32"))]
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize, Default)]
pub struct PcAnimCfg {
    pub idle: Option<String>,
    pub walk: Option<String>,
    pub sprint: Option<String>,
    pub strafe: Option<String>,
    // Back-compat single cast name
    pub cast: Option<String>,
    // Phase-specific cast clips
    pub cast_enter: Option<String>,
    pub cast_loop: Option<String>,
    pub cast_shoot: Option<String>,
    pub cast_exit: Option<String>,
    pub jump_start: Option<String>,
    pub jump_loop: Option<String>,
    pub jump_land: Option<String>,
}

#[cfg(not(target_arch = "wasm32"))]
fn data_root() -> PathBuf {
    let here = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let ws = here.join("../../data");
    if ws.is_dir() { ws } else { here.join("data") }
}

pub fn load_default() -> Result<PcAnimCfg> {
    let mut cfg = {
        #[cfg(target_arch = "wasm32")]
        {
            // On wasm, the filesystem is unavailable; embed the default config.
            let txt = include_str!("../../../../data/config/pc_animations.toml");
            toml::from_str::<PcAnimCfg>(txt).context("parse embedded pc_animations TOML")?
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let path = data_root().join("config/pc_animations.toml");
            if path.is_file() {
                let txt = std::fs::read_to_string(&path)
                    .with_context(|| format!("read {}", path.display()))?;
                toml::from_str::<PcAnimCfg>(&txt).context("parse pc_animations TOML")?
            } else {
                PcAnimCfg::default()
            }
        }
    };
    // Env overrides
    if let Ok(v) = std::env::var("PC_ANIM_IDLE") {
        cfg.idle = Some(v);
    }
    if let Ok(v) = std::env::var("PC_ANIM_WALK") {
        cfg.walk = Some(v);
    }
    if let Ok(v) = std::env::var("PC_ANIM_SPRINT") {
        cfg.sprint = Some(v);
    }
    if let Ok(v) = std::env::var("PC_ANIM_STRAFE") {
        cfg.strafe = Some(v);
    }
    if let Ok(v) = std::env::var("PC_ANIM_CAST") {
        cfg.cast = Some(v);
    }
    if let Ok(v) = std::env::var("PC_ANIM_CAST_ENTER") {
        cfg.cast_enter = Some(v);
    }
    if let Ok(v) = std::env::var("PC_ANIM_CAST_LOOP") {
        cfg.cast_loop = Some(v);
    }
    if let Ok(v) = std::env::var("PC_ANIM_CAST_SHOOT") {
        cfg.cast_shoot = Some(v);
    }
    if let Ok(v) = std::env::var("PC_ANIM_CAST_EXIT") {
        cfg.cast_exit = Some(v);
    }
    if let Ok(v) = std::env::var("PC_ANIM_JUMP_START") {
        cfg.jump_start = Some(v);
    }
    if let Ok(v) = std::env::var("PC_ANIM_JUMP_LOOP") {
        cfg.jump_loop = Some(v);
    }
    if let Ok(v) = std::env::var("PC_ANIM_JUMP_LAND") {
        cfg.jump_land = Some(v);
    }
    Ok(cfg)
}
