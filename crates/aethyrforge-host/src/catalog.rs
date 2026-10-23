//! Public asset metadata. Geometry, textures, and import implementation stay private.

use crate::Error;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const FANTASY_CATALOG_SCHEMA: &str = "openagents.verse.fantasy-catalog.v1";
pub const FANTASY_SOURCE_COUNT: usize = 94;
pub const MAX_CATALOG_BYTES: usize = 1024 * 1024;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct CatalogResource {
    pub path: String,
    pub sha256: String,
    pub bytes: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssetBounds {
    pub min: [f64; 3],
    pub max: [f64; 3],
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SourceAsset {
    pub id: String,
    pub name: String,
    pub source: String,
    pub resources: Vec<CatalogResource>,
    pub bounds: AssetBounds,
    pub vertices: u64,
    pub triangles: u64,
    pub material_slots: usize,
    pub animation_policy: String,
    pub source_clips: Vec<String>,
    pub uv1: bool,
    pub vertex_colors: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct EditableSurface {
    pub id: String,
    pub material_id: String,
    pub uv_set: u8,
    pub uv_bounds: [f64; 4],
    pub size: [u32; 2],
    pub shared_with_other_assets: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct RuntimeAsset {
    pub id: String,
    pub name: String,
    pub origin: String,
    pub rights: String,
    pub bounds: AssetBounds,
    pub collider_policy: String,
    pub editable_surfaces: Vec<EditableSurface>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct FantasyCatalog {
    pub schema: String,
    pub supplier: String,
    pub license: String,
    pub license_sha256: String,
    pub compiler: String,
    pub compiler_sha256: String,
    pub compiled_catalog_sha256: String,
    pub review_scene_sha256: String,
    pub entries: Vec<SourceAsset>,
    pub runtime: Vec<RuntimeAsset>,
    pub prepared_resources: Vec<CatalogResource>,
    pub texture_dimension: u32,
    pub base_color_space: String,
    pub data_color_space: String,
    pub normal_convention: String,
    pub source_channels: String,
    pub runtime_channels: String,
    pub transfer_bytes: u64,
    pub decoded_texture_budget_bytes: u64,
}

fn hash(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn relative(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 256
        && !s.contains(['\\', ':', '?', '#', '%', '\0'])
        && s.split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..")
}
fn bounds(b: &AssetBounds) -> bool {
    b.min.iter().zip(b.max).all(|(lo, hi)| {
        lo.is_finite() && hi.is_finite() && *lo <= hi && lo.abs() <= 1000. && hi.abs() <= 1000.
    })
}
fn resource(r: &CatalogResource) -> bool {
    relative(&r.path) && hash(&r.sha256) && r.bytes > 0 && r.bytes <= 32 * 1024 * 1024
}

impl FantasyCatalog {
    pub fn decode(bytes: &[u8]) -> Result<Self, Error> {
        if bytes.len() > MAX_CATALOG_BYTES {
            return Err(Error::Invalid("catalog size"));
        }
        let catalog: Self =
            serde_json::from_slice(bytes).map_err(|_| Error::Invalid("catalog JSON"))?;
        catalog.validate()?;
        Ok(catalog)
    }

    /// Validates metadata, without granting access to assets or confirming content hashes.
    pub fn validate(&self) -> Result<(), Error> {
        let invalid = || Error::Invalid("asset catalog");
        if self.schema != FANTASY_CATALOG_SCHEMA
            || self.supplier != "Quaternius"
            || self.license != "CC0-1.0"
            || ![
                &self.license_sha256,
                &self.compiler_sha256,
                &self.compiled_catalog_sha256,
                &self.review_scene_sha256,
            ]
            .iter()
            .all(|s| hash(s))
            || self.compiler.len() > 128
            || self.entries.len() != FANTASY_SOURCE_COUNT
            || self.runtime.is_empty()
            || self.runtime.len() > 40
            || self.prepared_resources.len() > 256
            || ![128, 256, 512].contains(&self.texture_dimension)
            || self.base_color_space != "srgb"
            || self.data_color_space != "linear"
            || self.normal_convention != "opengl"
            || self.source_channels != "occlusion-r-roughness-g-metallic-b"
            || self.runtime_channels != "metallic-r-roughness-g-occlusion-b"
            || self.transfer_bytes > 32 * 1024 * 1024
            || self.decoded_texture_budget_bytes > 32 * 1024 * 1024
        {
            return Err(invalid());
        }
        let mut ids = BTreeSet::new();
        let mut references = std::collections::BTreeMap::new();
        for entry in &self.entries {
            if entry.id
                != format!(
                    "quaternius:fantasy-standard:{}",
                    entry.name.to_ascii_lowercase()
                )
                || !crate::protocol::id(&entry.id)
                || !ids.insert(&entry.id)
                || !relative(&entry.source)
                || !bounds(&entry.bounds)
                || entry.resources.is_empty()
                || entry.resources.len() > 32
                || !entry.resources.iter().all(resource)
                || !entry.resources.iter().any(|r| r.path == entry.source)
                || entry.vertices == 0
                || entry.vertices > 1_000_000
                || entry.triangles == 0
                || entry.triangles > 1_000_000
                || entry.material_slots == 0
                || entry.material_slots > 128
            {
                return Err(invalid());
            }
            if entry.name == "Chest_Wood" {
                if entry.animation_policy != "baked Chest_Closed first frame"
                    || entry.source_clips
                        != ["Chest_Close", "Chest_Closed", "Chest_Open", "Chest_Opened"]
                {
                    return Err(invalid());
                }
            } else if entry.animation_policy != "static" || !entry.source_clips.is_empty() {
                return Err(invalid());
            }
            let mut paths = BTreeSet::new();
            for r in &entry.resources {
                if !paths.insert(&r.path) {
                    return Err(invalid());
                }
                if let Some(previous) = references.insert(&r.path, (&r.sha256, r.bytes)) {
                    if previous != (&r.sha256, r.bytes) {
                        return Err(invalid());
                    }
                }
            }
        }
        let mut runtime_ids = BTreeSet::new();
        let mut surfaces = BTreeSet::new();
        for asset in &self.runtime {
            let rights = if asset.id.starts_with("custom:haunt:") {
                "openagents-project-runtime"
            } else if asset.id.starts_with("medieval-town:") {
                "medieval-town-licensed-runtime"
            } else {
                "quaternius-cc0-1.0"
            };
            if !crate::protocol::id(&asset.id)
                || !runtime_ids.insert(&asset.id)
                || !bounds(&asset.bounds)
                || asset.name.len() > 128
                || asset.origin.len() > 256
                || asset.rights != rights
                || asset.collider_policy != "instance AABB or disabled decoration"
                || asset.editable_surfaces.len() > 16
                || !(ids.contains(&asset.id)
                    || asset.id.starts_with("custom:haunt:")
                    || asset.id.starts_with("medieval-town:"))
            {
                return Err(invalid());
            }
            for surface in &asset.editable_surfaces {
                let [u0, v0, u1, v1] = surface.uv_bounds;
                if !crate::protocol::id(&surface.id)
                    || !crate::protocol::id(&surface.material_id)
                    || !surfaces.insert(&surface.id)
                    || !surface.material_id.starts_with(&format!("{}:", asset.id))
                    || surface.uv_set != 0
                    || surface.shared_with_other_assets
                    || ![u0, v0, u1, v1]
                        .iter()
                        .all(|v| v.is_finite() && (0.0..=1.0).contains(v))
                    || u0 >= u1
                    || v0 >= v1
                    || surface.size.iter().any(|n| *n == 0 || *n > 1024)
                {
                    return Err(invalid());
                }
            }
        }
        let mut outputs = BTreeSet::new();
        let mut bytes = 0;
        for r in &self.prepared_resources {
            if !resource(r) || !outputs.insert(&r.path) {
                return Err(invalid());
            }
            bytes += r.bytes;
        }
        if bytes != self.transfer_bytes {
            return Err(invalid());
        }
        Ok(())
    }
}
