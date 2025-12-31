use std::path::{Path, PathBuf};
use std::process::Command;

use chrono::Local;

use crate::verification::TerminationChecklist;

pub struct SessionStats {
    pub session_id: String,
    pub start_time: chrono::DateTime<Local>,
    pub end_time: chrono::DateTime<Local>,
    pub iterations: u32,
    pub total_actions: u32,
    pub files_modified: u32,
    pub insertions: u32,
    pub deletions: u32,
    pub commits_made: Vec<String>,
    pub issues_resolved: Vec<String>,
}

impl SessionStats {
    pub fn duration_string(&self) -> String {
        let duration = self.end_time - self.start_time;
        let hours = duration.num_hours();
        let minutes = duration.num_minutes() % 60;
        let seconds = duration.num_seconds() % 60;

        if hours > 0 {
            format!("{}h {}m {}s", hours, minutes, seconds)
        } else if minutes > 0 {
            format!("{}m {}s", minutes, seconds)
        } else {
            format!("{}s", seconds)
        }
    }
}

pub struct AfterActionReport {
    pub stats: SessionStats,
    pub checklist: TerminationChecklist,
    pub force_stopped: bool,
    pub force_stop_reason: Option<String>,
    pub suggested_next_steps: Vec<String>,
    pub questions_for_user: Vec<String>,
    pub log_path: PathBuf,
}

impl AfterActionReport {
    pub fn generate_markdown(&self) -> String {
        let mut report = String::new();

        report.push_str("# Autopilot Session Report\n\n");

        report.push_str(&format!("**Session ID:** {}\n", self.stats.session_id));
        report.push_str(&format!("**Duration:** {}\n", self.stats.duration_string()));
        report.push_str(&format!("**Iterations:** {}\n", self.stats.iterations));
        report.push_str(&format!(
            "**Total Actions:** {}\n",
            self.stats.total_actions
        ));

        if self.force_stopped {
            report.push_str("\n## :warning: Session Force Stopped\n\n");
            if let Some(reason) = &self.force_stop_reason {
                report.push_str(&format!("**Reason:** {}\n\n", reason));
            }
        }

        report.push_str("\n## Summary\n\n");
        report.push_str(&format!("- {} files changed\n", self.stats.files_modified));
        report.push_str(&format!(
            "- +{} insertions, -{} deletions\n",
            self.stats.insertions, self.stats.deletions
        ));
        report.push_str(&format!(
            "- {} commits made\n",
            self.stats.commits_made.len()
        ));
        report.push_str(&format!(
            "- {} issues resolved\n",
            self.stats.issues_resolved.len()
        ));

        if !self.stats.commits_made.is_empty() {
            report.push_str("\n## Commits Made\n\n");
            for commit in &self.stats.commits_made {
                report.push_str(&format!("- {}\n", commit));
            }
        }

        if !self.stats.issues_resolved.is_empty() {
            report.push_str("\n## Issues Resolved\n\n");
            for issue in &self.stats.issues_resolved {
                report.push_str(&format!("- {}\n", issue));
            }
        }

        report.push_str("\n## Verification Results\n\n");
        report.push_str("| Check | Status | Details |\n");
        report.push_str("|-------|--------|--------|\n");

        let checks = [
            ("Build", &self.checklist.build_clean),
            ("Clippy", &self.checklist.clippy_clean),
            ("Tests", &self.checklist.tests_passing),
            ("Coverage", &self.checklist.coverage_adequate),
            ("TODOs", &self.checklist.todos_complete),
            ("User Stories", &self.checklist.user_stories_complete),
            ("Issues", &self.checklist.issues_complete),
            ("Git Clean", &self.checklist.git_clean),
            ("Git Pushed", &self.checklist.git_pushed),
        ];

        for (name, result) in checks {
            let status = if result.passed { "PASS" } else { "FAIL" };
            let message = result.message.replace('|', "\\|");
            report.push_str(&format!("| {} | {} | {} |\n", name, status, message));
        }

        if !self.suggested_next_steps.is_empty() {
            report.push_str("\n## Suggested Next Steps\n\n");
            for (i, step) in self.suggested_next_steps.iter().enumerate() {
                report.push_str(&format!("{}. {}\n", i + 1, step));
            }
        }

        if !self.questions_for_user.is_empty() {
            report.push_str("\n## Questions for User\n\n");
            for (i, question) in self.questions_for_user.iter().enumerate() {
                report.push_str(&format!("{}. {}\n", i + 1, question));
            }
        }

        report.push_str(&format!(
            "\n## Session Log\n\nFull session log available at: {}\n",
            self.log_path.display()
        ));

        report.push_str(&format!(
            "\n---\n*Generated at {}*\n",
            self.stats.end_time.format("%Y-%m-%d %H:%M:%S")
        ));

        report
    }

    pub fn save(&self, workdir: &Path) -> std::io::Result<PathBuf> {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let date_dir = self.stats.end_time.format("%Y%m%d").to_string();
        let reports_dir = PathBuf::from(&home)
            .join(".openagents/reports")
            .join(&date_dir);

        std::fs::create_dir_all(&reports_dir)?;

        let report_file = reports_dir.join(format!("{}-report.md", self.stats.session_id));
        std::fs::write(&report_file, self.generate_markdown())?;

        let latest_link = workdir.join(".openagents/latest-report.md");
        let _ = std::fs::remove_file(&latest_link);
        let _ = std::fs::write(&latest_link, self.generate_markdown());

        Ok(report_file)
    }
}

