//! Bounded, source-bound RGBA8 mip chains for content tools and render preparation.
use super::{Level, RECIPE_VERSION, Role, Variant};
use crate::assets::Pack;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
pub const MANIFEST: &str = "mips.json";
pub const PAYLOAD: &str = "mips.rgba";
pub const MAX_BYTES: usize = 256 * 1024 * 1024;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Slice {
    pub width: u32,
    pub height: u32,
    pub offset: u64,
    pub bytes: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub variant: Variant,
    pub source_sha256: String,
    pub source_width: u32,
    pub source_height: u32,
    pub levels: Vec<Slice>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub schema: String,
    pub recipe: u32,
    pub rgba_sha256: String,
    pub entries: Vec<Entry>,
}
/// Content identity binds the descriptor and every retained mip byte.
#[derive(Clone, Debug)]
pub struct Archive {
    manifest: Manifest,
    rgba: std::sync::Arc<[u8]>,
}
fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
impl Archive {
    #[cfg(feature = "asset-io")]
    pub fn cook(prepared: &crate::loading::Prepared) -> Result<Self, String> {
        Self::cook_reusing(prepared, None)
    }
    #[cfg(feature = "asset-io")]
    pub fn cook_reusing(
        prepared: &crate::loading::Prepared,
        retained: Option<&Self>,
    ) -> Result<Self, String> {
        let mut entries = vec![];
        let mut rgba = vec![];
        for variant in super::pack_variants(prepared.pack())? {
            let source = &prepared.pack().textures[variant.texture];
            let texture = &prepared.textures()[variant.texture];
            let expected = super::bytes(texture.width(), texture.height(), super::MAX_DIMENSION)?;
            if expected as usize > MAX_BYTES.saturating_sub(rgba.len()) {
                return Err("Cooked mip archive exceeds 256 MiB".into());
            }
            let reusable = retained.filter(|archive| {
                archive.manifest.entries.iter().any(|entry| {
                    entry.variant == variant
                        && entry.source_sha256 == source.sha256
                        && entry.source_width == source.width
                        && entry.source_height == source.height
                })
            });
            let chain = if let Some(archive) = reusable {
                archive.levels(variant, super::MAX_DIMENSION)?
            } else {
                super::cook(
                    texture.width(),
                    texture.height(),
                    texture.rgba(),
                    variant.role,
                    super::MAX_DIMENSION,
                )?
            };
            let levels = chain
                .into_iter()
                .map(|(width, height, bytes)| {
                    let row = Slice {
                        width,
                        height,
                        offset: rgba.len() as u64,
                        bytes: bytes.len() as u64,
                    };
                    rgba.extend(bytes);
                    row
                })
                .collect();
            entries.push(Entry {
                variant,
                source_sha256: source.sha256.clone(),
                source_width: source.width,
                source_height: source.height,
                levels,
            });
        }
        let manifest = Manifest {
            schema: "verse.mip.archive.v1".into(),
            recipe: RECIPE_VERSION,
            rgba_sha256: digest(&rgba),
            entries,
        };
        let result = Self {
            manifest,
            rgba: rgba.into(),
        };
        result.validate(prepared.pack())?;
        Ok(result)
    }
    pub fn from_bytes(pack: &Pack, manifest: &[u8], rgba: &[u8]) -> Result<Self, String> {
        if manifest.len() > 8 * 1024 * 1024 || rgba.len() > MAX_BYTES {
            return Err("Cooked mip archive exceeds its metadata or payload budget".into());
        }
        let manifest: Manifest =
            serde_json::from_slice(manifest).map_err(|e| format!("Invalid mip archive: {e}"))?;
        let archive = Self {
            manifest,
            rgba: std::sync::Arc::from(rgba),
        };
        archive.validate(pack)?;
        Ok(archive)
    }
    pub fn validate(&self, pack: &Pack) -> Result<(), String> {
        if self.manifest.schema != "verse.mip.archive.v1"
            || self.manifest.recipe != RECIPE_VERSION
            || self.manifest.entries.len() > 16384
            || self.rgba.len() > MAX_BYTES
            || digest(&self.rgba) != self.manifest.rgba_sha256
        {
            return Err("Mip archive schema, recipe, payload digest, or budget differs".into());
        }
        pack.validate()?;
        let expected = super::pack_variants(pack)?;
        let mut actual = BTreeSet::new();
        let mut offset = 0u64;
        for entry in &self.manifest.entries {
            let source = pack
                .textures
                .get(entry.variant.texture)
                .ok_or("Mip archive texture slot is absent")?;
            if !actual.insert(entry.variant)
                || entry.source_sha256 != source.sha256
                || entry.source_width != source.width
                || entry.source_height != source.height
            {
                return Err("Mip archive source identity, extent, or variant differs".into());
            }
            if matches!(entry.variant.role,Role::Mask{cutoff} if !f32::from_bits(cutoff).is_finite() || !(0.0..=1.0).contains(&f32::from_bits(cutoff)) || f32::from_bits(cutoff)==0.)
            {
                return Err("Invalid archived mip mask threshold".into());
            }
            if entry.levels.is_empty() || entry.levels.len() > 14 {
                return Err("Mip archive chain length exceeds its bound".into());
            }
            let (mut width, mut height) = (source.width, source.height);
            for (index, level) in entry.levels.iter().enumerate() {
                let bytes = u64::from(width) * u64::from(height) * 4;
                if level.width != width
                    || level.height != height
                    || level.bytes != bytes
                    || level.offset != offset
                    || (index > 0
                        && entry.levels[index - 1].width == 1
                        && entry.levels[index - 1].height == 1)
                {
                    return Err(
                        "Mip archive has a gap, overlap, missing level, or inconsistent extent"
                            .into(),
                    );
                }
                offset = offset
                    .checked_add(bytes)
                    .ok_or("Mip archive offset overflow")?;
                if offset > self.rgba.len() as u64 {
                    return Err("Mip archive level exceeds its payload".into());
                }
                width = (width / 2).max(1);
                height = (height / 2).max(1);
            }
            if entry
                .levels
                .last()
                .is_none_or(|l| l.width != 1 || l.height != 1)
            {
                return Err("Mip archive must end at one texel".into());
            }
        }
        if actual != expected || offset != self.rgba.len() as u64 {
            return Err("Mip archive variant closure or payload length differs".into());
        }
        Ok(())
    }
    pub fn encoded_manifest(&self) -> Result<Vec<u8>, String> {
        serde_json::to_vec(&self.manifest).map_err(|e| e.to_string())
    }
    pub fn payload(&self) -> &[u8] {
        &self.rgba
    }
    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }
    pub fn identity(&self) -> Result<[u8; 32], String> {
        let mut hash = Sha256::new();
        hash.update(b"verse.mip.archive.identity.v1\0");
        hash.update(self.encoded_manifest()?);
        hash.update(&self.rgba);
        Ok(hash.finalize().into())
    }
    /// Return only device-fitting levels. Retained bytes stay verified for recreation.
    pub fn levels(&self, variant: Variant, max: u32) -> Result<Vec<Level>, String> {
        if max == 0 {
            return Err("Mip device extent must be positive".into());
        }
        let entry = self
            .manifest
            .entries
            .iter()
            .find(|e| e.variant == variant)
            .ok_or("Mip archive variant is absent")?;
        let mut levels = vec![];
        for row in &entry.levels {
            if row.width <= max && row.height <= max {
                let start = row.offset as usize;
                let end = start + row.bytes as usize;
                levels.push((row.width, row.height, self.rgba[start..end].to_vec()));
            }
        }
        if levels.is_empty() {
            return Err("Mip chain has no fitting level".into());
        }
        Ok(levels)
    }
    #[cfg(feature = "asset-io")]
    pub fn read(pack: &Pack, root: &std::path::Path) -> Result<Option<Self>, String> {
        Self::read_budget(pack, root, MAX_BYTES + 8 * 1024 * 1024)
    }
    #[cfg(feature = "asset-io")]
    pub fn read_budget(
        pack: &Pack,
        root: &std::path::Path,
        available: usize,
    ) -> Result<Option<Self>, String> {
        let presence = |name| match std::fs::symlink_metadata(root.join(name)) {
            Ok(_) => Ok(true),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(e.to_string()),
        };
        match (presence(MANIFEST)?, presence(PAYLOAD)?) {
            (false, false) => return Ok(None),
            (true, true) => (),
            _ => return Err("Mip archive is incomplete".into()),
        };
        let read = |name: &str, limit: usize| -> Result<Vec<u8>, String> {
            use std::io::Read;
            let path = root.join(name);
            let metadata = std::fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
            if !metadata.is_file() || metadata.len() > limit as u64 {
                return Err("Mip archive input must be a bounded regular file".into());
            }
            #[cfg(unix)]
            let file = crate::loading::open_texture(
                &std::fs::File::open(root).map_err(|e| e.to_string())?,
                name,
            )?;
            #[cfg(not(unix))]
            let file = std::fs::File::open(&path).map_err(|e| e.to_string())?;
            let mut bytes = vec![];
            file.take(limit as u64 + 1)
                .read_to_end(&mut bytes)
                .map_err(|e| e.to_string())?;
            if bytes.len() > limit {
                return Err("Mip archive input grew beyond its budget".into());
            }
            Ok(bytes)
        };
        let manifest = read(MANIFEST, (8 * 1024 * 1024).min(available))?;
        let payload = read(
            PAYLOAD,
            MAX_BYTES.min(available.saturating_sub(manifest.len())),
        )?;
        Self::from_bytes(pack, &manifest, &payload).map(Some)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn pack() -> Pack {
        serde_json::from_value(serde_json::json!({"version":1,"source_revision":"archive-test","models":{"room":{"source":"test","source_sha256":"","surfaces":[],"bones":[],"clips":[],"height":1}},"textures":[{"file":"test.png","sha256":"a".repeat(64),"width":3,"height":2}]})).unwrap()
    }
    fn archive() -> Archive {
        let rgba = vec![120; 28];
        Archive {
            manifest: Manifest {
                schema: "verse.mip.archive.v1".into(),
                recipe: RECIPE_VERSION,
                rgba_sha256: digest(&rgba),
                entries: vec![Entry {
                    variant: Variant {
                        texture: 0,
                        role: Role::Color,
                    },
                    source_sha256: "a".repeat(64),
                    source_width: 3,
                    source_height: 2,
                    levels: vec![
                        Slice {
                            width: 3,
                            height: 2,
                            offset: 0,
                            bytes: 24,
                        },
                        Slice {
                            width: 1,
                            height: 1,
                            offset: 24,
                            bytes: 4,
                        },
                    ],
                }],
            },
            rgba: rgba.into(),
        }
    }
    #[test]
    fn round_trip_fitting_levels_and_bound_identity() {
        let pack = pack();
        let archive = archive();
        archive.validate(&pack).unwrap();
        let loaded = Archive::from_bytes(
            &pack,
            &archive.encoded_manifest().unwrap(),
            archive.payload(),
        )
        .unwrap();
        assert_eq!(loaded.identity().unwrap(), archive.identity().unwrap());
        assert_eq!(
            loaded
                .levels(
                    Variant {
                        texture: 0,
                        role: Role::Color
                    },
                    2
                )
                .unwrap(),
            vec![(1, 1, vec![120; 4])]
        );
        assert!(
            loaded
                .levels(
                    Variant {
                        texture: 0,
                        role: Role::Color
                    },
                    0
                )
                .is_err()
        );
        let mut changed = archive.clone();
        std::sync::Arc::make_mut(&mut changed.rgba)[24] = 121;
        changed.manifest.rgba_sha256 = digest(&changed.rgba);
        assert_ne!(archive.identity().unwrap(), changed.identity().unwrap());
    }
    #[test]
    fn gaps_extra_levels_stale_sources_and_corruption_are_refused() {
        let pack = pack();
        let mut value = archive();
        value.manifest.entries[0].levels[1].offset = 23;
        assert!(value.validate(&pack).is_err());
        let mut value = archive();
        value.manifest.entries[0].source_sha256 = "b".repeat(64);
        assert!(value.validate(&pack).is_err());
        let mut value = archive();
        value.manifest.entries[0].levels.pop();
        assert!(value.validate(&pack).is_err());
        let mut value = archive();
        value
            .manifest
            .entries
            .push(value.manifest.entries[0].clone());
        assert!(value.validate(&pack).is_err());
        let value = archive();
        assert!(Archive::from_bytes(&pack, &value.encoded_manifest().unwrap(), &[0; 28]).is_err());
    }
}
