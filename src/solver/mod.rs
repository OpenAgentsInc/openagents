use clap::Parser;

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

pub mod config;
pub mod display;
pub mod github;
pub mod planning;
pub mod solution;
pub mod types;

pub use config::*;
pub use display::*;
pub use github::*;
pub use planning::*;
pub use solution::*;
pub use types::*;