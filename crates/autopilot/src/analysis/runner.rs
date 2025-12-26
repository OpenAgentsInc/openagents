//! Free Analysis Runner
//!
//! Runs free analysis on connected repositories.

use std::time::SystemTime;

use compute::domain::{IndexType, RepoIndexRequest, RepoIndexResult, Symbol};

use super::{
    DetectedLanguage, HealthCheck, IssueComplexity, Priority, Recommendation,
    RecommendationCategory, RepoStats, SuggestedIssue,
};
use crate::compute::{ComputeBuyer, ComputeBuyerConfig};
use crate::github::models::ConnectedRepo;

/// Configuration for the analysis runner
#[derive(Debug, Clone)]
pub struct AnalysisConfig {
    /// Maximum number of files to analyze
    pub max_files: u32,
    /// Maximum tokens for embedding
    pub max_tokens: u64,
    /// Include embeddings analysis
    pub include_embeddings: bool,
    /// Run health checks
    pub run_health_checks: bool,
    /// Generate recommendations
    pub generate_recommendations: bool,
}

impl Default for AnalysisConfig {
    fn default() -> Self {
        Self {
            max_files: 500,
            max_tokens: 100_000,
            include_embeddings: false, // Expensive, off by default for free tier
            run_health_checks: true,
            generate_recommendations: true,
        }
    }
}

/// Full analysis report for a repository
#[derive(Debug, Clone)]
pub struct AnalysisReport {
    /// Repository owner/name
    pub repo_full_name: String,
    /// Git reference analyzed
    pub git_ref: String,
    /// When the analysis was performed
    pub analyzed_at: u64,
    /// Duration of analysis in milliseconds
    pub duration_ms: u64,
    /// Detected languages
    pub languages: Vec<DetectedLanguage>,
    /// Repository statistics
    pub stats: RepoStats,
    /// Health check results
    pub health_checks: Vec<HealthCheck>,
    /// Recommendations
    pub recommendations: Vec<Recommendation>,
    /// Suggested issues
    pub suggested_issues: Vec<SuggestedIssue>,
    /// Analysis cost in satoshis (for display)
    pub cost_sats: u64,
}

impl AnalysisReport {
    /// Create a new empty report
    pub fn new(repo_full_name: impl Into<String>, git_ref: impl Into<String>) -> Self {
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        Self {
            repo_full_name: repo_full_name.into(),
            git_ref: git_ref.into(),
            analyzed_at: now,
            duration_ms: 0,
            languages: Vec::new(),
            stats: RepoStats::default(),
            health_checks: Vec::new(),
            recommendations: Vec::new(),
            suggested_issues: Vec::new(),
            cost_sats: 0,
        }
    }

    /// Check if all health checks passed
    pub fn all_health_checks_passed(&self) -> bool {
        self.health_checks.iter().all(|c| c.passed)
    }

    /// Get high priority recommendations
    pub fn high_priority_recommendations(&self) -> Vec<&Recommendation> {
        self.recommendations
            .iter()
            .filter(|r| matches!(r.priority, Priority::High | Priority::Critical))
            .collect()
    }

    /// Get good first issues
    pub fn good_first_issues(&self) -> Vec<&SuggestedIssue> {
        self.suggested_issues
            .iter()
            .filter(|i| i.complexity == IssueComplexity::GoodFirstIssue)
            .collect()
    }

    /// Generate a summary text
    pub fn summary(&self) -> String {
        let mut lines = vec![
            format!("## Analysis Report: {}", self.repo_full_name),
            format!("**Branch:** {}", self.git_ref),
            String::new(),
        ];

        // Languages
        if !self.languages.is_empty() {
            lines.push("### Languages".to_string());
            for lang in &self.languages {
                lines.push(format!(
                    "- {} ({:.1}%, {} files)",
                    lang.name, lang.percentage, lang.file_count
                ));
            }
            lines.push(String::new());
        }

        // Stats
        lines.push("### Statistics".to_string());
        lines.push(format!("- **Files:** {}", self.stats.total_files));
        lines.push(format!("- **Lines of code:** {}", self.stats.total_lines));
        lines.push(format!("- **Symbols:** {}", self.stats.symbol_count));
        lines.push(format!("- **Dependencies:** {}", self.stats.dependency_count));
        lines.push(String::new());

        // Health checks
        if !self.health_checks.is_empty() {
            lines.push("### Health Checks".to_string());
            for check in &self.health_checks {
                let status = if check.passed { "✅" } else { "❌" };
                lines.push(format!("- {} {} ({}ms)", status, check.name, check.duration_ms));
                if let Some(ref error) = check.error {
                    lines.push(format!("  - Error: {}", error));
                }
            }
            lines.push(String::new());
        }

        // Recommendations
        let high_priority = self.high_priority_recommendations();
        if !high_priority.is_empty() {
            lines.push("### Priority Recommendations".to_string());
            for rec in high_priority {
                lines.push(format!(
                    "- **[{}]** {}: {}",
                    rec.priority.as_str().to_uppercase(),
                    rec.title,
                    rec.description
                ));
            }
            lines.push(String::new());
        }

        // Suggested issues
        let good_first = self.good_first_issues();
        if !good_first.is_empty() {
            lines.push("### Good First Issues".to_string());
            for issue in good_first {
                lines.push(format!("- **{}**", issue.title));
                lines.push(format!("  {}", issue.body));
            }
        }

        lines.join("\n")
    }
}

