//! WGPUI GitAfter GUI module.

mod app;
mod backend;
mod types;
mod view;

pub use app::{run_gui, run_gui_with_route};
pub use backend::GitafterBackendHandle;
pub use types::{GitafterCommand, GitafterTab, GitafterUpdate};
