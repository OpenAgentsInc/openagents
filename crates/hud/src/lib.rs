//! HUD - Arwes-style sci-fi HUD components for WGPUI.
//!
//! This crate provides GPU-accelerated sci-fi UI components inspired by
//! the Arwes framework, adapted for the WGPUI rendering system.
//!
//! # Components
//!
//! ## Animation
//! - [`animator::HudAnimator`] - Core animation state machine
//! - [`animator::AnimatorManager`] - Orchestrates child animations
//!
//! ## Frames
//! - [`frame::FrameCorners`] - Bracket-style corner frames
//! - [`frame::FrameLines`] - Edge line frames with gaps
//! - [`frame::FrameOctagon`] - 8-sided frame with clipped corners
//! - [`frame::FrameCircle`] - Circular frame border
//! - [`frame::FrameHeader`] - Header section with accents
//! - [`frame::FrameUnderline`] - Simple bottom line
//!
//! ## Backgrounds
//! - [`background::DotGridBackground`] - Animated dot grid
//! - [`background::GridLinesBackground`] - Grid line pattern
//! - [`background::MovingLinesBackground`] - Moving lines effect
//!
//! ## Text Animation
//! - [`text::TextSequence`] - Character-by-character reveal
//! - [`text::TextDecipher`] - Scramble/decipher effect
//!
//! ## Effects
//! - [`effects::Illuminator`] - Mouse-following glow
//!
//! ## Interactive
//! - [`button::HudButton`] - Animated button with frame
//!
//! ## Form Controls
//! - [`form::TextInput`] - Text input with animated underline
//! - [`form::Checkbox`] - Animated checkbox
//! - [`form::Toggle`] - On/off switch
//! - [`form::Select`] - Dropdown select menu
//!
//! ## Data Display
//! - [`data::List`] - Animated list with bullet markers
//! - [`data::Table`] - Data table with headers
//! - [`data::CodeBlock`] - Code display with line numbers
//! - [`data::Card`] - Content container with title and frame
//!
//! ## Navigation
//! - [`nav::Tabs`] - Tabbed navigation with animated indicator
//! - [`nav::Breadcrumbs`] - Path navigation breadcrumbs
//! - [`nav::Nav`] - Main navigation (horizontal/vertical)
//! - [`nav::Pagination`] - Page navigation
//!
//! ## Feedback
//! - [`feedback::Alert`] - Notification/alert messages
//! - [`feedback::Progress`] - Progress bars (linear, segmented, striped)
//! - [`feedback::Loading`] - Loading spinner
//! - [`feedback::Modal`] - Modal dialog
//! - [`feedback::Tooltip`] - Hover tooltips
//!
//! ## Indicators
//! - [`indicator::StatusLight`] - LED-style status indicator
//! - [`indicator::Meter`] - Value gauge with thresholds
//! - [`indicator::Separator`] - Horizontal/vertical dividers
//! - [`indicator::Figure`] - Image container with frame
//!
//! # Theme
//!
//! All components use a white-on-black color scheme with varying opacities.
//! See [`theme`] for color constants.

pub mod animator;
pub mod background;
pub mod button;
pub mod data;
pub mod easing;
pub mod effects;
pub mod feedback;
pub mod form;
pub mod frame;
pub mod indicator;
pub mod nav;
pub mod text;
pub mod theme;

// Re-export commonly used types
pub use animator::{AnimatorManager, AnimatorState, HudAnimator, ManagerMode};
pub use background::{DotGridBackground, GridLinesBackground, LineDirection, MovingLinesBackground};
pub use button::HudButton;
pub use data::{Card, CodeBlock, List, ListItem, Table, TableColumn, TableRow};
pub use effects::Illuminator;
pub use form::{Checkbox, Select, SelectOption, TextInput, Toggle};
pub use frame::{FrameCircle, FrameCorners, FrameHeader, FrameLines, FrameOctagon, FrameSides, FrameUnderline};
pub use feedback::{Alert, AlertLevel, Loading, Modal, Progress, ProgressStyle, Tooltip, TooltipPosition};
pub use indicator::{Figure, Meter, Separator, SeparatorStyle, StatusLight, StatusState};
pub use nav::{Breadcrumbs, Crumb, Nav, NavDirection, NavItem, Pagination, Tab, Tabs};
pub use text::{TextDecipher, TextSequence};
pub use theme::hud as colors;
