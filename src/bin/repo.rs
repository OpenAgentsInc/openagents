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
    
    // Run cargo test
    let test_output = run_cargo_tests(&ctx.temp_dir).await?;

    // Initialize DeepSeek service
    let service = DeepSeekService::new(ctx.api_key);

    if cli.test {
        println!("\nAnalyzing test coverage and generating test suggestions...");
        
        // First, analyze the test output to find uncovered modules/functions
        let coverage_prompt = format!(
            "You are a Rust testing expert. Analyze this test output and repository map to identify \
            specific functions or modules that lack test coverage. Focus only on test coverage analysis.\n\n\
            Test output:\n{}\n\nRepository map:\n{}\n\n\
            List the specific functions/modules that need test coverage, in order of importance.",
            test_output, map
        );
        
        let (coverage_analysis, _) = service.chat(coverage_prompt, false).await?;
        println!("\nTest Coverage Analysis:\n{}", coverage_analysis);

        // Then, generate a specific test implementation for the most important uncovered functionality
        let test_prompt = format!(
            "Based on the coverage analysis above, write a complete Rust test implementation for one of the \
            uncovered functions/modules. Include:\n\
            1. All necessary imports\n\
            2. Test module setup with #[cfg(test)]\n\
            3. Required test fixtures and mocks\n\
            4. Multiple test cases covering different scenarios\n\
            5. Comments explaining the test strategy\n\n\
            Write the complete test code that could be directly added to the appropriate test file.",
        );
        
        let (test_code, _) = service.chat(test_prompt, false).await?;
        println!("\nSuggested Test Implementation:\n{}", test_code);
    } else {
        // Run the regular analysis
        let analysis_result = analyze_repository(&service, &map, &test_output, &issue, &ctx.temp_dir).await?;

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
    }

    // Clean up at the end
    cleanup_temp_dir(&temp_dir);

    Ok(())
}