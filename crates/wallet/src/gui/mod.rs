//! GUI module for wallet application
//!
//! Provides a native desktop interface with wry/tao + Actix server + Maud/HTMX.

mod app;
mod server;
mod views;

pub use app::run_gui;
