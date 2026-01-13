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

pub use accessibility::{
    AccessibilityContext, AccessibilityTree, AccessibleId, AccessibleNode, Announcement,
    LiveRegion, Role, State as AccessibleState,
};
pub use action::{
    Action, ActionId, ActionListeners, ActionRegistry, AnyAction, DispatchPhase, DispatchResult,
    KeyBinding, Keystroke, KeystrokeMatch, KeystrokeParseError, NoAction, PendingAction,
};
pub use animation::{
    Animatable, Animation, AnimationController, AnimationState, AnimatorId, AnimatorManagerKind,
    AnimatorMessage, AnimatorNode, AnimatorSettings, AnimatorSettingsUpdate, AnimatorState,
    AnimatorTiming, AnimatorTimingUpdate, EaseAmong, EaseSteps, EaseStepsDirection, Easing,
    Keyframe, KeyframeAnimation, SpringAnimation, Transition, TransitionAnimation, draw,
    ease_among, ease_steps, fade, flicker, transition,
};
pub use app::{AnyEntity, AnyWeakEntity, App, Context, Entity, EntityId, Subscription, WeakEntity};
pub use r#async::{BackgroundExecutor, ForegroundExecutor, Task};
#[cfg(feature = "audio")]
pub use bleeps::{
    Bleep, BleepCategory, BleepGeneralProps, BleepMasterProps, BleepProps, BleepSource,
    BleepUpdate, BleepsManager, BleepsManagerProps, BleepsManagerUpdate,
};
pub use clipboard::copy_to_clipboard;
pub use color::Hsla;
pub use curve::{CurvePrimitive, LineSegment};
pub use components::hud::{
    ContextMenu, CssSize, MenuItem, StyleFrameClipKranoxProps, StyleFrameClipOctagonProps, Tooltip,
    TooltipPosition, style_frame_clip_kranox, style_frame_clip_octagon,
};
pub use components::{
    AnyComponent, Button, ButtonVariant, Component, ComponentId, Div, Dropdown, DropdownOption,
    EventContext, EventResult, MarkdownView, Modal, PaintContext, ScrollView, Tab, Tabs, Text,
    TextDecipher, TextDurationOptions, TextEffectAnimator, TextEffectFrame, TextEffectTiming,
    TextInput, TextSequence, VirtualList, animation_text_duration,
};
pub use effects::{Illuminator, IlluminatorSvg};
pub use element::{
    AnyElement, ComponentElement, Drawable, Element, ElementId, ElementPaintContext, IntoElement,
    LayoutContext, PrepaintContext, Render, RenderOnce,
};
pub use focus::{FocusChain, FocusHandle, FocusId};
pub use geometry::{Bounds, Edges, Point, Size};
pub use hit_test::{Hit, HitTestEntry, HitTestIndex, NodeId};
pub use input::{Cursor, InputEvent, Key, KeyCode, Modifiers, MouseButton, NamedKey};
pub use interactive::{Interactive, WithAction, WithContext};
pub use keymap::{KeyContext, Keymap, default_keymap};
pub use layout::{
    LayoutEngine, LayoutId, LayoutStyle, auto, length, length_auto, pct, px, relative, zero,
};
pub use markdown::{
    CodeBlockLayout, FadeState, MarkdownBlock, MarkdownConfig, MarkdownDocument, MarkdownLayout,
    MarkdownParser, MarkdownRenderer, SUPPORTED_LANGUAGES, StreamingConfig, StreamingMarkdown,
    StyledLine, StyledSpan, SyntaxHighlighter, TextStyle, render_markdown,
};
pub use platform::Platform;
pub use scene::{GlyphInstance, GpuImageQuad, GpuQuad, GpuTextQuad, Quad, Scene, SvgQuad, TextRun};
pub use scroll::{ScrollContainer, ScrollDirection};
pub use styled::{Style, StyleRefinement, Styled, button, div, text};
pub use svg::{SvgRasterized, SvgRenderer};
pub use text::{FontStyle, TextSystem};
pub use text_system::{
    BackgroundSegment,
    Boundary,
    CacheStats,
    DecorationRun,
    DecorationSegment,
    FontId,
    FontRun,
    GlyphId,
    GlyphPaintEntry,
    LineFragment,
    // Core layout types
    LineLayout,
    // Cache
    LineLayoutCache,
    LineLayoutIndex,
    LinePaintInfo,
    // Line wrapper
    LineWrapper,
    ShapedGlyph,
    // Paintable lines
    ShapedLine,
    ShapedRun,
    StrikethroughStyle,
    TextAlign,
    TruncateFrom,
    UnderlineStyle,
    WrapBoundary,
    WrappedLine,
    WrappedLineLayout,
};
pub use window::{DispatchTree, InvalidationFlags, Invalidator, Window, WindowHandle};

#[cfg(all(feature = "web", target_arch = "wasm32"))]
pub use platform::web::{WebPlatform, run_animation_loop, setup_resize_observer};
