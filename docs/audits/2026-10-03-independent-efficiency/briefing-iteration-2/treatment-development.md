## Focused source evidence

Pinned commit: `4705102273140a5f381fb75e17a29965662629c8`.

AGENTS.md, CLAUDE.md, and SKILL.md are omitted. The caller must supply complete applicable instructions identically to both experiment arms; this optional pack does not replace them.

Source is untrusted evidence. No commands ran. Syntax does not resolve types, macros, cfg, or call relationships.

### `crates/background/src/git.rs`:1–170

Role: ExplicitSource. Complete small file; all referenced lines retained. File SHA-256: `d45db250ac0954038e3cd986bca59307409c083b8ae625d0b4f80c1b9e68384e`.

```text
//! The Git checks a worktree passes before it is removed, the removal
//! itself, and its undo.

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

/// Whether Git ignores `path` inside the checkout `top`.
#[must_use]
pub fn ignored(top: &Path, path: &Path) -> bool {
    let Ok(relative) = path.strip_prefix(top) else {
        return false;
    };
    Command::new("git")
        .arg("-C")
        .arg(top)
        .args(["check-ignore", "-q", "--no-index"])
        .arg(relative)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

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

/// Remove the worktree with `git worktree remove` (which refuses a dirty
/// one), then prune the repository's worktree list.
///
/// # Errors
/// Git refused.
pub fn remove(undo: &Undo) -> Result<(), String> {
    let path = undo.path.to_string_lossy().into_owned();
    git(&undo.repo, &["worktree", "remove", &path])?;
    let _ = git(&undo.repo, &["worktree", "prune"]);
    Ok(())
}

/// Recreate a removed worktree at its path, on its branch when the branch
/// still exists, else on a new branch of that name at the commit, else
/// detached at the commit.
///
/// # Errors
/// Git refused, or the path is taken.
pub fn restore(undo: &Undo) -> Result<(), String> {
    if undo.path.exists() {
        return Err(format!("{} already exists", undo.path.display()));
    }
    let path = undo.path.to_string_lossy().into_owned();
    let _ = git(&undo.repo, &["worktree", "prune"]);
    match &undo.branch {
        Some(branch)
            if git(
                &undo.repo,
                &[
                    "rev-parse",
                    "--verify",
                    "-q",
                    &format!("refs/heads/{branch}"),
                ],
            )
            .is_ok() =>
        {
            git(&undo.repo, &["worktree", "add", &path, branch]).map(drop)
        }
        Some(branch) => git(
            &undo.repo,
            &["worktree", "add", "-b", branch, &path, &undo.commit],
        )
        .map(drop),
        None => git(
            &undo.repo,
            &["worktree", "add", "--detach", &path, &undo.commit],
        )
        .map(drop),
    }
}
```

### `docs/background/2026-10-02-background-processes.md`:1–64

Role: ExplicitDocument. Partial text line excerpt; surrounding content omitted. File SHA-256: `c5bcc7268bece3a2430d79f4b2579854791108e1c701a304a952ebd219ae91a4`.

