//! The public pages, one module per section.

mod connect;
mod content;
mod desktop;
mod home;
mod profile;
mod releases;

use axum::Router;

use crate::App;

pub(crate) use connect::TESTFLIGHT;
pub(crate) use desktop::MAC_DMG;
pub(crate) use home::UNIX_COMMAND;

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .merge(home::routes())
        .merge(content::routes())
        .merge(desktop::routes())
        .merge(connect::routes())
        .merge(releases::routes())
        .merge(profile::routes())
}
