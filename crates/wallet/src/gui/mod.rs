//! GUI module for wallet application.
//!
//! Native WGPUI interface for balance, send, and receive flows.

mod app;
mod backend;
mod types;
mod view;

pub use app::run_gui;
