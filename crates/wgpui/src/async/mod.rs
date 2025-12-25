//! Async task support for WGPUI.

mod executor;
mod task;

pub use executor::{BackgroundExecutor, ForegroundExecutor};
pub use task::Task;
