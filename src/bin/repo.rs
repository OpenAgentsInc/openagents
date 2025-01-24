use std::env;
use anyhow::{Result, bail};
use dotenvy::dotenv;
use clap::Parser;
use openagents::{
    repomap::generate_repo_map,
    server::services::{
        deepseek::DeepSeekService,
        github_issue::GitHubService,
    },
    repo::{
        RepoContext,
        cleanup_temp_dir,
        clone_repository,
        run_cargo_tests,
        analyze_repository,
        post_analysis,
    },
};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Generate test suggestions for uncovered functionality
    #[arg(long)]
    test: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Load .env file first
    if let Err(e) = dotenv() {
        bail!("Failed to load .env file: {}", e);
    }

    // Get API keys immediately and fail if not present
    let api_key = env::var("DEEPSEEK_API_KEY")
        .map_err(|_| anyhow::anyhow!("DEEPSEEK_API_KEY not found in environment or .env file"))?;
    let github_token = env::var("GITHUB_TOKEN").ok();

    // Define the temporary directory path
    let temp_dir = env::temp_dir().join("rust_app_temp");

    // Clean up any existing temp directory first
    cleanup_temp_dir(&temp_dir);

    // Create the temporary directory
    std::fs::create_dir_all(&temp_dir)
        .map_err(|e| anyhow::anyhow!("Failed to create temporary directory: {}", e))?;
    println!("Temporary directory created at: {:?}", temp_dir);

    // Create context
    let ctx = RepoContext::new(temp_dir.clone(), api_key, github_token.clone());

    // Clone the repository
    let repo_url = "https://github.com/OpenAgentsInc/openagents";
    let _repo = clone_repository(repo_url, &ctx.temp_dir)?;

    // Generate and store the repository map
    let map = generate_repo_map(&ctx.temp_dir);
    println!("Repository Map:\n{}", map);

    // Run cargo test
    let test_output = run_cargo_tests(&ctx.temp_dir).await?;

    // Fetch GitHub issue details
    println!("\nFetching GitHub issue #592...");
    let github_service = GitHubService::new(github_token.clone());
    let issue = github_service.get_issue("OpenAgentsInc", "openagents", 592).await?;
    println!("Issue fetched: {}", issue.title);

    // Initialize DeepSeek service
    let service = DeepSeekService::new(ctx.api_key);

    // Run the analysis
    let analysis_result = analyze_repository(&service, &map, &test_output, &issue, &ctx.temp_dir).await?;

    // If test flag is enabled, request test suggestions
    if cli.test {
        println!("\nGenerating test suggestions...");
        let test_prompt = format!(
            "Based on the repository map and test output, suggest a new test for uncovered functionality. \
            Focus on important functions or modules that lack test coverage. \
            Repository map:\n{}\n\nTest output:\n{}\n\n\
            Please write a complete test implementation in Rust that would improve test coverage. \
            Include necessary imports and test setup.",
            map, test_output
        );
        let (test_suggestion, _) = service.chat(test_prompt, false).await?;
        println!("\nTest Suggestion:\n{}", test_suggestion);
    }

    // Post the analysis as a comment on the GitHub issue
    if let Some(_) = github_token {
        post_analysis(
            &github_service,
            &analysis_result,
            592,
            "OpenAgentsInc",
            "openagents"
        ).await?;
    } else {
        println!("\nSkipping GitHub comment posting - GITHUB_TOKEN not found");
    }

    // Clean up at the end
    cleanup_temp_dir(&temp_dir);

    Ok(())
}