//! Zones in the phone shell (ADR 0009, ADR 0010).
//!
//! - The **manifest model**: every zone the launcher lists, with its runner.
//!   The Grid is built in; the Haunted House and the Verse are web releases,
//!   pinned by the same JSON the website pins
//!   (`crates/openagents-web/content/demos/*.json`), bundled at build time and
//!   refreshed from the website's public `GET /v1/zones` catalog.
//! - The **launcher** ([`CoreZones`]): Rust Native cards, one per zone.
//! - The **web runner** ([`CoreZoneRunner`]): what a platform web view may
//!   load, the WebGPU gate, loading and failure cards, and the native end of
//!   the `aethyrforge-host` bridge. Swift and Kotlin only present it.
//!
//! No credentials enter a web view. Sign-in and payments stay native sheets;
//! a zone asks the shell for them over the bridge.

mod bundled_grid;
mod launcher;
mod runner;
#[cfg(test)]
mod tests;

pub use bundled_grid::*;
pub use launcher::*;
pub use runner::*;

use aethyrforge_host::{
    HAUNTED_HOUSE_ZONE, UNIVERSE_ZONE, ZONE_CATALOG_SCHEMA, ZoneCatalog, ZoneEntry, ZoneRelease,
    kiki_zone,
};

/// The built-in coding-agent zone.
pub const GRID_ZONE: &str = "grid";

/// The release pins the website serves, bundled so the launcher works offline
/// and before the first catalog refresh.
const BUNDLED_HOUSE: &str =
    include_str!("../../../../../../crates/openagents-web/content/demos/haunted-house.json");
const BUNDLED_UNIVERSE: &str =
    include_str!("../../../../../../crates/openagents-web/content/demos/universe.json");

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum ZoneError {
    #[error("{message}")]
    Invalid { message: String },
    #[error("{message}")]
    Unavailable { message: String },
}
fn invalid(message: &str) -> ZoneError {
    ZoneError::Invalid {
        message: message.into(),
    }
}

/// How a zone runs on this device.
#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum ZoneRunnerKind {
    /// Compiled into the shell; nothing is downloaded.
    Builtin,
    /// The zone's web release in a platform web view.
    Web,
}

/// The least detail a zone accepts (ADR 0007). Placeholder until the shared
/// device profile lands; every zone accepts any device today.
#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum ZoneDetailFloor {
    Any,
}

/// One launcher entry.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct ZoneManifest {
    pub id: String,
    pub title: String,
    pub summary: String,
    pub runner: ZoneRunnerKind,
    /// The immutable release ID (web zones only).
    pub release_id: Option<String>,
    /// The page a web runner loads (web zones only).
    pub url: Option<String>,
    /// The exact origin a web runner may show (web zones only).
    pub origin: Option<String>,
    /// The website page to open in a browser when the web view can't run it.
    pub page_url: Option<String>,
    pub detail_floor: ZoneDetailFloor,
}

/// A launcher entry with its web release, kept on the Rust side.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Zone {
    pub manifest: ZoneManifest,
    pub release: Option<ZoneRelease>,
}

pub(crate) fn grid() -> Zone {
    Zone {
        manifest: ZoneManifest {
            id: GRID_ZONE.into(),
            title: "The Grid".into(),
            summary: "A shared workspace for your coding agents.".into(),
            runner: ZoneRunnerKind::Builtin,
            release_id: None,
            url: None,
            origin: None,
            page_url: None,
            detail_floor: ZoneDetailFloor::Any,
        },
        release: None,
    }
}

fn web(entry: ZoneEntry) -> Zone {
    Zone {
        manifest: ZoneManifest {
            id: entry.id,
            title: entry.title,
            summary: entry.summary,
            runner: ZoneRunnerKind::Web,
            release_id: Some(entry.release.release.clone()),
            url: Some(entry.release.engine_url.clone()),
            origin: Some(entry.release.origin()),
            page_url: Some(entry.page_url),
            detail_floor: ZoneDetailFloor::Any,
        },
        release: Some(entry.release),
    }
}

/// The catalog built from the bundled pins. A pin that fails the website's
/// checks is left out, never listed broken.
pub fn bundled_catalog() -> ZoneCatalog {
    let zones = [
        (HAUNTED_HOUSE_ZONE, BUNDLED_HOUSE),
        (UNIVERSE_ZONE, BUNDLED_UNIVERSE),
    ]
    .into_iter()
    .filter_map(|(id, text)| kiki_zone(id, ZoneRelease::parse(text).ok()?).ok())
    .collect();
    ZoneCatalog::new(zones).unwrap_or(ZoneCatalog {
        schema: ZONE_CATALOG_SCHEMA.into(),
        zones: vec![],
    })
}

/// Launcher order: the Grid first, then the web zones in catalog order. A
/// catalog cannot replace or shadow the built-in Grid.
pub(crate) fn zones(catalog: &ZoneCatalog) -> Vec<Zone> {
    std::iter::once(grid())
        .chain(
            catalog
                .zones
                .iter()
                .filter(|z| z.id != GRID_ZONE)
                .cloned()
                .map(web),
        )
        .collect()
}

/// Parse a fetched `GET /v1/zones` body under the same bounds as the site.
pub fn parse_catalog(text: &str) -> Result<ZoneCatalog, ZoneError> {
    ZoneCatalog::decode(text).map_err(|_| invalid("The zone list could not be read."))
}
