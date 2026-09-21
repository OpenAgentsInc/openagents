//! Inspect retained scratch commits without executing their hooks or tests.

use std::path::{Component, Path};
use std::process::Command;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use supervise::{Job, Limits};

/// Mechanical patch evidence. Passing this check does not establish correctness.
#[derive(Debug, Serialize, Deserialize)]
pub struct Artifact {
    pub base: String,
    pub seed: String,
    pub tip: String,
    pub changed_paths: Vec<String>,
    pub diff_digest: String,
    pub tests_verified: bool,
}

/// Require normalized repository-relative paths before comparing ownership.
pub fn relative(path: &str) -> Result<(), String> {
    let p = Path::new(path);
    if path.is_empty()
        || path.contains(['\0', '\n', '\r', '\\'])
        || p.is_absolute()
        || p.components().any(|c| !matches!(c, Component::Normal(_)))
        || path
            .split('/')
            .any(|s| s.is_empty() || s == "." || s == ".." || s == ".git" || s == ".coder-git")
    {
        return Err("artifact paths must be normalized repository-relative paths".into());
    }
    Ok(())
}

fn owned(path: &str, allowed: &[String]) -> bool {
    allowed.iter().any(|a| {
        path == a
            || path
                .strip_prefix(a)
                .is_some_and(|tail| tail.starts_with('/'))
    })
}

async fn observe(worktree: &Path, args: &[&str]) -> Result<String, String> {
    let mut command = Command::new("git");
    command
        .current_dir(worktree)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .env_remove("GIT_OBJECT_DIRECTORY")
        .env_remove("GIT_ALTERNATE_OBJECT_DIRECTORIES")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .args([
            "--no-pager",
            "--no-replace-objects",
            "--git-dir=.coder-git",
            "--work-tree=.",
            "-c",
            "core.fsmonitor=false",
            "-c",
            "core.hooksPath=/dev/null",
        ])
        .args(args);
    let result = Job::from_command(command)
        .bounded(Limits::within(Duration::from_secs(30)).keeping(8 * 1024 * 1024))
        .run()
        .await;
    if !result.ending.success() || result.truncated() {
        return Err("scratch Git observation failed or exceeded its output bound".into());
    }
    Ok(result.stdout.text)
}

