//! The manifest: an immutable, content-addressed description of one version
//! of a tenant's site or game.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

use crate::sha256_hex;

pub const SCHEMA: &str = "openagents.livehost.manifest.v1";

/// The manifest feature level this host understands. A manifest whose
/// `host_min` is higher is refused.
pub const HOST_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    pub schema: String,
    pub tenant: String,
    pub host_min: u32,
    /// The manifest this one replaced, which `livehost rollback` returns to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    /// Templates, styles, copy, and assets by path, each a SHA-256.
    pub objects: BTreeMap<String, String>,
    pub routes: Vec<Route>,
    /// Browser modules (wasm32-unknown-unknown) by name.
    #[serde(default)]
    pub modules: BTreeMap<String, Built>,
    /// Handler components (wasm32-wasip2) by name.
    #[serde(default)]
    pub handlers: BTreeMap<String, Built>,
    /// Compressed copies: an object's hash to `br` or `gzip` and the hash
    /// of that copy.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub encodings: BTreeMap<String, BTreeMap<String, String>>,
    /// Free-form data for the tenant, such as a game zone's description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<serde_json::Value>,
}

/// A build output and the digest of the sources it was built from, so a
/// deploy can skip a build whose inputs didn't change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Built {
    pub object: String,
    pub inputs: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Route {
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub template: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub handler: Option<String>,
}

impl Manifest {
    pub fn to_bytes(&self) -> Vec<u8> {
        serde_json::to_vec_pretty(self).expect("a manifest always serializes")
    }

    pub fn hash(&self) -> String {
        sha256_hex(&self.to_bytes())
    }

    /// Parses stored manifest bytes, checking that they hash to `expected`
    /// and that this host can serve them.
    pub fn open(bytes: &[u8], expected: &str) -> Result<Self> {
        let actual = sha256_hex(bytes);
        if actual != expected {
            bail!("manifest hashes to {actual}, not {expected}");
        }
        let manifest: Manifest = serde_json::from_slice(bytes)?;
        if manifest.schema != SCHEMA {
            bail!("unknown manifest schema {}", manifest.schema);
        }
        if manifest.host_min > HOST_VERSION {
            bail!(
                "manifest needs host version {}, this host is {HOST_VERSION}",
                manifest.host_min
            );
        }
        Ok(manifest)
    }

    /// Every object this manifest references.
    pub fn object_hashes(&self) -> BTreeSet<String> {
        self.objects
            .values()
            .chain(self.modules.values().map(|built| &built.object))
            .chain(self.handlers.values().map(|built| &built.object))
            .chain(self.encodings.values().flat_map(|copies| copies.values()))
            .cloned()
            .collect()
    }

    /// Whether this manifest differs from `older` in content only: the
    /// same routes, modules, and handlers. Content changes need no build.
    pub fn same_code(&self, older: &Manifest) -> bool {
        self.routes == older.routes
            && self.modules == older.modules
            && self.handlers == older.handlers
            && self.meta == older.meta
    }

    /// When the only difference from `older` is in stylesheets, the changed
    /// stylesheet paths and their new URLs. A browser can then swap the
    /// styles in place instead of reloading.
    pub fn css_patch(&self, older: &Manifest) -> Option<BTreeMap<String, String>> {
        if !self.same_code(older) {
            return None;
        }
        let keys: BTreeSet<_> = self.objects.keys().chain(older.objects.keys()).collect();
        let mut patch = BTreeMap::new();
        for key in keys {
            let (new, old) = (self.objects.get(key), older.objects.get(key));
            if new == old {
                continue;
            }
            match new {
                Some(hash) if old.is_some() && key.ends_with(".css") => {
                    patch.insert(key.clone(), object_url(key, hash));
                }
                _ => return None,
            }
        }
        (!patch.is_empty()).then_some(patch)
    }
}

/// The URL an object is served at. The extension only sets the content
/// type; the hash names the object.
pub fn object_url(path: &str, hash: &str) -> String {
    match extension(path) {
        Some(ext) => format!("/_o/{hash}.{ext}"),
        None => format!("/_o/{hash}"),
    }
}

fn extension(path: &str) -> Option<&str> {
    let name = path.rsplit('/').next()?;
    let (_, ext) = name.rsplit_once('.')?;
    (!ext.is_empty()).then_some(ext)
}

/// Whether a file of this extension is worth storing compressed copies of.
pub fn compressible(path: &str) -> bool {
    matches!(
        extension(path),
        Some("html" | "css" | "js" | "json" | "wasm" | "svg" | "txt" | "md" | "gltf")
    )
}

/// The content type for an object URL's extension.
pub fn content_type(ext: &str) -> &'static str {
    match ext {
        "html" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" => "text/javascript; charset=utf-8",
        "json" => "application/json",
        "wasm" => "application/wasm",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "txt" | "md" => "text/plain; charset=utf-8",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "glb" => "model/gltf-binary",
        "gltf" => "model/gltf+json",
        "ogg" => "audio/ogg",
        "mp3" => "audio/mpeg",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest() -> Manifest {
        Manifest {
            schema: SCHEMA.into(),
            tenant: "site".into(),
            host_min: 1,
            parent: None,
            objects: BTreeMap::from([
                ("static/style.css".into(), "a".repeat(64)),
                ("templates/index.html".into(), "b".repeat(64)),
            ]),
            routes: vec![Route {
                path: "/".into(),
                template: Some("index.html".into()),
                data: None,
                handler: None,
            }],
            modules: BTreeMap::new(),
            handlers: BTreeMap::new(),
            encodings: BTreeMap::new(),
            meta: None,
        }
    }

    #[test]
    fn open_checks_the_hash_and_host_version() {
        let m = manifest();
        let bytes = m.to_bytes();
        assert_eq!(Manifest::open(&bytes, &m.hash()).unwrap(), m);
        assert!(Manifest::open(&bytes, &"0".repeat(64)).is_err());
        let mut newer = manifest();
        newer.host_min = HOST_VERSION + 1;
        assert!(Manifest::open(&newer.to_bytes(), &newer.hash()).is_err());
    }

    #[test]
    fn css_patch_only_for_stylesheet_changes() {
        let old = manifest();
        let mut css = manifest();
        css.objects
            .insert("static/style.css".into(), "c".repeat(64));
        let patch = css.css_patch(&old).unwrap();
        assert_eq!(
            patch["static/style.css"],
            format!("/_o/{}.css", "c".repeat(64))
        );

        let mut template = manifest();
        template
            .objects
            .insert("templates/index.html".into(), "d".repeat(64));
        assert!(template.css_patch(&old).is_none());
        assert!(old.css_patch(&old).is_none());
    }
}
