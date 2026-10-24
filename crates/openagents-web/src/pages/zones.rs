//! `GET /v1/zones`: the public zone catalog shells open (ADR 0009, 0010).
//!
//! It lists the same pinned web releases the Kiki pages embed, so the website
//! and the mobile app's web runner load identical bytes. Read-only, no
//! sign-in, no credentials: guest zones are public.
use super::medieval_town::Release;
use crate::App;
use aethyrforge_host::{
    HAUNTED_HOUSE_ZONE, UNIVERSE_ZONE, ZONE_CATALOG_SCHEMA, ZoneCatalog, ZoneEntry, ZoneRelease,
    kiki_zone,
};
use axum::{
    Router,
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};

pub(crate) const PATH: &str = "/v1/zones";

pub(crate) fn routes() -> Router<App> {
    Router::new().route(PATH, get(|| async { catalog_response() }))
}

fn entry(id: &str, r: Release) -> Option<ZoneEntry> {
    let release: ZoneRelease = serde_json::from_value(serde_json::to_value(r).ok()?).ok()?;
    kiki_zone(id, release).ok()
}

/// The configured releases, in launcher order. A zone whose pin is missing or
/// invalid is left out rather than listed broken.
pub(crate) fn catalog() -> ZoneCatalog {
    let zones = [
        super::new_lander_house::release().and_then(|r| entry(HAUNTED_HOUSE_ZONE, r)),
        super::new_lander::universe_release().and_then(|r| entry(UNIVERSE_ZONE, r)),
    ];
    ZoneCatalog::new(zones.into_iter().flatten().collect()).unwrap_or(ZoneCatalog {
        schema: ZONE_CATALOG_SCHEMA.into(),
        zones: vec![],
    })
}

fn catalog_response() -> Response {
    match catalog().encode() {
        Ok(body) => {
            let mut response = (StatusCode::OK, body).into_response();
            let headers = response.headers_mut();
            headers.insert(
                header::CONTENT_TYPE,
                HeaderValue::from_static("application/json"),
            );
            headers.insert(
                header::CACHE_CONTROL,
                HeaderValue::from_static("public, max-age=300"),
            );
            response
        }
        Err(_) => StatusCode::SERVICE_UNAVAILABLE.into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_catalog_lists_the_pinned_house_and_universe() {
        let c = catalog();
        let ids: Vec<_> = c.zones.iter().map(|z| z.id.as_str()).collect();
        assert_eq!(ids, [HAUNTED_HOUSE_ZONE, UNIVERSE_ZONE]);
        let house: serde_json::Value =
            serde_json::from_str(include_str!("../../content/demos/haunted-house.json")).unwrap();
        assert_eq!(c.zones[0].release.engine_url, house["engine_url"]);
        assert_eq!(c.zones[0].release.release, house["release"]);
        assert_eq!(
            c.zones[0].page_url,
            format!(
                "https://openagents.com{}",
                super::super::new_lander_house::HOUSE
            )
        );
        let text = c.encode().unwrap();
        assert_eq!(ZoneCatalog::decode(&text).unwrap(), c);
    }

    #[test]
    fn the_endpoint_is_public_json_this_site_answers() {
        let response = catalog_response();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[header::CONTENT_TYPE], "application/json");
        assert!(crate::upstream::owned(PATH));
    }
}
