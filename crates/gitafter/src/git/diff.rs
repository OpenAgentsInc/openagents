//! Git diff and status operations

use anyhow::Result;
use git2::{Diff, DiffOptions, Repository, Status, StatusOptions};
use std::path::Path;

/// Represents a file change in the working directory
///
/// # Examples
///
/// ```no_run
/// use gitafter::git::{get_status, FileStatus};
/// use std::path::Path;
///
/// # fn example() -> anyhow::Result<()> {
/// let repo_path = Path::new("/path/to/repo");
/// let changes = get_status(repo_path)?;
///
/// for change in changes {
///     match change.status {
///         FileStatus::Modified => println!("Modified: {}", change.path),
///         FileStatus::Untracked => println!("Untracked: {}", change.path),
///         _ => println!("{:?}: {}", change.status, change.path),
///     }
/// }
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct FileChange {
    pub path: String,
    pub status: FileStatus,
}

/// Status of a file in the working directory
#[derive(Debug, Clone, PartialEq)]
pub enum FileStatus {
    Untracked,
    Modified,
    Added,
    Deleted,
    Renamed,
    Conflicted,
}

/// Get the status of all files in the working directory
pub fn get_status(repo_path: &Path) -> Result<Vec<FileChange>> {
    let repo = Repository::open(repo_path)?;

    let mut opts = StatusOptions::new();
    opts.include_untracked(true);
    opts.recurse_untracked_dirs(true);

    let statuses = repo.statuses(Some(&mut opts))?;

    let mut changes = Vec::new();
    for entry in statuses.iter() {
        let path = entry.path().unwrap_or("").to_string();

        let status = match entry.status() {
            s if s.contains(Status::WT_NEW) => FileStatus::Untracked,
            s if s.contains(Status::WT_MODIFIED) => FileStatus::Modified,
            s if s.contains(Status::INDEX_NEW) => FileStatus::Added,
            s if s.contains(Status::WT_DELETED) => FileStatus::Deleted,
            s if s.contains(Status::WT_RENAMED) => FileStatus::Renamed,
            s if s.contains(Status::CONFLICTED) => FileStatus::Conflicted,
            _ => continue,
        };

        changes.push(FileChange { path, status });
    }

    Ok(changes)
}

/// Generate a diff between two commits
pub fn diff_commits(repo_path: &Path, old_commit_id: &str, new_commit_id: &str) -> Result<String> {
    let repo = Repository::open(repo_path)?;

    let old_oid = git2::Oid::from_str(old_commit_id)?;
    let new_oid = git2::Oid::from_str(new_commit_id)?;

    let old_commit = repo.find_commit(old_oid)?;
    let new_commit = repo.find_commit(new_oid)?;

    let old_tree = old_commit.tree()?;
    let new_tree = new_commit.tree()?;

    let diff = repo.diff_tree_to_tree(Some(&old_tree), Some(&new_tree), None)?;

    format_diff(&diff)
}

/// Generate a diff of uncommitted changes
pub fn diff_working_directory(repo_path: &Path) -> Result<String> {
    let repo = Repository::open(repo_path)?;

    let head = repo.head()?;
    let tree = head.peel_to_tree()?;

    let mut opts = DiffOptions::new();
    let diff = repo.diff_tree_to_workdir_with_index(Some(&tree), Some(&mut opts))?;

    format_diff(&diff)
}

/// Generate a patch from a commit range
pub fn generate_patch(
    repo_path: &Path,
    base_commit_id: &str,
    head_commit_id: &str,
) -> Result<String> {
    diff_commits(repo_path, base_commit_id, head_commit_id)
}

/// Format a git2::Diff as a unified diff string
fn format_diff(diff: &Diff) -> Result<String> {
    let mut patch = String::new();

    diff.print(git2::DiffFormat::Patch, |_delta, _hunk, line| {
        let content = std::str::from_utf8(line.content()).unwrap_or("");

        let prefix = match line.origin() {
            '+' => "+",
            '-' => "-",
            ' ' => " ",
            '>' => ">",
            '<' => "<",
            'F' => "File: ",
            'H' => "Hunk: ",
            _ => "",
        };

        patch.push_str(prefix);
        patch.push_str(content);

        true
    })?;

    Ok(patch)
}

