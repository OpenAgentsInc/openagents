use crate::server::services::{
    github_issue::{GitHubIssueAnalyzer, GitHubService},
    openrouter::{OpenRouterService, OpenRouterConfig},
};
use crate::server::AppState;
use anyhow::Result;
use axum::{
    extract::{Path, State},
    response::Response,
};

pub async fn analyze_issue(
    State(state): State<AppState>,
    Path((owner, repo, issue_number)): Path<(String, String, i32)>,
) -> Response {
    let result = analyze_issue_internal(state, &owner, &repo, issue_number).await;
    match result {
        Ok(xml) => Response::builder()
            .header("Content-Type", "application/vnd.hyperview+xml")
            .body(xml.into())
            .unwrap(),
        Err(e) => Response::builder()
            .header("Content-Type", "application/vnd.hyperview+xml")
            .body(format!(
                r#"<view xmlns="https://hyperview.org/hyperview">
                    <text color="red">Error: {}</text>
                </view>"#,
                e
            ).into())
            .unwrap(),
    }
}

async fn analyze_issue_internal(
    state: AppState,
    owner: &str,
    repo: &str,
    issue_number: i32,
) -> Result<String> {
    let github_service = GitHubService::new(Some(state.github_token))?;
    let issue = github_service.get_issue(owner, repo, issue_number).await?;
    let comments = github_service.get_issue_comments(owner, repo, issue_number).await?;

    // Combine issue and comments into a single text for analysis
    let mut content = format!("Title: {}\n\n{}\n\n", issue.title, issue.body.unwrap_or_default());
    for comment in comments {
        content.push_str(&format!("Comment by {}: {}\n\n", comment.user.login, comment.body));
    }

    let openrouter = OpenRouterService::new(state.openrouter_key);
    let analyzer = GitHubIssueAnalyzer::new(openrouter);
    let analysis = analyzer.analyze_issue(&content).await?;

    // Format the analysis as Hyperview XML
    let xml = format!(
        r#"<view xmlns="https://hyperview.org/hyperview" id="issue_analysis" backgroundColor="black" flex="1" padding="16">
            <text color="white" fontSize="24" marginBottom="16">Issue Analysis: #{}</text>

            <text color="white" fontSize="18" marginBottom="8">Summary</text>
            <text color="white" marginBottom="16">{}</text>

            <text color="white" fontSize="18" marginBottom="8">Priority</text>
            <text color="white" marginBottom="16" style="priority_{:?}">
                {:?}
            </text>

            <text color="white" fontSize="18" marginBottom="8">Estimated Effort</text>
            <text color="white" marginBottom="16" style="effort_{:?}">
                {:?}
            </text>

            <text color="white" fontSize="18" marginBottom="8">Tags</text>
            <view marginBottom="16">
                {}
            </view>

            <text color="white" fontSize="18" marginBottom="8">Action Items</text>
            <view marginBottom="16">
                {}
            </view>

            <text color="white" backgroundColor="gray" padding="8" borderRadius="4" marginTop="16">
                <behavior
                    trigger="press"
                    action="replace"
                    href="/hyperview/repo/{}/{}/issues?github_id={}"
                    target="issue_analysis"
                />
                Back to Issues
            </text>
        </view>"#,
        issue_number,
        analysis.summary,
        analysis.priority,
        analysis.priority,
        analysis.estimated_effort,
        analysis.estimated_effort,
        analysis.tags
            .iter()
            .map(|tag| format!(
                r#"<text color="white" backgroundColor="blue" padding="4" margin="2" borderRadius="4">{}</text>"#,
                tag
            ))
            .collect::<Vec<_>>()
            .join("\n"),
        analysis.action_items
            .iter()
            .map(|item| format!(
                r#"<text color="white" marginBottom="4">• {}</text>"#,
                item
            ))
            .collect::<Vec<_>>()
            .join("\n"),
        owner,
        repo,
        state.github_id.unwrap_or_default()
    );

    Ok(xml)
}
