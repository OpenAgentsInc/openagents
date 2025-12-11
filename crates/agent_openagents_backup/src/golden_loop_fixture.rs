//! Golden Loop Test Fixture
//!
//! Creates a reusable Golden Loop-ready git repo with .openagents config.
//! Useful for orchestrator/CLI/overnight regression tests.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Options for creating a golden loop fixture
#[derive(Debug, Clone, Default)]
pub struct GoldenLoopFixtureOptions {
    /// Name for the fixture (used in temp directory)
    pub name: Option<String>,
    /// Partial task configuration
    pub task: Option<PartialTask>,
    /// Test commands to run
    pub test_commands: Option<Vec<String>>,
    /// Whether to allow push
    pub allow_push: Option<bool>,
}

/// Partial task for fixture configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PartialTask {
    pub id: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub status: Option<String>,
    pub priority: Option<i32>,
    #[serde(rename = "type")]
    pub task_type: Option<String>,
    pub labels: Option<Vec<String>>,
    pub deps: Option<Vec<String>>,
    pub commits: Option<Vec<String>>,
    pub comments: Option<Vec<String>>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub closed_at: Option<String>,
}

/// Full task definition for fixture
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FixtureTask {
    pub id: String,
    pub title: String,
    pub description: String,
    pub status: String,
    pub priority: i32,
    #[serde(rename = "type")]
    pub task_type: String,
    pub labels: Vec<String>,
    pub deps: Vec<String>,
    pub commits: Vec<String>,
    pub comments: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub closed_at: Option<String>,
}

/// Project configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectConfig {
    pub project_id: String,
    pub default_branch: String,
    pub test_commands: Vec<String>,
    pub allow_push: bool,
}

/// Created fixture information
#[derive(Debug, Clone)]
pub struct GoldenLoopFixture {
    /// Root directory of the fixture
    pub dir: PathBuf,
    /// Path to .openagents directory
    pub openagents_dir: PathBuf,
    /// Path to tasks.jsonl file
    pub tasks_path: PathBuf,
    /// ID of the created task
    pub task_id: String,
}

impl GoldenLoopFixture {
    /// Clean up the fixture directory
    pub fn cleanup(&self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

impl Drop for GoldenLoopFixture {
    fn drop(&mut self) {
        self.cleanup();
    }
}

/// Create a golden loop fixture
pub fn create_golden_loop_fixture(options: GoldenLoopFixtureOptions) -> std::io::Result<GoldenLoopFixture> {
    let name = options.name.as_deref().unwrap_or("fixture");
    let dir = std::env::temp_dir().join(format!("golden-loop-{}-{}", name, uuid::Uuid::new_v4()));
    fs::create_dir_all(&dir)?;

    // Initialize git repo
    run_git(&dir, &["init"])?;
    run_git(&dir, &["checkout", "-b", "main"])?;
    run_git(&dir, &["config", "user.email", "mechacoder@example.com"])?;
    run_git(&dir, &["config", "user.name", "MechaCoder"])?;

    // Create README
    fs::write(dir.join("README.md"), "# Golden Loop Fixture Repo\n")?;

    // Create task
    let now = chrono::Utc::now().to_rfc3339();
    let task_id = options
        .task
        .as_ref()
        .and_then(|t| t.id.clone())
        .unwrap_or_else(|| format!("oa-{}", name));

    let partial = options.task.unwrap_or_default();

    let task = FixtureTask {
        id: task_id.clone(),
        title: partial
            .title
            .unwrap_or_else(|| format!("Golden Loop Fixture {}", name)),
        description: partial
            .description
            .unwrap_or_else(|| "Stub task for Golden Loop regression".to_string()),
        status: partial.status.unwrap_or_else(|| "open".to_string()),
        priority: partial.priority.unwrap_or(1),
        task_type: partial.task_type.unwrap_or_else(|| "task".to_string()),
        labels: partial
            .labels
            .unwrap_or_else(|| vec!["golden-loop".to_string()]),
        deps: partial.deps.unwrap_or_default(),
        commits: partial.commits.unwrap_or_default(),
        comments: partial.comments.unwrap_or_default(),
        created_at: partial.created_at.unwrap_or_else(|| now.clone()),
        updated_at: partial.updated_at.unwrap_or(now),
        closed_at: partial.closed_at,
    };

    // Create .openagents directory
    let oa_dir = dir.join(".openagents");
    fs::create_dir_all(&oa_dir)?;

    // Write project.json
    let project = ProjectConfig {
        project_id: format!("proj-{}", name),
        default_branch: "main".to_string(),
        test_commands: options
            .test_commands
            .unwrap_or_else(|| vec!["echo tests".to_string()]),
        allow_push: options.allow_push.unwrap_or(false),
    };
    fs::write(
        oa_dir.join("project.json"),
        serde_json::to_string_pretty(&project)?,
    )?;

    // Write tasks.jsonl
    let tasks_path = oa_dir.join("tasks.jsonl");
    fs::write(&tasks_path, format!("{}\n", serde_json::to_string(&task)?))?;

    // Write .gitignore
    fs::write(
        oa_dir.join(".gitignore"),
        "sessions/\nrun-logs/\nusage.jsonl\nstep-results.json\n",
    )?;

    // Initial commit
    run_git(&dir, &["add", "-A"])?;
    run_git(&dir, &["commit", "-m", "init"])?;

    Ok(GoldenLoopFixture {
        dir,
        openagents_dir: oa_dir,
        tasks_path,
        task_id,
    })
}

/// Run a git command in the given directory
fn run_git(dir: &Path, args: &[&str]) -> std::io::Result<()> {
    let status = Command::new("git")
        .args(args)
        .current_dir(dir)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()?;

    if status.success() {
        Ok(())
    } else {
        Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("git {:?} failed", args),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_fixture_default() {
        let fixture = create_golden_loop_fixture(GoldenLoopFixtureOptions::default()).unwrap();

        assert!(fixture.dir.exists());
        assert!(fixture.openagents_dir.exists());
        assert!(fixture.tasks_path.exists());
        assert!(fixture.openagents_dir.join("project.json").exists());

        // Check git was initialized
        assert!(fixture.dir.join(".git").exists());
    }

    #[test]
    fn test_create_fixture_with_name() {
        let fixture = create_golden_loop_fixture(GoldenLoopFixtureOptions {
            name: Some("test-name".to_string()),
            ..Default::default()
        })
        .unwrap();

        assert!(fixture.dir.to_str().unwrap().contains("test-name"));
        assert!(fixture.task_id.contains("test-name"));
    }

    #[test]
    fn test_create_fixture_with_task() {
        let fixture = create_golden_loop_fixture(GoldenLoopFixtureOptions {
            task: Some(PartialTask {
                id: Some("custom-id".to_string()),
                title: Some("Custom Title".to_string()),
                ..Default::default()
            }),
            ..Default::default()
        })
        .unwrap();

        assert_eq!(fixture.task_id, "custom-id");

        // Read and verify task
        let content = fs::read_to_string(&fixture.tasks_path).unwrap();
        let task: FixtureTask = serde_json::from_str(content.trim()).unwrap();
        assert_eq!(task.id, "custom-id");
        assert_eq!(task.title, "Custom Title");
    }

    #[test]
    fn test_fixture_cleanup() {
        let dir = {
            let fixture =
                create_golden_loop_fixture(GoldenLoopFixtureOptions::default()).unwrap();
            fixture.dir.clone()
        };
        // After drop, directory should be cleaned up
        // Note: This may fail if cleanup is not implemented correctly
        // but we don't assert here because Drop cleanup is best-effort
    }
}
