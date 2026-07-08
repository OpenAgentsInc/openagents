# Sessions

Upstream: https://docs.x.ai/build/features/sessions

Grok saves every conversation to disk automatically — prompts, responses,
tool calls, and file snapshots — under `~/.grok/sessions/`, keyed by working
directory. Sessions work the same in the **TUI**, **headless mode**, and
**ACP**.

## Resuming

In the TUI, `/resume` opens a picker of recent sessions for the current
workspace; the welcome screen lists them too.

From the command line:

```bash
grok --resume <session-id>   # a specific session
grok --resume                # the most recent for this directory
grok -c                      # shorthand: continue the most recent
```

In headless mode, read the session ID from JSON output and pass it to `-r`:

```bash
grok -p "Start the refactor" --output-format json | jq -r '.sessionId'
```

### `-s` vs `-r`

| Flag | Meaning |
| --- | --- |
| `-s, --session-id <UUID>` | Name a **new** session with a UUID you supply. Must not already exist. **Does not resume.** |
| `-r, --resume [<ID>]` | Resume existing session (or most recent) |
| `-c, --continue` | Resume most recent for this directory |
| `--fork-session` | When resuming, branch into a new session ID (can name via `-s`) |

## Forking

`/fork [directive]` branches the current session into a peer that starts from
a copy of the conversation. Pass `--worktree` or `--no-worktree` to choose
whether the fork runs in an isolated repo copy — see [worktrees.md](./worktrees.md).

CLI: resume with `--fork-session` to continue without mutating the original
session id.

## Rewinding

`/rewind` (or `Esc Esc` while idle) lists a rewind point per prompt. Selecting
one restores all files to their state at that point and truncates the
conversation. **Rewind modifies files on disk** — reverted changes are lost
unless committed to git.

## Compacting

`/compact [context]` compresses conversation history to reclaim context
window, with optional instructions about what to preserve. Grok also
auto-compacts as the context window fills; check usage with `/context` or
`/session-info`.

## Housekeeping

| Command | What it does |
| --- | --- |
| `/sessions` | Switch, rename, or close active sessions (TUI) |
| `/rename <title>` | Rename the current session (`/title` alias) |
| `grok sessions list` | List recent sessions for this directory |
| `grok sessions search <query>` | Search session titles and prompts |
| `grok sessions delete <id>` | Permanently delete a session |
| `grok export <id> [file]` | Export transcript as Markdown (`--clipboard` to copy) |
