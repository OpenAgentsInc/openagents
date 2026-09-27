//! Sorceress asset configuration (path to model under assets/models/**).

use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize, Default)]
pub struct SorceressCfg {
    /// Relative path within the repo to the GLTF/GLB model (e.g., assets/models/sorceress.glb)
    pub model: Option<String>,
}

fn data_root() -> PathBuf {
    let here = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let ws = here.join("../../data");
    if ws.is_dir() { ws } else { here.join("data") }
}

pub fn load_default() -> Result<SorceressCfg> {
    let path = data_root().join("config/sorceress.toml");
    let mut cfg = if path.is_file() {
        let txt =
            std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
        toml::from_str::<SorcressToml>(&txt)
            .map(|t| t.sorceress)
            .context("parse sorceress TOML")?
    } else {
        SorceressCfg::default()
    };
    if let Ok(p) = std::env::var("RA_SORCERESS_MODEL") {
        cfg.model = Some(p);
    }
    Ok(cfg)
}

#[derive(Debug, Clone, Deserialize)]
struct SorcressToml {
    #[serde(default)]
    pub sorceress: SorceressCfg,
}
