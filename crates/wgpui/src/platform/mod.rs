//! Platform abstraction layer.
//!
//! This module provides a platform-agnostic interface for window management,
//! rendering, and input handling.

use crate::geometry::Size;
use crate::input::{Cursor, InputEvent};
use crate::scene::Scene;
use crate::text::TextSystem;

/// Platform-agnostic interface for window and rendering management.
///
/// This trait abstracts over different platform implementations (web, desktop, mobile)
/// to provide a unified interface for wgpui rendering.
pub trait Platform {
    /// Get the logical size of the rendering surface.
    fn logical_size(&self) -> Size;

    /// Get the device pixel ratio (scale factor).
    fn scale_factor(&self) -> f32;

    /// Get a mutable reference to the text system.
    fn text_system(&mut self) -> &mut TextSystem;

    /// Render a scene to the surface.
    fn render(&mut self, scene: &Scene) -> Result<(), String>;

    /// Request a redraw of the surface.
    fn request_redraw(&self);

    /// Set the cursor style.
    fn set_cursor(&self, cursor: Cursor);

    /// Handle a resize event.
    fn handle_resize(&mut self);
}

/// Event callback type for input events.
pub type EventCallback = Box<dyn FnMut(InputEvent)>;

// Web platform (WASM)
#[cfg(all(feature = "web", target_arch = "wasm32"))]
pub mod web;

#[cfg(all(feature = "web", target_arch = "wasm32"))]
pub use web::{run_animation_loop, setup_resize_observer, WebPlatform};

// Desktop platform (native)
#[cfg(all(feature = "desktop", not(target_arch = "wasm32")))]
pub mod desktop;

#[cfg(all(feature = "desktop", not(target_arch = "wasm32")))]
pub use desktop::DesktopPlatform;
