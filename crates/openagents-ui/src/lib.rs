//! Server-rendered web components for openagents.com in the Apps SDK UI
//! design language. See `docs/web/apps-sdk-ui-adoption-plan.md`.
//!
//! This crate owns the design tokens (Coder Light and Coder Noir), the
//! component stylesheets under `static/`, the icon set, and typed Maud
//! builders. It does not depend on Axum; `openagents-web` serves its assets.
//!
//! # Assets
//!
//! - [`stylesheet()`]: one CSS bundle, built by `build.rs` from the token
//!   files (`static/tokens-primitive.css`, `static/tokens-semantic.css`,
//!   `static/tokens-components.css`, generated from [`tokens`]),
//!   `static/base.css`, and every `static/components/*.css` in file-name
//!   order, each wrapped in `@layer components`. Adding a component
//!   stylesheet needs no edit anywhere else. Upstream's `alpha()` and
//!   `spacing()` calls are lowered to plain CSS. [`stylesheet_version()`] is a
//!   content hash for cache busting.
//! - [`script()`]: the theme toggle (`static/theme-toggle.js`) followed by
//!   every `static/components/*.js`. Load it with `defer` **before**
//!   [`assets::ALPINE_CSP_JS`], so `alpine:init` listeners that call
//!   `Alpine.data(...)` are registered before Alpine starts.
//! - [`assets::ALPINE_CSP_JS`]: the vendored Alpine.js CSP build, pinned by
//!   SHA-256. Works under `script-src 'self'` with no `'unsafe-eval'`.
//!
//! # Themes
//!
//! Set `data-theme="light"` or `"dark"` on `<html>` (or any element, to
//! nest) from the [`shell::THEME_COOKIE`] cookie; leave it off to follow the
//! system setting. [`shell::ThemeToggle`] renders the button the toggle
//! script binds to ([`shell::THEME_TOGGLE_ATTR`]).

pub mod actions;
pub mod catalog;
pub mod content;
pub mod css_classes;
mod css_lower;
pub mod forms;
pub mod icons;
pub mod overlays;
pub mod shell;
pub mod tokens;

include!(concat!(env!("OUT_DIR"), "/bundle_meta.rs"));

/// The full stylesheet: tokens, base, and every component stylesheet.
#[must_use]
pub fn stylesheet() -> &'static str {
    include_str!(concat!(env!("OUT_DIR"), "/openagents-ui.css"))
}

/// A content hash of [`stylesheet()`], for cache-busting URLs.
#[must_use]
pub fn stylesheet_version() -> &'static str {
    STYLESHEET_VERSION
}

/// The file names of the bundled `static/components/*.css`, in bundle order.
#[must_use]
pub fn component_stylesheets() -> &'static [&'static str] {
    COMPONENT_STYLESHEETS
}

/// The page script: the theme toggle and every `static/components/*.js`.
#[must_use]
pub fn script() -> &'static str {
    include_str!(concat!(env!("OUT_DIR"), "/openagents-ui.js"))
}

/// A content hash of [`script()`], for cache-busting URLs.
#[must_use]
pub fn script_version() -> &'static str {
    SCRIPT_VERSION
}

/// Vendored and standalone assets.
pub mod assets {
    /// Script load order for every page, all `<script src defer>` from
    /// `'self'`: first [`crate::script()`] (theme toggle, then every
    /// `static/components/*.js`, such as `forms.js`, whose `alpine:init`
    /// listeners register `Alpine.data(...)` components), then
    /// [`ALPINE_CSP_JS`]. Deferred scripts run in document order, so the
    /// listeners exist before Alpine starts.
    pub const SCRIPT_LOAD_ORDER: [&str; 2] = ["openagents_ui::script()", ALPINE_CSP_FILE];

    /// The theme toggle script alone (also the start of [`crate::script()`]).
    pub const THEME_TOGGLE_JS: &str = include_str!("../static/theme-toggle.js");

    /// Alpine.js CSP build (`@alpinejs/csp`), MIT, `dist/cdn.min.js` from
    /// <https://registry.npmjs.org/@alpinejs/csp/-/csp-3.17.4.tgz>
    /// (npm integrity `sha512-SlRXmqO6kYhnxlg+99etmuzJtE9Lk4QbKjBHqerXzaMflJqoJXdz/SI3IvHJGZ/vRVyC3bR0SSBz40oY7goBeg==`).
    pub const ALPINE_CSP_JS: &str = include_str!("../static/vendor/alpine-csp-3.17.4.min.js");
    /// The pinned Alpine CSP version.
    pub const ALPINE_CSP_VERSION: &str = "3.17.4";
    /// Suggested file name when serving [`ALPINE_CSP_JS`].
    pub const ALPINE_CSP_FILE: &str = "alpine-csp-3.17.4.min.js";
    /// SHA-256 of [`ALPINE_CSP_JS`], checked by a test.
    pub const ALPINE_CSP_SHA256: &str =
        "0d18d7f8d7910e2e0212f0f056b12f50bebc3abb7d88d2f7c7cb4c336fe4519a";
}

#[cfg(test)]
mod tests;
