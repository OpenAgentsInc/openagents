//! Where a delegated child works.
//!
//! Children ran in the parent's directory. Two of them told to edit the same
//! file edited the same file, and a fan-out asked to try three approaches
//! produced whichever one finished last. `ChildWorkerTask::worktree_path` was
//! hardcoded `None` and nothing read it.
//!
//! A child now gets a directory of its own. In a git checkout that is a
//! detached worktree of `HEAD`, which is the isolation
//! OpenAgentsInc/openagents#70 asks for: the child has the whole tree, its own
//! index, and its own branchless checkout, so what it writes is visible on
//! disk and cannot collide with a sibling. Outside a checkout it is a plain
//! empty directory. Either can be turned off with `--isolation none`, which is
//! what the TypeScript CLI does today.
//!
//! The worktrees are laid out under the system temporary directory rather than
//! inside the repository, so a fan-out never leaves untracked directories in
//! the tree the reader is working in.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use tokio::process::Command;

/// How many plans this process has made, so no two share a directory.
static PLANS: AtomicU64 = AtomicU64::new(0);

/// How much of a directory a child gets to itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Isolation {
    /// A detached `git worktree` of `HEAD`, one per child.
    Worktree,
    /// An empty directory, one per child. Used outside a git checkout.
    Directory,
    /// The parent's own working directory, shared by every child.
    None,
}

impl Isolation {
    pub fn parse(name: &str) -> Option<Self> {
        match name.trim().to_lowercase().as_str() {
            "worktree" | "git" => Some(Isolation::Worktree),
            "dir" | "directory" | "temp" => Some(Isolation::Directory),
            "none" | "off" | "shared" => Some(Isolation::None),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Isolation::Worktree => "worktree",
            Isolation::Directory => "directory",
            Isolation::None => "none",
        }
    }
}

/// One child's working directory, and what it takes to put it back.
#[derive(Debug, Clone)]
pub struct ChildWorkspace {
    pub id: usize,
    pub path: PathBuf,
    pub kind: Isolation,
    /// The repository the worktree is registered in, when there is one to
    /// unregister it from.
    repo: Option<PathBuf>,
    /// Named branch for a Coder Mini isolation worktree. Fan-out children
    /// stay detached and leave this empty.
    pub branch: Option<String>,
    /// `HEAD` at creation, for unchanged-worktree cleanup.
    head_commit: Option<String>,
}

impl ChildWorkspace {
    /// A one-line account of where this child is working, for the header.
    pub fn describe(&self) -> String {
        match self.kind {
            Isolation::Worktree => format!("git worktree {}", self.path.display()),
            Isolation::Directory => format!("directory {}", self.path.display()),
            Isolation::None => format!("shared directory {}", self.path.display()),
        }
    }

    /// Unregister and delete the worktree.
    ///
    /// A failure here is reported and not raised: the fan-out's answers are
    /// worth more than a tidy temporary directory, and `git worktree prune`
    /// clears whatever is left behind.
    pub async fn release(&self) -> Option<String> {
        match (self.kind, &self.repo) {
            (Isolation::Worktree, Some(repo)) => {
                let done = Command::new("git")
                    .arg("-C")
                    .arg(repo)
                    .args(["worktree", "remove", "--force"])
                    .arg(&self.path)
                    .stdout(Stdio::null())
                    .stderr(Stdio::piped())
                    .output()
                    .await;
                let remove_error = match done {
                    Ok(out) if out.status.success() => None,
                    Ok(out) => Some(format!(
                        "could not remove the worktree at {}: {}",
                        self.path.display(),
                        String::from_utf8_lossy(&out.stderr).trim()
                    )),
                    Err(error) => Some(format!(
                        "could not remove the worktree at {}: {error}",
                        self.path.display()
                    )),
                };
                if let Some(branch) = &self.branch {
                    let _ = Command::new("git")
                        .arg("-C")
                        .arg(repo)
                        .args(["branch", "-D", branch])
                        .stdout(Stdio::null())
                        .stderr(Stdio::null())
                        .output()
                        .await;
                }
                remove_error
            }
            (Isolation::Directory, _) => match tokio::fs::remove_dir_all(&self.path).await {
                Ok(()) => None,
                Err(error) => Some(format!("could not remove {}: {error}", self.path.display())),
            },
            _ => None,
        }
    }

