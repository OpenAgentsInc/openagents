//! Workspace discovery - find .openagents/ folder and read project context.

use crate::manifest::{DirectiveSummary, IssueSummary, WorkspaceManifest};
use serde::Deserialize;
use std::path::{Path, PathBuf};

/// Discover workspace by finding .openagents/ folder.
pub async fn discover_workspace() -> anyhow::Result<Option<WorkspaceManifest>> {
    // Find .openagents/ in current dir or parents
    let openagents_dir = match find_openagents_dir() {
        Some(dir) => dir,
        None => return Ok(None),
    };

    let root = openagents_dir
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));

    // Get project name from directory name
    let project_name = root
        .file_name()
        .and_then(|n| n.to_str())
        .map(String::from);

    // Parse directives
    let directives = parse_directives(&openagents_dir).await;

    // Find active directive (status: active, prefer urgent/high priority, most recent)
    let active_directive = directives
        .iter()
        .filter(|d| d.status == "active")
        .min_by_key(|d| {
            // Priority: urgent < high < medium < low < none
            // Then by ID descending (d-027 before d-001)
            let priority_rank = match d.priority.as_deref() {
                Some("urgent") => 0,
                Some("high") => 1,
                Some("medium") => 2,
                Some("low") => 3,
                _ => 4,
            };
            // Negative ID number to sort descending (higher IDs first)
            let id_num: i32 = d
                .id
                .trim_start_matches("d-")
                .parse()
                .unwrap_or(0);
            (priority_rank, -id_num)
        })
        .map(|d| d.id.clone());

    // Parse issues
    let issues = parse_issues(&openagents_dir).await;
    let open_issues = issues.iter().filter(|i| i.status == "open").count() as u32;

    // Count pending issues
    let pending_issues = count_pending_issues(&openagents_dir);

    Ok(Some(WorkspaceManifest {
        root,
        project_name,
        has_openagents: true,
        directives,
        issues,
        open_issues,
        pending_issues,
        active_directive,
    }))
}

/// Find .openagents/ directory in current dir or parents.
fn find_openagents_dir() -> Option<PathBuf> {
    let current_dir = std::env::current_dir().ok()?;

    let mut dir = current_dir.as_path();
    loop {
        let candidate = dir.join(".openagents");
        if candidate.is_dir() {
            return Some(candidate);
        }

        dir = dir.parent()?;
    }
}

/// Parse directives from .openagents/directives/*.md
async fn parse_directives(openagents_dir: &Path) -> Vec<DirectiveSummary> {
    let directives_dir = openagents_dir.join("directives");
    if !directives_dir.is_dir() {
        return Vec::new();
    }

    let mut directives = Vec::new();

    let entries = match std::fs::read_dir(&directives_dir) {
        Ok(e) => e,
        Err(_) => return Vec::new(),
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().map(|e| e == "md").unwrap_or(false) {
            if let Some(directive) = parse_directive_file(&path) {
                directives.push(directive);
            }
        }
    }

    // Sort by ID
    directives.sort_by(|a, b| a.id.cmp(&b.id));
    directives
}

/// Parse a single directive markdown file with YAML frontmatter.
fn parse_directive_file(path: &Path) -> Option<DirectiveSummary> {
    let content = std::fs::read_to_string(path).ok()?;

    // Extract YAML frontmatter between --- markers
    if !content.starts_with("---") {
        return None;
    }

    let parts: Vec<&str> = content.splitn(3, "---").collect();
    if parts.len() < 3 {
        return None;
    }

    let yaml_str = parts[1].trim();

    #[derive(Deserialize)]
    struct DirectiveFrontmatter {
        id: String,
        title: String,
        status: String,
        #[serde(default)]
        priority: Option<String>,
        #[serde(default)]
        progress: Option<String>,
    }

    let frontmatter: DirectiveFrontmatter = serde_yaml::from_str(yaml_str).ok()?;

    // Parse progress percentage
    let progress_pct = frontmatter.progress.and_then(|p| {
        p.trim_end_matches('%')
            .parse::<u8>()
            .ok()
    });

    Some(DirectiveSummary {
        id: frontmatter.id,
        title: frontmatter.title,
        status: frontmatter.status,
        priority: frontmatter.priority,
        progress_pct,
    })
}

/// Parse issues from .openagents/issues.json
async fn parse_issues(openagents_dir: &Path) -> Vec<IssueSummary> {
    let issues_file = openagents_dir.join("issues.json");
    if !issues_file.exists() {
        return Vec::new();
    }

    let content = match std::fs::read_to_string(&issues_file) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    #[derive(Deserialize)]
    struct IssueJson {
        number: u32,
        title: String,
        status: String,
        priority: String,
        #[serde(default)]
        is_blocked: bool,
    }

    let issues: Vec<IssueJson> = match serde_json::from_str(&content) {
        Ok(i) => i,
        Err(_) => return Vec::new(),
    };

    issues
        .into_iter()
        .map(|i| IssueSummary {
            number: i.number,
            title: i.title,
            status: i.status,
            priority: i.priority,
            is_blocked: i.is_blocked,
        })
        .collect()
}

/// Count pending issues in .openagents/pending-issues/*.json
fn count_pending_issues(openagents_dir: &Path) -> u32 {
    let pending_dir = openagents_dir.join("pending-issues");
    if !pending_dir.is_dir() {
        return 0;
    }

    std::fs::read_dir(&pending_dir)
        .map(|entries| {
            entries
                .flatten()
                .filter(|e| {
                    e.path()
                        .extension()
                        .map(|ext| ext == "json")
                        .unwrap_or(false)
                })
                .count() as u32
        })
        .unwrap_or(0)
}
