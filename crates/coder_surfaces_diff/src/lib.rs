//! # Diff Viewer Surface
//!
//! Side-by-side and inline diff rendering.
//!
//! This crate provides:
//! - Diff computation between text
//! - Side-by-side and unified diff views
//! - Syntax highlighting integration
//! - Line number gutters

pub mod diff;
pub mod view;

pub use diff::{Change, ChangeKind, DiffResult, FileDiff};
pub use view::{DiffView, DiffViewMode};
