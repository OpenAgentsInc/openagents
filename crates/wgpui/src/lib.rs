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
pub mod layout;
pub mod platform;
pub mod renderer;
pub mod scene;
pub mod text;
pub mod theme;

// Re-exports for convenience
pub use color::Hsla;
pub use geometry::{Bounds, CornerRadii, Edges, Point, Size};
pub use layout::{auto, length, length_auto, pct, px, relative, zero, LayoutEngine, LayoutId, LayoutStyle};
pub use scene::{GlyphInstance, GpuQuad, GpuTextQuad, Quad, Scene, TextRun};
pub use text::TextSystem;

#[cfg(all(feature = "web", target_arch = "wasm32"))]
pub use platform::web::{run_animation_loop, setup_resize_observer, WebPlatform};

// WASM entry point for demo
#[cfg(all(feature = "web", target_arch = "wasm32"))]
use wasm_bindgen::prelude::*;

#[cfg(all(feature = "web", target_arch = "wasm32"))]
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

    // Set up resize handler
    {
        let platform_clone = platform.clone();
        let canvas = platform.borrow().canvas().clone();
        setup_resize_observer(&canvas, move || {
            platform_clone.borrow_mut().handle_resize();
        });
    }

    // Animation loop
    let platform_clone = platform.clone();
    let frame_count_clone = frame_count.clone();
    run_animation_loop(move || {
        let mut platform = platform_clone.borrow_mut();
        let frame = *frame_count_clone.borrow();
        *frame_count_clone.borrow_mut() = frame.wrapping_add(1);

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

        // Animated pulse for accent color
        let pulse = ((frame as f32 * 0.05).sin() * 0.5 + 0.5) * 0.3 + 0.7;
        let accent_pulsed = theme::accent::PRIMARY.with_alpha(pulse);

        // Centered card
        let card_width = 400.0f32.min(size.width - 40.0);
        let card_height = 300.0;
        let card_x = (size.width - card_width) / 2.0;
        let card_y = 80.0;

        scene.draw_quad(
            Quad::new(Bounds::new(card_x, card_y, card_width, card_height))
                .with_background(theme::bg::SURFACE)
                .with_border(theme::border::DEFAULT, 1.0)
                .with_uniform_radius(8.0),
        );

        // Title text
        let title_run = platform
            .text_system()
            .layout("wgpui Demo", Point::new(card_x + 20.0, card_y + 24.0), 16.0, theme::accent::PRIMARY);
        scene.draw_text(title_run);

        // Subtitle text
        let subtitle_run = platform.text_system().layout(
            "GPU-accelerated UI rendering",
            Point::new(card_x + 20.0, card_y + 50.0),
            13.0,
            theme::text::SECONDARY,
        );
        scene.draw_text(subtitle_run);

        // Animated button
        let button_y = card_y + 90.0;
        scene.draw_quad(
            Quad::new(Bounds::new(card_x + 20.0, button_y, 160.0, 36.0))
                .with_background(accent_pulsed)
                .with_uniform_radius(4.0),
        );

        let button_text = platform.text_system().layout(
            "Get Started",
            Point::new(card_x + 55.0, button_y + 10.0),
            13.0,
            theme::bg::APP,
        );
        scene.draw_text(button_text);

        // Color boxes row
        let colors = [
            theme::status::SUCCESS,
            theme::status::ERROR,
            theme::accent::BLUE,
            theme::accent::PURPLE,
        ];
        let box_size = 40.0;
        let box_gap = 12.0;
        let boxes_width = colors.len() as f32 * box_size + (colors.len() - 1) as f32 * box_gap;
        let boxes_x = card_x + (card_width - boxes_width) / 2.0;
        let boxes_y = card_y + 150.0;

        for (i, color) in colors.iter().enumerate() {
            let offset = (frame as f32 * 0.03 + i as f32 * 0.5).sin() * 5.0;
            let x = boxes_x + i as f32 * (box_size + box_gap);
            let y = boxes_y + offset;

            scene.draw_quad(
                Quad::new(Bounds::new(x, y, box_size, box_size))
                    .with_background(*color)
                    .with_uniform_radius(6.0),
            );
        }

        // Info text
        let info_text = platform.text_system().layout(
            "wgpu + cosmic-text + taffy",
            Point::new(card_x + 20.0, card_y + 220.0),
            12.0,
            theme::text::MUTED,
        );
        scene.draw_text(info_text);

        // Border demo box
        scene.draw_quad(
            Quad::new(Bounds::new(card_x + 20.0, card_y + 250.0, card_width - 40.0, 30.0))
                .with_border(theme::accent::PRIMARY, 2.0)
                .with_uniform_radius(4.0),
        );

        let border_text = platform.text_system().layout(
            "SDF rounded borders",
            Point::new(card_x + 100.0, card_y + 258.0),
            12.0,
            theme::text::PRIMARY,
        );
        scene.draw_text(border_text);

        // Render
        if let Err(e) = platform.render(&scene) {
            log::error!("Render error: {}", e);
        }
    });

    Ok(())
}
