use std::fs;
use std::env;
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use git2::Repository;
use openagents::repomap::generate_repo_map;
use openagents::server::services::deepseek::{DeepSeekService, ChatMessage};
use openagents::server::services::github_issue::GitHubService;
use anyhow::{Result, bail};
use dotenvy::dotenv;

fn cleanup_temp_dir(temp_dir: &std::path::PathBuf) {
    if temp_dir.exists() {
        if let Err(e) = fs::remove_dir_all(temp_dir) {
            eprintln!("Warning: Failed to clean up temporary directory: {}", e);
        } else {
            println!("Temporary directory removed.");
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
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
    fs::create_dir_all(&temp_dir)
        .map_err(|e| anyhow::anyhow!("Failed to create temporary directory: {}", e))?;
    println!("Temporary directory created at: {:?}", temp_dir);

    // Use a closure to handle the main logic and ensure cleanup
    let result = (|| async {
        // Clone the OpenAgentsInc/openagents repository into the temporary directory
        let repo_url = "https://github.com/OpenAgentsInc/openagents";
        println!("Cloning repository: {}", repo_url);
        let _repo = Repository::clone(repo_url, &temp_dir)
            .map_err(|e| anyhow::anyhow!("Failed to clone repository: {}", e))?;
        println!("Repository cloned successfully into: {:?}", temp_dir);

        // Generate and store the repository map
        let map = generate_repo_map(&temp_dir);
        println!("Repository Map:\n{}", map);

        // Run cargo test in the cloned repository with streaming output and capture results
        println!("Running cargo test in the cloned repository...");
        let mut test_output = String::new();
        let mut child = Command::new("cargo")
            .current_dir(&temp_dir)
            .arg("test")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| anyhow::anyhow!("Failed to start cargo test: {}", e))?;

        // Stream stdout in real-time and capture it
        let stdout = child.stdout.take()
            .ok_or_else(|| anyhow::anyhow!("Failed to capture stdout"))?;
        let stderr = child.stderr.take()
            .ok_or_else(|| anyhow::anyhow!("Failed to capture stderr"))?;
        
        // Spawn a thread to handle stdout
        let stdout_thread = std::thread::spawn(move || {
            let reader = BufReader::new(stdout);
            let mut output = String::new();
            for line in reader.lines() {
                if let Ok(line) = line {
                    println!("{}", line);
                    output.push_str(&line);
                    output.push('\n');
                }
            }
            output
        });

        // Spawn a thread to handle stderr
        let stderr_thread = std::thread::spawn(move || {
            let reader = BufReader::new(stderr);
            let mut output = String::new();
            for line in reader.lines() {
                if let Ok(line) = line {
                    eprintln!("{}", line);
                    output.push_str(&line);
                    output.push('\n');
                }
            }
            output
        });

        // Wait for the command to complete
        let status = child.wait()
            .map_err(|e| anyhow::anyhow!("Failed to wait for cargo test: {}", e))?;

        // Wait for output threads to finish and collect their output
        let stdout_output = stdout_thread.join()
            .map_err(|_| anyhow::anyhow!("Failed to join stdout thread"))?;
        let stderr_output = stderr_thread.join()
            .map_err(|_| anyhow::anyhow!("Failed to join stderr thread"))?;
        test_output.push_str(&stdout_output);
        test_output.push_str(&stderr_output);

        // Print final test status
        if status.success() {
            println!("\nTests completed successfully!");
        } else {
            println!("\nTests failed!");
        }

        // Fetch GitHub issue details
        println!("\nFetching GitHub issue #592...");
        let github_service = GitHubService::new(github_token.clone());
        let issue = github_service.get_issue("OpenAgentsInc", "openagents", 592).await?;
        println!("Issue fetched: {}", issue.title);

        // Initialize DeepSeek service
        let service = DeepSeekService::new(api_key);

        // Create analysis prompt
        let analysis_prompt = format!(
            "I want you to help solve this GitHub issue. Here's all the context:\n\n\
            GitHub Issue #{} - {}:\n{}\n\n\
            Repository Structure:\n{}\n\n\
            Test Results:\n{}\n\n\
            Based on this information, analyze the codebase and suggest specific code changes to solve this issue. \
            Focus on implementing proper environment isolation as described in the issue. \
            Format your response in a way that would be appropriate for a GitHub issue comment, \
            with code blocks using triple backticks and clear section headings.",
            issue.number, issue.title, issue.body.unwrap_or_default(), map, test_output
        );

        println!("\nRequesting DeepSeek analysis...");

        // Use reasoning mode for better analysis
        let mut stream = service.chat_stream(analysis_prompt, true).await;
        
        use openagents::server::services::StreamUpdate;
        let mut in_reasoning = true;
        let mut stdout = std::io::stdout();
        let mut analysis_result = String::new();

        println!("\nReasoning Process:");
        while let Some(update) = stream.recv().await {
            match update {
                StreamUpdate::Reasoning(r) => {
                    print!("{}", r);
                    stdout.flush().ok();
                }
                StreamUpdate::Content(c) => {
                    if in_reasoning {
                        println!("\n\nAnalysis & Recommendations:");
                        in_reasoning = false;
                    }
                    print!("{}", c);
                    stdout.flush().ok();
                    analysis_result.push_str(&c);
                }
                StreamUpdate::Done => break,
                _ => {}
            }
        }
        println!();

        // Post the analysis as a comment on the GitHub issue
        if let Some(token) = github_token {
            println!("\nPosting analysis to GitHub issue #592...");
            let comment = format!(
                "🤖 **Automated Analysis Report**\n\n\
                I've analyzed the codebase and test results to help implement environment isolation. \
                Here's my suggested implementation:\n\n\
                {}", 
                analysis_result
            );

            use openagents::server::services::github_issue::post_github_comment;
            post_github_comment(
                592,
                &comment,
                "OpenAgentsInc",
                "openagents",
                &token
            ).await?;
            println!("Analysis posted as comment on issue #592");
        } else {
            println!("\nSkipping GitHub comment posting - GITHUB_TOKEN not found");
        }

        Ok(())
    })().await;

    // Always clean up, regardless of success or failure
    cleanup_temp_dir(&temp_dir);

    // Return the result
    result
}