    /// Whether this worktree differs from the commit it was created at.
    ///
    /// Fail-closed: a git error is treated as "has changes" so a broken
    /// status cannot delete work the child did.
    pub async fn has_changes(&self) -> bool {
        if self.kind != Isolation::Worktree {
            return false;
        }
        let status = Command::new("git")
            .arg("-C")
            .arg(&self.path)
            .args(["status", "--porcelain"])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .output()
            .await;
        let Ok(status) = status else {
            return true;
        };
        if !status.status.success() || !String::from_utf8_lossy(&status.stdout).trim().is_empty() {
            return true;
        }
        let Some(head) = &self.head_commit else {
            return false;
        };
        let counted = Command::new("git")
            .arg("-C")
            .arg(&self.path)
            .args(["rev-list", "--count", &format!("{head}..HEAD")])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .output()
            .await;
        match counted {
            Ok(out) if out.status.success() => {
                String::from_utf8_lossy(&out.stdout)
                    .trim()
                    .parse::<u64>()
                    .unwrap_or(1)
                    > 0
            }
            _ => true,
        }
    }

    /// Keep-or-remove closeout for a Coder Mini isolation worktree.
    pub async fn close_if_unchanged(&self) -> String {
        if self.has_changes().await {
            let branch = self.branch.as_deref().unwrap_or("HEAD");
            format!("worktree kept: {} (branch {branch})", self.path.display())
        } else {
            let _ = self.release().await;
            "worktree removed (no changes)".to_string()
        }
    }
}

