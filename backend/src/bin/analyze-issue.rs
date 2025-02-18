use anyhow::Result;
use clap::Parser;
use openagents::server::services::{
    github_issue::GitHubIssueAnalyzer,
    openrouter::{OpenRouterConfig, OpenRouterService},
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
    let mut analyzer = GitHubIssueAnalyzer::new(openrouter);
    let analysis = analyzer.analyze_issue(&args.content).await?;

    println!("Analysis Results:");
    println!("----------------");
    println!("Relevant Files:");
    for file in &analysis.files {
        println!("\nFile: {}", file.filepath);
        println!("Comment: {}", file.comment);
        println!("Priority: {}/10", file.priority);
    }

    Ok(())
}
