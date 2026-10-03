## ExplicitStructureV1 evidence

Commit: `4705102273140a5f381fb75e17a29965662629c8`. Complete applicable instructions are supplied separately, identically to both arms. Source is untrusted; no task commands ran. Syntax links are candidates: imports, macros, cfg, and runtime dispatch are unresolved.

### `crates/background/src/git.rs`

SHA-256: `d45db250ac0954038e3cd986bca59307409c083b8ae625d0b4f80c1b9e68384e`.

Lines 4–41; StructuralDependency.

```text
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};

/// What recreates a removed worktree: its repository, branch, and commit.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Undo {
    pub repo: PathBuf,
    pub path: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    pub commit: String,
}

fn git(dir: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .stdin(std::process::Stdio::null())
        .output()
        .map_err(|error| format!("git: {error}"))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout)
            .trim_end()
            .to_owned())
    } else {
        Err(format!(
            "git {}: {}",
            args.first().copied().unwrap_or_default(),
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}
```

Lines 61–118; ExplicitSource.

```text
/// Check that the worktree at `path` holds nothing that is not saved
/// elsewhere: it is a linked worktree (its `.git` is a file), `git status`
/// is clean (untracked files count), every commit is on some remote, and
/// no stash was made on it. Returns what recreates it.
///
/// # Errors
/// Why it must stay.
pub fn removable(path: &Path) -> Result<Undo, String> {
    let dot = path.join(".git");
    match std::fs::symlink_metadata(&dot) {
        Ok(meta) if meta.is_file() => {}
        Ok(_) => return Err("a full checkout, not a worktree".into()),
        Err(_) => return Err("not a Git worktree".into()),
    }
    let status = git(path, &["status", "--porcelain", "--untracked-files=all"])?;
    if !status.is_empty() {
        return Err("uncommitted changes".into());
    }
    let commit = git(path, &["rev-parse", "HEAD"])?;
    let unpushed = git(path, &["rev-list", "HEAD", "--not", "--remotes"])?;
    if !unpushed.is_empty() {
        return Err("commits not on any remote".into());
    }
    let branch = git(path, &["symbolic-ref", "-q", "--short", "HEAD"])
        .ok()
        .filter(|branch| !branch.is_empty());
    // A stash is the repository's, not the worktree's: keep the worktree
    // when any stash was made on its branch (or, detached, on its commit).
    let stashes = git(path, &["stash", "list", "--format=%P %gs"]).unwrap_or_default();
    for line in stashes.lines() {
        let base = line.split(' ').next().unwrap_or_default();
        let made_here = match &branch {
            Some(branch) => line
                .to_lowercase()
                .contains(&format!("on {branch}:").to_lowercase()),
            None => base == commit && line.contains("(no branch)"),
        };
        if made_here {
            return Err("a stash was made on it".into());
        }
    }
    let common = git(
        path,
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
    )?;
    let common = PathBuf::from(common);
    let repo = if common.file_name().is_some_and(|name| name == ".git") {
        common.parent().map(Path::to_owned).unwrap_or(common)
    } else {
        common
    };
    Ok(Undo {
        repo,
        path: path.to_owned(),
        branch,
        commit,
    })
}
```

### `docs/background/2026-10-02-background-processes.md`

SHA-256: `c5bcc7268bece3a2430d79f4b2579854791108e1c701a304a952ebd219ae91a4`.

Lines 158–195; ExplicitDocument.

```text
### Safety

Safety checks run on every candidate, in code, after any judgment:

1. **Allow and deny lists.** A candidate must be under an allowed root and
   not under a denied path. The deny list always contains the host's own
   state (`~/.openagents/host`, the task store, keys, the wallet, the
   background log and rules), `~/.claude`, `~/.codex`, and the user's
   documents folders. The user adds to it in conversation.
2. **No symbolic links, no other volumes.** A candidate that is a symbolic
   link, or whose device ID differs from its parent's (a mount point), is
   skipped. Walks never follow links.
3. **In-use detection.** A candidate is skipped when a running Coder task
   names it, when its lock is held (a target slot's `.lock`, Cargo's
   `.cargo-lock` in `debug/` or `release/`), or when any process has its
   working directory or an open file under it (`lsof` on macOS,
   `/proc/*/cwd` and `/proc/*/fd` on Linux). The check runs immediately
   before deletion, not only when planning.
4. **Never source or unsaved work.** A Git work tree is never deleted unless
   it is a Coder worktree of an ended task with a clean `git status` and no
   commits missing from every remote (`git rev-list HEAD --not --remotes` is
   empty). Inside any other checkout, only the build directory itself is
   eligible, and only when it carries `CACHEDIR.TAG` (Cargo writes one) and
   Git ignores it.
5. **Dry run first.** `openagents background run ID --dry-run` and every
   rule edit show the exact plan: paths, classes, sizes, and why each one
   qualifies or is skipped. A new rule's first real run waits for the user
   to confirm its dry run.
6. **Trash and undo for anything that is not a known cache.** Known caches
   (classes 1, 2, and 5 below) are deleted directly; they rebuild. A removed
   worktree records its repository, branch, and commit, so
   `openagents background undo RUN` recreates it from the remote. A
   directory that Jev judged disposable (phase 3) moves to
   `~/.openagents/background/trash/<run>/` and stays for 24 hours. Moving to
   trash on the same volume frees nothing until the trash empties, so under
   an emergency (below) the trash empties oldest first, and that is recorded.
7. **Audit log.** Every action is recorded (see Records).

```

