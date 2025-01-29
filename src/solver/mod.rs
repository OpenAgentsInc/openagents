use clap::Parser;
use serde::{Deserialize, Serialize};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    /// GitHub issue number to solve
    #[arg(short, long)]
    pub issue: i32,

    /// GitHub repository (format: owner/name)
    #[arg(short, long, default_value = "OpenAgentsInc/openagents")]
    pub repo: String,

    /// Execute changes on GitHub (create branch, post comments, create PR)
    #[arg(long)]
    pub live: bool,
}

mod changes;
mod config;
mod context;
mod display;
mod file_list;
mod github;
mod parser;
mod planning;

pub use changes::*;
pub use config::*;
pub use context::*;
pub use display::*;
pub use file_list::*;
pub use github::*;
pub use parser::*;
pub use planning::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Change {
    pub path: String,
    pub search: String,
    pub replace: String,
}