//! Shadcn-style UI components for GPUI
//!
//! Components follow these patterns:
//! - Builder API: `Button::new("label").variant(v).size(s)`
//! - RenderOnce for stateless, Render for stateful
//! - Theme colors from `theme::ui::*`

mod button;
mod label;
mod separator;
mod kbd;
mod skeleton;
mod spinner;
mod progress;
mod checkbox;
mod switch;

pub use button::{Button, ButtonVariant, ButtonSize};
pub use label::Label;
pub use separator::Separator;
pub use kbd::Kbd;
pub use skeleton::Skeleton;
pub use spinner::Spinner;
pub use progress::Progress;
pub use checkbox::Checkbox;
pub use switch::Switch;
