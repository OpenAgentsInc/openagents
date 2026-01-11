//! Directive parsing and management
//!
//! Directives are high-level goals stored as markdown files with YAML frontmatter.
//! They represent epics like "Implement Nostr Protocol" or "Add test coverage".

use crate::issue::{Priority, Status};
use chrono::NaiveDate;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

/// Errors that can occur when working with directives
#[derive(Debug, Error)]
pub enum DirectiveError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("YAML parsing error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    #[error("Invalid directive format: {0}")]
    InvalidFormat(String),

    #[error("Directive not found: {0}")]
    NotFound(String),

    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),
}

/// Result type for directive operations
pub type Result<T> = std::result::Result<T, DirectiveError>;

/// Directive status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum DirectiveStatus {
    #[default]
    Active,
    Paused,
    Completed,
}

impl DirectiveStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            DirectiveStatus::Active => "active",
            DirectiveStatus::Paused => "paused",
            DirectiveStatus::Completed => "completed",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "paused" => DirectiveStatus::Paused,
            "completed" => DirectiveStatus::Completed,
            _ => DirectiveStatus::Active,
        }
    }
}

/// YAML frontmatter for a directive
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectiveFrontmatter {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub status: DirectiveStatus,
    #[serde(default)]
    pub priority: DirectivePriority,
    pub created: NaiveDate,
    #[serde(default = "today")]
    pub updated: NaiveDate,
}

fn today() -> NaiveDate {
    chrono::Utc::now().date_naive()
}

/// Priority for directives (mirrors issue priority)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum DirectivePriority {
    Urgent,
    High,
    #[default]
    Medium,
    Low,
}

impl DirectivePriority {
    pub fn as_str(&self) -> &'static str {
        match self {
            DirectivePriority::Urgent => "urgent",
            DirectivePriority::High => "high",
            DirectivePriority::Medium => "medium",
            DirectivePriority::Low => "low",
        }
    }

    pub fn to_issue_priority(&self) -> Priority {
        match self {
            DirectivePriority::Urgent => Priority::Urgent,
            DirectivePriority::High => Priority::High,
            DirectivePriority::Medium => Priority::Medium,
            DirectivePriority::Low => Priority::Low,
        }
    }
}

/// A directive representing a high-level goal
#[derive(Debug, Clone)]
pub struct Directive {
    pub id: String,
    pub title: String,
    pub status: DirectiveStatus,
    pub priority: DirectivePriority,
    pub created: NaiveDate,
    pub updated: NaiveDate,
    pub body: String,
    pub file_path: PathBuf,
}

impl Directive {
    /// Parse a directive from a markdown file with YAML frontmatter
    pub fn from_file(path: &Path) -> Result<Self> {
        let content = fs::read_to_string(path)?;
        Self::parse(&content, path.to_path_buf())
    }

    /// Parse directive content
    fn parse(content: &str, file_path: PathBuf) -> Result<Self> {
        // Split frontmatter from body
        let (frontmatter_str, body) = Self::split_frontmatter(content)?;

        // Parse YAML frontmatter
        let frontmatter: DirectiveFrontmatter = serde_yaml::from_str(&frontmatter_str)?;

        Ok(Directive {
            id: frontmatter.id,
            title: frontmatter.title,
            status: frontmatter.status,
            priority: frontmatter.priority,
            created: frontmatter.created,
            updated: frontmatter.updated,
            body,
            file_path,
        })
    }

    /// Split YAML frontmatter from markdown body
    fn split_frontmatter(content: &str) -> Result<(String, String)> {
        let content = content.trim();

        // Must start with ---
        if !content.starts_with("---") {
            return Err(DirectiveError::InvalidFormat(
                "File must start with YAML frontmatter (---)".to_string(),
            ));
        }

        // Find the closing ---
        let after_first = &content[3..];
        let end_idx = after_first
            .find("\n---")
            .ok_or_else(|| DirectiveError::InvalidFormat("Missing closing ---".to_string()))?;

        let frontmatter = after_first[..end_idx].trim().to_string();
        let body = after_first[end_idx + 4..].trim().to_string();

        Ok((frontmatter, body))
    }

    /// Save directive back to file
    pub fn save(&self) -> Result<()> {
        let frontmatter = DirectiveFrontmatter {
            id: self.id.clone(),
            title: self.title.clone(),
            status: self.status,
            priority: self.priority,
            created: self.created,
            updated: chrono::Utc::now().date_naive(),
        };

        let yaml = serde_yaml::to_string(&frontmatter)?;
        let content = format!("---\n{}---\n\n{}\n", yaml, self.body);

        fs::write(&self.file_path, content)?;
        Ok(())
    }

