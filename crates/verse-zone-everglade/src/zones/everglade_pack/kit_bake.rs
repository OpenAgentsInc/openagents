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
pub const KIT_BAKE_SHA256: &str = "fc5414a1bfef9e730f3d7d779e4447f12cc86d4e571042eec42518abb30ef7c2";
/// Transfer size of the reviewed light layers; zero while none is
/// published.
pub const KIT_BAKE_BYTES: u64 = 51684139;
// Retain previous reviewed digests here when changing KIT_BAKE_SHA256.
const KIT_BAKE_HISTORY: &[&str] = &[KIT_BAKE_SHA256];
/// Exact content identity of the phone tier's light layers, the pinned
/// layers with only [`PHONE_SUNS`] (`verse-bake --phone-layers`), or empty
/// while none is published.
#[rustfmt::skip]
pub const KIT_BAKE_PHONE_SHA256: &str = "";
/// Transfer size of the phone tier's light layers; zero while none is
/// published.
pub const KIT_BAKE_PHONE_BYTES: u64 = 0;
/// The phone tier's light-layer transfer budget.
pub const KIT_BAKE_PHONE_BUDGET: u64 = 40 * 1024 * 1024;
// Retain previous reviewed digests here when changing KIT_BAKE_PHONE_SHA256.
const KIT_BAKE_PHONE_HISTORY: &[&str] = &[KIT_BAKE_PHONE_SHA256];
/// The suns the phone tier keeps, of the four baked (8:00, 12:00, 15:30,
/// and 17:30): 8:00 and 15:30, so mornings and afternoons each keep a sun
/// on their side and noon blends the two.
pub const PHONE_SUNS: [usize; 2] = [0, 2];
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

/// The layers a client `tier` fetches: the phone layers when the tier is
/// phone and they are published, the full layers otherwise.
#[must_use]
pub fn pinned_for(tier: super::kit::Tier) -> PinnedFile {
    if tier == super::kit::Tier::Phone && KIT_BAKE_PHONE_BYTES > 0 {
        return PinnedFile {
            label: "Everglade phone light layers",
            sha256: KIT_BAKE_PHONE_SHA256,
            bytes: KIT_BAKE_PHONE_BYTES,
            url: format!("{KIT_ORIGIN}/bake/{KIT_BAKE_PHONE_SHA256}.vlay"),
            extension: "vlay",
            temp_prefix: ".everglade-kit-bake-phone-",
            history: KIT_BAKE_PHONE_HISTORY,
        };
    }
    pinned()
}

/// The phone tier's layers derived from the full layers `full`.
///
/// # Errors
///
/// Returns a message when `full` lacks a phone sun.
pub fn phone_layers(full: &Layers) -> Result<Layers, String> {
    full.with_suns(&PHONE_SUNS)
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
    fetch_tier(cache, download, cancel, super::kit::Tier::Full)
}

/// [`fetch`] for a client `tier` ([`pinned_for`]).
///
/// # Errors
///
/// As [`fetch`].
pub fn fetch_tier(
    cache: &Path,
    download: bool,
    cancel: &AtomicBool,
    tier: super::kit::Tier,
) -> Result<Layers, String> {
    let file = pinned_for(tier);
    if file.bytes == 0 {
        return Err("No kit light layers are published".into());
    }
    let decode = |bytes: &[u8]| {
        file.verify(bytes)?;
        Layers::decode(bytes)
    };
    if download {
        return file.fetch(cache, cancel, &mut |_, _| (), decode);
    }
    let bytes = file.read_bounded(&cache.join(file.cache_name()))?;
    decode(&bytes)
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

/// Exact platform scenes audited against the completed artifact.
#[must_use]
pub fn compatibility() -> Arc<verse_pbr::pbr::baked_layers::SceneCompatibility> {
    static RECORD: std::sync::OnceLock<Arc<verse_pbr::pbr::baked_layers::SceneCompatibility>> =
        std::sync::OnceLock::new();
    RECORD
        .get_or_init(|| {
            let mut record: verse_pbr::pbr::baked_layers::SceneCompatibility =
                serde_json::from_str(include_str!(
                    "../../../../../assets/verse/everglade-layer-compatibility.json"
                ))
                .expect("the checked-in layer compatibility record is valid");
            // The phone layers are derived from the reviewed artifact, so
            // the scenes that accept it accept them.
            if KIT_BAKE_PHONE_BYTES > 0 && record.artifact_sha256 == KIT_BAKE_SHA256 {
                record
                    .derived
                    .push(verse_pbr::pbr::baked_layers::DerivedArtifact {
                        artifact_sha256: KIT_BAKE_PHONE_SHA256.into(),
                        artifact_bytes: KIT_BAKE_PHONE_BYTES,
                    });
            }
            Arc::new(record)
        })
        .clone()
}

#[cfg(test)]
mod compatibility_tests {
    use super::*;

    #[test]
    #[ignore = "requires the private completed layer artifact in VERSE_KIT_BAKE"]
    fn completed_layers_match_reviewed_platform_identities() {
        let path =
            std::env::var_os(LOCAL_ENV).expect("set VERSE_KIT_BAKE to the completed artifact");
        let bytes = std::fs::read(path).unwrap();
        let layers = Layers::decode(&bytes).unwrap();
        let record = compatibility();
        for target in &record.targets {
            assert!(record.accepts(&layers, &target.scene, target.bake_key.as_deref()));
        }
        assert!(!record.accepts(&layers, "unknown", None));
    }
}
