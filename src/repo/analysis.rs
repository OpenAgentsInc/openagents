use anyhow::Result;
use serde::{Deserialize, Serialize};
use crate::server::services::{
    deepseek::{DeepSeekService, ChatMessage},
    github_issue::GitHubService,
    StreamUpdate,
};
use std::io::Write;

#[derive(Debug, Serialize, Deserialize)]
pub struct FileRequest {
    pub paths: Vec<String>,
}

pub async fn analyze_repository(
    service: &DeepSeekService,
    map: &str,
    test_output: &str,
    issue: &crate::server::services::github_issue::GitHubIssue,
) -> Result<String> {
    // First analysis prompt to get file list
    let file_request_prompt = format!(
        "Based on this repository map, select up to 10 most relevant files that you'd like to examine in detail to help solve this issue. Return only a JSON object with a 'paths' array containing the file paths.\n\nRepository Map:\n{}\n\nIssue #{} - {}:\n{}\n\nTest Results:\n{}",
        map,
        issue.number,
        issue.title,
        issue.body.as_deref().unwrap_or_default(),
        test_output
    );

    println!("\nRequesting file selection from DeepSeek...");
    let (file_response, _) = service.chat(file_request_prompt, false).await?;
    
    // Parse the JSON response to get file paths
    let file_request: FileRequest = serde_json::from_str(&file_response)
        .map_err(|e| anyhow::anyhow!("Failed to parse file request JSON: {}", e))?;

    println!("\nSelected files for detailed analysis:");
    for path in &file_request.paths {
        println!("- {}", path);
    }

    // TODO: Read contents of selected files
    // This part would need to be implemented to actually read the files
    let mut file_contents = String::new();
    for path in &file_request.paths {
        file_contents.push_str(&format!("\n=== {} ===\n", path));
        // Here you would add the actual file contents
        // file_contents.push_str(&fs::read_to_string(path)?);
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
        issue.body.unwrap_or_default(),
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