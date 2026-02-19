use clap::Parser;

mod citrea_cli;
mod moltbook_cli;
mod nostr_cli;
mod spark_cli;

#[derive(Parser)]
#[command(name = "openagents")]
#[command(about = "OpenAgents umbrella CLI")]
pub struct OpenAgentsCli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(clap::Subcommand)]
pub enum Commands {
    /// Nostr utilities (keys, events, encoding, encryption, auth)
    Nostr(nostr_cli::NostrArgs),
    /// Spark wallet utilities (keys, payments, tokens)
    Spark(spark_cli::SparkArgs),
    /// Citrea utilities (keys, signatures, RPC helpers)
    Citrea(citrea_cli::CitreaArgs),
    /// Moltbook utilities (agents, posts, comments, watch)
    Moltbook(moltbook_cli::MoltbookArgs),
}

pub fn run() -> anyhow::Result<()> {
    let cli = OpenAgentsCli::parse();
    match cli.command {
        Commands::Nostr(args) => nostr_cli::run(args),
        Commands::Spark(args) => spark_cli::run(args),
        Commands::Citrea(args) => citrea_cli::run(args),
        Commands::Moltbook(args) => moltbook_cli::run(args),
    }
}
