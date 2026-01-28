use clap::Parser;

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
}

pub fn run() -> anyhow::Result<()> {
    let cli = OpenAgentsCli::parse();
    match cli.command {
        Commands::Nostr(args) => nostr_cli::run(args),
        Commands::Spark(args) => spark_cli::run(args),
    }
}
