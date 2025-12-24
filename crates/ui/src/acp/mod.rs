//! ACP (Agent Client Protocol) UI components for Claude Code conversation rendering.
//!
//! This module provides components that mirror Zed's ACP implementation,
//! allowing OpenAgents to render Claude Code conversations with structural parity.
//!
//! # Architecture
//!
//! Components are organized using atomic design principles:
//! - **Atoms**: Simple, single-purpose elements (icons, badges, buttons)
//! - **Molecules**: Compositions of atoms (headers, action bars)
//! - **Organisms**: Complex components (messages, tool calls)
//! - **Sections**: Page-level layouts (thread view, editor)

pub mod atoms;
pub mod molecules;
pub mod organisms;
pub mod sections;
pub mod styles;
