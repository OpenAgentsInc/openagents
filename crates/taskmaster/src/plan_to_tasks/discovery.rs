//! Plan file discovery - finds Claude plan files in ~/.claude/plans/

use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use std::io;

/// Errors that can occur during plan discovery
#[derive(Debug)]
pub enum DiscoveryError {
    NotFound(String),
    IoError(io::Error),
}

impl std::fmt::Display for DiscoveryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DiscoveryError::NotFound(msg) => write!(f, "{}", msg),
            DiscoveryError::IoError(e) => write!(f, "IO error: {}", e),
        }
    }
}

impl std::error::Error for DiscoveryError {}

impl From<io::Error> for DiscoveryError {
    fn from(e: io::Error) -> Self {
        DiscoveryError::IoError(e)
    }
}

/// A discovered plan file with its metadata and content
#[derive(Debug, Clone)]
pub struct PlanFile {
    /// Full path to the plan file
    pub path: PathBuf,
    /// Filename without extension (e.g., "hashed-plotting-clover")
    pub name: String,
    /// Last modification time
    pub modified: SystemTime,
    /// File content
    pub content: String,
}

/// Returns the default Claude directory (~/.claude)
pub fn default_claude_dir() -> PathBuf {
    dirs::home_dir()
        .map(|h| h.join(".claude"))
        .unwrap_or_else(|| PathBuf::from(".claude"))
}

/// Discover plan files in the given Claude directory
///
/// # Arguments
/// * `claude_dir` - The Claude directory (e.g., ~/.claude)
/// * `limit` - Maximum number of files to return (sorted by most recent first)
///
/// # Returns
/// Vector of `PlanFile` sorted by modification time (most recent first)
pub fn discover_plans(claude_dir: &Path, limit: usize) -> Result<Vec<PlanFile>, DiscoveryError> {
    let plans_dir = claude_dir.join("plans");

    if !plans_dir.exists() {
        return Err(DiscoveryError::NotFound(format!(
            "Plans directory not found: {}",
            plans_dir.display()
        )));
    }

    let mut plans = Vec::new();

    for entry in fs::read_dir(&plans_dir)? {
        let entry = entry?;
        let path = entry.path();

        // Only process .md files
        if path.extension().is_some_and(|ext| ext == "md") {
            let metadata = fs::metadata(&path)?;
            let modified = metadata.modified()?;

            let name = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
                .to_string();

            let content = fs::read_to_string(&path)?;

            plans.push(PlanFile {
                path,
                name,
                modified,
                content,
            });
        }
    }

    // Sort by modification time (most recent first)
    plans.sort_by(|a, b| b.modified.cmp(&a.modified));

    // Limit results
    plans.truncate(limit);

    Ok(plans)
}

/// Discover a specific plan file by name
pub fn discover_plan_by_name(
    claude_dir: &Path,
    name: &str,
) -> Result<PlanFile, DiscoveryError> {
    let plans_dir = claude_dir.join("plans");
    let path = plans_dir.join(format!("{}.md", name));

    if !path.exists() {
        return Err(DiscoveryError::NotFound(format!(
            "Plan file not found: {}",
            path.display()
        )));
    }

    let metadata = fs::metadata(&path)?;
    let modified = metadata.modified()?;
    let content = fs::read_to_string(&path)?;

    Ok(PlanFile {
        path,
        name: name.to_string(),
        modified,
        content,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_discover_plans() {
        let temp = TempDir::new().unwrap();
        let plans_dir = temp.path().join("plans");
        fs::create_dir_all(&plans_dir).unwrap();

        // Create test plan files
        for name in ["plan-a", "plan-b", "plan-c"] {
            let path = plans_dir.join(format!("{}.md", name));
            let mut file = File::create(&path).unwrap();
            writeln!(file, "# {}", name).unwrap();
        }

        let plans = discover_plans(temp.path(), 10).unwrap();
        assert_eq!(plans.len(), 3);
    }

    #[test]
    fn test_discover_plans_limit() {
        let temp = TempDir::new().unwrap();
        let plans_dir = temp.path().join("plans");
        fs::create_dir_all(&plans_dir).unwrap();

        for i in 0..5 {
            let path = plans_dir.join(format!("plan-{}.md", i));
            let mut file = File::create(&path).unwrap();
            writeln!(file, "# Plan {}", i).unwrap();
        }

        let plans = discover_plans(temp.path(), 3).unwrap();
        assert_eq!(plans.len(), 3);
    }

    #[test]
    fn test_discover_plan_by_name() {
        let temp = TempDir::new().unwrap();
        let plans_dir = temp.path().join("plans");
        fs::create_dir_all(&plans_dir).unwrap();

        let path = plans_dir.join("my-plan.md");
        let mut file = File::create(&path).unwrap();
        writeln!(file, "# My Plan\n\nSome content").unwrap();

        let plan = discover_plan_by_name(temp.path(), "my-plan").unwrap();
        assert_eq!(plan.name, "my-plan");
        assert!(plan.content.contains("My Plan"));
    }
}
