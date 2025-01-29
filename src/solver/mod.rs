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
pub mod context;
pub mod display;
pub mod file_list;
pub mod github;
pub mod planning;
pub mod types;

// Re-export specific items
pub use config::Config;
pub use context::SolutionContext;
pub use display::print_colored;
pub use file_list::generate_file_list;
pub use github::GitHubContext;
pub use planning::PlanningContext;
pub use types::*;
