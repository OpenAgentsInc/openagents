# CLI reference

Upstream: https://docs.x.ai/build/cli/reference
Local: `grok --help` / `grok <subcommand> --help` (0.2.91)

Running `grok` with no arguments starts the interactive TUI. This page lists
subcommands and flags you are most likely to use.

## Subcommands

| Command | What it does |
| --- | --- |
| `grok login` | Sign in. `--device-auth` uses device-code auth for headless/remote |
| `grok logout` | Sign out and clear cached credentials |
| `grok inspect [--json]` | Show config discovered for this directory: rules, skills, plugins, hooks, MCP |
| `grok models` | List available models |
| `grok mcp <list\|add\|remove\|doctor>` | Manage MCP servers — [mcp-servers.md](./mcp-servers.md) |
| `grok plugin <list\|install\|uninstall\|update\|enable\|disable\|details\|validate>` | Manage plugins |
| `grok plugin marketplace <list\|add\|remove\|update>` | Manage marketplace sources |
| `grok sessions <list\|search\|delete>` | List, search, or delete sessions — [sessions.md](./sessions.md) |
| `grok export <session-id> [output]` | Export a session transcript as Markdown |
| `grok import [targets...]` | Import sessions (e.g. from Claude Code) |
| `grok memory clear [--workspace\|--global\|--all]` | Clear cross-session memory files |
| `grok worktree <list\|show\|rm\|gc>` | Manage git worktrees — [worktrees.md](./worktrees.md) |
| `grok dashboard` | Open the Agent Dashboard — [agent-dashboard.md](./agent-dashboard.md) |
| `grok agent stdio` | ACP agent over stdin/stdout — [headless-scripting.md](./headless-scripting.md#acp-agent-client-protocol) |
| `grok agent headless` | Headless agent over Grok WebSocket relay *(local CLI)* |
| `grok agent serve` | Agent as WebSocket server *(local CLI)* |
| `grok agent leader` | Shared leader process *(local CLI)* |
| `grok wrap <command...>` | Local PTY that forwards OSC 52 clipboard — [terminal-support.md](./terminal-support.md) |
| `grok update` | Check/install updates (`--check`, `--version <V>`, `--alpha`, `--stable`) |
| `grok version` | Print version (`-v`) |
| `grok completions <shell>` | Generate shell completion scripts |
| `grok setup` | Fetch and install managed configuration |
| `grok trace` | Export or upload session trace data *(local CLI)* |
| `grok leader` | Manage running leader processes *(local CLI)* |

## Common flags

Headless-specific flags are detailed in [headless-scripting.md](./headless-scripting.md).

| Flag | What it does |
| --- | --- |
| `--cwd <PATH>` | Working directory |
| `-r, --resume [<ID>]` | Resume a session by ID, or most recent if omitted |
| `-c, --continue` | Continue most recent session for current directory |
| `-s, --session-id <UUID>` | UUID for a **new** session (not a resume) |
| `--fork-session` | When resuming, fork into a new session ID |
| `-w, --worktree [<NAME>]` | Start session in a new git worktree |
| `--ref <REF>` / `--worktree-ref` | Branch, tag, or commit to base the worktree on |
| `-m, --model <MODEL>` | Model ID |
| `--reasoning-effort <LEVEL>` | Reasoning effort (`--effort` alias) |
| `--always-approve` | Auto-approve all tool executions (`--yolo`) |
| `--permission-mode <MODE>` | Permission mode |
| `--allow <RULE>`, `--deny <RULE>` | Permission rules — [enterprise.md](./enterprise.md) |
| `--sandbox <PROFILE>` | Sandbox profile — [enterprise.md](./enterprise.md) |
| `--rules <TEXT>` | Extra rules appended to the system prompt |
| `--system-prompt-override <TEXT>` | Replace the system prompt entirely |
| `--tools <LIST>`, `--disallowed-tools <LIST>` | Allow or remove built-in tools |
| `--max-turns <N>` | Maximum agent turns |
| `--no-plan`, `--no-subagents`, `--no-memory`, `--disable-web-search` | Disable features for this session |
| `--experimental-memory` | Enable cross-session memory |
| `--oauth` | Use OAuth when welcome screen starts authentication |
| `--output-format <FMT>` | Headless: `plain` \| `json` \| `streaming-json` |
| `-p, --single <PROMPT>` | Headless single-turn prompt |
| `--no-alt-screen` | Inline terminal (no alt screen) |
| `--minimal` | Experimental scrollback-native rendering *(local CLI)* |
| `--restore-code` | Check out original session commit when resuming *(local CLI)* |
| `--verbatim` | Send prompt exactly as given *(local CLI)* |
| `--debug` / `--debug-file <FILE>` | Debug logging |

### Claude Code aliases

Accepted where they overlap:

- `--allowedTools` / `--disallowedTools`
- `--append-system-prompt` / `--system-prompt`
- `--dangerously-skip-permissions`

## Models (example local listing)

As of the local check (logged into grok.com):

```text
Default model: grok-4.5

Available models:
  * grok-4.5 (default)
  - grok-composer-2.5-fast
```

Re-run `grok models` for current account offerings.

## Interactive entry

```bash
grok                          # open TUI
grok "fix the bug"            # TUI with initial prompt
grok --worktree=feat "create this feature"
```
