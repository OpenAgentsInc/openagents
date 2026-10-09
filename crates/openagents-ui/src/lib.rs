//! Server-rendered web components for openagents.com in the Apps SDK UI
//! design language. See `docs/web/apps-sdk-ui-adoption-plan.md`.
//!
//! This crate owns the design tokens (Coder Light and Coder Noir), the
//! component stylesheets under `static/`, the icon set, and typed Maud
//! builders. It does not depend on Axum; `openagents-web` serves its assets.
