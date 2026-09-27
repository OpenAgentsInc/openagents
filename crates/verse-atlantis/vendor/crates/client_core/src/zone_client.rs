//! Client-side Zone loader that builds a CPU presentation for the renderer.
//!
//! For now this is a thin wrapper over `data_runtime::zone_snapshot` that
//! simply records which snapshot exists. GPU upload happens in `render_wgpu`.

use anyhow::{Context, Result};

#[derive(Clone, Debug)]
pub struct ZonePresentation {
    pub slug: String,
    pub trees: Option<Vec<[[f32; 4]; 4]>>, // baked tree transforms
                                           // In the future: decoded CPU batches (instances, clusters, etc.)
}

impl ZonePresentation {
    pub fn load(slug: &str) -> Result<Self> {
        // Snapshots live under workspace `packs/zones/<slug>/snapshot.v1`.
        let root = workspace_packs_root();
        let snap = data_runtime::zone_snapshot::ZoneSnapshot::load(root, slug)
            .with_context(|| format!("load zone snapshot: {slug}"))?;
        let trees = snap.trees.map(|t| t.models);
        Ok(Self {
            slug: slug.to_string(),
            trees,
        })
    }
}

fn workspace_packs_root() -> std::path::PathBuf {
    // Test override: allow tests to point to a temporary packs dir.
    if let Ok(override_root) = std::env::var("ROA_PACKS_ROOT_FOR_TESTS") {
        let p = std::path::PathBuf::from(override_root);
        if p.exists() {
            return p;
        }
    }
    // Try workspace root (../../packs) first, fall back to local packs.
    let here = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let ws = here.join("../../packs");
    if ws.exists() {
        ws
    } else {
        here.join("../../packs")
    }
}
