//! Advanced text layout system for WGPUI.
//!
//! This module provides comprehensive text layout capabilities including:
//! - Line wrapping with word boundary detection
//! - Font runs for mixed-font text
//! - Text decorations (underline, strikethrough, background)
//! - Layout caching for performance
//! - Position/index queries for text interaction
//!
//! # Architecture
//!
//! The text system is built around these key types:
//!
//! - [`LineLayout`] - A shaped line of text with positioned glyphs
//! - [`WrappedLineLayout`] - A line layout with wrap boundaries
//! - [`LineWrapper`] - Wraps text to a given width
//! - [`ShapedLine`] / [`WrappedLine`] - Paintable lines with decorations
//! - [`LineLayoutCache`] - Caches layouts across frames
//!
//! # Example
//!
//! ```ignore
//! use wgpui::text_system::{LineWrapper, LineLayout, TextAlign};
//!
//! // Wrap text to 200 pixels
//! let mut wrapper = LineWrapper::new(font_id, font_size, text_system);
//! let boundaries = wrapper.wrap_line(&[LineFragment::text("Hello world")], 200.0);
//!
//! // Layout and paint
//! let layout = text_system.layout_line("Hello world", font_size, &runs);
//! let shaped = ShapedLine::new(layout, "Hello world", decorations);
//! shaped.paint(origin, line_height, TextAlign::Left, cx);
//! ```

mod cache;
mod line;
mod line_layout;
mod line_wrapper;

pub use cache::*;
pub use line::*;
pub use line_layout::*;
pub use line_wrapper::*;
