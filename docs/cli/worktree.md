# `openagents worktree`

Coder runs every task in its own Git worktree, in the `worktrees` folder
beside the task store (`~/.openagents/worktrees` for the default store,
`$OPENAGENTS_TASKS/../worktrees` otherwise). `openagents worktree` lists
them, removes an ended task's on purpose, and brings one back. It is P2.1 of
the [Paseo worktree gap analysis](../worktrees/2026-10-03-paseo-worktree-gap-analysis.md)
(#10294); the code is `crates/openagents-cli/src/worktree.rs`, over the same
`coder::task::worktrees` archive and restore as the terminal's `/worktrees`
(#10296).

```sh
openagents worktree ls                 # task worktrees, archived ones, then build slot sizes
openagents worktree ls --no-size       # skip sizing (it walks every file)
openagents worktree archive NAME       # remove an ended task's worktree, keep what restores it
openagents worktree archive NAME --force --confirm NAME
                                       # remove it even with unsaved files (deleted for good)
openagents worktree restore NAME       # recreate it at its path, at the recorded commit
```

Every command takes `--tasks DIR` for another task store, and `--json`.

## `ls`

One row per worktree: its name (`PROJECT-TASK12`, the task identity's first
twelve characters; a project's ready spare is `PROJECT.spare-KEY`), the
task's state (`ended`, `running`, `queued`, …, or `no task` when the task
store has none), its size, how long since it was last used, and what
removing it would lose:

- `nothing unsaved`: clean, every commit is on a remote, and no ignored
  file outside a build cache (`target`, `node_modules`, …);
- `uncommitted changes`, `commits not on any remote`,
  `holds ignored files: …`, or `a stash was made on it`.

These are the same checks the background cleanup uses before it removes a
worktree (`background::git::removable`). Archived worktrees follow, with
their commit and when they went. Build slots follow, with one size per slot;
JSON includes a `slots` array with `name`, `path`, and `bytes`. `--no-size`
skips both worktree and slot sizing (`bytes` is `null`). The background
watchers view shows the same slot sizes. For pruning limits and environment
overrides, see [build slots](../coder/guides/tasks.md).

## `archive`

`NAME` is the folder name, its path, or the task's identity (at least six
characters). Refuses a running task's worktree, a spare, and a folder with
no task. Otherwise it runs the checks above; when one fails, nothing is
removed and the reason is printed. When they pass, it runs the repository's
teardown hook (#10297), records what restores the worktree in the run's
record (`<task store>/local/TASK.json`, `archived`: commit, branch if any,
when), and removes it: the same removal a task's end makes (#10291), so a
follow-up recreates it by itself.

`--force` first deletes its uncommitted, untracked, and ignored files for
good (`git reset --hard`, `git clean -fdx`), then archives it; `restore`
brings back the committed state. Commits not on any remote and a stash made
on it still keep it. It asks you to type the worktree's name, or takes
`--confirm NAME` when not run from a terminal.

## `restore`

Recreates the worktree at its old path at the recorded commit, detached as
Coder made it (or on its branch when it had one and the branch still
exists), and drops the record. It refuses when the path is taken.

`NAME` is the folder name or the task's identity. Worktrees a task's end
removed (#10291) are listed and restored the same way, as are archives made
before that record existed (`<task store>/archived-worktrees/TASK.json`).
