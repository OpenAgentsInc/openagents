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

Coverage `bench/terminal-bench/experiments/2026-09-24-acceptance-first/records/pass2-calibrated/embedding-drift-monitor/suite/tests/T5.sh`: The 24-file read bound omitted this admitted path.
Coverage `bench/terminal-bench/experiments/2026-09-24-acceptance-first/records/pass2-calibrated/embedding-drift-monitor/suite/tests/T6.sh`: The 24-file read bound omitted this admitted path.
Coverage `bench/terminal-bench/experiments/2026-09-24-acceptance-first/records/pass2-calibrated/embedding-drift-monitor/suite/tests/T7.sh`: The 24-file read bound omitted this admitted path.
Coverage `bench/terminal-bench/experiments/2026-09-24-acceptance-first/records/pass2-calibrated/embedding-drift-monitor/suite/tests/T8.sh`: The 24-file read bound omitted this admitted path.
Coverage `bench/terminal-bench/experiments/2026-09-24-acceptance-first/records/pass2-calibrated/fin-saccr-rwa/suite/tests/T2.sh`: The 24-file read bound omitted this admitted path.
Coverage `bench/terminal-bench/experiments/2026-09-24-acceptance-first/records/pass2-calibrated/fin-saccr-rwa/suite/tests/T3.sh`: The 24-file read bound omitted this admitted path.
Coverage `bench/terminal-bench/experiments/2026-09-24-acceptance-first/records/pass2-calibrated/fin-saccr-rwa/suite/tests/T7.sh`: The 24-file read bound omitted this admitted path.
Coverage `bench/terminal-bench/experiments/2026-09-24-acceptance-first/records/pass2-calibrated/fin-saccr-rwa/suite/tests/T8.sh`: The 24-file read bound omitted this admitted path.
Coverage `bench/terminal-bench/experiments/2026-09-24-acceptance-first/records/pass2-calibrated/fin-saccr-rwa/suite/tests/T9.sh`: The 24-file read bound omitted this admitted path.
Coverage `bench/terminal-bench/experiments/2026-09-24-acceptance-first/records/pass2-calibrated/html-js-filter/suite/tests/T1.sh`: The 24-file read bound omitted this admitted path.
Coverage `bench/terminal-bench/experiments/2026-09-24-acceptance-first/records/pass2-calibrated/html-js-filter/suite/tests/T2.sh`: The 24-file read bound omitted this admitted path.
Coverage `bench/terminal-bench/experiments/2026-09-24-acceptance-first/records/pass2-calibrated/html-js-filter/suite/tests/T3.sh`: The 24-file read bound omitted this admitted path.
Coverage `bench/terminal-bench/experiments/2026-09-24-acceptance-first/records/pass2-calibrated/html-js-filter/suite/tests/T4.sh`: The 24-file read bound omitted this admitted path.
Coverage `bench/terminal-bench/experiments/2026-09-24-acceptance-first/records/pass2-calibrated/html-js-filter/suite/tests/T5.sh`: The 24-file read bound omitted this admitted path.
Coverage `bench/terminal-bench/experiments/2026-09-24-acceptance-first/records/pass2-calibrated/html-js-filter/suite/tests/T6.sh`: The 24-file read bound omitted this admitted path.
Coverage `bench/terminal-bench/experiments/2026-09-24-acceptance-first/records/pass2-calibrated/interleaved-vigenere/suite/tests/T1.sh`: The 24-file read bound omitted this admitted path.
Coverage `bench/terminal-bench/experiments/2026-09-24-acceptance-first/records/pass2-calibrated/interleaved-vigenere/suite/tests/T2.sh`: The 24-file read bound omitted this admitted path.
Coverage `bench/terminal-bench/experiments/2026-09-24-acceptance-first/records/pass2-calibrated/interleaved-vigenere/suite/tests/T3.sh`: The 24-file read bound omitted this admitted path.
Coverage `bench/terminal-bench/experiments/2026-09-24-acceptance-first/records/pass2-calibrated/interleaved-vigenere/suite/tests/T4.sh`: The 24-file read bound omitted this admitted path.
Coverage `bench/terminal-bench/experiments/2026-09-24-acceptance-first/records/pass2-calibrated/interleaved-vigenere/suite/tests/T5.sh`: The 24-file read bound omitted this admitted path.
Coverage `bench/terminal-bench/experiments/2026-09-24-acceptance-first/records/pass2-calibrated/interleaved-vigenere/suite/tests/T6.sh`: The 24-file read bound omitted this admitted path.
Coverage `bench/terminal-bench/experiments/2026-09-24-acceptance-first/records/pass2-calibrated/ks-solver-cpp/suite/tests/T2.sh`: The 24-file read bound omitted this admitted path.
Coverage `bench/terminal-bench/experiments/2026-09-24-acceptance-first/records/pass2-calibrated/mvcc-lsm-compaction/suite/tests/T2.sh`: The 24-file read bound omitted this admitted path.
Coverage `bench/terminal-bench/experiments/2026-09-24-acceptance-first/records/pass2-calibrated/mvcc-lsm-compaction/suite/tests/T3.sh`: The 24-file read bound omitted this admitted path.
Coverage `bench/terminal-bench/experiments/2026-09-24-acceptance-first/records/pass2-calibrated/production-planning/suite/tests/T1.sh`: The 24-file read bound omitted this admitted path.
Coverage `bench/terminal-bench/experiments/2026-09-24-acceptance-first/records/pass2-calibrated/production-planning/suite/tests/T10.sh`: The 24-file read bound omitted this admitted path.
Coverage `bench/terminal-bench/experiments/2026-09-24-acceptance-first/records/pass2-calibrated/production-planning/suite/tests/T2.sh`: The 24-file read bound omitted this admitted path.
Coverage `bench/terminal-bench/experiments/2026-09-24-acceptance-first/records/pass2-calibrated/production-planning/suite/tests/T3.sh`: The 24-file read bound omitted this admitted path.
Coverage `bench/terminal-bench/experiments/2026-09-24-acceptance-first/records/pass2-calibrated/production-planning/suite/tests/T4.sh`: The 24-file read bound omitted this admitted path.
Coverage `bench/terminal-bench/experiments/2026-09-24-acceptance-first/records/pass2-calibrated/production-planning/suite/tests/T5.sh`: The 24-file read bound omitted this admitted path.
Coverage `bench/terminal-bench/experiments/2026-09-24-acceptance-first/records/pass2-calibrated/production-planning/suite/tests/T7.sh`: The 24-file read bound omitted this admitted path.
Coverage `bench/terminal-bench/experiments/2026-09-24-acceptance-first/records/pass2-calibrated/production-planning/suite/tests/T8.sh`: The 24-file read bound omitted this admitted path.
Coverage `bench/terminal-bench/experiments/2026-09-24-acceptance-first/records/pass2-calibrated/risk-scorer-replay/suite/tests/T1.sh`: The 24-file read bound omitted this admitted path.
Coverage `bench/terminal-bench/experiments/2026-09-24-acceptance-first/records/pass2-calibrated/risk-scorer-replay/suite/tests/T2.sh`: The 24-file read bound omitted this admitted path.
Coverage `bench/terminal-bench/experiments/2026-09-24-acceptance-first/records/pass2-calibrated/sound-change-cascade/suite/tests/T1.sh`: The 24-file read bound omitted this admitted path.
Coverage `bench/terminal-bench/experiments/2026-09-24-acceptance-first/records/pass2-calibrated/sound-change-cascade/suite/tests/T2.sh`: The 24-file read bound omitted this admitted path.
Coverage `bench/terminal-bench/experiments/2026-09-24-acceptance-first/records/pass2-calibrated/sound-change-cascade/suite/tests/T3.sh`: The 24-file read bound omitted this admitted path.
Coverage `bench/terminal-bench/experiments/2026-09-24-acceptance-first/records/pass2-calibrated/sound-change-cascade/suite/tests/T4.sh`: The 24-file read bound omitted this admitted path.

Coverage: 4 complete ranges; 38/128 retained warnings shown; 127 additional warnings omitted. Missing, ambiguous, and budget-omitted evidence remains unresolved. Full selection provenance is in briefing.json.