    /// Create a new directive file
    pub fn create(
        dir: &Path,
        id: &str,
        title: &str,
        priority: DirectivePriority,
        body: &str,
    ) -> Result<Self> {
        let today = chrono::Utc::now().date_naive();
        let file_path = dir.join(format!("{}.md", id));

        let directive = Directive {
            id: id.to_string(),
            title: title.to_string(),
            status: DirectiveStatus::Active,
            priority,
            created: today,
            updated: today,
            body: body.to_string(),
            file_path,
        };

        // Ensure directory exists
        fs::create_dir_all(dir)?;

        directive.save()?;
        Ok(directive)
    }

    /// Update directive status
    pub fn set_status(&mut self, status: DirectiveStatus) -> Result<()> {
        self.status = status;
        self.save()
    }
}

/// Load all directives from a directory
pub fn load_directives(dir: &Path) -> Result<Vec<Directive>> {
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut directives = Vec::new();

    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.extension().is_some_and(|ext| ext == "md") {
            match Directive::from_file(&path) {
                Ok(directive) => directives.push(directive),
                Err(e) => {
                    // Log but don't fail on individual parse errors
                    eprintln!("Warning: Failed to parse directive {:?}: {}", path, e);
                }
            }
        }
    }

    // Sort by priority then created date
    directives.sort_by(|a, b| {
        let priority_cmp = (a.priority as u8).cmp(&(b.priority as u8));
        if priority_cmp != std::cmp::Ordering::Equal {
            return priority_cmp;
        }
        a.created.cmp(&b.created)
    });

    Ok(directives)
}

/// Get active directives only
pub fn get_active_directives(dir: &Path) -> Result<Vec<Directive>> {
    let all = load_directives(dir)?;
    Ok(all
        .into_iter()
        .filter(|d| d.status == DirectiveStatus::Active)
        .collect())
}

/// Get a directive by ID
pub fn get_directive_by_id(dir: &Path, id: &str) -> Result<Option<Directive>> {
    let all = load_directives(dir)?;
    Ok(all.into_iter().find(|d| d.id == id))
}

/// Progress information for a directive
#[derive(Debug, Clone)]
pub struct DirectiveProgress {
    pub directive_id: String,
    pub total_issues: i32,
    pub completed_issues: i32,
    pub in_progress_issues: i32,
    pub blocked_issues: i32,
}

impl DirectiveProgress {
    pub fn percentage(&self) -> u8 {
        if self.total_issues == 0 {
            return 0;
        }
        ((self.completed_issues as f64 / self.total_issues as f64) * 100.0) as u8
    }

    /// Returns true if this directive needs more work.
    /// A directive needs work if:
    /// - It has no linked issues (needs issue creation)
    /// - It has incomplete issues
    pub fn needs_work(&self) -> bool {
        self.total_issues == 0 || self.completed_issues < self.total_issues
    }

    /// Returns true if directive is truly complete (has issues AND all done)
    pub fn is_complete(&self) -> bool {
        self.total_issues > 0 && self.completed_issues == self.total_issues
    }
}

/// Calculate progress for a directive from linked issues
pub fn calculate_progress(conn: &Connection, directive_id: &str) -> Result<DirectiveProgress> {
    let total: i32 = conn.query_row(
        "SELECT COUNT(*) FROM issues WHERE directive_id = ?",
        [directive_id],
        |row| row.get(0),
    )?;

    let completed: i32 = conn.query_row(
        "SELECT COUNT(*) FROM issues WHERE directive_id = ? AND status = 'done'",
        [directive_id],
        |row| row.get(0),
    )?;

    let in_progress: i32 = conn.query_row(
        "SELECT COUNT(*) FROM issues WHERE directive_id = ? AND status = 'in_progress'",
        [directive_id],
        |row| row.get(0),
    )?;

    let blocked: i32 = conn.query_row(
        "SELECT COUNT(*) FROM issues WHERE directive_id = ? AND is_blocked = 1",
        [directive_id],
        |row| row.get(0),
    )?;

    Ok(DirectiveProgress {
        directive_id: directive_id.to_string(),
        total_issues: total,
        completed_issues: completed,
        in_progress_issues: in_progress,
        blocked_issues: blocked,
    })
}

