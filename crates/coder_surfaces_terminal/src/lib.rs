//! # Terminal Surface
//!
//! ANSI-capable terminal emulator rendered via wgpui.
//!
//! This crate provides:
//! - ANSI escape sequence parsing
//! - Scrollback buffer with styled cells
//! - Terminal widget with virtual scrolling

pub mod ansi;
pub mod buffer;
pub mod terminal;

pub use ansi::{AnsiParser, AnsiStyle};
pub use buffer::{Cell, TerminalBuffer};
pub use terminal::Terminal;
