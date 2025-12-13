//! Markdown parsing, syntax highlighting, and rendering.
//!
//! This module provides GPU-accelerated markdown rendering with:
//! - Full markdown support (headers, lists, code blocks, blockquotes, tables)
//! - Syntax highlighting via syntect (WASM-compatible)
//! - Streaming support for incremental rendering
//!
//! # Quick Start
//!
//! ```rust,ignore
//! use wgpui::markdown::{render_markdown, MarkdownParser, MarkdownRenderer};
//! use wgpui::{Point, Scene};
//!
//! // Simple rendering
//! let size = render_markdown(
//!     "# Hello\n\nThis is **markdown**.",
//!     Point::new(10.0, 10.0),
//!     400.0,
//!     &mut text_system,
//!     &mut scene,
//! );
//!
//! // Or with more control
//! let parser = MarkdownParser::new();
//! let renderer = MarkdownRenderer::new();
//! let doc = parser.parse("# Hello");
//! renderer.render(&doc, origin, max_width, &mut text_system, &mut scene);
//! ```
//!
//! # Streaming
//!
//! For incremental rendering (e.g., LLM responses):
//!
//! ```rust,ignore
//! use wgpui::markdown::StreamingMarkdown;
//!
//! let mut streaming = StreamingMarkdown::new();
//!
//! // In your update loop:
//! streaming.append(chunk_from_api);
//! if streaming.tick() {
//!     // Document was updated, re-render
//!     renderer.render(streaming.document(), ...);
//! }
//!
//! // When stream completes:
//! streaming.complete();
//! ```

mod highlighter;
mod parser;
mod remend;
mod renderer;
mod streaming;
mod types;

// Re-export public types
pub use highlighter::{SyntaxHighlighter, SUPPORTED_LANGUAGES};
pub use parser::MarkdownParser;
pub use renderer::{render_markdown, MarkdownRenderer};
pub use streaming::{FadeState, StreamingConfig, StreamingMarkdown};
pub use types::{
    MarkdownBlock, MarkdownConfig, MarkdownDocument, StyledLine, StyledSpan, TextStyle,
};
