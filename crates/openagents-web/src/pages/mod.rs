//! The public pages, one module per section.

mod connect;
mod content;
mod home;
mod install;
mod profile;

use axum::Router;

use crate::App;

#[cfg(test)]
pub(crate) use connect::TESTFLIGHT;
#[cfg(test)]
pub(crate) use content::DOCS;
#[cfg(test)]
pub(crate) use install::{MAC_DMG, SOURCE, TERMINAL_PS1, TERMINAL_SH};

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .merge(home::routes())
        .merge(content::routes())
        .merge(install::routes())
        .merge(connect::routes())
        .merge(profile::routes())
}
