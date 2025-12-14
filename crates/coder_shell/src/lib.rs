//! # coder_shell - Application Shell for Coder
//!
//! The application shell provides:
//! - **Router**: URL ↔ View mapping and navigation
//! - **Navigation**: History management, back/forward
//! - **Chrome**: Window frame, status bar, global UI
//!
//! This crate ties together the domain, runtime, and widgets
//! to create the complete Coder application structure.

pub mod chrome;
pub mod navigation;
pub mod router;
pub mod views;

// Re-exports
pub use chrome::Chrome;
pub use navigation::Navigation;
pub use router::{Route, Router};
pub use views::{View, ViewRegistry};
