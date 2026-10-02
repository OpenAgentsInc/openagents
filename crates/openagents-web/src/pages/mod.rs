//! The public pages, one module per section.

mod connect;
mod content;
mod download;
mod home;
mod live;
mod profile;

use axum::Router;

use crate::App;

#[cfg(test)]
pub(crate) use connect::TESTFLIGHT;
#[cfg(test)]
pub(crate) use content::DOCS;
#[cfg(test)]
pub(crate) use download::{MAC_DMG, SOURCE, TERMINAL_PS1, TERMINAL_SH};

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .merge(home::routes())
        .merge(live::routes())
        .merge(content::routes())
        .merge(download::routes())
        .merge(connect::routes())
        .merge(profile::routes())
}
