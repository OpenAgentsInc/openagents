//! The kit town's offline-baked light: the sky, sun, and lamp layers
//! `verse-bake --layers` bakes from the town with the licensed kit
//! installed ([`verse_pbr::pbr::baked_layers`]).
//!
//! The layers are derived from licensed geometry, so they ship the way the
//! kit pack does: built outside the repository, kept in the private bucket
//! and the zone cache, and pinned here by digest through the artifact queue
//! (`openagents artifact submit everglade-kit-bake`). They sit beside the kit
//! pack, not inside it, so the kit pack stays small for the web and
//! phones, and a layout change that rebakes the light doesn't rebuild the
//! kit.
//!
//! A loader offers the layers it finds ([`offer`]); the zone uses them only
//! when their scene digest matches the town it builds. Without layers, or
//! with layers for another scene, the town bakes its light at load as
//! before (`textured_bake::BakeJob`).

use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use verse_pbr::pbr::baked_layers::Layers;

use super::kit::KIT_ORIGIN;
use super::pinned::PinnedFile;

/// Exact content identity of the reviewed light layers, or empty while
/// none is published.
#[rustfmt::skip]
pub const KIT_BAKE_SHA256: &str = "14ae7f75e9ce4f81483f6f44369753545cb2cab892177607438b3077ebbbae23";
/// Transfer size of the reviewed light layers; zero while none is
/// published.
pub const KIT_BAKE_BYTES: u64 = 51682623;
// Retain previous reviewed digests here when changing KIT_BAKE_SHA256.
const KIT_BAKE_HISTORY: &[&str] = &[KIT_BAKE_SHA256];
/// Environment variable naming a local layer file for offline tools, such
/// as `everglade_capture`.
pub const LOCAL_ENV: &str = "VERSE_KIT_BAKE";

/// The reviewed light layers and their source,
/// `<KIT_ORIGIN>/bake/<KIT_BAKE_SHA256>.vlay`.
#[must_use]
pub fn pinned() -> PinnedFile {
    PinnedFile {
        label: "Everglade kit light layers",
        sha256: KIT_BAKE_SHA256,
        bytes: KIT_BAKE_BYTES,
        url: format!("{KIT_ORIGIN}/bake/{KIT_BAKE_SHA256}.vlay"),
        extension: "vlay",
        temp_prefix: ".everglade-kit-bake-",
        history: KIT_BAKE_HISTORY,
    }
}

static OFFERED: Mutex<Option<Arc<Layers>>> = Mutex::new(None);

/// Offers `layers` to the next town that bakes its light.
pub fn offer(layers: Layers) {
    *OFFERED
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(Arc::new(layers));
}

/// The layers last offered, if any.
#[must_use]
pub fn offered() -> Option<Arc<Layers>> {
    OFFERED
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone()
}

/// Verifies the pinned digest, then decodes the layers.
///
/// # Errors
///
/// Returns a message when the bytes are not the pinned layers or fail to
/// decode.
pub fn decode_pinned(bytes: &[u8]) -> Result<Layers, String> {
    pinned().verify(bytes)?;
    Layers::decode(bytes)
}

/// The pinned layers in `cache`, downloaded from [`KIT_ORIGIN`] when
/// `download` allows it.
///
/// # Errors
///
/// Returns a message when none are published, the cache holds none and
/// downloading is off, or the transfer or decoding fails.
pub fn fetch(cache: &Path, download: bool, cancel: &AtomicBool) -> Result<Layers, String> {
    let file = pinned();
    if file.bytes == 0 {
        return Err("No kit light layers are published".into());
    }
    if download {
        return file.fetch(cache, cancel, &mut |_, _| (), decode_pinned);
    }
    let bytes = file.read_bounded(&cache.join(file.cache_name()))?;
    decode_pinned(&bytes)
}

/// Layers from a local file, for offline tools: the pinned layers, or,
/// while none are pinned or with the kit's unpinned switch set
/// ([`super::kit::UNPINNED_ENV`]), any layer file that decodes. The zone
/// still uses them only for the scene they were baked for.
///
/// # Errors
///
/// Returns a message when the file is unreadable, is not the pinned
/// layers, or fails to decode.
pub fn load_local(path: &Path) -> Result<Layers, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if KIT_BAKE_BYTES == 0 || std::env::var_os(super::kit::UNPINNED_ENV).is_some() {
        return Layers::decode(&bytes);
    }
    decode_pinned(&bytes)
}
