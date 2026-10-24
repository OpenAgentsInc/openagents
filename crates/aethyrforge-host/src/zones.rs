//! Zone releases as the website pins them, and the public catalog that lists
//! them for shells (ADR 0009, 0010).
//!
//! A [`ZoneRelease`] is the exact shape of `crates/openagents-web/content/demos/*.json`.
//! A [`ZoneCatalog`] is what `GET /v1/zones` serves and what the mobile app
//! bundles: public, read-only, and free of credentials.
use crate::*;
use serde::{Deserialize, Serialize};

pub const ZONE_CATALOG_SCHEMA: &str = "openagents.zones.v1";
pub const MAX_ZONE_CATALOG_BYTES: usize = 64 * 1024;
pub const MAX_ZONES: usize = 32;
const MAX_TITLE: usize = 80;
const MAX_SUMMARY: usize = 280;

/// One immutable web release of a zone, as the website pins it.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ZoneRelease {
    pub engine_url: String,
    pub sdk_url: String,
    pub release: String,
    pub engine_abi: String,
    pub project_format: u16,
    pub project: Project,
}
impl ZoneRelease {
    pub fn parse(text: &str) -> Result<Self, Error> {
        if text.len() > MAX_ZONE_CATALOG_BYTES {
            return Err(Error::Invalid("bytes"));
        }
        let r: Self = serde_json::from_str(text).map_err(|_| Error::Invalid("json"))?;
        r.validate()?;
        Ok(r)
    }
    /// The same checks the website applies before it embeds a release.
    pub fn validate(&self) -> Result<(), Error> {
        Host::new(
            &self.engine_url,
            "configuration".into(),
            self.release.clone(),
            self.engine_abi.clone(),
            self.project_format,
            self.project.clone(),
        )?;
        let engine = endpoint(&self.engine_url)?;
        let sdk = endpoint(&self.sdk_url)?;
        if self.release == "unconfigured"
            || engine.origin() != sdk.origin()
            || !sdk.path().ends_with("/aethyrforge_host.js")
        {
            return Err(Error::Invalid("release"));
        }
        Ok(())
    }
    /// The exact origin a runner may load and accept frames from.
    pub fn origin(&self) -> String {
        endpoint(&self.engine_url)
            .map(|u| u.origin().ascii_serialization())
            .unwrap_or_default()
    }
}

