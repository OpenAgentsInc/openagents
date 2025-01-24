use anyhow::Result;
use serde::{Deserialize, Serialize};
use crate::server::services::{
    deepseek::DeepSeekService,
    github_issue::GitHubService,
    StreamUpdate,
};
use std::{io::Write, fs};
use std::path::Path;

#[derive(Debug, Serialize, Deserialize)]
pub struct FileRequest {
    pub paths: Vec<String>,
}

pub async fn analyze_repository(
    service: &DeepSeekService,
    map: &str,
    test_output: &str,
    issue: &crate::server::services::github_issue::GitHubIssue,
    repo_path: &Path,
) -> Result<String> {
    // First analysis prompt to get file list
    let file_request_prompt = format!(
        "Based on this repository map, select up to 10 most relevant files that you'd like to examine in detail to help solve this issue. Return ONLY a JSON object in this exact format: {{\"paths\": [\"path1\", \"path2\"]}} with no other text.\n\nRepository Map:\n{}\n\nIssue #{} - {}:\n{}\n\nTest Results:\n{}",
        map,
        issue.number,
        issue.title,
        issue.body.as_deref().unwrap_or_default(),
        test_output
    );

    println!("\nRequesting file selection from DeepSeek...");
    let (file_response, _) = service.chat(file_request_prompt, false).await?;
    
    println!("\nReceived response: {}", file_response);
    
    // Parse the JSON response to get file paths
    let file_request: FileRequest = serde_json::from_str(&file_response)
        .map_err(|e| anyhow::anyhow!("Failed to parse file request JSON: {} - Response was: {}", e, file_response))?;

    println!("\nSelected files for detailed analysis:");
    for path in &file_request.paths {
        println!("- {}", path);
    }

    // Read contents of selected files
    let mut file_contents = String::new();
    for path in &file_request.paths {
        let full_path = repo_path.join(path);
        if full_path.exists() {
            file_contents.push_str(&format!("\n=== {} ===\n", path));
            match fs::read_to_string(&full_path) {
                Ok(content) => file_contents.push_str(&content),
                Err(e) => println!("Warning: Could not read {}: {}", path, e),
            }
        } else {
            println!("Warning: File not found: {}", path);
        }
    }

    // Create final analysis prompt
    let analysis_prompt = format!(
        "I want you to help solve this GitHub issue. Here's all the context:\n\n\
        GitHub Issue #{} - {}:\n{}\n\n\
        Repository Structure:\n{}\n\n\
        Selected File Contents:\n{}\n\n\
        Test Results:\n{}\n\n\
        Based on this information, analyze the codebase and suggest specific code changes to solve this issue. \
        Focus on implementing proper environment isolation as described in the issue. \
        Format your response in a way that would be appropriate for a GitHub issue comment, \
        with code blocks using triple backticks and clear section headings.",
        issue.number,
        issue.title,
        issue.body.as_deref().unwrap_or_default(),
        map,
        file_contents,
        test_output
    );

    println!("\nRequesting DeepSeek analysis...");

    // Use reasoning mode for better analysis
    let mut stream = service.chat_stream(analysis_prompt, true).await;
    
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

    Ok(analysis_result)
}

pub async fn post_analysis(
    github_service: &GitHubService,
    analysis: &str,
    issue_number: i32,
    owner: &str,
    repo: &str,
) -> Result<()> {
    println!("\nPosting analysis to GitHub issue #{}...", issue_number);
    let comment = format!(
        "🤖 **Automated Analysis Report**\n\n\
        I've analyzed the codebase and test results to help implement environment isolation. \
        Here's my suggested implementation:\n\n\
        {}", 
        analysis
    );

    github_service.post_comment(owner, repo, issue_number, &comment).await?;
    println!("Analysis posted as comment on issue #{}", issue_number);
    Ok(())
}