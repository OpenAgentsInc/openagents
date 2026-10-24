//! The Aethyr UI protocol (ADR 0005).
//!
//! A controller publishes a [`View`]: a tree of semantic [`Node`]s with
//! stable IDs, a revision number, design-token references (never raw colors),
//! and accessibility roles and labels. A renderer draws it and sends back an
//! [`Intent`] naming the node, the revision it saw, and what the person did
//! (`activate`, `select`, `submit`, `dismiss`, `input`). The renderer holds no
//! state beyond focus and hover; the controller decides what happens.
//!
//! The vocabulary starts from Rust Native's view (`rust-native.view.v3`):
//! closed node kinds, bounded size and depth, and intents that carry identity
//! rather than a replacement payload. It replaces `ui.screen.v1` for new work.
//!
//! The schema is defined here once. [`codegen`] writes the JSON Schema and
//! the TypeScript types the web controller and renderer import; a test fails
//! when either checked-in file drifts.

pub mod codegen;
mod validate;
mod view;

pub use validate::{IntentError, ValidatedView, ViewError};
pub use view::*;

/// The schema name every view carries.
pub const SCHEMA: &str = "aethyr.ui.v1";
/// The most nodes one view may hold.
pub const MAX_NODES: usize = 1_024;
/// The deepest a view may nest; the root is depth 1.
pub const MAX_DEPTH: usize = 16;
/// The largest encoded view accepted from JSON.
pub const MAX_VIEW_BYTES: usize = 512 * 1024;
/// The longest node, instance, or slot ID, in bytes.
pub const MAX_ID_BYTES: usize = 128;
/// The longest single string (label, text, value), in bytes.
pub const MAX_TEXT_BYTES: usize = 16 * 1024;
/// The largest `max_bytes` a field may declare.
pub const MAX_FIELD_BYTES: usize = 64 * 1024;
/// The most options one select may offer.
pub const MAX_OPTIONS: usize = 256;
/// The largest file an upload slot may accept, in bytes.
pub const MAX_UPLOAD_BYTES: usize = 32 * 1024 * 1024;
/// The most `accept` entries one upload slot may list.
pub const MAX_ACCEPT: usize = 32;
/// The most files one `input` intent may name.
pub const MAX_FILES: usize = 64;
/// The widest or tallest a canvas may be, in CSS pixels.
pub const MAX_CANVAS_PIXELS: u32 = 4096;

#[cfg(test)]
mod tests;