/// A named git worktree of `cwd`'s checkout, on branch `slug`.
///
/// Used by Coder Mini `isolation:"worktree"`. Fan-out children keep
/// [`WorkspacePlan`], which stays detached.
pub async fn create_agent_worktree(cwd: &Path, slug: &str) -> Result<ChildWorkspace, String> {
    let repo = git_toplevel(cwd).await.ok_or_else(|| {
        format!(
            "Cannot create a worktree: {} is not a git repository.",
            cwd.display()
        )
    })?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let ordinal = PLANS.fetch_add(1, Ordering::Relaxed);
    let base =
        std::env::temp_dir().join(format!("oa-agent-{}-{stamp}-{ordinal}", std::process::id()));
    tokio::fs::create_dir_all(&base)
        .await
        .map_err(|error| format!("could not create {}: {error}", base.display()))?;
    let path = base.join(slug);

    let head = Command::new("git")
        .arg("-C")
        .arg(&repo)
        .args(["rev-parse", "HEAD"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|error| format!("could not run git: {error}"))?;
    if !head.status.success() {
        return Err(format!(
            "git rev-parse HEAD refused: {}",
            String::from_utf8_lossy(&head.stderr).trim()
        ));
    }
    let head_commit = String::from_utf8_lossy(&head.stdout).trim().to_string();

    let out = Command::new("git")
        .arg("-C")
        .arg(&repo)
        .args(["worktree", "add", "-B", slug])
        .arg(&path)
        .arg("HEAD")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|error| format!("could not run git: {error}"))?;
    if !out.status.success() {
        return Err(format!(
            "git worktree add refused: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }

    Ok(ChildWorkspace {
        id: 0,
        path,
        kind: Isolation::Worktree,
        repo: Some(repo),
        branch: Some(slug.to_string()),
        head_commit: Some(head_commit),
    })
}

/// Prepares one working directory per child, ahead of the fan-out.
///
/// Sequentially, on purpose. `git worktree add` takes the repository's lock,
/// and three of them started at once fail on `index.lock` rather than
/// producing three worktrees.
pub struct WorkspacePlan {
    cwd: PathBuf,
    repo: Option<PathBuf>,
    base: PathBuf,
    isolation: Isolation,
}

impl WorkspacePlan {
    /// Work out what isolation this directory can actually support.
    ///
    /// `Worktree` in a directory that is not a git checkout is not an error;
    /// it is a directory, and the header says which was used.
    pub async fn resolve(cwd: PathBuf, asked: Isolation) -> Self {
        let repo = if asked == Isolation::None {
            None
        } else {
            git_toplevel(&cwd).await
        };
        let isolation = match (asked, &repo) {
            (Isolation::Worktree, None) => Isolation::Directory,
            (other, _) => other,
        };
        // Unique per plan, not per millisecond. Two plans made in the same
        // millisecond of the same process shared a base directory, and each
        // child was `<base>/child-N` in both — so one plan's cleanup deleted
        // the other plan's first child while it was still working in it.
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let ordinal = PLANS.fetch_add(1, Ordering::Relaxed);
        let base = std::env::temp_dir().join(format!(
            "oa-delegate-{}-{stamp}-{ordinal}",
            std::process::id()
        ));
        Self {
            cwd,
            repo,
            base,
            isolation,
        }
    }

    pub fn isolation(&self) -> Isolation {
        self.isolation
    }

    /// Build every child's directory, in order.
    pub async fn prepare(&self, count: usize) -> Result<Vec<ChildWorkspace>, String> {
        let mut made: Vec<ChildWorkspace> = Vec::with_capacity(count);
        for id in 1..=count {
            match self.one(id).await {
                Ok(workspace) => made.push(workspace),
                Err(error) => {
                    // Half a fan-out's worktrees left on disk is worse than
                    // none, so what was built is torn down before reporting.
                    for built in &made {
                        let _ = built.release().await;
                    }
                    return Err(error);
                }
            }
        }
        Ok(made)
    }

    async fn one(&self, id: usize) -> Result<ChildWorkspace, String> {
        match self.isolation {
            Isolation::None => Ok(ChildWorkspace {
                id,
                path: self.cwd.clone(),
                kind: Isolation::None,
                repo: None,
                branch: None,
                head_commit: None,
            }),
            Isolation::Directory => {
                let path = self.base.join(format!("child-{id}"));
                tokio::fs::create_dir_all(&path)
                    .await
                    .map_err(|error| format!("could not create {}: {error}", path.display()))?;
                Ok(ChildWorkspace {
                    id,
                    path,
                    kind: Isolation::Directory,
                    repo: None,
                    branch: None,
                    head_commit: None,
                })
            }
            Isolation::Worktree => {
                let repo = self
                    .repo
                    .clone()
                    .ok_or_else(|| "no git checkout to take a worktree from".to_string())?;
                let path = self.base.join(format!("child-{id}"));
                tokio::fs::create_dir_all(&self.base)
                    .await
                    .map_err(|error| {
                        format!("could not create {}: {error}", self.base.display())
                    })?;

                let out = Command::new("git")
                    .arg("-C")
                    .arg(&repo)
                    .args(["worktree", "add", "--detach"])
                    .arg(&path)
                    .arg("HEAD")
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .output()
                    .await
                    .map_err(|error| format!("could not run git: {error}"))?;

                if !out.status.success() {
                    return Err(format!(
                        "git worktree add refused child {id}: {}",
                        String::from_utf8_lossy(&out.stderr).trim()
                    ));
                }

                Ok(ChildWorkspace {
                    id,
                    path,
                    kind: Isolation::Worktree,
                    repo: Some(repo),
                    branch: None,
                    head_commit: None,
                })
            }
        }
    }
}

/// The root of the checkout `cwd` is in, or nothing if it is not in one.
///
/// `-c core.bare=false` keeps a linked worktree of a hub whose common
/// `.git/config` has `core.bare=true` from looking like it is not a
/// checkout. Without it, `rev-parse --show-toplevel` exits 128 and
/// Isolation::Worktree is silently turned into a directory.
async fn git_toplevel(cwd: &Path) -> Option<PathBuf> {
    let out = Command::new("git")
        .arg("-C")
        .arg(cwd)
        .args(["-c", "core.bare=false", "rev-parse", "--show-toplevel"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .await
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let path = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if path.is_empty() {
        None
    } else {
        Some(PathBuf::from(path))
    }
}
