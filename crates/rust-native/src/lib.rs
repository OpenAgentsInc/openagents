//! Rust Native's experimental renderer-independent foundation.
//!
//! Applications describe semantic views and typed intents. Platform adapters
//! own native controls, focus, input composition, and mounting. The `layout`
//! module lays out transcripts on an adapter's behalf: exact row heights and
//! display lists the adapter paints. This crate does not start an application
//! runtime, execute an intent, or render a native screen. See the crate's
//! docs for implemented and planned layers.

pub mod edit;
pub mod input;
pub mod layout;
pub mod markdown;
pub mod selection;
pub mod style;
pub mod surface;
pub mod syntax;
pub mod view;

pub use input::{InputError, InputRequest};
pub use view::{
    Activation, Axis, ComposerChoice, Earlier, Element, Glyph, Icon, MAX_COMPOSER_CHOICES,
    MessageRole, Node, TextRole, ToolState, ValidatedView, View, ViewError,
};

pub(crate) fn valid_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 96
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.' | b':'))
}
