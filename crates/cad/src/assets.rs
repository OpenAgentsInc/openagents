//! Source files pinned to a CAD version instead of a machine's build cache.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, path::Path};

pub const MAX_BYTES: usize = 2 * 1024 * 1024;
pub const MAX_TOTAL: usize = 8 * 1024 * 1024;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Asset {
    pub sha256: String,
    pub format: String,
    pub size_bytes: u64,
}
impl Asset {
    pub fn valid(&self) -> bool {
        self.sha256.len() == 64
            && self
                .sha256
                .bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
            && matches!(self.format.as_str(), "step" | "stl")
            && self.size_bytes <= MAX_BYTES as u64
    }
    pub fn name(&self) -> String {
        format!("asset-{}.{}", self.sha256, self.format)
    }
    pub fn verify(&self, bytes: &[u8]) -> bool {
        self.valid()
            && bytes.len() as u64 == self.size_bytes
            && format!("{:x}", Sha256::digest(bytes)) == self.sha256
    }
}
pub fn references(document: &Value) -> Result<Vec<(String, String)>, String> {
    fn walk(value: &Value, out: &mut BTreeMap<String, String>) -> Result<(), String> {
        match value {
            Value::Object(map) => {
                if let Some(format) = match map.get("op").and_then(Value::as_str) {
                    Some("solid.import_step") => Some("step"),
                    Some("solid.import_stl") => Some("stl"),
                    _ => None,
                } {
                    let hash = map
                        .get("params")
                        .and_then(|p| p["file"].as_str())
                        .and_then(|v| v.strip_prefix("sha256:"))
                        .unwrap_or_default();
                    let asset = Asset {
                        sha256: hash.into(),
                        format: format.into(),
                        size_bytes: 0,
                    };
                    if !asset.valid() {
                        return Err("An imported model has an invalid file reference.".into());
                    }
                    out.insert(format!("{}.{}", hash, format), hash.into());
                }
                for v in map.values() {
                    walk(v, out)?;
                }
            }
            Value::Array(items) => {
                for v in items {
                    walk(v, out)?;
                }
            }
            _ => (),
        }
        Ok(())
    }
    let mut out = BTreeMap::new();
    walk(document, &mut out)?;
    if out.len() > 128 {
        return Err("A design can reference at most 128 imported models.".into());
    }
    Ok(out.into_iter().collect())
}
pub fn save(document: &Value, cache: &Path, version: &Path) -> Result<(), String> {
    let mut assets = Vec::new();
    let mut total = 0;
    for (name, sha256) in references(document)? {
        let bytes = crate::sys::read(&crate::build::files_dir(cache).join(&name))
            .ok_or("An imported model's source file is missing.")?;
        total += bytes.len();
        if bytes.len() > MAX_BYTES || total > MAX_TOTAL {
            return Err("Imported models must be at most 2 MiB each and 8 MiB per design.".into());
        }
        let format = name.rsplit('.').next().unwrap().to_owned();
        let asset = Asset {
            sha256,
            format,
            size_bytes: bytes.len() as u64,
        };
        if !asset.verify(&bytes) {
            return Err("An imported model failed its SHA-256 check.".into());
        }
        crate::sys::write(&version.join("out").join(asset.name()), &bytes)?;
        assets.push(asset);
    }
    crate::sys::write(
        &version.join("assets.json"),
        &serde_json::to_vec(&assets).map_err(|e| e.to_string())?,
    )
}
pub fn restore(cache: &Path, version: &Path) -> Result<(), String> {
    let Some(text) = crate::sys::read_text(&version.join("assets.json")) else {
        return Ok(());
    };
    let assets: Vec<Asset> = serde_json::from_str(&text)
        .map_err(|_| "The model's source file list couldn't be read.")?;
    if assets.len() > 128 {
        return Err("This design references too many model files.".into());
    }
    let mut total = 0;
    for asset in assets {
        if !asset.valid() {
            return Err("An imported model's file reference is invalid.".into());
        }
        total += asset.size_bytes;
        if total > MAX_TOTAL as u64 {
            return Err("This design's imported models exceed 8 MiB.".into());
        }
        let cached =
            crate::build::files_dir(cache).join(format!("{}.{}", asset.sha256, asset.format));
        if crate::sys::read(&cached).is_some_and(|bytes| asset.verify(&bytes)) {
            continue;
        }
        let bytes = crate::sys::read(&version.join("out").join(asset.name()))
            .ok_or("An imported model's source file is missing.")?;
        if !asset.verify(&bytes) {
            return Err("An imported model failed its SHA-256 check.".into());
        }
        crate::build::store_file(cache, &bytes, &asset.format)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn imported_sources_can_be_recovered_after_the_build_cache_is_removed() {
        let root = std::env::temp_dir().join(format!(
            "cad-assets-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let cache = root.join("cache");
        let version = root.join("v1");
        let bytes = b"ISO-10303-21;\nDATA;ENDSEC;END-ISO-10303-21;";
        let reference = crate::build::store_file(&cache, bytes, "step").unwrap();
        let doc = serde_json::json!({"features":[{"op":"solid.import_step","params":{"file":reference}}]});
        save(&doc, &cache, &version).unwrap();
        std::fs::remove_dir_all(&cache).unwrap();
        restore(&cache, &version).unwrap();
        let name = references(&doc).unwrap()[0].0.clone();
        assert_eq!(
            crate::sys::read(&crate::build::files_dir(&cache).join(&name)).unwrap(),
            bytes
        );
        let asset: Vec<Asset> =
            serde_json::from_str(&crate::sys::read_text(&version.join("assets.json")).unwrap())
                .unwrap();
        std::fs::remove_dir_all(&cache).unwrap();
        std::fs::write(version.join("out").join(asset[0].name()), b"changed").unwrap();
        assert!(restore(&cache, &version).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
}
