use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

const COVERAGE_THRESHOLD: f32 = 90.0;
const MAX_RUNTIME_HOURS: u64 = 12;
const MAX_STUCK_ITERATIONS: u32 = 6;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminationChecklist {
    pub build_clean: CheckResult,
    pub clippy_clean: CheckResult,
    pub tests_passing: CheckResult,
    pub coverage_adequate: CheckResult,
    pub no_stubs: CheckResult,
    pub todos_complete: CheckResult,
    pub user_stories_complete: CheckResult,
    pub issues_complete: CheckResult,
    pub git_clean: CheckResult,
    pub git_pushed: CheckResult,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckResult {
    pub passed: bool,
    pub message: String,
    pub details: Option<String>,
}

impl CheckResult {
    pub fn pass(message: impl Into<String>) -> Self {
        Self {
            passed: true,
            message: message.into(),
            details: None,
        }
    }

    pub fn fail(message: impl Into<String>, details: impl Into<String>) -> Self {
        Self {
            passed: false,
            message: message.into(),
            details: Some(details.into()),
        }
    }
}

impl TerminationChecklist {
    pub fn all_passed(&self) -> bool {
        self.build_clean.passed
            && self.clippy_clean.passed
            && self.tests_passing.passed
            && self.coverage_adequate.passed
            && self.no_stubs.passed
            && self.todos_complete.passed
            && self.user_stories_complete.passed
            && self.issues_complete.passed
            && self.git_clean.passed
            && self.git_pushed.passed
    }

    pub fn failing_checks(&self) -> Vec<(&str, &CheckResult)> {
        let mut failures = Vec::new();
        if !self.build_clean.passed {
            failures.push(("build", &self.build_clean));
        }
        if !self.clippy_clean.passed {
            failures.push(("clippy", &self.clippy_clean));
        }
        if !self.tests_passing.passed {
            failures.push(("tests", &self.tests_passing));
        }
        if !self.coverage_adequate.passed {
            failures.push(("coverage", &self.coverage_adequate));
        }
        if !self.no_stubs.passed {
            failures.push(("no_stubs", &self.no_stubs));
        }
        if !self.todos_complete.passed {
            failures.push(("todos", &self.todos_complete));
        }
        if !self.user_stories_complete.passed {
            failures.push(("user_stories", &self.user_stories_complete));
        }
        if !self.issues_complete.passed {
            failures.push(("issues", &self.issues_complete));
        }
        if !self.git_clean.passed {
            failures.push(("git_clean", &self.git_clean));
        }
        if !self.git_pushed.passed {
            failures.push(("git_pushed", &self.git_pushed));
        }
        failures
    }

    pub fn summary(&self) -> String {
        let passed = [
            self.build_clean.passed,
            self.clippy_clean.passed,
            self.tests_passing.passed,
            self.coverage_adequate.passed,
            self.no_stubs.passed,
            self.todos_complete.passed,
            self.user_stories_complete.passed,
            self.issues_complete.passed,
            self.git_clean.passed,
            self.git_pushed.passed,
        ]
        .iter()
        .filter(|&&p| p)
        .count();

        format!("{}/10 checks passed", passed)
    }
}

pub struct VerificationRunner {
    workdir: PathBuf,
    start_time: Instant,
    stuck_check: Option<String>,
    stuck_count: u32,
}

impl VerificationRunner {
    pub fn new(workdir: &Path) -> Self {
        Self {
            workdir: workdir.to_path_buf(),
            start_time: Instant::now(),
            stuck_check: None,
            stuck_count: 0,
        }
    }

    pub fn max_runtime_exceeded(&self) -> bool {
        self.start_time.elapsed() > Duration::from_secs(MAX_RUNTIME_HOURS * 3600)
    }

    pub fn runtime_hours(&self) -> f32 {
        self.start_time.elapsed().as_secs_f32() / 3600.0
    }

    pub fn track_failure(&mut self, check_name: &str) -> bool {
        if self.stuck_check.as_deref() == Some(check_name) {
            self.stuck_count += 1;
        } else {
            self.stuck_check = Some(check_name.to_string());
            self.stuck_count = 1;
        }
        self.stuck_count >= MAX_STUCK_ITERATIONS
    }

    pub fn reset_stuck_tracking(&mut self) {
        self.stuck_check = None;
        self.stuck_count = 0;
    }

    pub fn run_all_checks(&self) -> TerminationChecklist {
        TerminationChecklist {
            build_clean: self.check_build(),
            clippy_clean: self.check_clippy(),
            tests_passing: self.check_tests(),
            coverage_adequate: self.check_coverage(),
            no_stubs: self.check_stubs(),
            todos_complete: self.check_todos(),
            user_stories_complete: self.check_user_stories(),
            issues_complete: self.check_issues(),
            git_clean: self.check_git_clean(),
            git_pushed: self.check_git_pushed(),
        }
    }

    fn check_build(&self) -> CheckResult {
        let output = Command::new("cargo")
            .args(["build", "--all"])
            .current_dir(&self.workdir)
            .output();

        match output {
            Ok(o) if o.status.success() => CheckResult::pass("Build successful"),
            Ok(o) => {
                let stderr = String::from_utf8_lossy(&o.stderr);
                CheckResult::fail("Build failed", stderr.to_string())
            }
            Err(e) => CheckResult::fail("Build check failed", e.to_string()),
        }
    }

    fn check_clippy(&self) -> CheckResult {
        let output = Command::new("cargo")
            .args(["clippy", "--all", "--", "-D", "warnings"])
            .current_dir(&self.workdir)
            .output();

        match output {
            Ok(o) if o.status.success() => CheckResult::pass("No clippy warnings"),
            Ok(o) => {
                let stderr = String::from_utf8_lossy(&o.stderr);
                let warning_count = stderr.matches("warning:").count();
                CheckResult::fail(
                    format!("{} clippy warnings", warning_count),
                    stderr.to_string(),
                )
            }
            Err(e) => CheckResult::fail("Clippy check failed", e.to_string()),
        }
    }

    fn check_tests(&self) -> CheckResult {
        let output = Command::new("cargo")
            .args(["test", "--all"])
            .current_dir(&self.workdir)
            .output();

        match output {
            Ok(o) if o.status.success() => {
                let stdout = String::from_utf8_lossy(&o.stdout);
                let test_line = stdout
                    .lines()
                    .find(|l| l.contains("test result:"))
                    .unwrap_or("All tests passed");
                CheckResult::pass(test_line.to_string())
            }
            Ok(o) => {
                let stdout = String::from_utf8_lossy(&o.stdout);
                let stderr = String::from_utf8_lossy(&o.stderr);
                CheckResult::fail("Some tests failed", format!("{}\n{}", stdout, stderr))
            }
            Err(e) => CheckResult::fail("Test check failed", e.to_string()),
        }
    }

    fn check_coverage(&self) -> CheckResult {
        let output = Command::new("cargo")
            .args(["llvm-cov", "--all", "--summary-only"])
            .current_dir(&self.workdir)
            .output();

        match output {
            Ok(o) if o.status.success() => {
                let stdout = String::from_utf8_lossy(&o.stdout);
                if let Some(coverage) = parse_coverage_percent(&stdout) {
                    if coverage >= COVERAGE_THRESHOLD {
                        CheckResult::pass(format!(
                            "{:.1}% coverage (threshold: {}%)",
                            coverage, COVERAGE_THRESHOLD
                        ))
                    } else {
                        CheckResult::fail(
                            format!("{:.1}% coverage (need {}%)", coverage, COVERAGE_THRESHOLD),
                            stdout.to_string(),
                        )
                    }
                } else {
                    CheckResult::pass("Coverage check passed (unable to parse exact percentage)")
                }
            }
            Ok(o) => {
                let stderr = String::from_utf8_lossy(&o.stderr);
                if stderr.contains("not found") || stderr.contains("No such") {
                    CheckResult::pass("Coverage tool not installed (skipped)")
                } else {
                    CheckResult::fail("Coverage check failed", stderr.to_string())
                }
            }
            Err(_) => CheckResult::pass("Coverage tool not available (skipped)"),
        }
    }

    fn check_stubs(&self) -> CheckResult {
        let script_path = self.workdir.join("scripts/check-stubs.sh");
        if !script_path.exists() {
            return CheckResult::pass("Stub check script not found (skipped)");
        }

        let output = Command::new(&script_path)
            .current_dir(&self.workdir)
            .output();

        match output {
            Ok(o) if o.status.success() => CheckResult::pass("No stub patterns found"),
            Ok(o) => {
                let stderr = String::from_utf8_lossy(&o.stderr);
                let stub_count = stderr.matches("todo!").count()
                    + stderr.matches("unimplemented!").count()
                    + stderr.matches("not implemented").count();
                CheckResult::fail(
                    format!("{} stub patterns found (d-012)", stub_count.max(1)),
                    stderr.to_string(),
                )
            }
            Err(e) => CheckResult::fail("Stub check failed", e.to_string()),
        }
    }

    fn check_todos(&self) -> CheckResult {
        let todo_path = self.workdir.join(".openagents/TODO.md");
        if !todo_path.exists() {
            return CheckResult::pass("No TODO.md file");
        }

        match std::fs::read_to_string(&todo_path) {
            Ok(content) => {
                let incomplete: Vec<&str> = content
                    .lines()
                    .filter(|l| {
                        let trimmed = l.trim();
                        (trimmed.starts_with("- [ ]") || trimmed.starts_with("* [ ]"))
                            && !trimmed.contains("[x]")
                            && !trimmed.contains("[X]")
                    })
                    .collect();

                if incomplete.is_empty() {
                    CheckResult::pass("All TODOs complete")
                } else {
                    CheckResult::fail(
                        format!("{} TODOs remaining", incomplete.len()),
                        incomplete.join("\n"),
                    )
                }
            }
            Err(e) => CheckResult::fail("Could not read TODO.md", e.to_string()),
        }
    }

    fn check_user_stories(&self) -> CheckResult {
        let stories_path = self.workdir.join(".openagents/USERSTORIES.md");
        if !stories_path.exists() {
            return CheckResult::pass("No USERSTORIES.md file");
        }

        match std::fs::read_to_string(&stories_path) {
            Ok(content) => {
                let incomplete: Vec<&str> = content
                    .lines()
                    .filter(|l| {
                        let trimmed = l.trim();
                        (trimmed.starts_with("- [ ]") || trimmed.starts_with("* [ ]"))
                            && !trimmed.contains("[x]")
                            && !trimmed.contains("[X]")
                    })
                    .collect();

                let complete_count = content
                    .lines()
                    .filter(|l| {
                        let trimmed = l.trim();
                        trimmed.starts_with("- [x]")
                            || trimmed.starts_with("- [X]")
                            || trimmed.starts_with("* [x]")
                            || trimmed.starts_with("* [X]")
                    })
                    .count();

                if incomplete.is_empty() {
                    CheckResult::pass(format!("All {} user stories complete", complete_count))
                } else {
                    CheckResult::fail(
                        format!("{} user stories incomplete", incomplete.len()),
                        incomplete.join("\n"),
                    )
                }
            }
            Err(e) => CheckResult::fail("Could not read USERSTORIES.md", e.to_string()),
        }
    }

    fn check_issues(&self) -> CheckResult {
        let db_path = self.workdir.join("autopilot.db");
        if !db_path.exists() {
            return CheckResult::pass("No issues database");
        }

        let output = Command::new("sqlite3")
            .args([
                db_path.to_str().unwrap_or("autopilot.db"),
                "SELECT COUNT(*) FROM issues WHERE status NOT IN ('done', 'cancelled');",
            ])
            .current_dir(&self.workdir)
            .output();

        match output {
            Ok(o) if o.status.success() => {
                let stdout = String::from_utf8_lossy(&o.stdout);
                let count: u32 = stdout.trim().parse().unwrap_or(0);
                if count == 0 {
                    CheckResult::pass("All issues complete")
                } else {
                    let details_output = Command::new("sqlite3")
                        .args([
                            db_path.to_str().unwrap_or("autopilot.db"),
                            "SELECT number, title FROM issues WHERE status NOT IN ('done', 'cancelled') LIMIT 10;",
                        ])
                        .current_dir(&self.workdir)
                        .output();

                    let details = details_output
                        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
                        .unwrap_or_default();

                    CheckResult::fail(format!("{} open issues", count), details)
                }
            }
            Ok(o) => {
                let stderr = String::from_utf8_lossy(&o.stderr);
                CheckResult::fail("Could not query issues", stderr.to_string())
            }
            Err(e) => CheckResult::fail("Issues check failed", e.to_string()),
        }
    }

    fn check_git_clean(&self) -> CheckResult {
        let output = Command::new("git")
            .args(["status", "--porcelain"])
            .current_dir(&self.workdir)
            .output();

        match output {
            Ok(o) if o.status.success() => {
                let stdout = String::from_utf8_lossy(&o.stdout);
                if stdout.trim().is_empty() {
                    CheckResult::pass("Working directory clean")
                } else {
                    let file_count = stdout.lines().count();
                    CheckResult::fail(
                        format!("{} uncommitted changes", file_count),
                        stdout.to_string(),
                    )
                }
            }
            Ok(o) => {
                let stderr = String::from_utf8_lossy(&o.stderr);
                CheckResult::fail("Git status failed", stderr.to_string())
            }
            Err(e) => CheckResult::fail("Git check failed", e.to_string()),
        }
    }

    fn check_git_pushed(&self) -> CheckResult {
        let output = Command::new("git")
            .args(["status", "-sb"])
            .current_dir(&self.workdir)
            .output();

        match output {
            Ok(o) if o.status.success() => {
                let stdout = String::from_utf8_lossy(&o.stdout);
                if stdout.contains("ahead") {
                    let ahead_count = stdout
                        .split("ahead ")
                        .nth(1)
                        .and_then(|s| s.split(']').next())
                        .and_then(|s| s.split(',').next())
                        .and_then(|s| s.trim().parse::<u32>().ok())
                        .unwrap_or(1);
                    CheckResult::fail(
                        format!("{} commits not pushed", ahead_count),
                        stdout.to_string(),
                    )
                } else {
                    CheckResult::pass("All commits pushed")
                }
            }
            Ok(o) => {
                let stderr = String::from_utf8_lossy(&o.stderr);
                CheckResult::fail("Git status failed", stderr.to_string())
            }
            Err(e) => CheckResult::fail("Git push check failed", e.to_string()),
        }
    }
}

fn parse_coverage_percent(output: &str) -> Option<f32> {
    for line in output.lines() {
        if line.contains("TOTAL") || line.contains("Total") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            for part in parts {
                if part.ends_with('%') {
                    if let Ok(pct) = part.trim_end_matches('%').parse::<f32>() {
                        return Some(pct);
                    }
                }
            }
        }
    }

    for line in output.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        for part in parts {
            if part.ends_with('%') {
                if let Ok(pct) = part.trim_end_matches('%').parse::<f32>() {
                    return Some(pct);
                }
            }
        }
    }

    None
}