/// List issues linked to a directive
pub fn list_issues_by_directive(
    conn: &Connection,
    directive_id: &str,
) -> rusqlite::Result<Vec<crate::issue::Issue>> {
    let mut stmt = conn.prepare(
        "SELECT * FROM issues WHERE directive_id = ? ORDER BY
         CASE priority
           WHEN 'urgent' THEN 0
           WHEN 'high' THEN 1
           WHEN 'medium' THEN 2
           WHEN 'low' THEN 3
         END,
         created_at ASC",
    )?;

    let issues = stmt
        .query_map([directive_id], |row| {
            Ok(crate::issue::Issue {
                id: row.get("id")?,
                number: row.get("number")?,
                title: row.get("title")?,
                description: row.get("description")?,
                status: Status::from_str(&row.get::<_, String>("status")?),
                priority: Priority::from_str(&row.get::<_, String>("priority")?),
                issue_type: crate::issue::IssueType::from_str(&row.get::<_, String>("issue_type")?),
                agent: row
                    .get::<_, Option<String>>("agent")?
                    .unwrap_or_else(|| "codex".to_string()),
                directive_id: row.get("directive_id")?,
                project_id: row.get("project_id")?,
                is_blocked: row.get::<_, i32>("is_blocked")? != 0,
                blocked_reason: row.get("blocked_reason")?,
                claimed_by: row.get("claimed_by")?,
                claimed_at: row
                    .get::<_, Option<String>>("claimed_at")?
                    .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
                    .map(|dt| dt.with_timezone(&chrono::Utc)),
                created_at: chrono::DateTime::parse_from_rfc3339(
                    &row.get::<_, String>("created_at")?,
                )
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .unwrap_or_else(|_| chrono::Utc::now()),
                updated_at: chrono::DateTime::parse_from_rfc3339(
                    &row.get::<_, String>("updated_at")?,
                )
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .unwrap_or_else(|_| chrono::Utc::now()),
                completed_at: row
                    .get::<_, Option<String>>("completed_at")?
                    .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
                    .map(|dt| dt.with_timezone(&chrono::Utc)),
                auto_created: row.get::<_, Option<i32>>("auto_created")?.unwrap_or(0) != 0,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    Ok(issues)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_parse_directive() {
        let content = r#"---
id: "d-001"
title: "Test Directive"
status: active
priority: high
created: 2025-12-20
updated: 2025-12-20
---

## Goal

This is the goal.

## Success Criteria

- [ ] Item 1
- [ ] Item 2
"#;

        let directive = Directive::parse(content, PathBuf::from("test.md")).unwrap();

        assert_eq!(directive.id, "d-001");
        assert_eq!(directive.title, "Test Directive");
        assert_eq!(directive.status, DirectiveStatus::Active);
        assert_eq!(directive.priority, DirectivePriority::High);
        assert!(directive.body.contains("This is the goal"));
    }

    #[test]
    fn test_create_and_load_directive() {
        let temp_dir = TempDir::new().unwrap();
        let dir = temp_dir.path();

        // Create a directive
        let directive = Directive::create(
            dir,
            "d-test",
            "Test Title",
            DirectivePriority::Medium,
            "## Goal\n\nTest goal content.",
        )
        .unwrap();

        assert_eq!(directive.id, "d-test");
        assert_eq!(directive.title, "Test Title");

        // Load it back
        let loaded = get_directive_by_id(dir, "d-test").unwrap().unwrap();
        assert_eq!(loaded.id, "d-test");
        assert_eq!(loaded.title, "Test Title");
    }

    #[test]
    fn test_load_directives_empty_dir() {
        let temp_dir = TempDir::new().unwrap();
        let directives = load_directives(temp_dir.path()).unwrap();
        assert!(directives.is_empty());
    }

    #[test]
    fn test_directive_status_transition() {
        let temp_dir = TempDir::new().unwrap();
        let dir = temp_dir.path();

        let mut directive = Directive::create(
            dir,
            "d-status",
            "Status Test",
            DirectivePriority::Low,
            "Test body",
        )
        .unwrap();

        assert_eq!(directive.status, DirectiveStatus::Active);

        directive.set_status(DirectiveStatus::Paused).unwrap();
        let loaded = get_directive_by_id(dir, "d-status").unwrap().unwrap();
        assert_eq!(loaded.status, DirectiveStatus::Paused);

        directive.set_status(DirectiveStatus::Completed).unwrap();
        let loaded = get_directive_by_id(dir, "d-status").unwrap().unwrap();
        assert_eq!(loaded.status, DirectiveStatus::Completed);
    }
}
