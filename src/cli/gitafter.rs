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
            // For now, print a message. The GUI will be launched via
            // the unified GUI module that integrates GitAfter as a tab.
            println!("GitAfter GUI is integrated into the unified OpenAgents desktop.");
            println!("Run `openagents` without arguments to launch the GUI.");
            Ok(())
        }
        GitafterCommands::Repos => {
            println!("Repository listing requires Nostr relay connection.");
            println!("Launch GUI with `openagents` to browse repositories.");
            Ok(())
        }
        GitafterCommands::Repo { id } => {
            println!("Repository info for: {}", id);
            println!("Launch GUI with `openagents` to view repository details.");
            Ok(())
        }
    }
}
