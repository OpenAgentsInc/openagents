//! GUI module for the NIP-69 orderbook viewer
//!
//! Bloomberg terminal-style graphical interface using wgpui.

pub mod colors;
pub mod state;

mod app;

pub use app::OrderbookApp;
pub use state::{GuiState, RelayStatus};
