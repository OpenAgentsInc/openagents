//! GitAfter CLI subcommands
//!
//! Launches GitAfter desktop UI with optional deep links.

use clap::Subcommand;

#[derive(Subcommand)]
pub enum GitafterCommands {
    /// List repositories
    Repos,

    /// Show repository info
    Repo {
        /// Repository identifier (npub or name)
        id: String,
    },
}

pub fn run(cmd: GitafterCommands) -> anyhow::Result<()> {
    match cmd {
        GitafterCommands::Repos => gitafter::run_with_route(Some("/")),
        GitafterCommands::Repo { id } => {
            let route = format!("/repo/{}", id);
            gitafter::run_with_route(Some(&route))
        }
    }
}