/// Free analysis runner for the demo funnel
pub struct FreeAnalysisRunner {
    /// Configuration
    config: AnalysisConfig,
    /// Compute buyer for requesting analysis jobs
    compute_buyer: ComputeBuyer,
}

impl FreeAnalysisRunner {
    /// Create a new analysis runner
    pub fn new(config: AnalysisConfig) -> Self {
        Self {
            config,
            compute_buyer: ComputeBuyer::default_config(),
        }
    }

    /// Create with default configuration
    pub fn default_config() -> Self {
        Self::new(AnalysisConfig::default())
    }

    /// Generate an analysis report for a repository
    ///
    /// This creates a mock/simulated report for now.
    /// In production, it would use ComputeBuyer to request real analysis.
    pub fn analyze(&self, repo: &ConnectedRepo) -> AnalysisReport {
        let start = SystemTime::now();
        let mut report = AnalysisReport::new(&repo.full_name, &repo.default_branch);

        // Parse languages from repo metadata
        if !repo.languages.is_empty() {
            let count = repo.languages.len();
            let per_lang = 100.0 / count as f32;
            for (i, name) in repo.languages.iter().enumerate() {
                // Distribute percentages (first language gets more weight)
                let percentage = per_lang * (1.0 + 0.5 * ((count - i) as f32 / count as f32));
                report.languages.push(DetectedLanguage {
                    name: name.clone(),
                    percentage,
                    file_count: (percentage * 10.0) as u32,
                });
            }
        }

        // Sort languages by percentage
        report
            .languages
            .sort_by(|a, b| b.percentage.partial_cmp(&a.percentage).unwrap());

        // Generate stats (would be from actual analysis)
        report.stats = RepoStats {
            total_files: 100,
            total_lines: 10_000,
            symbol_count: 500,
            dependency_count: 25,
            test_file_count: 15,
            doc_file_count: 5,
        };

        // Add health checks if enabled
        if self.config.run_health_checks {
            report.health_checks = self.generate_health_checks(&report.languages);
        }

        // Add recommendations if enabled
        if self.config.generate_recommendations {
            report.recommendations = self.generate_recommendations(&report);
        }

        // Generate suggested issues
        report.suggested_issues = self.generate_suggested_issues(&report);

        // Calculate duration
        let duration = start.elapsed().unwrap_or_default();
        report.duration_ms = duration.as_millis() as u64;

        // Estimate cost (free for initial analysis)
        report.cost_sats = 0;

        report
    }

    /// Create an analysis request for the compute marketplace
    pub fn create_index_request(&self, repo: &ConnectedRepo) -> RepoIndexRequest {
        let mut request =
            RepoIndexRequest::new(&repo.full_name, &repo.default_branch)
                .add_index_type(IndexType::Symbols);

        if self.config.include_embeddings {
            request = request.add_index_type(IndexType::Embeddings);
        }

        request
            .exclude("**/node_modules/**")
            .exclude("**/target/**")
            .exclude("**/.git/**")
            .exclude("**/vendor/**")
            .exclude("**/dist/**")
            .exclude("**/build/**")
    }

    /// Generate health checks based on detected languages
    fn generate_health_checks(&self, languages: &[DetectedLanguage]) -> Vec<HealthCheck> {
        let mut checks = Vec::new();

        for lang in languages.iter().take(3) {
            match lang.name.to_lowercase().as_str() {
                "rust" => {
                    checks.push(HealthCheck::passed("cargo check", 1500));
                    checks.push(HealthCheck::passed("cargo test --no-run", 3000));
                }
                "typescript" | "javascript" => {
                    checks.push(HealthCheck::passed("npm install", 5000));
                    checks.push(HealthCheck::passed("npm run build", 2000));
                }
                "python" => {
                    checks.push(HealthCheck::passed("pip install -e .", 2000));
                    checks.push(HealthCheck::passed("python -m pytest --collect-only", 1000));
                }
                "go" => {
                    checks.push(HealthCheck::passed("go build ./...", 1000));
                    checks.push(HealthCheck::passed("go test -run=^$ ./...", 500));
                }
                _ => {}
            }
        }

        checks
    }

