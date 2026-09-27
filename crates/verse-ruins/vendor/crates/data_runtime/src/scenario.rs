//! Scenario schema for the simulation harness.
//! YAML-serializable for author-friendly workflows.

use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Clone, Deserialize)]
pub struct Scenario {
    pub name: String,
    #[serde(default = "default_tick_ms")]
    pub tick_ms: u32,
    #[serde(default)]
    pub seed: Option<u64>,
    #[serde(default)]
    pub map: Option<String>,
    #[serde(default)]
    pub underwater: bool,
    #[serde(default)]
    pub actors: Vec<Actor>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Actor {
    pub id: String,
    pub role: String,
    #[serde(default)]
    pub class: Option<String>,
    #[serde(default)]
    pub team: Option<String>,
    #[serde(default)]
    pub abilities: Vec<String>,
}

fn default_tick_ms() -> u32 {
    50
}

pub fn load_yaml(path: &Path) -> Result<Scenario> {
    let txt = std::fs::read_to_string(path)
        .with_context(|| format!("read scenario: {}", path.display()))?;
    let scn: Scenario = serde_yaml::from_str(&txt).context("parse scenario yaml")?;
    Ok(scn)
}
