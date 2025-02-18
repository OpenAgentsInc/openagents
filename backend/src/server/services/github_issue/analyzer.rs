use crate::server::services::openrouter::{types::GitHubIssueFiles, OpenRouterService};
use anyhow::Result;

#[derive(Debug)]
pub struct GitHubIssueAnalyzer {
    pub(crate) openrouter: OpenRouterService,
}

impl GitHubIssueAnalyzer {
    pub fn new(openrouter: OpenRouterService) -> Self {
        Self { openrouter }
    }

    pub async fn analyze_issue(&mut self, issue_content: &str) -> Result<GitHubIssueFiles> {
        self.openrouter.analyze_issue(issue_content).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::services::openrouter::OpenRouterConfig;

    #[tokio::test]
    #[ignore = "Requires OPENROUTER_API_KEY in environment"]
    async fn test_analyze_issue() {
        // Load .env file if it exists
        dotenvy::dotenv().ok();

        let api_key = match std::env::var("OPENROUTER_API_KEY") {
            Ok(key) => key,
            Err(_) => {
                println!("Skipping test: OPENROUTER_API_KEY not set in environment");
                return;
            }
        };

        let config = OpenRouterConfig {
            test_mode: true,
            ..Default::default()
        };
        let openrouter = OpenRouterService::with_config(api_key, config);
        let mut analyzer = GitHubIssueAnalyzer::new(openrouter);

        let test_issue = r#"
            Title: Add dark mode support

            We need to add dark mode support to improve user experience during nighttime usage.
            This should include:
            - A toggle switch in the settings
            - Dark color palette
            - Persistent preference storage
            - Automatic switching based on system preferences
        "#;

        let analysis = analyzer.analyze_issue(test_issue).await.unwrap();
        assert!(!analysis.files.is_empty());
        assert!(analysis.files.iter().all(|f| !f.filepath.is_empty()));
        assert!(analysis.files.iter().all(|f| !f.comment.is_empty()));
        assert!(analysis
            .files
            .iter()
            .all(|f| f.priority >= 1 && f.priority <= 10));
    }
}
