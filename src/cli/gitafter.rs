//! GitAfter CLI subcommands
//!
//! GitAfter's GUI is being rebuilt with WGPUI; CLI commands are minimal for now.

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
        GitafterCommands::Repos => {
            anyhow::bail!("Repository listing requires Nostr relay connection. GitAfter UI is being rebuilt with WGPUI.")
        }
        GitafterCommands::Repo { id } => {
            anyhow::bail!("Repository info for '{}' requires Nostr relay connection. GitAfter UI is being rebuilt with WGPUI.", id)
        }
    }
}