/// Inspect an ended writing attempt before offering its patch for review.
///
/// The supplied worktree must be the retained path from the dispatch record.
/// This function does not run code from the artifact, integrate it, or close an issue.
pub async fn inspect(
    repository: &Path,
    worktree: &Path,
    base: &str,
    allowed: &[String],
) -> Result<Artifact, String> {
    if base.len() != 40 || !base.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("artifact base must be a full commit ID".into());
    }
    if allowed.is_empty() {
        return Err("writing acceptance requires explicit owned paths".into());
    }
    for path in allowed {
        relative(path)?;
    }
    let parent = repository
        .join(".coder/worktrees")
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let canonical = worktree.canonicalize().map_err(|e| e.to_string())?;
    if canonical.parent() != Some(parent.as_path()) {
        return Err("artifact is not a direct retained worktree of this repository".into());
    }
    let metadata =
        std::fs::symlink_metadata(canonical.join(".coder-git")).map_err(|e| e.to_string())?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err("scratch Git metadata must be a local directory".into());
    }
    let status = observe(
        &canonical,
        &["status", "--porcelain=v1", "--untracked-files=all"],
    )
    .await?;
    if !status.is_empty() {
        return Err("scratch artifact has uncommitted changes".into());
    }
    let roots = observe(&canonical, &["rev-list", "--max-parents=0", "HEAD"]).await?;
    let roots: Vec<_> = roots.lines().collect();
    if roots.len() != 1 {
        return Err("scratch history must have one seeded root".into());
    }
    let seed = roots[0].to_string();
    let seeded_tree = observe(&canonical, &["rev-parse", &format!("{seed}^{{tree}}")]).await?;
    let base_tree = crate::git(repository, &["rev-parse", &format!("{base}^{{tree}}")]).await?;
    if seeded_tree.trim() != base_tree.trim() {
        return Err("scratch root does not match the recorded task base".into());
    }
    let tip = observe(&canonical, &["rev-parse", "HEAD"])
        .await?
        .trim()
        .to_string();
    let paths = observe(
        &canonical,
        &[
            "diff",
            "--no-ext-diff",
            "--no-textconv",
            "--no-renames",
            "--name-only",
            "-z",
            &seed,
            &tip,
            "--",
        ],
    )
    .await?;
    let changed_paths: Vec<String> = paths
        .split('\0')
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect();
    if changed_paths.is_empty() {
        return Err("writing task produced no committed patch".into());
    }
    for path in &changed_paths {
        relative(path)?;
        if !owned(path, allowed) {
            return Err(format!("artifact changed an unowned path: {path}"));
        }
    }
    let diff = observe(
        &canonical,
        &[
            "diff",
            "--no-ext-diff",
            "--no-textconv",
            "--no-renames",
            "--binary",
            &seed,
            &tip,
            "--",
        ],
    )
    .await?;
    Ok(Artifact {
        base: base.into(),
        seed,
        tip,
        changed_paths,
        diff_digest: atif::digest(&serde_json::json!(diff)),
        tests_verified: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ownership_compares_path_components() {
        let owned_paths = vec!["crates/coder".into()];
        assert!(owned("crates/coder/src/lib.rs", &owned_paths));
        assert!(!owned("crates/coder-boundary/src/lib.rs", &owned_paths));
        assert!(!owned("crates/elsewhere", &owned_paths));
    }

    #[test]
    fn path_aliases_and_git_metadata_are_refused() {
        for path in [
            "",
            "/etc/passwd",
            "a/../b",
            "./a",
            "a//b",
            "a/",
            ".git/config",
            "a/.coder-git/config",
            "a\nb",
        ] {
            assert!(relative(path).is_err(), "{path}");
        }
        assert!(relative("crates/coder/src/lib.rs").is_ok());
    }

    #[tokio::test]
    async fn scratch_acceptance_checks_base_cleanliness_and_owned_diff() {
        let repository = tempfile::tempdir().unwrap();
        let repo = repository.path();
        crate::git(repo, &["init", "--quiet"]).await.unwrap();
        std::fs::write(repo.join("a.rs"), "old\n").unwrap();
        std::fs::write(repo.join("b.rs"), "other\n").unwrap();
        crate::git(repo, &["add", "a.rs", "b.rs"]).await.unwrap();
        let commit = [
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "--quiet",
            "-m",
            "Fixture",
        ];
        crate::git(repo, &commit).await.unwrap();
        let base = crate::git(repo, &["rev-parse", "HEAD"])
            .await
            .unwrap()
            .trim()
            .to_string();
        let worktree = repo.join(".coder/worktrees/fixture");
        std::fs::create_dir_all(&worktree).unwrap();
        std::fs::copy(repo.join("a.rs"), worktree.join("a.rs")).unwrap();
        std::fs::copy(repo.join("b.rs"), worktree.join("b.rs")).unwrap();
        observe(&worktree, &["init", "--quiet"]).await.unwrap();
        std::fs::write(worktree.join(".coder-git/info/exclude"), ".coder-git/\n").unwrap();
        observe(&worktree, &["add", "a.rs", "b.rs"]).await.unwrap();
        observe(&worktree, &commit).await.unwrap();
        let allowed = vec!["a.rs".into()];
        assert!(
            inspect(repo, &worktree, &base, &allowed)
                .await
                .unwrap_err()
                .contains("no committed patch")
        );
        std::fs::write(worktree.join("a.rs"), "new\n").unwrap();
        assert!(
            inspect(repo, &worktree, &base, &allowed)
                .await
                .unwrap_err()
                .contains("uncommitted")
        );
        observe(&worktree, &["add", "a.rs"]).await.unwrap();
        observe(&worktree, &commit).await.unwrap();
        let report = inspect(repo, &worktree, &base, &allowed).await.unwrap();
        assert_eq!(report.changed_paths, ["a.rs"]);
        assert!(!report.tests_verified);
        assert_ne!(report.tip, report.seed);
        assert!(
            inspect(repo, &worktree, &base, &["b.rs".into()])
                .await
                .unwrap_err()
                .contains("unowned")
        );
        std::fs::write(repo.join("a.rs"), "changed base\n").unwrap();
        crate::git(repo, &["add", "a.rs"]).await.unwrap();
        crate::git(repo, &commit).await.unwrap();
        let changed = crate::git(repo, &["rev-parse", "HEAD"]).await.unwrap();
        assert!(
            inspect(repo, &worktree, changed.trim(), &allowed)
                .await
                .unwrap_err()
                .contains("recorded task base")
        );
    }
}
