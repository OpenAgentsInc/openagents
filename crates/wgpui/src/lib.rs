//! WGPUI - Cross-platform wgpu-based UI Framework
//!
//! A lightweight UI framework built on wgpu that runs on both web (WASM) and native.
//! Inspired by GPUI's element model but simplified for portability.
//!
//! # Example
//!
//! ```ignore
//! use wgpui::prelude::*;
//!
//! fn main_ui() -> impl Element {
//!     div()
//!         .flex()
//!         .size(Length::Percent(100.0))
//!         .bg(theme::bg::APP)
//!         .child(
//!             text("Hello, WGPUI!")
//!                 .color(theme::text::PRIMARY)
//!         )
//! }
//! ```

pub mod color;
pub mod element;
pub mod elements;
pub mod layout;
pub mod platform;
pub mod scene;
pub mod styled;
pub mod text;
pub mod theme;

// Re-export core types
pub use color::Hsla;
pub use element::{AnyElement, Element, IntoElement, LayoutContext, PaintContext, ParentElement, RenderOnce};
pub use layout::{px, pct, Bounds, LayoutEngine, LayoutId, Length, Point, Size};
pub use scene::{Quad, Scene, TextQuad};
pub use styled::{Style, Styled};
pub use text::TextSystem;
pub use theme::Theme;

// Re-export element constructors
pub use elements::{div, text, Div, Text};

// Re-export platform types
#[cfg(all(feature = "web", target_arch = "wasm32"))]
pub use platform::web::{run_app, WebPlatform};
pub use platform::{Event, Modifiers, MouseButton};

/// Prelude for convenient imports
pub mod prelude {
    pub use crate::color::Hsla;
    pub use crate::element::{AnyElement, Element, IntoElement, ParentElement, RenderOnce};
    pub use crate::elements::{div, text};
    pub use crate::layout::{px, pct, Bounds, Length, Size};
    pub use crate::styled::Styled;
    pub use crate::theme;

    #[cfg(all(feature = "web", target_arch = "wasm32"))]
    pub use crate::platform::web::run_app;
}

// Demo entry point for WASM
#[cfg(all(feature = "web", target_arch = "wasm32"))]
mod demo {
    use crate::prelude::*;
    use wasm_bindgen::prelude::*;

    /// Demo UI that shows off WGPUI capabilities
    fn demo_ui() -> impl Element {
        div()
            .flex()
            .flex_col()
            .w_full()
            .h_full()
            .bg(theme::bg::APP)
            .p(20.0)
            .gap(16.0)
            .child(
                // Header
                div()
                    .w_full()
                    .h(48.0)
                    .bg(theme::bg::SURFACE)
                    .rounded(8.0)
                    .p(12.0)
                    .child(text("WGPUI Demo").color(theme::text::PRIMARY).size(14.0)),
            )
            .child(
                // Main content card
                div()
                    .flex_1()
                    .bg(theme::bg::CARD)
                    .border(1.0)
                    .border_color(theme::border::DEFAULT)
                    .rounded(8.0)
                    .p(20.0)
                    .flex()
                    .flex_col()
                    .gap(12.0)
                    .child(
                        text("Welcome to WGPUI!")
                            .color(theme::text::PRIMARY)
                            .size(16.0),
                    )
                    .child(
                        text("A cross-platform wgpu-based UI framework")
                            .color(theme::text::SECONDARY)
                            .size(12.0),
                    )
                    .child(
                        div()
                            .flex()
                            .gap(8.0)
                            .child(
                                div()
                                    .flex()
                                    .w(100.0)
                                    .h(32.0)
                                    .bg(theme::accent::PRIMARY)
                                    .rounded(4.0)
                                    .items_center()
                                    .justify_center()
                                    .on_click(|| log::info!("Button 1 clicked!"))
                                    .child(text("Button 1").color(theme::bg::APP).size(11.0)),
                            )
                            .child(
                                div()
                                    .flex()
                                    .w(100.0)
                                    .h(32.0)
                                    .bg(theme::accent::BLUE)
                                    .rounded(4.0)
                                    .items_center()
                                    .justify_center()
                                    .on_click(|| log::info!("Button 2 clicked!"))
                                    .child(text("Button 2").color(theme::bg::APP).size(11.0)),
                            ),
                    ),
            )
    }

    #[wasm_bindgen(start)]
    pub async fn main() {
        if let Err(e) = run_app("wgpui-canvas", demo_ui).await {
            log::error!("Error: {}", e);
        }
    }
}
