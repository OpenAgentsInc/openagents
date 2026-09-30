//! The public pages, one module per section.

mod boards;
mod connect;
mod content;
mod desktop;
mod forum;
mod gym;
mod home;
mod profile;
mod releases;
mod traces;

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
        .merge(gym::routes())
        .merge(traces::routes())
        .merge(forum::routes())
        .merge(boards::routes())
        .merge(profile::routes())
}
