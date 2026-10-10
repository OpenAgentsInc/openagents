//! The vault, tier "only you" (#11240, `nips/openagents/NIP-VAULT.md`).
//!
//! Files are locked in the person's browser (or Coder, or the apps) before
//! they reach this server. This server keeps what NIP-VAULT's service API
//! names: key slots, the sealed key index, and sealed objects. It never
//! receives a vault master key, a slot's method secret, a data key or a
//! file's plaintext, with one labelled exception: a **Fast** answer
//! (`POST /vault/api/answer`), where the person's browser sends one turn's
//! decrypted files to be passed to Google Gemini. That route stores nothing
//! and logs no content.
//!
//! | Route | What |
//! | --- | --- |
//! | `GET /settings/vault`, `GET /projects/{id}/vault` | The vault page ([`page`]) |
//! | `GET /vault/vault.js`, `/vault/vault.css`, `/vault/assets/{file}` | The page's script, styles and the WebAssembly build of `oa-vault` |
//! | `GET /vault/release.json` | The digests of those files, for checking what the page runs |
//! | `/vault/api/...` | The NIP-VAULT service API |
//!
//! Storage: under the account's folder in [`crate::Config::vault_store`]
//! (production: a bucket with no soft delete and no versions):
//! `vault/vault.json` (the vault id, its slots and the current epoch),
//! `vault/index/{epoch}` (the sealed index) and `vault/objects/{id}`.

mod api;
mod page;

use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::routing::{get, post};

use crate::App;
use crate::chat_store::Store;

pub(crate) use page::settings_row;

/// The vault page in Settings.
pub(crate) const PAGE: &str = "/settings/vault";
/// The page's script.
pub(crate) const SCRIPT: &str = "/vault/vault.js";
/// The page's styles.
pub(crate) const STYLE: &str = "/vault/vault.css";
/// The digests of what the page runs.
pub(crate) const RELEASE: &str = "/vault/release.json";
const STATE: &str = "/vault/api/state";
const CREATE: &str = "/vault/api/create";
const SLOTS: &str = "/vault/api/slots";
const SLOT_DELETE: &str = "/vault/api/slots/{slot}/delete";
const OBJECT: &str = "/vault/api/objects/{object}";
const INDEX: &str = "/vault/api/index";
const DELETE: &str = "/vault/api/delete";
const ANSWER: &str = "/vault/api/answer";
const ASSET: &str = "/vault/assets/{file}";
const PROJECT_PAGE: &str = "/projects/{id}/vault";

/// The largest stored object: a 10 MB file and its tags and header.
pub(crate) const MAX_OBJECT_BYTES: usize = 11 * 1024 * 1024;
/// The most objects (files and answers) one vault holds.
pub(crate) const MAX_OBJECTS: usize = 250;
/// The most key slots.
pub(crate) const MAX_SLOTS: usize = 16;
/// The largest sealed index.
pub(crate) const MAX_INDEX_BYTES: usize = 2 * 1024 * 1024;

/// A project's vault page.
pub(crate) fn project_href(project: &str) -> String {
    format!("/projects/{project}/vault")
}

/// Whether this site answers `path` (never the previous server).
pub(crate) fn owns(path: &str) -> bool {
    path == PAGE
        || path.starts_with("/vault/")
        || path
            .strip_prefix("/projects/")
            .and_then(|rest| rest.strip_suffix("/vault"))
            .is_some_and(|id| !id.is_empty() && !id.contains('/'))
}

/// Where vault data is kept.
pub(crate) fn store(app: &App) -> &Store {
    app.config
        .vault_store
        .as_deref()
        .unwrap_or(&app.config.chat_store)
}

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route(PAGE, get(page::settings_page))
        .route(PROJECT_PAGE, get(page::project_page))
        .route(SCRIPT, get(page::script))
        .route(STYLE, get(page::style))
        .route(ASSET, get(page::asset))
        .route(RELEASE, get(page::release))
        .route(STATE, get(api::state))
        .route(CREATE, post(api::create))
        .route(SLOTS, post(api::add_slot))
        .route(SLOT_DELETE, post(api::delete_slot))
        .route(
            OBJECT,
            get(api::read_object)
                .put(api::write_object)
                .layer(DefaultBodyLimit::max(MAX_OBJECT_BYTES + 1024)),
        )
        .route(
            INDEX,
            post(api::write_index).layer(DefaultBodyLimit::max(MAX_INDEX_BYTES * 2)),
        )
        .route(DELETE, post(api::delete_vault))
        .route(
            ANSWER,
            post(api::answer).layer(DefaultBodyLimit::max(api::MAX_ANSWER_BODY)),
        )
}

#[cfg(test)]
mod tests;
