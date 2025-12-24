//! # wgpui - GPU-Accelerated UI Rendering Library
//!
//! A cross-platform GPU-accelerated UI rendering library built on wgpu.
//! Designed for high-performance canvas rendering with text, quads, and
//! SDF-based primitives.
//!
//! ## Features
//!
//! - **GPU Rendering**: Hardware-accelerated rendering via wgpu (WebGPU/WebGL/Vulkan/Metal/DX12)
//! - **Text Rendering**: High-quality text with cosmic-text shaping and glyph atlas
//! - **SDF Primitives**: Sharp-cornered quads and borders using signed distance fields
//! - **Layout Engine**: CSS Flexbox layout via Taffy
//! - **Theme System**: Bloomberg-inspired dark theme out of the box
//!
//! ## Architecture
//!
//! - `scene` - Accumulated draw primitives (Quad, TextRun)
//! - `renderer` - GPU rendering pipeline
//! - `text` - Text shaping and glyph atlas
//! - `layout` - Taffy-based flexbox layout
//! - `platform` - Platform abstraction (web, native)
//! - `theme` - Color and style tokens

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

pub use color::Hsla;
pub use geometry::{Bounds, Edges, Point, Size};
pub use hit_test::{Hit, HitTestEntry, HitTestIndex, NodeId};
pub use input::{Cursor, InputEvent, Key, KeyCode, Modifiers, MouseButton, NamedKey};
pub use layout::{LayoutEngine, LayoutId, LayoutStyle, auto, length, length_auto, pct, px, relative, zero};
pub use platform::Platform;
pub use scene::{GlyphInstance, GpuQuad, GpuTextQuad, Quad, Scene, TextRun};
pub use scroll::{ScrollContainer, ScrollDirection};
pub use text::{FontStyle, TextSystem};

#[cfg(all(feature = "web", target_arch = "wasm32"))]
pub use platform::web::{WebPlatform, run_animation_loop, setup_resize_observer};
