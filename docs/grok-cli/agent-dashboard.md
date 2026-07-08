# Agent dashboard

Upstream: https://docs.x.ai/build/features/dashboard

The dashboard is a fullscreen overview of every session: which agents need
input, which are working, and which are done.

## Open

| Method | Action |
| --- | --- |
| `Ctrl+\` | Toggle dashboard (TUI) |
| `/dashboard` | Command |
| `grok dashboard` | From the shell |

Rows are grouped by state — Needs input, Working, Idle, Inactive, Completed,
Failed — and update live. Press `Ctrl+G` to group by directory instead.

## Working with agents

- Selecting a row opens a **peek panel** with latest activity.
- Type to reply: idle agents receive immediately; busy agents queue.
- Permission prompts and questions can be answered inline with number keys.
- `Enter` attaches to the session in a full details view.
- `Ctrl+\` returns to the dashboard.
- `Ctrl+[` / `Ctrl+]` cycle between sessions.

Bottom input bar dispatches prompts to **new** sessions:

- `Ctrl+L` — change working directory for new agents
- `Ctrl+W` — toggle whether new agents start in a git worktree

## Keys

| Keys | Action |
| --- | --- |
| `↑` / `↓` | Select row |
| `Enter` | Open selected session |
| `Ctrl+/` | Search — `a:<name>` by agent, `s:<state>` by state, or plain text |
| `Ctrl+T` | Pin / unpin agent |
| `Ctrl+R` | Rename agent |
| `Ctrl+X` | Stop / close agent (press twice) |
| `Shift+↑` / `Shift+↓` | Reorder pinned agents |
| `Esc` | Close peek, then filter, then the dashboard |

## Config

Grouping and pins persist under `[dashboard]` in `~/.grok/config.toml`.

Disable:

```toml
[dashboard]
enabled = false
```

Or:

```bash
export GROK_AGENT_DASHBOARD=0
```
