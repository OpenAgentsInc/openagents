//! Maud/HTMX/Tailwind components for OpenAgents.
//!
//! This crate provides:
//! - Shared UI components (Button, etc.)
//! - Tailwind CSS theme with semantic color tokens
//! - Base document layout
//!
//! # Design Principles
//!
//! 1. **Tailwind-first**: All styling via Tailwind classes, no inline styles
//! 2. **Semantic colors**: Use `bg-background`, `text-foreground`, etc. - never `bg-zinc-900`
//! 3. **Sharp corners**: No border-radius (enforced by pre-push hook)
//! 4. **Monospace**: Berkeley Mono / system monospace fonts
//!
//! # Quick Start
//!
//! ```rust,ignore
//! use ui::{Button, ButtonVariant, base_document};
//! use maud::html;
//!
//! let page = base_document("My Page", html! {
//!     div class="p-8" {
//!         h1 class="text-2xl font-bold mb-4" { "Hello" }
//!         (Button::new("Click me").variant(ButtonVariant::Primary).render())
//!     }
//! });
//! ```

mod button;
mod colors;
mod full_auto_switch;
mod layout;
pub mod recorder;

pub use button::{Button, ButtonSize, ButtonVariant};
pub use colors::COLOR_TOKENS;
pub use full_auto_switch::FullAutoSwitch;
pub use layout::base_document;

/// Tailwind CDN script (Play CDN for development).
pub const TAILWIND_CDN: &str = include_str!("static/tailwind.js");

/// Custom Tailwind theme with semantic color tokens (dark mode).
///
/// Defines CSS custom properties that map to Tailwind color utilities:
/// - `bg-background`, `text-foreground`, etc.
/// - `bg-primary`, `text-primary-foreground`, etc.
/// - `text-green`, `text-red`, etc. for accents
pub const TAILWIND_THEME: &str = r#"
@theme {
  --color-background: oklch(0.145 0 0);
  --color-foreground: oklch(0.985 0 0);
  --color-card: oklch(0.205 0 0);
  --color-card-foreground: oklch(0.985 0 0);
  --color-popover: oklch(0.269 0 0);
  --color-popover-foreground: oklch(0.985 0 0);
  --color-primary: oklch(0.922 0 0);
  --color-primary-foreground: oklch(0.205 0 0);
  --color-secondary: oklch(0.269 0 0);
  --color-secondary-foreground: oklch(0.985 0 0);
  --color-muted: oklch(0.269 0 0);
  --color-muted-foreground: oklch(0.708 0 0);
  --color-accent: oklch(0.371 0 0);
  --color-accent-foreground: oklch(0.985 0 0);
  --color-destructive: oklch(0.704 0.191 22.216);
  --color-border: oklch(1 0 0 / 10%);
  --color-input: oklch(1 0 0 / 15%);
  --color-ring: oklch(0.556 0 0);

  /* Platform accents */
  --color-green: #00A645;
  --color-red: #FF0000;
  --color-orange: #FF6600;
  --color-cyan: #00FFFF;
  --color-blue: #0000FF;
  --color-magenta: #FF00FF;
  --color-yellow: #FFBF00;
}
"#;
