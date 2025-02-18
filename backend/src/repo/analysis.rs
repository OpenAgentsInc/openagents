use crate::server::services::deepseek::DeepSeekService;
use anyhow::Result;

#[derive(serde::Deserialize)]
struct FileRequest {
    path: String,
}

pub async fn analyze_repository(
    service: &DeepSeekService,
    map: &str,
    test_output: &str,
    issue: &crate::server::services::github_issue::GitHubIssue,
    repo_path: &std::path::Path,
) -> Result<String> {
    // First, get the AI to identify which files we should look at
    let prompt = format!(
        "You are analyzing a Rust repository. Based on this repository map and GitHub issue, \
        identify which files we should analyze in detail to help implement the requested changes.\n\n\
        Repository Map:\n{}\n\nGitHub Issue:\nTitle: {}\nBody:\n{}\n\n\
        Respond with a JSON object containing a 'path' field with the most relevant file path to analyze.",
        map,
        issue.title,
        issue.body.as_deref().unwrap_or("No description provided")
    );

    let (file_response, _) = service.chat(prompt, false).await?;
    let file_request: FileRequest = serde_json::from_str(file_response.trim())?;

    // Read the identified file
    let file_content = std::fs::read_to_string(repo_path.join(&file_request.path))?;

    // Now analyze the file content
    let analysis_prompt = format!(
        "You are analyzing a Rust repository to implement changes requested in a GitHub issue.\n\n\
        Issue:\nTitle: {}\nBody:\n{}\n\n\
        File content ({}):\n{}\n\n\
        Test output:\n{}\n\n\
        Analyze this code and suggest specific changes to implement the requested functionality. \
        Consider:\n\
        1. Required modifications to existing functions\n\
        2. New functions or structs needed\n\
        3. Test coverage implications\n\
        4. Potential edge cases to handle\n\
        5. Error handling requirements\n\
        Be specific and provide code examples where appropriate.",
        issue.title,
        issue.body.as_deref().unwrap_or("No description provided"),
        file_request.path,
        file_content,
        test_output
    );

    let (analysis, _) = service.chat(analysis_prompt, false).await?;
    Ok(analysis)
}

pub async fn post_analysis(
    github_service: &crate::server::services::github_issue::GitHubService,
    analysis: &str,
    issue_number: i32,
    owner: &str,
    repo: &str,
) -> Result<()> {
    let comment = format!(
        "🤖 **Analysis Results**\n\n\
        Based on the repository analysis, here are the suggested changes:\n\n\
        {}\n\n\
        Please review these suggestions and let me know if you need any clarification.",
        analysis
    );

    github_service
        .post_comment(owner, repo, issue_number, &comment)
        .await?;
    Ok(())
}
