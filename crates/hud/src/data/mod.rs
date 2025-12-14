//! Data display components for HUD UI.
//!
//! This module provides components for displaying structured data
//! with sci-fi styling.

mod card;
mod code_block;
mod list;
mod table;

pub use card::Card;
pub use code_block::CodeBlock;
pub use list::{List, ListItem};
pub use table::{Table, TableColumn, TableRow};
