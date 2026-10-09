//! The public pages, one module per section.

mod blue_rush;
pub(crate) mod chat;
mod cloud;
mod connect;
mod content;
mod download;
mod efficiency;
mod everglade;
mod home;
mod live;
mod profile;
mod stats;

use axum::Router;

use crate::App;

#[cfg(test)]
pub(crate) use blue_rush::{ASSETS as BLUE_RUSH_ASSETS, PATH as BLUE_RUSH};
#[cfg(test)]
pub(crate) use connect::TESTFLIGHT;
#[cfg(test)]
pub(crate) use content::DOCS;
#[cfg(test)]
pub(crate) use download::{CODER_BASE, CODER_PLATFORMS, CODER_PS1, CODER_SH, CODER_VERSION};
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
        .merge(content::routes())
        .merge(download::routes())
        .merge(connect::routes())
        .merge(profile::routes())
}
