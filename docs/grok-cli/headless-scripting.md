# Headless & scripting

Upstream: https://docs.x.ai/build/cli/headless-scripting
Also: local `grok --help` (0.2.91)

## Headless mode

Use headless mode for scripts, bots, CI, or other machine-friendly tasks.

```bash
grok -p "Your prompt here"
```

### Common flags

| Flag | What it does |
| --- | --- |
| `-p, --single <PROMPT>` | Send one prompt; print response to stdout and exit |
| `--prompt-file <PATH>` | Single-turn prompt from a file *(local CLI)* |
| `--prompt-json <JSON>` | Single-turn prompt as JSON content blocks *(local CLI)* |
| `-m, --model <MODEL>` | Choose a model |
| `--reasoning-effort <LEVEL>` | Reasoning effort (`--effort` alias) |
| `-s, --session-id <UUID>` | Name a **new** session with a UUID you supply (must not already exist). Does **not** resume — use `-r` / `-c` |
| `-r, --resume [<ID>]` | Resume a session by ID, or the most recent if omitted |
| `-c, --continue` | Continue the most recent session for the current directory |
| `--fork-session` | When resuming, fork into a new session ID (optionally named via `-s`) |
| `--cwd <PATH>` | Set the working directory |
| `--output-format <FMT>` | `plain` (default), `json`, or `streaming-json` |
| `--json-schema <SCHEMA>` | Constrain output to a JSON Schema (implies `--output-format json`) *(local CLI)* |
| `--always-approve` | Auto-approve tool executions (alias `--yolo` in docs) |
| `--permission-mode <MODE>` | `default`, `acceptEdits`, `auto`, `dontAsk`, `bypassPermissions`, `plan` |
| `--sandbox <PROFILE>` | Sandbox profile (see [enterprise.md](./enterprise.md)) |
| `--max-turns <N>` | Maximum agent turns |
| `--no-alt-screen` | Run inline (no alternate-screen / fullscreen TUI takeover) |
| `--no-auto-update` | Skip background update checks (important in CI/scripts) |
| `-w, --worktree [<NAME>]` | Start in a new git worktree |
| `--ref <REF>` / `--worktree-ref` | Base worktree on branch/tag/commit |
| `--best-of-n <N>` | Run the task N ways in parallel and pick the best (headless only) *(local CLI)* |
| `--check` | Append a self-verification loop to the prompt (headless only) *(local CLI)* |

### Sessions

Headless sessions (via `--session-id`, `--resume`, `--continue`) are stored under
`~/.grok/sessions`, same as the TUI and ACP.

Multi-step automation pattern:

```bash
SESSION=$(grok --no-auto-update -p "Start the refactor" \
  --output-format json --always-approve | jq -r '.sessionId')

grok --no-auto-update -p "Continue with step 2" \
  -r "$SESSION" --output-format json --always-approve
```

### Suppressing auto-update

When using headless mode (`-p`) or ACP (`grok agent stdio`) in scripts, CI, or
other automated environments, pass `--no-auto-update`:

```bash
grok --no-auto-update -p "..."
```

Persistently disable via `~/.grok/config.toml`:

```toml
[cli]
auto_update = false
```

### Auth for automation

| Method | How |
| --- | --- |
| Cached login | `grok login` once on the machine |
| Device code | `grok login --device-auth` (SSH / no browser) |
| API key | `export XAI_API_KEY=xai-...` |

Example:

```bash
export XAI_API_KEY="xai-..."
grok --no-auto-update -p "Review this diff" \
  --output-format json --always-approve
```

## Output formats

| Format | Behavior |
| --- | --- |
| `plain` | Human-readable text (default) |
| `json` | One JSON object at the end |
| `streaming-json` | Newline-delimited JSON events as they arrive |

```bash
grok -p "List TODO comments" --output-format json
grok -p "Explain the architecture" --output-format streaming-json
```

Structured output with schema (local CLI):

```bash
grok -p "Name the primary package manager" \
  --json-schema '{"type":"object","properties":{"name":{"type":"string"}},"required":["name"]}'
```

## ACP (Agent Client Protocol)

Use ACP when you want IDE or tool integration rather than a terminal session.

```bash
grok agent stdio
```

Runs Grok as an ACP agent over **JSON-RPC on stdin/stdout**. Assumes local
auth (`grok login`) or `XAI_API_KEY`.

### Protocol notes

1. Client sends `initialize` with `protocolVersion` and `clientCapabilities`.
2. Client picks an auth method from `init.authMethods` and calls `authenticate`.
3. Client creates a session with `session/new` (`cwd`, `mcpServers`).
4. Client sends work with `session/prompt`.
5. **Assistant text arrives as `session/update` chunks**
   (`sessionUpdate: "agent_message_chunk"`), not only in the `session/prompt`
   result. The prompt result is completion **metadata** (e.g. `stopReason`).

Related agent subcommands (local CLI):

| Command | Role |
| --- | --- |
| `grok agent stdio` | ACP over stdio |
| `grok agent headless` | Headless over Grok WebSocket relay |
| `grok agent serve` | WebSocket server |
| `grok agent leader` | Shared leader process for other clients |

### Minimal Node client

See [`examples/acp-stdio-hello.mjs`](./examples/acp-stdio-hello.mjs). Run:

```bash
# After grok login, or with XAI_API_KEY set
node docs/grok-cli/examples/acp-stdio-hello.mjs
```

Flow summary:

```text
initialize → authenticate → session/new → session/prompt
                ↑
         session/update (agent_message_chunk) streams text
```

## Practical OpenAgents patterns

```bash
# CI-style: no updates, no prompts, JSON out, deny dangerous shell
grok --no-auto-update \
  -p "Summarize open TODOs under apps/" \
  --cwd /path/to/openagents \
  --output-format json \
  --permission-mode dontAsk \
  --allow 'Read' \
  --allow 'Grep' \
  --allow 'Bash(git *)' \
  --deny 'Bash(rm *)' \
  --sandbox workspace

# Resume most recent session in this directory
grok --no-auto-update -c -p "Continue from last step" --output-format plain
```

Claude Code flag aliases are accepted where they overlap (docs):
`--allowedTools`, `--disallowedTools`, `--append-system-prompt`,
`--system-prompt`, `--dangerously-skip-permissions`.
