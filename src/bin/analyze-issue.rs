use anyhow::Result;
use clap::Parser;
use openagents::server::services::{
    github_issue::{GitHubIssueAnalyzer, GitHubIssueAnalysis},
    openrouter::{OpenRouterService, OpenRouterConfig},
};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// The GitHub issue content to analyze
    #[arg(short, long)]
    content: String,

    /// OpenRouter API key
    #[arg(short, long, env = "OPENROUTER_API_KEY")]
    api_key: String,

    /// Enable test mode
    #[arg(short, long)]
    test_mode: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let config = OpenRouterConfig {
        test_mode: args.test_mode,
        ..Default::default()
    };

    let openrouter = OpenRouterService::with_config(args.api_key, config);
    let analyzer = GitHubIssueAnalyzer::new(openrouter);

    let analysis = analyzer.analyze_issue(&args.content).await?;

    println!("Analysis Results:");
    println!("----------------");
    println!("Summary: {}", analysis.summary);
    println!("Priority: {:?}", analysis.priority);
    println!("Estimated Effort: {:?}", analysis.estimated_effort);
    println!("\nTags:");
    for tag in &analysis.tags {
        println!("- {}", tag);
    }
    println!("\nAction Items:");
    for item in &analysis.action_items {
        println!("- {}", item);
    }

    Ok(())
}
