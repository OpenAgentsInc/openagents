//! HUD Test Infrastructure
//!
//! This crate provides testing utilities for HUD GPUI components,
//! including message factories, test fixtures, and assertion helpers.
//!
//! # Overview
//!
//! The test infrastructure mirrors the TypeScript E2E test patterns:
//! - `protocol` - HUD message types (Rust equivalents of `src/hud/protocol.ts`)
//! - `messages` - Factory functions and pre-built sequences
//! - `fixtures` - GraphViewFixture (MainviewPage equivalent)
//!
//! # Example
//!
//! ```rust,ignore
//! use hud_test::prelude::*;
//!
//! #[gpui::test]
//! fn test_session_renders(cx: &mut TestAppContext) {
//!     let mut fixture = GraphViewFixture::new(cx);
//!
//!     fixture.inject(session_start(None));
//!     fixture.inject(task_selected(task_info(None, "Test Task")));
//!     fixture.wait_for_settled();
//!
//!     assert!(fixture.node_count() > 0);
//! }
//! ```

pub mod fixtures;
pub mod messages;
pub mod protocol;

/// Prelude for convenient imports
pub mod prelude {
    pub use crate::fixtures::*;
    pub use crate::messages::factories::*;
    pub use crate::messages::sequences::*;
    pub use crate::protocol::*;
}

// Re-export commonly used items at crate root
pub use messages::factories;
pub use messages::sequences;
pub use protocol::HudMessage;
