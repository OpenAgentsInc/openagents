use anyhow::Result;
use crate::server::services::openrouter::{OpenRouterService, types::GitHubIssueFiles};

#[derive(Debug)]
pub struct GitHubIssueAnalyzer {
    pub(crate) openrouter: OpenRouterService,
}

impl GitHubIssueAnalyzer {
    pub fn new(openrouter: OpenRouterService) -> Self {
        Self { openrouter }
    }

    pub async fn analyze_issue(&self, issue_content: &str) -> Result<GitHubIssueFiles> {
        self.openrouter.analyze_issue(issue_content).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::services::openrouter::OpenRouterConfig;

    #[tokio::test]
    async fn test_analyze_issue() {
        let api_key = std::env::var("OPENROUTER_API_KEY").expect("OPENROUTER_API_KEY must be set");
        let config = OpenRouterConfig {
            test_mode: true,
            ..Default::default()
        };
        let openrouter = OpenRouterService::with_config(api_key, config);
        let analyzer = GitHubIssueAnalyzer::new(openrouter);

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
        assert!(!analysis.summary.is_empty());
        assert!(!analysis.tags.is_empty());
        assert!(!analysis.action_items.is_empty());
    }
}
