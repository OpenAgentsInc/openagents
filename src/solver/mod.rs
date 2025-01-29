mod changes;
mod changes_gen;
mod config;
mod context;
mod display;
mod file_list;
mod fs;
mod github;
mod parser;
mod planning;
mod solution;

pub use changes::*;
pub use changes_gen::*;
pub use config::*;
pub use context::*;
pub use display::*;
pub use file_list::*;
pub use fs::*;
pub use github::*;
pub use parser::*;
pub use planning::*;
pub use solution::*;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Change {
    pub path: String,
    pub search: String,
    pub replace: String,
}