/// Get a summary of changes in a diff
pub fn diff_stats(repo_path: &Path, old_commit_id: &str, new_commit_id: &str) -> Result<DiffStats> {
    let repo = Repository::open(repo_path)?;

    let old_oid = git2::Oid::from_str(old_commit_id)?;
    let new_oid = git2::Oid::from_str(new_commit_id)?;

    let old_commit = repo.find_commit(old_oid)?;
    let new_commit = repo.find_commit(new_oid)?;

    let old_tree = old_commit.tree()?;
    let new_tree = new_commit.tree()?;

    let diff = repo.diff_tree_to_tree(Some(&old_tree), Some(&new_tree), None)?;
    let stats = diff.stats()?;

    Ok(DiffStats {
        files_changed: stats.files_changed(),
        insertions: stats.insertions(),
        deletions: stats.deletions(),
    })
}

/// Statistics about a diff
#[derive(Debug, Clone)]
pub struct DiffStats {
    pub files_changed: usize,
    pub insertions: usize,
    pub deletions: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use git2::Repository;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_repo() -> (TempDir, Repository) {
        let dir = TempDir::new().unwrap();
        let repo = Repository::init(dir.path()).unwrap();

        // Configure user
        let mut config = repo.config().unwrap();
        config.set_str("user.name", "Test User").unwrap();
        config.set_str("user.email", "test@example.com").unwrap();

        (dir, repo)
    }

    fn create_commit(repo: &Repository, _dir: &Path, message: &str) -> git2::Oid {
        let sig = repo.signature().expect("Failed to get signature");
        let tree_id = {
            let mut index = repo.index().expect("Failed to get index");
            index
                .add_all(["."].iter(), git2::IndexAddOption::DEFAULT, None)
                .expect("Failed to add files to index");
            index.write().expect("Failed to write index");
            index.write_tree().expect("Failed to write tree")
        };
        let tree = repo.find_tree(tree_id).expect("Failed to find tree");

        let parent_commit = if let Ok(head) = repo.head() {
            Some(head.peel_to_commit().unwrap())
        } else {
            None
        };

        let parents = if let Some(ref p) = parent_commit {
            vec![p]
        } else {
            vec![]
        };

        repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &parents)
            .unwrap()
    }

    #[test]
    fn test_get_status_empty() {
        let (_dir, repo) = create_test_repo();
        let status = get_status(repo.path()).unwrap();
        assert_eq!(status.len(), 0);
    }

    #[test]
    fn test_get_status_untracked() {
        let (dir, repo) = create_test_repo();

        // Create a new file
        fs::write(dir.path().join("test.txt"), "hello").unwrap();

        let status = get_status(repo.path()).unwrap();
        assert_eq!(status.len(), 1);
        assert_eq!(status[0].status, FileStatus::Untracked);
    }

    #[test]
    fn test_diff_working_directory() {
        let (dir, repo) = create_test_repo();

        // Create initial file and commit
        fs::write(dir.path().join("test.txt"), "hello\n").unwrap();
        create_commit(&repo, dir.path(), "Initial");

        // Modify the file
        fs::write(dir.path().join("test.txt"), "hello\nworld\n").unwrap();

        let diff = diff_working_directory(repo.path()).unwrap();
        assert!(diff.contains("+world"));
    }

    #[test]
    fn test_generate_patch() {
        let (dir, repo) = create_test_repo();

        // Create initial file and commit
        fs::write(dir.path().join("test.txt"), "line1\n").unwrap();
        let c1 = create_commit(&repo, dir.path(), "First commit");

        // Modify and commit again
        fs::write(dir.path().join("test.txt"), "line1\nline2\n").unwrap();
        let c2 = create_commit(&repo, dir.path(), "Second commit");

        let patch = generate_patch(repo.path(), &c1.to_string(), &c2.to_string()).unwrap();

        assert!(patch.contains("+line2"));
    }
}