pub fn collect_session_stats(
    workdir: &Path,
    session_id: &str,
    start_time: chrono::DateTime<Local>,
    iterations: u32,
) -> SessionStats {
    let end_time = Local::now();

    let (files_modified, insertions, deletions) = get_diff_stats(workdir, &start_time);
    let commits_made = get_recent_commits(workdir, &start_time);
    let issues_resolved = get_resolved_issues(workdir);

    SessionStats {
        session_id: session_id.to_string(),
        start_time,
        end_time,
        iterations,
        total_actions: 0,
        files_modified,
        insertions,
        deletions,
        commits_made,
        issues_resolved,
    }
}

fn get_diff_stats(workdir: &Path, since: &chrono::DateTime<Local>) -> (u32, u32, u32) {
    let since_str = since.format("%Y-%m-%d %H:%M:%S").to_string();

    let output = Command::new("git")
        .args([
            "diff",
            "--shortstat",
            &format!("--since={}", since_str),
            "HEAD",
        ])
        .current_dir(workdir)
        .output();

    if let Ok(o) = output {
        let stdout = String::from_utf8_lossy(&o.stdout);
        return parse_diff_stats(&stdout);
    }

    let output = Command::new("git")
        .args(["diff", "--shortstat", "HEAD~10", "HEAD"])
        .current_dir(workdir)
        .output();

    if let Ok(o) = output {
        let stdout = String::from_utf8_lossy(&o.stdout);
        return parse_diff_stats(&stdout);
    }

    (0, 0, 0)
}

fn parse_diff_stats(output: &str) -> (u32, u32, u32) {
    let mut files = 0u32;
    let mut insertions = 0u32;
    let mut deletions = 0u32;

    for part in output.split(',') {
        let part = part.trim();
        if part.contains("file") {
            if let Some(n) = part.split_whitespace().next() {
                files = n.parse().unwrap_or(0);
            }
        } else if part.contains("insertion") {
            if let Some(n) = part.split_whitespace().next() {
                insertions = n.parse().unwrap_or(0);
            }
        } else if part.contains("deletion") {
            if let Some(n) = part.split_whitespace().next() {
                deletions = n.parse().unwrap_or(0);
            }
        }
    }

    (files, insertions, deletions)
}

fn get_recent_commits(workdir: &Path, since: &chrono::DateTime<Local>) -> Vec<String> {
    let since_str = since.format("%Y-%m-%d %H:%M:%S").to_string();

    let output = Command::new("git")
        .args([
            "log",
            "--oneline",
            &format!("--since={}", since_str),
            "-n",
            "50",
        ])
        .current_dir(workdir)
        .output();

    match output {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout)
            .lines()
            .map(|s| s.to_string())
            .collect(),
        _ => Vec::new(),
    }
}

fn get_resolved_issues(workdir: &Path) -> Vec<String> {
    let db_path = workdir.join("autopilot.db");
    if !db_path.exists() {
        return Vec::new();
    }

    let output = Command::new("sqlite3")
        .args([
            db_path.to_str().unwrap_or("autopilot.db"),
            "SELECT '#' || number || ': ' || title FROM issues WHERE status = 'done' ORDER BY updated_at DESC LIMIT 20;",
        ])
        .current_dir(workdir)
        .output();

    match output {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout)
            .lines()
            .map(|s| s.to_string())
            .collect(),
        _ => Vec::new(),
    }
}

pub fn generate_suggested_next_steps(checklist: &TerminationChecklist) -> Vec<String> {
    let mut steps = Vec::new();

    if !checklist.coverage_adequate.passed {
        steps.push("Improve test coverage to meet the 90% threshold".to_string());
    }

    if !checklist.clippy_clean.passed {
        steps.push("Address remaining clippy warnings".to_string());
    }

    if !checklist.todos_complete.passed {
        steps.push("Review and complete remaining TODO items".to_string());
    }

    if !checklist.user_stories_complete.passed {
        steps.push("Complete remaining user stories".to_string());
    }

    if steps.is_empty() {
        steps.push("Consider adding integration tests for recently added features".to_string());
        steps.push("Review documentation for completeness".to_string());
        steps.push("Consider performance profiling for hot paths".to_string());
    }

    steps
}

pub fn generate_questions_for_user(
    checklist: &TerminationChecklist,
    force_stopped: bool,
    force_reason: &Option<String>,
) -> Vec<String> {
    let mut questions = Vec::new();

    if force_stopped {
        if let Some(reason) = force_reason {
            if reason.contains("Stuck") {
                questions.push(
                    "Should I continue trying to fix the stuck check, or skip it?".to_string(),
                );
            }
            if reason.contains("runtime") {
                questions.push("Would you like to extend the runtime and continue?".to_string());
            }
        }
    }

    if !checklist.coverage_adequate.passed {
        questions.push(
            "Should the coverage threshold be adjusted, or should more tests be added?".to_string(),
        );
    }

    if !checklist.tests_passing.passed {
        questions.push(
            "Some tests are failing. Are these known failures, or should they be fixed?"
                .to_string(),
        );
    }

    questions
}
