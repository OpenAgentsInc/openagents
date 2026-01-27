use clap::Parser;

mod nostr_cli;

#[derive(Parser)]
#[command(name = "openagents")]
#[command(about = "OpenAgents umbrella CLI")]
pub struct OpenAgentsCli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(clap::Subcommand)]
pub enum Commands {
    /// Nostr utilities (NIP-06 key management)
    Nostr(nostr_cli::NostrArgs),
}

pub fn run() -> anyhow::Result<()> {
    let cli = OpenAgentsCli::parse();
    match cli.command {
        Commands::Nostr(args) => nostr_cli::run(args),
    }
}

