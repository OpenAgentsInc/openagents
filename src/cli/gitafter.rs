//! GitAfter CLI subcommands
//!
//! GitAfter is primarily a GUI application. CLI commands are minimal.

use clap::Subcommand;

#[derive(Subcommand)]
pub enum GitafterCommands {
    /// Launch the GitAfter GUI
    Gui,

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
        GitafterCommands::Gui => {
            anyhow::bail!("GitAfter GUI is integrated into the unified OpenAgents desktop. Run `openagents` without arguments to launch the GUI.")
        }
        GitafterCommands::Repos => {
            anyhow::bail!("Repository listing requires Nostr relay connection. Launch GUI with `openagents` to browse repositories.")
        }
        GitafterCommands::Repo { id } => {
            anyhow::bail!("Repository info for '{}' requires Nostr relay connection. Launch GUI with `openagents` to view repository details.", id)
        }
    }
}
