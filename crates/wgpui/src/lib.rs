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
pub mod r#async;
pub mod animation;
pub mod app;
pub mod element;
#[cfg(feature = "audio")]
pub mod bleeps;
pub mod color;
pub mod components;
pub mod effects;
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
#[cfg(any(test, feature = "testing"))]
pub mod testing;
pub mod text;
pub mod theme;
pub mod tools;
pub mod styles;
pub mod window;

pub use color::Hsla;
pub use geometry::{Bounds, Edges, Point, Size};
pub use hit_test::{Hit, HitTestEntry, HitTestIndex, NodeId};
pub use input::{Cursor, InputEvent, Key, KeyCode, Modifiers, MouseButton, NamedKey};
pub use layout::{LayoutEngine, LayoutId, LayoutStyle, auto, length, length_auto, pct, px, relative, zero};
pub use platform::Platform;
pub use scene::{GlyphInstance, GpuQuad, GpuTextQuad, Quad, Scene, TextRun};
pub use scroll::{ScrollContainer, ScrollDirection};
pub use text::{FontStyle, TextSystem};
pub use components::{
    AnyComponent, Button, ButtonVariant, Component, ComponentId, Div, Dropdown, DropdownOption,
    EventContext, EventResult, Modal, PaintContext, ScrollView, Tab, Tabs, Text, TextDecipher,
    TextEffectAnimator, TextEffectFrame, TextEffectTiming, TextDurationOptions,
    animation_text_duration, TextInput, TextSequence, VirtualList,
};
pub use effects::{Illuminator, IlluminatorSvg};
pub use focus::{FocusChain, FocusHandle, FocusId};
pub use markdown::{
    FadeState, MarkdownBlock, MarkdownConfig, MarkdownDocument, MarkdownParser,
    MarkdownRenderer, StreamingConfig, StreamingMarkdown, StyledLine, StyledSpan,
    SyntaxHighlighter, TextStyle, render_markdown, SUPPORTED_LANGUAGES,
};
pub use animation::{
    Animation, AnimationController, AnimationState, Animatable,
    AnimatorId, AnimatorManagerKind, AnimatorMessage, AnimatorNode, AnimatorSettings,
    AnimatorSettingsUpdate, AnimatorState, AnimatorTiming, AnimatorTimingUpdate, EaseAmong,
    EaseSteps, EaseStepsDirection, Easing,
    Keyframe, KeyframeAnimation, SpringAnimation,
    Transition, TransitionAnimation, draw, ease_among, ease_steps, fade, flicker, transition,
};
pub use accessibility::{
    AccessibilityContext, AccessibilityTree, AccessibleId, AccessibleNode,
    Announcement, LiveRegion, Role, State as AccessibleState,
};
pub use r#async::{BackgroundExecutor, ForegroundExecutor, Task};
pub use components::hud::{
    ContextMenu, CssSize, MenuItem, StyleFrameClipKranoxProps, StyleFrameClipOctagonProps,
    Tooltip, TooltipPosition, style_frame_clip_kranox, style_frame_clip_octagon,
};
#[cfg(feature = "audio")]
pub use bleeps::{
    Bleep, BleepCategory, BleepGeneralProps, BleepMasterProps, BleepProps, BleepsManager,
    BleepsManagerProps, BleepsManagerUpdate, BleepSource, BleepUpdate,
};
pub use app::{
    App, AnyEntity, AnyWeakEntity, Context, Entity, EntityId, Subscription, WeakEntity,
};
pub use element::{
    AnyElement, ComponentElement, Drawable, Element, ElementId, ElementPaintContext, IntoElement,
    LayoutContext, PrepaintContext, Render, RenderOnce,
};
pub use styled::{Style, StyleRefinement, Styled, button, div, text};
pub use window::{DispatchTree, InvalidationFlags, Invalidator, Window, WindowHandle};
pub use action::{Action, ActionId, AnyAction, ActionRegistry, ActionListeners, DispatchPhase, DispatchResult, PendingAction, KeyBinding, Keystroke, KeystrokeMatch, KeystrokeParseError, NoAction};
pub use interactive::{Interactive, WithAction, WithContext};
pub use keymap::{KeyContext, Keymap, default_keymap};

#[cfg(all(feature = "web", target_arch = "wasm32"))]
pub use platform::web::{WebPlatform, run_animation_loop, setup_resize_observer};