/// One entry in the public zone catalog.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ZoneEntry {
    /// Stable zone ID, such as `haunted-house` or `universe`.
    pub id: String,
    pub title: String,
    pub summary: String,
    /// The website page that hosts this zone in a browser (the fallback).
    pub page_url: String,
    pub release: ZoneRelease,
}
impl ZoneEntry {
    pub fn validate(&self) -> Result<(), Error> {
        let page = endpoint(&self.page_url)?;
        if !crate::protocol::id(&self.id)
            || self.title.trim().is_empty()
            || self.title.len() > MAX_TITLE
            || self.summary.len() > MAX_SUMMARY
            || self.title.contains('\0')
            || self.summary.contains('\0')
            || page.scheme() != "https" && !loopback(&page)
        {
            return Err(Error::Invalid("zone"));
        }
        self.release.validate()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ZoneCatalog {
    pub schema: String,
    pub zones: Vec<ZoneEntry>,
}
impl ZoneCatalog {
    pub fn new(zones: Vec<ZoneEntry>) -> Result<Self, Error> {
        let c = Self {
            schema: ZONE_CATALOG_SCHEMA.into(),
            zones,
        };
        c.validate()?;
        Ok(c)
    }
    pub fn validate(&self) -> Result<(), Error> {
        if self.schema != ZONE_CATALOG_SCHEMA {
            return Err(Error::Version);
        }
        if self.zones.len() > MAX_ZONES {
            return Err(Error::Invalid("zones"));
        }
        for (at, z) in self.zones.iter().enumerate() {
            z.validate()?;
            if self.zones[..at].iter().any(|o| o.id == z.id) {
                return Err(Error::Invalid("duplicate zone"));
            }
        }
        Ok(())
    }
    pub fn decode(text: &str) -> Result<Self, Error> {
        if text.len() > MAX_ZONE_CATALOG_BYTES {
            return Err(Error::Invalid("bytes"));
        }
        let c: Self = serde_json::from_str(text).map_err(|_| Error::Invalid("json"))?;
        c.validate()?;
        Ok(c)
    }
    pub fn encode(&self) -> Result<String, Error> {
        self.validate()?;
        let s = serde_json::to_string(self).map_err(|_| Error::Invalid("json"))?;
        if s.len() > MAX_ZONE_CATALOG_BYTES {
            return Err(Error::Invalid("bytes"));
        }
        Ok(s)
    }
}

pub const HAUNTED_HOUSE_ZONE: &str = "haunted-house";
pub const UNIVERSE_ZONE: &str = "universe";
/// The public site that hosts the Kiki zones in a browser.
pub const ZONE_SITE: &str = "https://openagents.com";

/// The catalog entry for a first Kiki zone, from the release the website pins
/// for it. The website's catalog endpoint and the mobile app's bundled copy
/// both build entries here, so their titles and fallbacks match.
pub fn kiki_zone(id: &str, release: ZoneRelease) -> Result<ZoneEntry, Error> {
    let (title, summary, page) = match id {
        HAUNTED_HOUSE_ZONE => (
            "The Haunted House",
            "Kiki's house. Walk in as a guest and look around.",
            "/new-lander/house",
        ),
        UNIVERSE_ZONE => (
            "The Verse",
            "The universe of planets. Pick one and land.",
            "/new-lander/verse",
        ),
        _ => return Err(Error::Invalid("zone")),
    };
    if release.project.id != id || release.project.branch != "main" {
        return Err(Error::Invalid("zone release"));
    }
    let entry = ZoneEntry {
        id: id.into(),
        title: title.into(),
        summary: summary.into(),
        page_url: format!("{ZONE_SITE}{page}"),
        release,
    };
    entry.validate()?;
    Ok(entry)
}

fn loopback(u: &url::Url) -> bool {
    u.host_str()
        .is_some_and(|h| h == "localhost" || h == "127.0.0.1" || h == "[::1]")
}

/// May a runner showing a zone from `origin` navigate to `url`? Only the exact
/// origin (scheme, host, and port) is allowed. Paths, queries, and fragments
/// inside that origin are the zone's own business; credentials never are.
pub fn zone_navigation_allowed(origin: &str, url: &str) -> bool {
    let Ok(u) = url::Url::parse(url) else {
        return false;
    };
    let secure = u.scheme() == "https" || (u.scheme() == "http" && loopback(&u));
    secure
        && u.username().is_empty()
        && u.password().is_none()
        && !origin.is_empty()
        && u.origin().ascii_serialization() == origin
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOUSE: &str = r#"{"engine_url":"https://house.example/","sdk_url":"https://house.example/sdk/abc/aethyrforge_host.js","release":"z1","engine_abi":"aethyrforge.browser.v1","project_format":1,"project":{"id":"haunted-house","branch":"main","revision":0}}"#;

    fn entry() -> ZoneEntry {
        ZoneEntry {
            id: "haunted-house".into(),
            title: "The Haunted House".into(),
            summary: "Kiki's house.".into(),
            page_url: "https://openagents.com/new-lander/house".into(),
            release: ZoneRelease::parse(HOUSE).unwrap(),
        }
    }

    #[test]
    fn releases_parse_like_the_website_and_reject_bad_pins() {
        let r = ZoneRelease::parse(HOUSE).unwrap();
        assert_eq!(r.origin(), "https://house.example");
        for bad in [
            HOUSE.replace("https://house.example/sdk", "https://cdn.example/sdk"),
            HOUSE.replace("\"z1\"", "\"unconfigured\""),
            HOUSE.replace("https://house.example/\"", "http://house.example/\""),
            HOUSE.replace("aethyrforge_host.js", "other.js"),
            HOUSE.replace("}}", "},\"extra\":1}"),
        ] {
            assert!(ZoneRelease::parse(&bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn the_catalog_round_trips_and_stays_bounded() {
        let c = ZoneCatalog::new(vec![entry()]).unwrap();
        let text = c.encode().unwrap();
        assert_eq!(ZoneCatalog::decode(&text).unwrap(), c);
        assert!(ZoneCatalog::new(vec![entry(), entry()]).is_err());
        let mut bad = entry();
        bad.page_url = "http://openagents.com/x".into();
        assert!(ZoneCatalog::new(vec![bad]).is_err());
        let mut bad = entry();
        bad.id = "has space".into();
        assert!(ZoneCatalog::new(vec![bad]).is_err());
        assert_eq!(
            ZoneCatalog::decode(&text.replace(ZONE_CATALOG_SCHEMA, "openagents.zones.v0")),
            Err(Error::Version)
        );
        assert!(ZoneCatalog::decode(&" ".repeat(MAX_ZONE_CATALOG_BYTES + 1)).is_err());
    }

    #[test]
    fn kiki_zones_bind_their_project_and_point_at_the_site() {
        let r = ZoneRelease::parse(HOUSE).unwrap();
        let e = kiki_zone(HAUNTED_HOUSE_ZONE, r.clone()).unwrap();
        assert_eq!(e.page_url, "https://openagents.com/new-lander/house");
        assert_eq!(e.title, "The Haunted House");
        // The house release cannot be listed as the universe, or as anything else.
        assert!(kiki_zone(UNIVERSE_ZONE, r.clone()).is_err());
        assert!(kiki_zone("grid", r).is_err());
    }

    #[test]
    fn navigation_is_exact_origin_only() {
        let o = "https://house.example";
        assert!(zone_navigation_allowed(o, "https://house.example/"));
        assert!(zone_navigation_allowed(
            o,
            "https://house.example/a/b?x=1#y"
        ));
        assert!(zone_navigation_allowed(o, "https://house.example:443/"));
        for bad in [
            "https://house.example.evil.example/",
            "https://evil.example/?https://house.example",
            "https://house.example:8443/",
            "http://house.example/",
            "https://user:pw@house.example/",
            "https://sub.house.example/",
            "javascript:alert(1)",
            "data:text/html,hi",
            "about:blank",
            "",
        ] {
            assert!(!zone_navigation_allowed(o, bad), "{bad}");
        }
        assert!(!zone_navigation_allowed("", "https://house.example/"));
    }
}