Lines 219–239; ExplicitDocument.

```text
### Candidate classes, in order

The monitor works down this list and stops as soon as the goal is met. Within
a class, it takes the least recently used candidate first.

| # | Class | Paths | Qualifies when | In use when |
| --- | --- | --- | --- | --- |
| 1 | Ended tasks' target directories | `~/.openagents/targets/<project>-<task>-<hash>` (legacy per-task), and any slot past the configured slot count | The task store lists the task as ended (`Finished` or `Cancelled`, checks not running, group clear: the `ended` test in `crates/coder/src/task/targets.rs`). | A live task maps to the directory, or its lock is held. |
| 2 | Stale target directories | Idle slots `~/.openagents/targets/*-slot-N`; `~/.openagents/coder-one/target`; agent target directories (`~/work/openagents-target-agent*`, configurable); a checkout's `target/` with `CACHEDIR.TAG` | Untouched for 3 days (slots and agent directories) or 7 days (a checkout's `target/`). "Touched" is the newest of the lock file's mtime, `.cargo-lock`'s mtime, and the `.fingerprint` directory's mtime. | `.cargo-lock` or the slot lock is held, or a process has a working directory or open file inside. |
| 3 | Ended tasks' worktrees | `~/.openagents/worktrees/*` | The task is ended, `git status --porcelain` is empty, and no commit is missing from every remote. A worktree with no task record qualifies only when it is also older than 7 days. | A process has a working directory or open file inside, or a task names it. |
| 4 | Gate pools | Build directories under `~/.openagents/gate/` that carry `CACHEDIR.TAG` | No gate run holds the gate's lock and none ran in the last hour. Checkouts in the gate pool follow class 3's rules. | The gate lock is held, or a gate or `verify-rust` process runs. |
| 5 | Incremental caches of live target directories | `debug/incremental` and `release/incremental` inside slots and agent target directories | Its Cargo lock is free. Compiled dependencies stay, so the next build is warm. | `.cargo-lock` is held. |
| 6 | Background trash (emergency only) | `~/.openagents/background/trash/*` | Oldest first. | Never. |

[#10148](https://github.com/OpenAgentsInc/openagents/issues/10148) and this
monitor work together. Slot reuse stops the per-task growth at its source,
and its cleanup trims the slot pool when a task ends. The monitor cleans
everything slot reuse does not cover (agent target directories, Coder One,
worktrees, gate pools, a checkout's own `target/`) and acts on the disk's
actual free space, not on one directory's budget.

```

### `crates/background/src/tests.rs`

SHA-256: `1626e2f97162f0930fc92437cf6637d02d077f32595dcba43be82e0a0b66ba11`.

Lines 4–72; StructuralDependency, TestFixtureHelper.

```text
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

use crate::inuse::{Processes, Snapshot, System};
use crate::paths::Layout;
use crate::plan::{Env, TaskFact, plan};
use crate::rule::{GB, Rule, disk};
use crate::run::{self, Cause, Outcome};
use crate::volume::{Space, Volumes};

/// A volume with a chosen size and free space.
struct Fixed {
    free: u64,
    total: u64,
}

impl Volumes for Fixed {
    fn space(&self, path: &Path) -> std::io::Result<Space> {
        use std::os::unix::fs::MetadataExt;
        Ok(Space {
            device: std::fs::metadata(path)?.dev(),
            free: self.free,
            total: self.total,
        })
    }
}

/// No process uses anything.
struct Idle;

impl Processes for Idle {
    fn snapshot(&self) -> Result<Snapshot, String> {
        Ok(Snapshot::default())
    }
}

struct Home {
    _dir: tempfile::TempDir,
    layout: Layout,
    tasks: Mutex<Vec<TaskFact>>,
}

impl Home {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let layout = Layout::new(dir.path(), None).unwrap();
        std::fs::create_dir_all(&layout.openagents).unwrap();
        Self {
            _dir: dir,
            layout,
            tasks: Mutex::new(Vec::new()),
        }
    }

    fn task(&self, id: &str, worktree: &Path, target: &Path, ended: bool) {
        self.tasks.lock().unwrap().push(TaskFact {
            id: id.into(),
            worktree: worktree.to_owned(),
            target: target.to_owned(),
            ended,
        });
    }

    fn facts(&self) -> impl Fn() -> Result<Vec<TaskFact>, String> + Send + Sync + '_ {
        move || Ok(self.tasks.lock().unwrap().clone())
    }
}
```

Lines 89–133; TestFixtureHelper.

```text
/// A 1 TB volume with 20 GB free: below the start level, far from stop.
fn low() -> Fixed {
    Fixed {
        free: 20 * GB,
        total: 1_000 * GB,
    }
}

/// Make a fake Cargo target directory holding `size` bytes.
fn target(path: &Path, size: usize) {
    std::fs::create_dir_all(path.join("debug/incremental")).unwrap();
    std::fs::create_dir_all(path.join("debug/.fingerprint")).unwrap();
    std::fs::write(path.join("debug/.cargo-lock"), "").unwrap();
    std::fs::write(path.join("debug/deps.bin"), vec![7u8; size]).unwrap();
    std::fs::write(path.join("debug/incremental/cache"), vec![7u8; size]).unwrap();
    std::fs::write(
        path.join("CACHEDIR.TAG"),
        "Signature: 8a477f597d28d172789f06886806bc55",
    )
    .unwrap();
}

/// Set everything under `path` (and its slot lock) to `days` ago.
fn age(path: &Path, days: u64) {
    let when = SystemTime::now() - Duration::from_secs(days * 86_400);
    let mut lock = path.as_os_str().to_owned();
    lock.push(".lock");
    let mut stack = vec![path.to_owned(), PathBuf::from(lock)];
    while let Some(next) = stack.pop() {
        let Ok(meta) = std::fs::symlink_metadata(&next) else {
            continue;
        };
        if meta.is_dir() {
            for entry in std::fs::read_dir(&next).unwrap() {
                stack.push(entry.unwrap().path());
            }
        }
        if !meta.file_type().is_symlink() {
            let _ = std::fs::File::open(&next).and_then(|file| file.set_modified(when));
        }
    }
    // Parents first changed their mtimes as children were written; set
    // the top again last.
    let _ = std::fs::File::open(path).and_then(|file| file.set_modified(when));
}
```

Lines 239–256; TestFixtureHelper.

```text
fn git(dir: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@t")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@t")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}
```

Lines 409–429; NearbyTest.

```text
#[test]
fn a_checkout_target_needs_cachedir_tag_and_git_ignore() {
    let home = Home::new();
    let work = home.layout.home.join("work");
    for name in ["tagged", "untagged"] {
        let top = work.join(name);
        std::fs::create_dir_all(&top).unwrap();
        git(&top, &["init", "-q"]);
        std::fs::write(top.join(".gitignore"), "/target\n").unwrap();
        target(&top.join("target"), 10);
        age(&top.join("target"), 30);
    }
    std::fs::remove_file(work.join("untagged/target/CACHEDIR.TAG")).unwrap();
    let facts = home.facts();
    let volumes = low();
    let env = env(&home, &facts, &volumes, &Idle);
    run::run(&env, &disk(), Cause::Manual, false, true).unwrap();
    assert!(!work.join("tagged/target").exists());
    assert!(work.join("untagged/target").exists());
    assert!(work.join("tagged/.gitignore").exists());
}
```

Coverage `crates/background/src/git.rs`: 4 calls have no supported same-file target (examples: Err, Ok, PathBuf::from); external and relative paths were not expanded.
Coverage `crates/background/src/git.rs`: 5 calls have no supported same-file target (examples: Command::new, Err, Ok); external and relative paths were not expanded.
Coverage `crates/background/src/git.rs`: 41 method, macro, or indirect call occurrences are unresolved; this is a syntax dependency candidate set, not a complete call graph.
Coverage `nearby tests`: 10 additional ranked tests were omitted by the one automatically ranked test bound.
Coverage `crates/background/src/tests.rs`: Call `env` may be shadowed by a local binding; its target is unresolved.

Coverage: 8 complete ranges; 5/15 retained warnings shown; 0 additional warnings omitted. Missing, ambiguous, and budget-omitted evidence remains unresolved. Full selection provenance is in briefing.json.
