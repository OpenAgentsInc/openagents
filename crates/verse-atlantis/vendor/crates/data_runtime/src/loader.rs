//! Data loaders. For now, stubs that resolve paths under `data/`.
//! Implement JSON parsing later to avoid adding new deps mid-prototype.

use crate::class::ClassSpec;
use crate::monster::MonsterSpec;
use crate::spell::SpellSpec;
use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

fn data_root() -> PathBuf {
    // Prefer top-level workspace `data/` so tests and tools can run from any crate.
    let here = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let ws = here.join("../../data");
    if ws.is_dir() { ws } else { here.join("data") }
}

/// Read a raw JSON file under `data/` and return its string.
pub fn read_json(rel: impl AsRef<Path>) -> Result<String> {
    let path = data_root().join(rel);
    let s = fs::read_to_string(&path).with_context(|| format!("read data: {}", path.display()))?;
    Ok(s)
}

/// Load and deserialize a spell JSON (from data/spells/*).
pub fn load_spell_spec(rel: impl AsRef<Path>) -> Result<SpellSpec> {
    let txt = read_json(rel)?;
    let spec: SpellSpec = serde_json::from_str(&txt).context("parse spell json")?;
    Ok(spec)
}

pub fn load_class_spec(rel: impl AsRef<Path>) -> Result<ClassSpec> {
    let txt = read_json(rel)?;
    let spec: ClassSpec = serde_json::from_str(&txt).context("parse class json")?;
    Ok(spec)
}

pub fn load_monster_spec(rel: impl AsRef<Path>) -> Result<MonsterSpec> {
    let txt = read_json(rel)?;
    let spec: MonsterSpec = serde_json::from_str(&txt).context("parse monster json")?;
    Ok(spec)
}