    /// Generate recommendations based on the report
    fn generate_recommendations(&self, report: &AnalysisReport) -> Vec<Recommendation> {
        let mut recommendations = Vec::new();

        // Check test coverage
        if report.stats.test_file_count < 5 {
            recommendations.push(Recommendation {
                category: RecommendationCategory::Testing,
                priority: Priority::Medium,
                title: "Improve test coverage".to_string(),
                description: "The repository has limited test files. Consider adding more unit and integration tests.".to_string(),
                effort: Some("2-4 hours".to_string()),
            });
        }

        // Check documentation
        if report.stats.doc_file_count < 2 {
            recommendations.push(Recommendation {
                category: RecommendationCategory::Documentation,
                priority: Priority::Low,
                title: "Add documentation".to_string(),
                description: "Consider adding a README and API documentation.".to_string(),
                effort: Some("1-2 hours".to_string()),
            });
        }

        // Check for Rust-specific recommendations
        if report
            .languages
            .iter()
            .any(|l| l.name.to_lowercase() == "rust")
        {
            recommendations.push(Recommendation {
                category: RecommendationCategory::Quality,
                priority: Priority::Low,
                title: "Run clippy lints".to_string(),
                description: "Consider running `cargo clippy` to catch common issues.".to_string(),
                effort: Some("30 minutes".to_string()),
            });
        }

        recommendations
    }

    /// Generate suggested issues based on the report
    fn generate_suggested_issues(&self, report: &AnalysisReport) -> Vec<SuggestedIssue> {
        let mut issues = Vec::new();

        // Suggest adding tests if low coverage
        if report.stats.test_file_count < 5 {
            issues.push(SuggestedIssue {
                title: "Add unit tests for core functionality".to_string(),
                body: "The repository would benefit from additional unit tests. This is a good opportunity to improve code coverage.".to_string(),
                labels: vec!["good first issue".to_string(), "testing".to_string()],
                complexity: IssueComplexity::GoodFirstIssue,
                affected_files: vec!["src/".to_string()],
            });
        }

        // Suggest documentation improvements
        if report.stats.doc_file_count < 2 {
            issues.push(SuggestedIssue {
                title: "Improve README documentation".to_string(),
                body: "Add more detailed installation and usage instructions to the README."
                    .to_string(),
                labels: vec![
                    "good first issue".to_string(),
                    "documentation".to_string(),
                ],
                complexity: IssueComplexity::GoodFirstIssue,
                affected_files: vec!["README.md".to_string()],
            });
        }

        issues
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_repo() -> ConnectedRepo {
        ConnectedRepo {
            id: 1,
            owner: "test".to_string(),
            repo: "repo".to_string(),
            full_name: "test/repo".to_string(),
            default_branch: "main".to_string(),
            languages: vec!["Rust".to_string(), "TypeScript".to_string()],
            connected_at: chrono::Utc::now(),
        }
    }

    #[test]
    fn test_default_config() {
        let config = AnalysisConfig::default();
        assert_eq!(config.max_files, 500);
        assert!(!config.include_embeddings);
        assert!(config.run_health_checks);
    }

    #[test]
    fn test_analysis_report_new() {
        let report = AnalysisReport::new("owner/repo", "main");
        assert_eq!(report.repo_full_name, "owner/repo");
        assert_eq!(report.git_ref, "main");
        assert!(report.analyzed_at > 0);
    }

    #[test]
    fn test_analysis_report_all_checks_passed() {
        let mut report = AnalysisReport::new("test", "main");
        report.health_checks = vec![
            HealthCheck::passed("check1", 100),
            HealthCheck::passed("check2", 200),
        ];
        assert!(report.all_health_checks_passed());

        report
            .health_checks
            .push(HealthCheck::failed("check3", 50, "error"));
        assert!(!report.all_health_checks_passed());
    }

    #[test]
    fn test_runner_analyze() {
        let runner = FreeAnalysisRunner::default_config();
        let repo = create_test_repo();

        let report = runner.analyze(&repo);

        assert_eq!(report.repo_full_name, "test/repo");
        assert!(!report.languages.is_empty());
        assert!(!report.health_checks.is_empty());
    }

    #[test]
    fn test_runner_create_index_request() {
        let runner = FreeAnalysisRunner::default_config();
        let repo = create_test_repo();

        let request = runner.create_index_request(&repo);

        assert_eq!(request.repo, "test/repo");
        assert!(request.index_types.contains(&IndexType::Symbols));
        assert!(!request.exclude_patterns.is_empty());
    }

    #[test]
    fn test_report_summary() {
        let mut report = AnalysisReport::new("test/repo", "main");
        report.languages.push(DetectedLanguage {
            name: "Rust".to_string(),
            percentage: 80.0,
            file_count: 50,
        });
        report.stats.total_files = 100;
        report.stats.total_lines = 10000;

        let summary = report.summary();
        assert!(summary.contains("test/repo"));
        assert!(summary.contains("Rust"));
        assert!(summary.contains("100"));
    }
}
