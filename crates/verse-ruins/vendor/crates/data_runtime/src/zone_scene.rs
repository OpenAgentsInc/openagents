//! Minimal zone scene schema + validation helper.
//!
//! This is a placeholder for a future JSON Schema based validator. For now we
//! rely on serde with `deny_unknown_fields` to catch unexpected fields.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ZoneScene {
    pub version: String,
    pub seed: u32,
    pub layers: Vec<serde_json::Value>,
    pub instances: Vec<serde_json::Value>,
    pub logic: Logic,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Logic {
    pub triggers: Vec<serde_json::Value>,
    pub spawns: Vec<serde_json::Value>,
    pub waypoints: Vec<serde_json::Value>,
    pub links: Vec<serde_json::Value>,
}

/// Validate a scene JSON string by attempting to deserialize into `ZoneScene`.
pub fn validate_scene_against_schema(txt: &str) -> Result<()> {
    let _: ZoneScene = serde_json::from_str(txt).context("parse scene json")?;
    Ok(())
}
