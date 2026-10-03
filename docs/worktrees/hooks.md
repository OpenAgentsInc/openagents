# Worktree setup and teardown hooks

Coder runs every task in a worktree of its own. A fresh worktree has the
repository's tracked files and nothing else: no `.env`, no installed
dependencies, no local configuration. A repository says how to make one ready
in a committed `.openagents/worktree.json` (#10297, modelled on Paseo's
`paseo.json` `worktree.setup`/`teardown`; see
[the gap analysis](2026-10-03-paseo-worktree-gap-analysis.md)):

```json
{
  "setup": ["cp \"$OPENAGENTS_SOURCE_CHECKOUT/.env\" .env", "npm ci"],
  "teardown": "docker compose down",
  "timeout_seconds": 900
}
```

- `setup` and `teardown` are each a command or a list of commands. They run
  one after another with `/bin/sh -c` in the worktree, with standard input
  closed, and stop at the first command that fails.
- `timeout_seconds` is optional. Without it there is no time limit.
- Unknown keys make the file invalid, and its hooks are skipped.

## Environment

| Variable | Value |
| --- | --- |
| `OPENAGENTS_SOURCE_CHECKOUT` | The checkout the worktree came from, to copy local files from |
| `OPENAGENTS_WORKTREE` | The worktree |
| `OPENAGENTS_WORKTREE_PORT` | A port that was free when the worktree was set up. It is kept for that worktree (in its Git administrative directory), so teardown sees the same one |

## When they run

- **Setup** runs once per task: after the task has its worktree (a spare it
  took, or a fresh one) and before the engine starts. Spares are never set up,
  so they stay free of ignored files. Follow-up turns reuse the worktree as it
  is. A failing setup ends the start with the command, its exit, the end of
  its output, and the log path, and the worktree is removed.
- **Teardown** runs before Coder removes a worktree
  (`coder::task::worktree_hooks::teardown`, the hook point for removal and
  archive). A teardown that fails keeps the worktree.
- Output goes to `<task store>/hooks/<task>.setup.log` and
  `<task store>/hooks/<worktree>.teardown.log`.
- The time setup took is recorded in the start's timings (`setup_ms`), and
  what ran (or why it was skipped) in the task's local record (`hooks`).

## Never the source checkout

The hooks run outside the engine's boundary, as you would run them yourself,
but under the worktree's source guard (`sandbox-exec` on macOS, `bwrap` on
Linux): they may read the source checkout, never write it. Where the guard
can't be enforced (no `bwrap`, another platform) a repository's hooks don't
run; the task still starts, and its local record (`hooks.skipped`) says why.

## Trust

Only the repository's own committed configuration runs: the file as committed
at the source checkout's `HEAD`, the commit you have checked out.

- Uncommitted edits to the file never run.
- A task whose commit carries a different file, such as a pull request from a
  fork that adds or changes setup, is not trusted. Its hooks are skipped and
  the reason is recorded.
- Teardown applies the same rule to the worktree's own `HEAD`.
