# Grok CLI — OpenAgents notes

Orientation docs for the **Grok Build TUI / CLI** (`grok`), mirrored from public
xAI docs and cross-checked against a local install.

**Local version checked:** `grok 0.2.91` (stable), 2026-07-08
**Package:** `@xai-official/grok` / binary typically at `~/.grok/bin/grok`

These notes are for OpenAgents agents and humans scripting Grok (fleet work,
CI, ACP integrations). They are **not** product promises and do not replace
live `grok --help` / `docs.x.ai`.

## Contents

| Doc | Topic |
| --- | --- |
| [`headless-scripting.md`](./headless-scripting.md) | `-p` headless mode, output formats, ACP (`grok agent stdio`) |
| [`reference.md`](./reference.md) | Subcommands + common flags |
| [`sessions.md`](./sessions.md) | Resume, fork, compact, export, `~/.grok/sessions` |
| [`worktrees.md`](./worktrees.md) | Isolated git worktrees for parallel agents |
| [`mcp-servers.md`](./mcp-servers.md) | MCP add/list/doctor, config.toml, scopes |
| [`terminal-support.md`](./terminal-support.md) | Colors, clipboard, chords, alt-screen, `grok wrap` |
| [`agent-dashboard.md`](./agent-dashboard.md) | Multi-agent dashboard (`Ctrl+\`, `grok dashboard`) |
| [`enterprise.md`](./enterprise.md) | Network, auth, sandbox, permissions, ZDR (CI-relevant) |
| [`examples/acp-stdio-hello.mjs`](./examples/acp-stdio-hello.mjs) | Minimal ACP JSON-RPC client |

## Upstream sources

Fetched 2026-07-08:

- https://docs.x.ai/build/cli/headless-scripting
- https://docs.x.ai/build/cli/reference
- https://docs.x.ai/build/cli/terminal-support
- https://docs.x.ai/build/features/sessions
- https://docs.x.ai/build/features/worktrees
- https://docs.x.ai/build/features/mcp-servers
- https://docs.x.ai/build/features/dashboard
- https://docs.x.ai/build/enterprise

Local: `grok --help`, `grok agent --help`, `grok models`, `grok version`.

## Quick start (headless)

```bash
# Authenticate once (interactive or device code for SSH)
grok login
# or: grok login --device-auth
# or: export XAI_API_KEY=xai-...

# One-shot prompt
grok --no-auto-update -p "Summarize this repo" --cwd .

# Machine-readable result
grok --no-auto-update -p "List top-level packages" \
  --output-format json --always-approve
```

## When to use which mode

| Mode | Command | Use for |
| --- | --- | --- |
| Interactive TUI | `grok` | Human-driven coding sessions |
| Headless single-turn | `grok -p "..."` | Scripts, bots, CI one-shots |
| Headless multi-step | `-p` + `--session-id` / `-r` / `-c` | Pipelines that resume context |
| ACP | `grok agent stdio` | IDE / custom tool host over JSON-RPC |
| Dashboard | `grok dashboard` | Supervise many parallel agents |

## Paths worth knowing

| Path | Purpose |
| --- | --- |
| `~/.grok/config.toml` | User config |
| `~/.grok/sessions/` | Session storage (TUI + headless + ACP) |
| `~/.grok/worktrees/` | Session git worktrees |
| `~/.grok/mcp_credentials.json` | MCP OAuth tokens |
| `~/.grok/logs/mcp/` | MCP stderr logs |
| `/etc/grok/requirements.toml` | Enterprise pinned policy (highest) |
| `.grok/config.toml` | Project-scoped config (walked up to git root) |

## Related in this monorepo

- `docs/grok/` — Grok's product/strategy analysis sandbox (different folder)
- Fleet/coding-agent work still prefers monorepo `EXECUTION.md` norms when
  shipping OpenAgents code; Grok CLI is an alternate runtime you may script.
