# Worktrees

Upstream: https://docs.x.ai/build/features/worktrees

A worktree session runs in an **isolated copy** of your repository so
parallel agents cannot overwrite each other's files.

## Requirements and layout

- Requires a **git repository**
- Lives under `~/.grok/worktrees/<repo>/<name>`
- Starts from your current HEAD, **including uncommitted changes**
  (unless `--ref` points at a clean checkout of a ref)

A worktree is a real git checkout, detached at its base commit. Land
changes with ordinary git.

## Starting one

```bash
grok -w
grok --worktree=feat "refactor module X"   # = keeps the prompt out of the name
grok -w --ref main "fix the flaky test"    # clean checkout of the ref
grok -w -r <session-id>                    # resume in a fresh worktree
```

In the TUI:

- `/fork --worktree` — fork current session into a worktree
- `Ctrl+W` on welcome — New Worktree dialog
- `Ctrl+W` in the Agent Dashboard — dispatch new agents into worktrees

Whether `/new` and `/fork` offer a worktree is configurable in TOML settings
(see upstream settings reference).

## Housekeeping

Worktrees **persist** until you remove them: ending or deleting a session
leaves its worktree in place. `gc` runs only when you invoke it.

| Command | What it does |
| --- | --- |
| `grok worktree list` | List tracked worktrees |
| `grok worktree show <id>` | Show details for one worktree |
| `grok worktree rm <ids...>` | Remove worktrees (`--dry-run` to preview) |
| `grok worktree gc` | Remove entries whose directory is gone; `--max-age 7d` also expires idle worktrees not in use by a running process |

## OpenAgents note

Monorepo agents already prefer clean worktrees when the main checkout is
dirty (`AGENTS.md` / fable EXECUTION norms). Grok's `-w` is the same idea
wired into the Grok session lifecycle.
