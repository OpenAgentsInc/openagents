//! # wgpui - GPU-Accelerated UI Rendering Library
//!
//! A cross-platform GPU-accelerated UI rendering library built on wgpu.
//! Designed for high-performance canvas rendering with text, quads, and
//! SDF-based rounded corners/borders.
//!
//! ## Features
//!
//! - **GPU Rendering**: Hardware-accelerated rendering via wgpu (WebGPU/WebGL/Vulkan/Metal/DX12)
//! - **Text Rendering**: High-quality text with cosmic-text shaping and glyph atlas
//! - **SDF Primitives**: Smooth rounded corners and borders using signed distance fields
//! - **Layout Engine**: CSS Flexbox layout via Taffy
//! - **Theme System**: Bloomberg-inspired dark theme out of the box
//!
//! ## Quick Start
//!
//! ```rust,ignore
//! use wgpui::{Scene, Quad, Bounds, theme};
//!
//! // Create a scene
//! let mut scene = Scene::new();
//!
//! // Draw a rounded rectangle
//! scene.draw_quad(Quad::new(Bounds::new(10.0, 10.0, 200.0, 100.0))
//!     .with_background(theme::bg::SURFACE)
//!     .with_uniform_radius(8.0));
//! ```
//!
//! ## Architecture
//!
//! - `scene` - Accumulated draw primitives (Quad, TextRun)
//! - `renderer` - GPU rendering pipeline
//! - `text` - Text shaping and glyph atlas
//! - `layout` - Taffy-based flexbox layout
//! - `platform` - Platform abstraction (web, native)
//! - `theme` - Color and style tokens

// Public modules
pub mod color;
pub mod geometry;
pub mod hit_test;
pub mod input;
pub mod layout;
pub mod markdown;
pub mod platform;
pub mod renderer;
pub mod scene;
pub mod scroll;
pub mod text;
pub mod theme;

// Re-exports for convenience
pub use color::Hsla;
pub use geometry::{Bounds, CornerRadii, Edges, Point, Size};
pub use hit_test::{Hit, HitTestEntry, HitTestIndex, NodeId};
pub use input::{Cursor, InputEvent, Key, KeyCode, Modifiers, MouseButton, NamedKey};
pub use layout::{auto, length, length_auto, pct, px, relative, zero, LayoutEngine, LayoutId, LayoutStyle};
pub use platform::Platform;
pub use scene::{GlyphInstance, GpuQuad, GpuTextQuad, Quad, Scene, TextRun};
pub use scroll::{ScrollContainer, ScrollDirection};
pub use text::{FontStyle, TextSystem};

#[cfg(all(feature = "web", target_arch = "wasm32"))]
pub use platform::web::{run_animation_loop, setup_resize_observer, WebPlatform};

// WASM entry point for demo (only when demo feature is enabled)
#[cfg(all(feature = "web", target_arch = "wasm32"))]
use wasm_bindgen::prelude::*;