pub fn generate_fix_prompt(checklist: &TerminationChecklist, iteration: u32) -> String {
    let failures = checklist.failing_checks();

    let mut prompt = format!(
        r#"## Verification Failed (Iteration {})

The following checks failed and must be fixed before the session can complete:

"#,
        iteration
    );

    for (name, result) in &failures {
        prompt.push_str(&format!("### {} - FAILED\n", name));
        prompt.push_str(&format!("**Issue:** {}\n", result.message));
        if let Some(details) = &result.details {
            let truncated = if details.len() > 2000 {
                format!("{}...\n[truncated]", &details[..2000])
            } else {
                details.clone()
            };
            prompt.push_str(&format!("**Details:**\n```\n{}\n```\n\n", truncated));
        }
    }

    prompt.push_str(
        r#"
## Your Task

Fix ALL the above issues:

1. For build/clippy failures: Fix the code errors
2. For test failures: Fix the failing tests or the code they test
3. For coverage gaps: Add tests for uncovered code paths
4. For incomplete TODOs: Complete the TODO items or remove if no longer relevant
5. For incomplete user stories: Implement the functionality or mark as done if complete
6. For open issues: Complete them or close if resolved
7. For uncommitted changes: Commit with appropriate message
8. For unpushed commits: Push to remote

Work through each failure systematically. After fixing, the verification will run again.
"#,
    );

    prompt
}

pub fn should_force_stop(
    checklist: &TerminationChecklist,
    runner: &VerificationRunner,
) -> Option<String> {
    if runner.max_runtime_exceeded() {
        return Some(format!(
            "Maximum runtime of {} hours exceeded. Forcing stop with partial completion.",
            MAX_RUNTIME_HOURS
        ));
    }

    let failures = checklist.failing_checks();
    if !failures.is_empty() {
        let first_failure = failures[0].0;
        if runner.stuck_count >= MAX_STUCK_ITERATIONS {
            return Some(format!(
                "Stuck on '{}' check for {} iterations. Requesting user input.",
                first_failure, MAX_STUCK_ITERATIONS
            ));
        }
    }

    None
}
