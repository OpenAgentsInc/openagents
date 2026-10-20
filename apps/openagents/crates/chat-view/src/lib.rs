//! The desktop app's chat views, portable to the web.
//!
//! This crate holds the parts of `openagents-ui` that draw with GPUI alone:
//! the theme, the shared typography sizes, and the GPUI adapter for Rust
//! Native views. It builds for the desktop and for `wasm32-unknown-unknown`,
//! so the desktop app and the web apps under `apps/openagents/web` draw from
//! one copy. `openagents-ui` re-exports each module under its old path and
//! keeps the platform parts (font registration, saved settings, the native
//! window) itself. `docs/audits/2026-10-19-gpui-web-chat.md` is the plan.

pub mod rust_native_gpui;
pub mod theme;
pub mod typography;