#[cfg(all(feature = "web", target_arch = "wasm32"))]
#[allow(dead_code)]
#[wasm_bindgen(start)]
pub async fn main() -> Result<(), JsValue> {
    console_error_panic_hook::set_once();

    // Initialize platform
    let platform = WebPlatform::init("wgpui-canvas")
        .await
        .map_err(|e| JsValue::from_str(&e))?;

    // Hide loading indicator
    if let Some(window) = web_sys::window() {
        if let Some(document) = window.document() {
            if let Some(loading) = document.get_element_by_id("loading") {
                loading
                    .dyn_ref::<web_sys::HtmlElement>()
                    .map(|el| el.style().set_property("display", "none"));
            }
        }
    }

    // Set up state
    use std::cell::RefCell;
    use std::rc::Rc;

    let platform = Rc::new(RefCell::new(platform));
    let frame_count = Rc::new(RefCell::new(0u64));

    // Demo markdown content
    const DEMO_MARKDOWN: &str = r#"# BUILD v5

This is a **GPU-accelerated** markdown renderer with *streaming* support.

## Features

- Syntax highlighting via syntect
- Streaming text support
- Full markdown rendering

## Code Example

```rust
fn main() {
    let greeting = "Hello, wgpui!";
    println!("{}", greeting);
}
```

> Blockquotes are styled with a yellow accent bar

---

### Inline Styles

You can use `inline code`, **bold**, *italic*, and ~~strikethrough~~.

1. Ordered lists
2. Work great
3. With numbers
"#;

    // Set up streaming markdown with fade-in enabled
    let streaming_config = markdown::StreamingConfig {
        fade_in_frames: Some(15), // Fade in over ~250ms at 60fps
        ..Default::default()
    };
    let streaming = Rc::new(RefCell::new(markdown::StreamingMarkdown::with_config(streaming_config)));
    let char_index = Rc::new(RefCell::new(0usize));
    let demo_text = DEMO_MARKDOWN.to_string();

    // Set up resize handler
    {
        let platform_clone = platform.clone();
        let canvas = platform.borrow().canvas().clone();
        setup_resize_observer(&canvas, move || {
            // Use try_borrow_mut to avoid panic if animation loop has it borrowed
            if let Ok(mut p) = platform_clone.try_borrow_mut() {
                p.handle_resize();
            }
        });
    }

    // Animation loop
    let platform_clone = platform.clone();
    let frame_count_clone = frame_count.clone();
    let streaming_clone = streaming.clone();
    let char_index_clone = char_index.clone();

    run_animation_loop(move || {
        let mut platform = platform_clone.borrow_mut();
        let frame = *frame_count_clone.borrow();
        *frame_count_clone.borrow_mut() = frame.wrapping_add(1);

        // Simulate streaming: append characters over time
        {
            let mut sm = streaming_clone.borrow_mut();
            let mut idx = char_index_clone.borrow_mut();

            // Append characters (faster at start, then slow down)
            let chars_per_frame = if *idx < 100 { 5 } else { 2 };

            if *idx < demo_text.len() {
                let end = (*idx + chars_per_frame).min(demo_text.len());
                sm.append(&demo_text[*idx..end]);
                *idx = end;
            } else if !sm.document().is_complete {
                sm.complete();
            }

            sm.tick();
        }

        // Build scene
        let mut scene = Scene::new();
        let size = platform.logical_size();

        // Background
        scene.draw_quad(
            Quad::new(Bounds::new(0.0, 0.0, size.width, size.height))
                .with_background(theme::bg::APP),
        );

        // Header bar
        scene.draw_quad(
            Quad::new(Bounds::new(0.0, 0.0, size.width, 48.0))
                .with_background(theme::bg::SURFACE)
                .with_border(theme::border::DEFAULT, 1.0),
        );

        // Header title
        let title_run = platform.text_system().layout(
            "wgpui Markdown Demo",
            Point::new(16.0, 16.0),
            14.0,
            theme::accent::PRIMARY,
        );
        scene.draw_text(title_run);

        // Streaming status
        let streaming_ref = streaming_clone.borrow();
        let status_text = if streaming_ref.document().is_complete {
            "Complete"
        } else {
            "Streaming..."
        };
        let status_color = if streaming_ref.document().is_complete {
            theme::status::SUCCESS
        } else {
            theme::accent::PRIMARY
        };
        let status_run = platform.text_system().layout(
            status_text,
            Point::new(size.width - 140.0, 16.0),
            12.0,
            status_color,
        );
        scene.draw_text(status_run);

        // Content area
        let content_x = 20.0;
        let content_y = 64.0;
        let content_width = (size.width - 40.0).min(700.0);

        // Render markdown with fade-in effect
        let renderer = markdown::MarkdownRenderer::new();
        let fade = streaming_ref.fade_state();
        renderer.render_with_opacity(
            streaming_ref.document(),
            Point::new(content_x, content_y),
            content_width,
            platform.text_system(),
            &mut scene,
            fade.new_content_opacity,
        );

        drop(streaming_ref);

        // Render
        if let Err(e) = platform.render(&scene) {
            log::error!("Render error: {}", e);
        }
    });

    Ok(())
}
