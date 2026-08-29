//! Bounded workspace snapshot for session start (#316).
//!
//! Host-known git and issue-board facts, capped so a dirty tree cannot dump
//! hundreds of paths onto the first request. The text is placed on the wire
//! as a user message the model must not treat as a prompt. It is not a TUI
//! Notice: painting it into the transcript is how 0.2.0-rc1 opened with a
//! wall of `git log` and issue titles.

use std::path::Path;
use std::process::Command;
use std::time::Duration;

use crate::tracker::{IssueListOptions, RepoTarget, TrackerClient};

/// Hard ceiling for the rendered snapshot, including headers.
pub const CHAR_LIMIT: usize = 2_000;
/// Most porcelain paths listed. The rest are counted, not printed.
pub const MAX_STATUS_PATHS: usize = 20;
/// `git log --oneline` lines.
pub const MAX_LOG: usize = 5;
/// Open issue rows.
pub const MAX_ISSUES: usize = 15;
const ISSUE_TIMEOUT: Duration = Duration::from_millis(1_500);

/// Collect git facts, then open issues if a repository can be named, and
/// render one bounded block.
pub async fn workspace_snapshot(cwd: &Path, checkpoint: Option<&str>) -> String {
    let git = git_snapshot(cwd);
    let issues = open_issue_lines(cwd).await;
    render(&git, &issues, checkpoint)
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct GitSnapshot {
    pub cwd: String,
    pub inside_work_tree: bool,
    pub branch: Option<String>,
    pub detached: bool,
    pub sha: Option<String>,
    pub status: Vec<String>,
    pub status_total: usize,
    pub log: Vec<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct IssueLine {
    pub number: u64,
    pub title: String,
    pub blocked: bool,
}

pub fn git_snapshot(cwd: &Path) -> GitSnapshot {
    let mut snap = GitSnapshot {
        cwd: cwd.display().to_string(),
        ..GitSnapshot::default()
    };
    let inside = git(cwd, &["rev-parse", "--is-inside-work-tree"]);
    if inside.as_deref() != Some("true") {
        return snap;
    }
    snap.inside_work_tree = true;
    let head_ref = git(cwd, &["rev-parse", "--abbrev-ref", "HEAD"]);
    match head_ref.as_deref() {
        Some("HEAD") => {
            snap.detached = true;
            snap.branch = None;
        }
        Some(name) if !name.is_empty() => snap.branch = Some(name.to_string()),
        _ => {}
    }
    snap.sha = git(cwd, &["rev-parse", "--short", "HEAD"]);
    let porcelain = git(cwd, &["status", "--porcelain"]).unwrap_or_default();
    let mut paths: Vec<String> = porcelain
        .lines()
        .map(str::trim_end)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect();
    snap.status_total = paths.len();
    paths.truncate(MAX_STATUS_PATHS);
    snap.status = paths;
    snap.log = git(cwd, &["log", "-n", &MAX_LOG.to_string(), "--oneline"])
        .unwrap_or_default()
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect();
    snap
}

pub fn render(git: &GitSnapshot, issues: &[IssueLine], checkpoint: Option<&str>) -> String {
    let mut lines = Vec::new();
    lines.push("Workspace snapshot (host). Use this instead of `git status` or `openagents issue list` unless you need a fresher read.".to_string());
    lines.push(format!("cwd: {}", git.cwd));
    if !git.inside_work_tree {
        lines.push("git: not a repository".to_string());
    } else {
        let sha = git.sha.as_deref().unwrap_or("unknown");
        if git.detached {
            lines.push(format!("branch: detached @ {sha}"));
        } else {
            let branch = git.branch.as_deref().unwrap_or("unknown");
            lines.push(format!("branch: {branch} @ {sha}"));
        }
        if git.status_total == 0 {
            lines.push("status: clean".to_string());
        } else {
            let omitted = git.status_total.saturating_sub(git.status.len());
            if omitted == 0 {
                lines.push(format!("status ({}):", git.status_total));
            } else {
                lines.push(format!(
                    "status ({} , showing {}):",
                    git.status_total,
                    git.status.len()
                ));
            }
            for path in &git.status {
                lines.push(format!("  {path}"));
            }
            if omitted > 0 {
                lines.push(format!("  … {omitted} more"));
            }
        }
        if git.log.is_empty() {
            lines.push("recent: none".to_string());
        } else {
            lines.push("recent:".to_string());
            for line in &git.log {
                lines.push(format!("  {line}"));
            }
        }
    }
    if issues.is_empty() {
        lines.push("open issues: none listed".to_string());
    } else {
        lines.push(format!("open issues ({}):", issues.len()));
        for issue in issues {
            let blocked = if issue.blocked { " [blocked]" } else { "" };
            lines.push(format!("  #{} {}{blocked}", issue.number, issue.title));
        }
    }
    if let Some(note) = checkpoint.map(str::trim).filter(|note| !note.is_empty()) {
        lines.push("last checkpoint:".to_string());
        lines.push(note.to_string());
    }
    let mut text = lines.join("\n");
    if text.len() > CHAR_LIMIT {
        text.truncate(CHAR_LIMIT);
        text.push_str("\n… truncated");
    }
    text
}

async fn open_issue_lines(cwd: &Path) -> Vec<IssueLine> {
    let Some(slug) = repo_slug(cwd) else {
        return Vec::new();
    };
    let Ok(target) = RepoTarget::parse(&slug) else {
        return Vec::new();
    };
    let api_base = crate::coder::runtime::api_base();
    let token = crate::coder::runtime::user_token();
    let client = TrackerClient::new(&api_base, token);
    let options = IssueListOptions {
        limit: MAX_ISSUES as u32,
        state: Some("open".into()),
        ..IssueListOptions::default()
    };
    let listed = tokio::time::timeout(ISSUE_TIMEOUT, client.list_issues(&target, &options)).await;
    let Ok(Ok(result)) = listed else {
        return Vec::new();
    };
    result
        .issues
        .iter()
        .filter_map(|issue| {
            let number = issue.get("number")?.as_u64()?;
            let title = issue
                .get("title")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim()
                .to_string();
            if title.is_empty() {
                return None;
            }
            let blocked = issue
                .get("openagents")
                .and_then(|ext| ext.get("blocked"))
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            Some(IssueLine {
                number,
                title,
                blocked,
            })
        })
        .collect()
}

fn repo_slug(cwd: &Path) -> Option<String> {
    for remote in ["openagents", "origin"] {
        let url = git(cwd, &["remote", "get-url", remote])?;
        if let Some(slug) = crate::tracker::slug_from_remote_url(&url) {
            return Some(slug);
        }
    }
    None
}

fn git(cwd: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if text.is_empty() { None } else { Some(text) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::process::Command;

    fn git_repo() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let cwd = dir.path();
        assert!(
            Command::new("git")
                .args(["-c", "init.defaultBranch=main", "init"])
                .current_dir(cwd)
                .status()
                .unwrap()
                .success()
        );
        assert!(
            Command::new("git")
                .args(["config", "user.email", "dev@example.com"])
                .current_dir(cwd)
                .status()
                .unwrap()
                .success()
        );
        assert!(
            Command::new("git")
                .args(["config", "user.name", "dev"])
                .current_dir(cwd)
                .status()
                .unwrap()
                .success()
        );
        fs::write(cwd.join("README"), "hi\n").unwrap();
        assert!(
            Command::new("git")
                .args(["add", "README"])
                .current_dir(cwd)
                .status()
                .unwrap()
                .success()
        );
        assert!(
            Command::new("git")
                .args(["commit", "-m", "initial"])
                .current_dir(cwd)
                .status()
                .unwrap()
                .success()
        );
        dir
    }

    #[test]
    fn a_clean_repo_names_branch_and_sha_and_stays_under_the_ceiling() {
        let dir = git_repo();
        let snap = git_snapshot(dir.path());
        assert!(snap.inside_work_tree);
        assert_eq!(snap.branch.as_deref(), Some("main"));
        assert!(!snap.detached);
        assert!(snap.sha.as_deref().is_some_and(|sha| sha.len() >= 7));
        assert_eq!(snap.status_total, 0);
        assert!(snap.log.iter().any(|line| line.contains("initial")));
        let text = render(&snap, &[], None);
        assert!(text.len() <= CHAR_LIMIT, "{}", text.len());
        assert!(text.contains("branch: main @"));
        assert!(text.contains("status: clean"));
        assert!(!text.contains("DUMP_MARKER"));
    }

    #[test]
    fn two_hundred_untracked_files_are_counted_not_dumped() {
        let dir = git_repo();
        for i in 0..200 {
            fs::write(dir.path().join(format!("u{i}.txt")), "x").unwrap();
        }
        let snap = git_snapshot(dir.path());
        assert_eq!(snap.status_total, 200);
        assert_eq!(snap.status.len(), MAX_STATUS_PATHS);
        let text = render(&snap, &[], None);
        assert!(text.len() <= CHAR_LIMIT + 20, "{}", text.len());
        assert!(text.contains("status (200 , showing 20)"));
        assert!(text.contains("… 180 more"));
        let listed = text.matches("u").count();
        assert!(
            listed < 80,
            "the snapshot listed too many untracked paths: {listed}"
        );
    }

    #[test]
    fn a_non_repo_says_so_and_a_checkpoint_is_appended() {
        let dir = tempfile::tempdir().unwrap();
        let snap = git_snapshot(dir.path());
        assert!(!snap.inside_work_tree);
        let text = render(
            &snap,
            &[IssueLine {
                number: 316,
                title: "inject snapshot".into(),
                blocked: false,
            }],
            Some("#316 next: tests"),
        );
        assert!(text.contains("git: not a repository"));
        assert!(text.contains("#316 inject snapshot"));
        assert!(text.contains("last checkpoint:"));
        assert!(text.contains("#316 next: tests"));
        assert!(text.len() <= CHAR_LIMIT);
    }

    #[test]
    fn a_blocked_issue_is_marked() {
        let git = GitSnapshot {
            cwd: "/work/repo".into(),
            inside_work_tree: true,
            branch: Some("main".into()),
            sha: Some("abc1234".into()),
            ..GitSnapshot::default()
        };
        let text = render(
            &git,
            &[IssueLine {
                number: 310,
                title: "mail ownership".into(),
                blocked: true,
            }],
            None,
        );
        assert!(text.contains("#310 mail ownership [blocked]"));
        assert!(text.contains("Workspace snapshot (host)"));
    }
}
