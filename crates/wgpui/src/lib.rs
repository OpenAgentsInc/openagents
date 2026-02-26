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

pub mod accessibility;
#[macro_use]
pub mod action;
pub mod animation;
pub mod app;
pub mod r#async;
#[cfg(feature = "audio")]
pub mod bleeps;
pub mod clipboard;
pub mod color;
pub mod components;
pub mod curve;
pub mod effects;
pub mod element;
pub mod focus;
pub mod geometry;
pub mod hit_test;
pub mod input;
pub mod integration;
pub mod interactive;
pub mod keymap;
pub mod layout;
pub mod layout_helpers;
pub mod markdown;
pub mod platform;
pub mod renderer;
pub mod scene;
pub mod scroll;
pub mod styled;
pub mod styles;
pub mod svg;
#[cfg(any(test, feature = "testing"))]
pub mod testing;
pub mod text;
pub mod text_system;
pub mod theme;
pub mod tools;
pub mod window;

pub mod prelude {
    pub mod core {
        pub use crate::animation::{Animation, AnimationController, AnimatorState, Easing, SpringAnimation};
        pub use crate::color::Hsla;
        pub use crate::components::hud::{ContextMenu, MenuItem, Tooltip, TooltipPosition};
        pub use crate::components::{
            Button, ButtonVariant, Component, ComponentId, Div, EventContext, EventResult,
            PaintContext, Text, TextDecipher, TextEffectTiming, TextInput, TextSequence,
            VirtualList,
        };
        pub use crate::effects::Illuminator;
        pub use crate::geometry::{Bounds, Point, Size};
        pub use crate::input::{InputEvent, Key, Modifiers, MouseButton, NamedKey};
        pub use crate::layout_helpers::{layout_header_nav_content, stack_bounds};
        pub use crate::markdown::{MarkdownRenderer, StreamingMarkdown};
        pub use crate::scene::{Quad, Scene, SvgQuad};
        pub use crate::text::{FontStyle, TextSystem};
        pub use crate::text_system::{
            Boundary, FontRun, LineFragment, LineLayout, LineLayoutCache, LineWrapper,
            ShapedGlyph, ShapedRun, TruncateFrom,
        };
        pub use crate::theme;
    }

    #[cfg(feature = "desktop")]
    pub mod desktop {
        pub use super::core::*;
        pub use crate::platform::desktop::DesktopPlatform;
        pub use crate::renderer::Renderer;
    }
}

pub use animation::{Animation, AnimationController, AnimatorState, Easing, SpringAnimation};
pub use action::{Action, AnyAction};
pub use color::Hsla;
pub use components::hud::{ContextMenu, MenuItem, Tooltip, TooltipPosition};
pub use components::{
    Button, ButtonVariant, Component, ComponentId, Div, EventContext, EventResult, PaintContext,
    Text, TextDecipher, TextEffectTiming, TextInput, TextSequence, VirtualList,
};
pub use effects::Illuminator;
pub use focus::{FocusChain, FocusHandle, FocusId};
pub use geometry::{Bounds, Point, Size};
pub use input::{Cursor, InputEvent, Key, Modifiers, MouseButton, NamedKey};
pub use layout_helpers::{layout_header_nav_content, stack_bounds};
pub use markdown::{MarkdownRenderer, StreamingMarkdown};
pub use platform::Platform;
pub use scene::{Quad, Scene, SvgQuad};
pub use text::{FontStyle, TextSystem};
pub use text_system::{
    Boundary, FontRun, LineFragment, LineLayout, LineLayoutCache, LineWrapper, ShapedGlyph,
    ShapedRun, TruncateFrom,
};

#[cfg(all(feature = "web", target_arch = "wasm32"))]
pub use platform::web::{WebPlatform, run_animation_loop, setup_resize_observer};

#[cfg(feature = "ios")]
pub use platform::ios::IosBackgroundState;
