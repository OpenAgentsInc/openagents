//! Rust Native's experimental renderer-independent foundation.
//!
//! Applications describe semantic views and typed intents. Platform adapters
//! own native controls, layout, focus, input composition, and mounting. This
//! crate does not start an application runtime, execute an intent, or render
//! a native screen. See the crate's docs for implemented and planned layers.

pub mod input;
pub mod markdown;
pub mod style;
pub mod surface;
pub mod view;

pub use input::{InputError, InputRequest};
pub use view::{
    Activation, Axis, Earlier, Element, MessageRole, Node, TextRole, ToolState, ValidatedView,
    View, ViewError,
};

pub(crate) fn valid_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 96
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.' | b':'))
}
