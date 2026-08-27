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
                match done {
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
                }
            }
            (Isolation::Directory, _) => match tokio::fs::remove_dir_all(&self.path).await {
                Ok(()) => None,
                Err(error) => Some(format!("could not remove {}: {error}", self.path.display())),
            },
            _ => None,
        }
    }
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
                })
            }
        }
    }
}

/// The root of the checkout `cwd` is in, or nothing if it is not in one.
async fn git_toplevel(cwd: &Path) -> Option<PathBuf> {
    let out = Command::new("git")
        .arg("-C")
        .arg(cwd)
        .args(["rev-parse", "--show-toplevel"])
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
