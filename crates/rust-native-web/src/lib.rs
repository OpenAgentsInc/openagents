//! Product-neutral HTML rendering. Application intents never enter markup.
//!
//! The application resolves identity-bearing events against its current
//! validated view and checks authority before performing any domain effect.

mod editing;
mod html;
pub use html::{RenderError, render, render_view};

/// Product-neutral browser styles. Applications can supply their own font
/// and color defaults through the root custom properties documented here.
pub const CSS: &str = include_str!("style.css");

#[cfg(feature = "browser")]
mod browser;
#[cfg(feature = "browser")]
pub use browser::{dispose, mount};
