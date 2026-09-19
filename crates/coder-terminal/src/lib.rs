//! The Coder terminal: one amber, four intensities, and the composer.
//!
//! The crate is built around a small, renderer-agnostic core:
//!
//! - [`Intensity`] is a four-step brightness scale over a single amber hue.
//!   Tone — faintest to brightest — carries every distinction the UI needs;
//!   hue does not vary.
//! - [`Ladder`] maps an [`Intensity`] to a concrete color for the terminal at
//!   hand: exact RGB when truecolor is available, the nearest cube entry under
//!   a 256-color palette, and dim text when color is off.
//! - [`frame`] and [`rail`] draw the hairline box and the labels set into
//!   its rules.
//! - [`Editor`] is the composer's multi-line editing model: caret, grapheme-
//!   correct motions, word deletes, soft-wrap, a scroll window, and prompt
//!   history.
//! - [`Composer`] draws the framed input box into a ratatui buffer, and
//!   [`handle_key`] maps terminal key events onto the editor.

mod composer;
mod editor;
pub mod hairline;
mod intensity;
mod keys;
mod ladder;
pub mod markdown;
mod spinner;
mod wrap;

pub use composer::{CARET, Composer, GUTTER, PROMPT};
pub use editor::{Editor, ROWS_MAX, ROWS_MIN, Window};
pub use hairline::{frame, rail};
pub use intensity::{Intensity, NEAR_BLACK, NEAR_BLACK_TINT};
pub use keys::{ComposerAction, handle_key};
pub use ladder::{Colorless, Colors, Ladder, drain_color, rgb};
pub use markdown::{Marked, Marks, Rendered};
pub use spinner::{
    CYCLE, FRAMES, FRAMES_ASCII, SPINNER_COUNT, SPINNER_FRAME, frame_at, frame_for,
    frame_for_ascii, spinner,
};
pub use wrap::{byte_at_column, row_of, wrap_rows};
