//! # coder_widgets - Widget System for Coder
//!
//! Composable UI building blocks for the Coder interface.
//! Widgets are the foundation for building complex UI surfaces.
//!
//! ## Core Concepts
//!
//! - **Widget trait**: The core abstraction for all UI components
//! - **Context**: Provides access to paint and event handling resources
//! - **Virtual scrolling**: Efficient rendering of large lists
//!
//! ## Example
//!
//! ```rust,ignore
//! use coder_widgets::{Widget, Div, Text};
//!
//! // Create a container with text
//! let widget = Div::new()
//!     .background(wgpui::theme::bg::SURFACE)
//!     .child(Text::new("Hello, Coder!"));
//! ```

pub mod context;
pub mod div;
pub mod list;
pub mod scroll;
pub mod text;
pub mod widget;

// Re-exports
pub use context::{EventContext, PaintContext};
pub use div::Div;
pub use list::{VirtualList, VirtualListItem};
pub use scroll::ScrollView;
pub use text::Text;
pub use widget::{AnyWidget, EventResult, Widget, WidgetId};
