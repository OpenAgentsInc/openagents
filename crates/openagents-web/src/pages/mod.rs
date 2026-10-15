//! The public pages, one module per section.

pub(crate) mod api_docs;
mod att;
mod blue_rush;
mod bunny;
pub(crate) mod cad;
pub(crate) mod chat;
mod cloud;
pub(crate) mod connect;
pub(crate) mod content;
pub(crate) mod download;
pub(crate) mod efficiency;
mod everglade;
pub(crate) mod home;
mod icecream;
mod icecream_guide;
mod landing;
pub(crate) mod live;
mod pricing;
pub(crate) mod profile;
mod snap;
mod snap_shares;
pub(crate) mod stats;

use axum::Router;

use crate::App;

#[cfg(test)]
pub(crate) use api_docs::API_DOCS;
#[cfg(test)]
pub(crate) use blue_rush::{
    ASSETS as BLUE_RUSH_ASSETS, GUIDE as BLUE_RUSH_GUIDE,
    ICECREAM_GUIDE as BLUE_RUSH_ICECREAM_GUIDE, PATH as BLUE_RUSH,
    SNAP_GUIDE as BLUE_RUSH_SNAP_GUIDE,
};
pub(crate) use blue_rush::{HOSTS as STUDIO_HOSTS, STUDIO_HEADER, studio_path};
pub(crate) use bunny::{BUNNY_CANVAS, BUNNY_GLUE, BUNNY_POLICY, BUNNY_START, BUNNY_WASM};
#[cfg(test)]
pub(crate) use cad::{CAD_FILES, CAD_PATH, CAD_POLICY};
#[cfg(test)]
pub(crate) use connect::TESTFLIGHT;
#[cfg(test)]
pub(crate) use content::{DOCS, section_anchor, section_of};
#[cfg(test)]
pub(crate) use download::{
    CODER_BASE, CODER_PLATFORMS, CODER_PS1, CODER_SH, CODER_VERSION, DESKTOP_RELEASED, Part,
    TESTFLIGHT_APP, coder_archive, published_as_archives, shown,
};
#[cfg(test)]
pub(crate) use everglade::GRID_POLICY;
pub(crate) use everglade::{CANVAS_ID, EVERGLADE_POLICY, GLUE, WASM};
#[cfg(test)]
pub(crate) use icecream::{
    ICECREAM_CANVAS, ICECREAM_GLUE, ICECREAM_PATH, ICECREAM_POLICY, ICECREAM_START,
    ICECREAM_VERSION_FILE, ICECREAM_WASM,
};
#[cfg(test)]
pub(crate) use icecream_guide::ICECREAM_GUIDE_PATH;
pub(crate) use pricing::PRICING;
#[cfg(test)]
pub(crate) use snap::{SNAP_CANVAS, SNAP_GLUE, SNAP_PATH, SNAP_START, SNAP_WASM};
#[cfg(test)]
pub(crate) use stats::utc;

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .merge(home::routes())
        .merge(landing::routes())
        .merge(blue_rush::routes())
        .merge(chat::routes())
        .merge(cloud::routes())
        .merge(live::routes())
        .merge(stats::routes())
        .merge(efficiency::routes())
        .merge(everglade::routes())
        .merge(bunny::routes())
        .merge(icecream::routes())
        .merge(snap::routes())
        .merge(att::routes())
        .merge(cad::routes())
        .merge(content::routes())
        .merge(api_docs::routes())
        .merge(download::routes())
        .merge(connect::routes())
        .merge(profile::routes())
        .merge(pricing::routes())
}
