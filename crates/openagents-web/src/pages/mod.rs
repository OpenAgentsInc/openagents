//! The public pages, one module per section.

pub(crate) mod api_docs;
mod att;
mod blue_rush;
mod bunny;
pub(crate) mod chat;
mod cloud;
mod connect;
pub(crate) mod content;
pub(crate) mod download;
mod efficiency;
mod everglade;
pub(crate) mod home;
mod live;
mod profile;
mod stats;

use axum::Router;

use crate::App;

#[cfg(test)]
pub(crate) use api_docs::API_DOCS;
#[cfg(test)]
pub(crate) use blue_rush::{ASSETS as BLUE_RUSH_ASSETS, PATH as BLUE_RUSH};
pub(crate) use bunny::{BUNNY_CANVAS, BUNNY_GLUE, BUNNY_POLICY, BUNNY_START, BUNNY_WASM};
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
pub(crate) use stats::utc;

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .merge(home::routes())
        .merge(blue_rush::routes())
        .merge(chat::routes())
        .merge(cloud::routes())
        .merge(live::routes())
        .merge(stats::routes())
        .merge(efficiency::routes())
        .merge(everglade::routes())
        .merge(bunny::routes())
        .merge(att::routes())
        .merge(content::routes())
        .merge(api_docs::routes())
        .merge(download::routes())
        .merge(connect::routes())
        .merge(profile::routes())
}
