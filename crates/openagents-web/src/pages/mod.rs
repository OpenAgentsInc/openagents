//! The public pages, one module per section.

mod connect;
mod content;
mod download;
mod efficiency;
mod home;
mod live;
mod profile;
mod stats;

use axum::Router;

use crate::App;

#[cfg(test)]
pub(crate) use connect::TESTFLIGHT;
#[cfg(test)]
pub(crate) use content::DOCS;
#[cfg(test)]
pub(crate) use download::{MAC_DMG, SOURCE, TERMINAL_PS1, TERMINAL_SH};
#[cfg(test)]
pub(crate) use stats::utc;

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .merge(home::routes())
        .merge(live::routes())
        .merge(stats::routes())
        .merge(efficiency::routes())
        .merge(content::routes())
        .merge(download::routes())
        .merge(connect::routes())
        .merge(profile::routes())
}
