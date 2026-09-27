//! Input/camera controller configuration loaded from data/config/input_camera.toml.

use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize)]
pub struct InputCameraCfg {
    pub sensitivity_deg_per_count: Option<f32>,
    pub invert_y: Option<bool>,
    pub min_pitch_deg: Option<f32>,
    pub max_pitch_deg: Option<f32>,
    /// If true, ALT acts as hold-to-hold for cursor mode; otherwise toggle.
    pub alt_hold: Option<bool>,
    /// Default input profile name: "ActionCombat" or "ClassicCursor" (optional)
    pub profile: Option<String>,
}

impl Default for InputCameraCfg {
    fn default() -> Self {
        Self {
            sensitivity_deg_per_count: Some(0.15),
            invert_y: Some(false),
            min_pitch_deg: Some(-80.0),
            max_pitch_deg: Some(80.0),
            alt_hold: Some(false),
            profile: None,
        }
    }
}

fn data_root() -> PathBuf {
    let here = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let ws = here.join("../../data");
    if ws.is_dir() { ws } else { here.join("data") }
}

pub fn load_default() -> Result<InputCameraCfg> {
    let path = data_root().join("config/input_camera.toml");
    let mut cfg = if path.is_file() {
        let txt =
            std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
        toml::from_str::<InputCameraCfg>(&txt).context("parse input_camera TOML")?
    } else {
        InputCameraCfg::default()
    };
    // Env overrides for quick tuning (optional)
    if let Ok(s) = std::env::var("MOUSE_SENS_DEG") {
        cfg.sensitivity_deg_per_count = s.parse().ok();
    }
    if let Ok(v) = std::env::var("INVERT_Y") {
        cfg.invert_y = v.parse().ok();
    }
    if let Ok(v) = std::env::var("MIN_PITCH_DEG") {
        cfg.min_pitch_deg = v.parse().ok();
    }
    if let Ok(v) = std::env::var("MAX_PITCH_DEG") {
        cfg.max_pitch_deg = v.parse().ok();
    }
    if let Ok(v) = std::env::var("ALT_HOLD") {
        cfg.alt_hold = v.parse().ok();
    }
    if let Ok(v) = std::env::var("INPUT_PROFILE") {
        cfg.profile = Some(v);
    }
    Ok(cfg)
}