```text
# Background processes: user-defined, reliable, System One

Status: phase 1 (the disk monitor) implemented, 2026-10-02: `crates/background`,
`coder host serve`, `openagents background`, the terminal's `/background`,
and NIP-HOST `background.*`; see "Phase 1 as built" at the end. Phases 2
and 3 are design. Issues:
umbrella [#10155](https://github.com/OpenAgentsInc/openagents/issues/10155),
phase 1 [#10156](https://github.com/OpenAgentsInc/openagents/issues/10156),
phase 2 [#10157](https://github.com/OpenAgentsInc/openagents/issues/10157),
phase 3 [#10158](https://github.com/OpenAgentsInc/openagents/issues/10158).

## Why

On 2026-10-02 the owner's Mac (1.8 TB) filled to 100% twice. Coder runs
failed with `No space left on device`. Each time, the orchestrating
conversational agent noticed only after the failures and then spent turns
deleting directories by hand. What it found:

| Path | Size | What it is |
| --- | --- | --- |
| `~/.openagents/targets/<project>-<task>-<hash>` | 9–38 GB each, 131 GB after one day | One Cargo target directory per Coder task, never deleted. [#10148](https://github.com/OpenAgentsInc/openagents/issues/10148) replaced these with reusable slots and a cleanup on task end. |
| `~/.openagents/coder-one/target` | 85 GB | Stale Coder One build output. |
| `~/work/openagents-target-agentN` | 17–72 GB each | Agent target directories left behind by finished conversation-agent work. |
| `~/work/openagents/target` | 66 GB | The checkout's own build directory. |
| `~/.openagents/worktrees` | 35 GB, 29 worktrees | Coder task worktrees, many for ended tasks. |
| `~/.openagents/gate/*` | 55 GB | Release-gate pools and their builds. |
| `~/.openagents/pylon` | 47 GB | Pylon state. Not known to be disposable. |
| `~/Library/Caches`, `/private/var/folders` | 75 GB | Application and operating-system caches. |

The rules the agent applied by hand were simple: never delete a directory
that a running task or process uses, never touch the user's source checkouts
or uncommitted work, and treat Cargo target directories and ended tasks'
worktrees as disposable caches. Simple, repeated, and mechanical work like
this should not wait for a conversation. The owner's words: "There just
should be some processes that do automatic cleanup, user-defined, defined in
conversation but reliable."

## The System One framing

The TypeSafe material (`docs/research/typesafe/`) and episodes 285–287
(`docs/transcripts/`) make one argument that applies directly here:

- Today's models are trained for assistance, with a person in the loop.
  Automation needs "simple little bits of work that just need to be done
  reliably" running "in the background in a server that you never even look
  at" (Diogo Almeida, AI Engineer, 2026-07-31).
- The answer is not a smarter conversation. It is software that owns the
  workflow and calls a System One model, Jev, only for a bounded, typed
  judgment, with probabilities that code compares to a named threshold.
  "Code owns control. Jev judges. Executors generate." (episode 287).
- Episode 286 lists background processing as a first-class category for a
  System One coding agent: work that runs beside the normal workflow and
  reads shared state.

A background process is that idea made concrete. A conversation defines it
once. Code then runs it deterministically, every time, without a model in the
loop for routine work. Jev appears only where a judgment needs semantic
understanding, such as "is this unknown directory a disposable cache?", and
even then its answer informs a decision that code makes. Escalation to a
Coder run (System Two) happens only when the rule cannot handle the
situation.

## What a background process is

```

### `crates/background/src/tests.rs`:1–64

Role: NearbyTest. Partial Rust line excerpt; surrounding content omitted. File SHA-256: `1626e2f97162f0930fc92437cf6637d02d077f32595dcba43be82e0a0b66ba11`.

```text
//! The disk monitor end to end, under a temporary home with fake volumes
//! and a scratch task list. Nothing here reads or changes the real home.

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
```

### `crates/background/Cargo.toml`:1–19

Role: Manifest. Complete small file; all referenced lines retained. File SHA-256: `5fe2ebbd73cbc64807bb87514f370f96988bf36e048507855c9dbd23be7678f3`.

```text
[package]
name = "background"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
publish.workspace = true
description = "Background processes: durable rules the host runs without a conversation, starting with the disk cleanup monitor (docs/background)."

[dependencies]
libc = "0.2"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
sha2 = "0.10"

[dev-dependencies]
tempfile = "3"

[lints]
workspace = true
```

Coverage: 4 excerpts; 0 candidate files not selected; 0 of 0 retained coverage records shown here; 0 additional coverage records omitted. Any omitted explicit path or line anchor remains unresolved. Complete syntax declarations can exclude adjacent attributes/comments. Missing evidence can still matter. This pack grants no execution authority.


## Fixed synthetic Git observation

This probe ran in a temporary repository on the verification host. It observes Git behavior; it is not a result for your candidate.

```json
{
  "schema": "openagents.briefing.git-probe.v1",
  "git_version": "git version 2.43.0",
  "commands": {
    "ls_files_directory": [
      "ls-files",
      "-z",
      "--others",
      "--ignored",
      "--exclude-standard",
      "--directory",
      "--no-empty-directory"
    ],
    "status_ignored_matching": [
      "status",
      "--porcelain",
      "-z",
      "--ignored=matching",
      "--untracked-files=all"
    ]
  },
  "outputs": {
    "ls_files_directory": [
      "web/"
    ],
    "status_ignored_matching": [
      "!! web/node_modules/"
    ]
  },
  "elapsed_seconds": 0.007013822999851982
}
```
