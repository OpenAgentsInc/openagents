//! The desktop app's chat views, portable to the web.
//!
//! This crate holds the parts of `openagents-ui` that draw with GPUI alone:
//! the theme, the shared typography sizes, the motion catalog and loaders,
//! the icons, popovers and menus, the markdown renderer with its link and
//! workspace-file handling, and the GPUI adapter for Rust Native views. It
//! builds for the desktop and for `wasm32-unknown-unknown`, so the desktop
//! app and the web apps under `apps/openagents/web` draw from one copy.
//! `openagents-ui` re-exports each module under its old path and keeps the
//! platform parts (font registration, saved settings, the native window)
//! itself. The host fills two seams: [`prefs`] for the preferences a view
//! changes, and on the web [`icons::set_web_painter`] for icons.
//! `docs/audits/2026-10-19-gpui-web-chat.md` is the plan.

pub mod appshots;
pub mod attachments;
pub mod badges;
pub mod comments;
pub mod composer_chips;
pub mod composer_markdown;
pub mod composer_mentions;
pub mod composer_tokens;
pub mod composer_wizard;
pub mod diff;
pub mod edge_fade;
pub mod file_icons;
pub mod frost;
pub mod icons;
pub mod loaders;
pub mod markdown;
pub mod motion;
pub mod notice;
pub mod overscroll;
pub mod popover;
pub mod prefs;
pub mod rail;
pub mod roll_text;
pub mod rpc;
pub mod rust_native_gpui;
pub mod settings_forms;
pub mod syntax_cache;
pub mod theme;
pub mod tool_image_paths;
pub mod tool_progress;
pub mod tooltip;
pub mod transcript;
pub mod typography;
pub mod web_address;
pub mod workspace_links;

#[cfg(feature = "visuals")]
pub mod visuals;
