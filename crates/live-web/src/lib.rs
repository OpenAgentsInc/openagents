//! The browser half of a livehost zone (`docs/verse/ice-cream-man-live.md`):
//! follow a tenant's channel through the host's event stream
//! (`/t/TENANT/_live`), fetch objects by hash, ack an applied version on
//! livehost's ack stream, and count the bytes a version moved. A game's own
//! client (`icecream-web`'s, `haunt-web`'s) decides what a version means.

use serde::Deserialize;

/// A channel pointer as the host streams it.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct Pointer {
    pub manifest: String,
    #[serde(default)]
    pub seq: u64,
}

/// The address of `path` in a manifest's objects.
///
/// # Errors
/// The manifest has no such path.
pub fn url_of(objects: &icecream_zone::Objects, path: &str) -> Result<String, String> {
    objects
        .get(path)
        .map(|hash| icecream_zone::object_url(path, hash))
        .ok_or_else(|| format!("the manifest has no {path}"))
}

#[cfg(target_arch = "wasm32")]
mod browser;
#[cfg(target_arch = "wasm32")]
pub use browser::*;
