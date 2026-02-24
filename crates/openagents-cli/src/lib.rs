use clap::Parser;

mod citrea_cli;
mod communityfeed_cli;
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
    /// CommunityFeed utilities (agents, posts, comments, watch)
    CommunityFeed(communityfeed_cli::CommunityFeedArgs),
}

pub fn run() -> anyhow::Result<()> {
    let cli = OpenAgentsCli::parse();
    match cli.command {
        Commands::Nostr(args) => nostr_cli::run(args),
        Commands::Spark(args) => spark_cli::run(args),
        Commands::Citrea(args) => citrea_cli::run(args),
        Commands::CommunityFeed(args) => communityfeed_cli::run(args),
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser;
    use clap::error::ErrorKind;

    use super::OpenAgentsCli;

    #[test]
    fn cli_requires_subcommand() {
        let err = match OpenAgentsCli::try_parse_from(["openagents"]) {
            Ok(_) => panic!("expected missing subcommand parse error"),
            Err(err) => err,
        };
        assert_eq!(
            err.kind(),
            ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand
        );
    }

    #[test]
    fn cli_rejects_unknown_subcommand() {
        let err = match OpenAgentsCli::try_parse_from(["openagents", "unknown-subcommand"]) {
            Ok(_) => panic!("expected invalid subcommand parse error"),
            Err(err) => err,
        };
        assert_eq!(err.kind(), ErrorKind::InvalidSubcommand);
    }
}
