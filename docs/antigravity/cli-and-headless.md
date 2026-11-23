# Antigravity CLI (`agy`) & Headless (`stream-json`) Reference

Complete reference for Google Antigravity's CLI (`agy`), verified against all official documentation pages at <https://antigravity.google/docs/cli/> (`v1.5.12` changelog scrape, Nov 2026) and live inspection of `/Users/christopherdavid/.local/bin/agy` (`v1.5.14`).

Related docs in this folder:
- [`AUDIT.md`](./AUDIT.md) — Forensic audit of OpenAgents' Antigravity wrapper stack (`#13993`) and unconsidered gaps
- [`acp-and-endpoints.md`](./acp-and-endpoints.md) — Live-verified ACP JSON-RPC 2.0 spec, network endpoints, models, modes & auth
- [`platform-security-and-extensibility.md`](./platform-security-and-extensibility.md) — Permissions, terminal sandbox, hooks, MCP/MCP Apps, customizations, swarms, remote & enterprise
- [`sdk-and-ides.md`](./sdk-and-ides.md) — Python SDK (`google-antigravity`), IDE extensions matrix & changelog drift analysis

---

## 1. Root `agy` Flags & Aliases

Verified against `cli/reference.md`, `cli/headless.md`, and `agy --help` (`v1.5.14`).

| Flag | Alias(es) | Type / Values | Default | `settings.json` / Env Equivalent | Behavior & Version Notes |
| --- | --- | --- | --- | --- | --- |
| `--mode` | — | `accept-edits` \| `auto` \| `plan` | `""` (inherits `agentMode`; since `v1.5.7`, unset defaults to `auto` where available, else `default`) | `agentMode` | Session-only execution mode override. Where `auto` is unavailable (Enterprise/business accounts or hosts without OS sandbox), falls back to `default` (prints warning in headless mode). Does **not** accept `--permission-mode`. |
| `--model` | — | string (model slug, display label, or base model name) | — | `model` | Session-only model override. Since `v1.3.4`, accepts base model names without `--effort` (resolves server default -> `medium` -> lowest available). Headless mode hard-fails on unknown model (exit `1`, `ERROR` JSON status); interactive warns and falls back. |
| `--effort` | — | `low` \| `medium` \| `high` \| `xhigh` \| `max` | — | Persisted via `/effort` as model variant | Session-only reasoning effort override (not persisted). Valid levels depend on model family; unsupported on fixed-budget models such as `Orchestrator`. |
| `--agent` | — | string (agent name from `agy agents`) | — | — | Selects custom agent for current session. Headless exits `1` listing available agents if unknown; interactive warns `Agent Not Found` and falls back to default. |
| `--add-dir` | — | string (repeatable) | `[]` | — | Adds an extra directory to the active workspace. |
| `--continue` | `-c` | bool | `false` | — | Resumes most recent conversation for current workspace (`~/.gemini/antigravity-cli/cache/last_conversations.json`, with parent/child fallback since `v1.2.1`). With `--remote-agent`, resumes most recent remote conversation. |
| `--conversation` | — | string (UUID) | — | — | Resumes specific conversation by ID. With `--remote-agent`, only opens remote conversations (warns `is a local conversation` otherwise). |
| `--prompt-interactive` | `-i` | string | — | — | Launches interactive TUI with an initial prompt and continues the session. Incompatible with `--print`/`-p` (exits `2`). Works with `--remote-agent` + `--continue`/`--conversation` (`v1.5.9+`). |
| `--print` | `-p`, `--prompt` | string | — | — | Runs a single prompt non-interactively and prints response to `stdout` (diagnostics to `stderr`). Incompatible with `--remote-agent`. |
| `--output-format` | — | `text` \| `json` \| `stream-json` | `text` | — | Output format for print mode (also supported on `agy models` and `agy agents` since `v1.1.12`). |
| `--input-format` | — | `text` \| `stream-json` | `text` | — | Input format for print mode. `stream-json` reads one NDJSON object per line from `stdin` and runs one turn per message in a persistent process; **requires** `--output-format stream-json`. Prompts passed via `-p` are invalid in `stream-json` input mode. |
| `--json-schema` | — | string (inline JSON Schema or path to `.json` file) | — | — | Enforces structured JSON output (`structured_output` + `json_schema` fields in `json` envelope or terminal `result` event of `stream-json`). Since `v1.2.14`, rejects bare type names (e.g. `string`), missing files, and any schema whose root is not `"type": "object"` (exits `1` at startup). |
| `--print-timeout` | — | Go duration string (e.g. `15m`, `10m`, `0s`) | `0s` (no limit) | — | Wall-clock time limit for print mode; `0`/`0s` waits until turn AND non-daemon background tasks complete (`v1.5.0+`). On timeout, exits **`3`**, emits `finished_reason: "print_timeout"`, prints `AGY_ERROR` naming terminated tasks, and writes `[agy] print timeout after ...` to `stderr`. |
| `--disable-slash-commands` | — | bool | `false` | `disableSlashCommands` | Disables slash command and skill expansion in print mode; sends leading `/` prompts verbatim to model. |
| `--sandbox` | — | bool | `false` | `enableTerminalSandbox` | Enables OS terminal sandbox for this session, overriding `settings.json`. Since `v1.4.4`, no-op if OS sandbox is unavailable on the host. |
| `--dangerously-skip-permissions` | — | bool | `false` | `toolPermission: "always-proceed"` | Auto-approves tool permission requests. **Exceptions:** (1) Since `v1.3.2`, does **not** auto-approve `/plan` artifacts (`artifactReviewPolicy` governs those); (2) Since `v1.5.2`, built-in protected commands and Enterprise admin-enforced review policies are NOT bypassed; (3) Explicit `deny` rules always still apply. |
| `--remote-control` | — | bool | `false` | — | Creates a session-scoped Remote Control reverse tunnel on startup (tears down on CLI exit). Exits **`2`** with `GEMINI_API_KEY` (`v1.5.12+`); errors with `AGY_ADC_AUTH`. |
| `--remote-agent` | — | bool | `false` | — | Runs conversations on Google-managed Remote Agents (personal Google AI Pro/Ultra only; exits **`2`** with `GEMINI_API_KEY`). With `--project`, opens that remote project (by ID or name); otherwise uses `CLI Project (<hostname>)`. Incompatible with `--print`. |
| `--project` | — | string (project ID or name) | — | — | Sets project for current CLI session (local or remote with `--remote-agent`). |
| `--new-project` | — | bool | `false` | — | Creates a new project for this session (with `--remote-agent`, names it after current directory). |
| `--settings` | — | string (inline JSON object or path to JSON file) | — | In-memory `settings.json` override | Session-only `settings.json` overrides, **never saved to disk** (`v1.5.8+`, live in `agy v1.5.14 --help`; supports UTF-8 BOM). |
| `--log-file` | — | string (file path) | `~/.gemini/antigravity-cli/cli.log` | — | Overrides CLI log file path. |
| `--version` | — | — | — | — | Prints CLI version string (e.g. `1.5.14`). |
| `--help` | `-h` | — | — | — | Prints root usage and subcommand list. |

---

## 2. Subcommands & Subcommand Flags

Verified against official docs and live `agy <subcommand> --help` (`v1.5.14`).

### 2.1 `agy acp [flags]`
Starts the Agent Client Protocol (ACP) v1 JSON-RPC 2.0 server over stdio.

**Documented flags (`ide/extensions/acp.md`):**
- `--add-dir` (repeatable string, default `[]`): Add workspace directory.
- `--agent` (string): Agent for the ACP session.
- `--sandbox` (bool, default `false`): Enable terminal sandbox restrictions.
- `--dangerously-skip-permissions` (bool, default `false`): Auto-approve all tool permission requests (subject to `v1.5.2` protected command & Enterprise admin ceilings).
- `--process-per-session` (bool, default `false`): Serve each session from an isolated `--worker` subprocess (required for editor-configured HTTP MCP servers).
- `--ide-mcp-port` / `--ide_mcp_port` (int, default `0`): Port for host IDE tools MCP server (requires `nc` in `$PATH`).
- `--login` (bool, default `false`): Interactive terminal Google OAuth (code paste, works over SSH), then exit.
- `--log-file` / `--cli_log_file` (string): Override ACP log file path (default `~/.gemini/antigravity-cli/log/acp-*.log`).
- `--disable-telemetry` / `--disable_telemetry` (bool, default `false`): Disable telemetry.
- `-h`, `--help`: Show help.

**Additional / internal flags exposed in live `agy acp --help` (`v1.5.14`):**
- `--stdio` (bool): Use stdio JSON-RPC transport (default).
- `--app-data-dir` / `--app_data_dir` (string, default `antigravity-cli`): Subdirectory under `gemini-dir` where application data is stored.
- `--cli_dir_name` (string): Subdirectory under `app_data_dir` for CLI settings and logs.
- `--gemini-dir` / `--gemini_dir` (string, default `.gemini`): Base path where Gemini files are stored.
- `--include-default-customizations` (bool, default `true`): Include built-in default customizations.
- `--cloud_code_endpoint` (string, default `https://daily-cloudcode-pa.googleapis.com`): Cloud Code Assist API base URL.
- `--override_user_agent_name` (string): Override HTTP `User-Agent` name.
- `--is_google3_workspace` (bool; deprecated alias `--is_google_internal`): Google internal workspace flag.
- `--use_stubby_auth` (bool): Use LOAS authentication.
- `--worker` (bool): Serve a single session for an ACP front router process (internal).

### 2.2 `agy models [flags]`
Lists available models (`slug` + display name).
- `-h`, `--help`
- `--output-format json|stream-json` (supported since `v1.1.12`; spinner & errors go to `stderr` so `stdout` stays clean).

### 2.3 `agy agent [flags]` / `agy agents [flags]`
Lists available custom agents (`agents` is an alias for `agent`).
- `-h`, `--help`
- `--output-format json|stream-json` (`v1.1.12+`).

### 2.4 `agy changelog [flags]`
Prints CLI release notes directly to `stdout` (caches to `~/.gemini/antigravity-cli/cache/CHANGELOG.md`).
- `-h`, `--help`

### 2.5 `agy install [flags]`
Configures environment paths and shell profile settings.
- `--dir <path>`: Custom directory target to configure `PATH` for.
- `--skip-aliases`: Bypasses purging legacy `agy`/`antigravity` shell aliases.
- `--skip-path`: Bypasses appending to shell profile `$PATH`.
- `-h`, `--help`

### 2.6 `agy update`
Updates the `agy` binary in place from Google's release manifest.

### 2.7 `agy mic-serve [flags]`
Serves local microphone audio (raw uncompressed PCM ~32 kB/s) and plays back Gemini Live voice output over loopback TCP for a remote CLI over SSH (`ANTIGRAVITY_MIC`).
- `--addr` (string, default `127.0.0.1:4713`): Listen address (keep on loopback).
- `-h`, `--help`

### 2.8 `agy mcp <subcommand> [flags] [args]`
Manages user-level MCP servers in `~/.gemini/config/mcp_config.json`.
- `agy mcp add [flags] <name> <commandOrUrl> [args...]`:
  - `-t`, `--type` (`stdio` \| `http`, default `stdio`; auto-detected when `http://`/`https://` URL is given)
  - `-e`, `--env` (`KEY=value`, repeatable)
  - `-H`, `--header` (`Key: Value`, repeatable)
  - `-h`, `--help`
  - **Argument rule:** Flags must precede `<name>`; use `--` before commands/args starting with `-`.
- `agy mcp remove <name>`
- `agy mcp list`
- `agy mcp enable <name>`
- `agy mcp disable <name>`

### 2.9 `agy plugin <command> [args]` (alias: `agy plugins`)
Manages plugins in `~/.gemini/config/plugins/`.
- `agy plugin list [--json]`
- `agy plugin import [gemini|claude]` — imports from `~/.gemini/extensions/` or `~/.claude/plugins/` (including Claude Code marketplace plugins, `v1.4.11+`).
- `agy plugin install <target>` — local dir, marketplace (`<plugin>@<marketplace>`, e.g. `@antigravity-plugins-official`), or GitHub subpath.
- `agy plugin uninstall <name>`
- `agy plugin enable <name>` / `agy plugin disable <name>`
- `agy plugin validate [target] [--strict] [--json] [-h|--help]` — validates a dir, `plugin.json`, `.zip` path/URL, `github.com` URL (`/tree/<branch>/<path>`), or `marketplace.json`. `--strict` fails on warnings.
- `agy plugin link <marketplace> <target>` — generates marketplace link.
- `agy plugin help`

### 2.10 `agy remote-control <subcommand> [flags]`
Manages the OS-registered persistent headless Remote Control daemon (`systemd --user` on Linux, LaunchAgent on macOS, Scheduled Task on Windows; background process fallback on systemd-less Linux/containers since `v1.2.14`).
- `agy remote-control start [--name <label>] [--session] [-h|--help]` — registers and starts daemon (`--name` sets `cliRemoteControlHostname` in `~/.gemini/config/config.json`; `--session` registers login-scoped instead of boot-scoped).
- `agy remote-control status [-h|--help]` — prints status, instance name, PID.
- `agy remote-control stop [-h|--help]` — stops and unregisters daemon.

### 2.11 `agy help [subcommand]`
Prints help for subcommands.

---

## 3. Headless / Print Mode (`-p`) & `stream-json` Wire Contract

### 3.1 Execution Semantics & Stream Separation
- **`stdout`**: Strictly the response payload (`text`, single JSON line for `json`, or NDJSON lines for `stream-json`).
- **`stderr`**: Diagnostics, errors, auth prompts, soft-denial notices, `[agy] print timeout after ...`, and structured `AGY_ERROR: {...}` JSON lines.
- **Auth**: Uses cached OS keyring/file token, `GEMINI_API_KEY` (with `modelProvider: "gemini"`), or `AGY_ADC_AUTH=true`. Supports OAuth code paste if `/dev/tty` / `CONIN$` exists (`v1.1.2+`); fails fast with `authentication required` in non-TTY headless hosts.
- **Auto-resolution in headless mode**: Implementation plans proceed automatically (`v1.1.28+`); clarifying questions are self-settled (`v1.1.12+`); subagent questions are skipped (`v1.3.6+`).
- **Background tasks**: Without `--print-timeout`, waits indefinitely for non-daemon background tasks and scheduled timers (`v1.5.0+`), terminating daemon tasks on exit (`v1.2.9+`).
- **Soft denials**: Actions requiring approval without an interactive TTY do not abort; the turn continues, exits `0`, prints a `stderr` notice with the required allow rule, and records the action in `denied_actions` (`v1.1.27+`).

### 3.2 Single-Turn `--output-format json` Envelope
```json
{
  "conversation_id": "string (UUID; empty if failed before creation)",
  "status": "SUCCESS | ERROR | CANCELED | INTERRUPTED | INVALID | WAITING | RUNNING",
  "response": "string (free text; with --json-schema, serialized JSON string)",
  "error": "string (only on failure)",
  "finished_reason": "print_timeout | interrupted | error (omitted on normal completion; v1.4.11+)",
  "denied_actions": ["... (present when tool actions were soft-denied; v1.1.27+)"],
  "duration_seconds": 7.16,
  "num_turns": 1,
  "structured_output": { "...": "parsed JSON object; only with --json-schema" },
  "json_schema": { "...": "enforced JSON schema object; only with --json-schema" },
  "usage": {
    "input_tokens": 10415,
    "output_tokens": 657,
    "thinking_tokens": 616,
    "cache_read_tokens": 8113,
    "total_tokens": 11072
  }
}
```

### 3.3 Streaming `--output-format stream-json` (NDJSON)
Each line is discriminated by `"event"`:

1. **`init`** (emitted once per process, even in multi-turn stdin sessions):
```json
{
  "event": "init",
  "conversation_id": "c3b66b04-872b-4fbe-a3a4-058a026ef20a",
  "init": {
    "cwd": "/home/user/project",
    "tools": ["ask_permission", "run_command", "write_to_file"],
    "permission_mode": "request-review",
    "model": "optional (only when overridden with --model)",
    "agent": "optional (only when overridden with --agent)",
    "json_schema": { "...": "optional (only when --json-schema set)" }
  }
}
```

2. **`step_update`** (per step state transition or incremental text chunk):
```json
{
  "event": "step_update",
  "step_update": {
    "conversation_id": "string",
    "step_index": 0,
    "state": "ACTIVE | DONE",
    "step_type": "user_input | agent_response | tool | checkpoint",
    "tool_name": "string (on tool steps)",
    "text_delta": "string (incremental text)",
    "duration_seconds": 6.28,
    "usage": {
      "input_tokens": 10302,
      "output_tokens": 582,
      "thinking_tokens": 551,
      "cache_read_tokens": 8113,
      "total_tokens": 10884
    },
    "tool_info": {
      "name": "run_command",
      "parameters": { "CommandLine": "echo hello" },
      "output": "hello\r\n",
      "error": { "type": "string", "message": "string" }
    },
    "subagent_info": {
      "subagents": [
        {
          "type_name": "string",
          "role": "string",
          "conversation_id": "string",
          "log_uri": "string",
          "workspace_uris": ["string"]
        }
      ]
    }
  }
}
```
- `subagent_info` replaces `tool_info` on subagent-spawning steps.

3. **`result`** (once per turn at completion; same fields as `--output-format json`):
```json
{
  "event": "result",
  "result": {
    "conversation_id": "string",
    "status": "SUCCESS",
    "response": "string",
    "error": "optional",
    "finished_reason": "optional: print_timeout | interrupted | error",
    "denied_actions": ["optional"],
    "duration_seconds": 6.88,
    "num_turns": 1,
    "structured_output": { "...": "optional" },
    "json_schema": { "...": "optional" },
    "usage": { "...": "..." }
  }
}
```

### 3.4 Multi-Turn `--input-format stream-json` (NDJSON on `stdin`)
- Requires `--output-format stream-json`; do **not** pass a `-p` prompt string.
- Emits one `init`, then for each `user` line emits `step_update`s + one `result`. Closing `stdin` exits `0` cleanly.
- **Cumulative vs per-turn in multi-turn `result`:** `response` is current turn only; `num_turns`, `usage`, and `duration_seconds` are **cumulative** across the entire session.
- **Valid `stdin` payloads:**
```json
{"event": "user", "message": {"content": "Prompt string"}}
{"event": "user", "message": {"content": [{"type": "text", "text": "Prompt string"}]}}
```
- **Input validation behavior:**
  - Unrecognized `event` name -> logs warning to `stderr` and continues.
  - `control_request`, `control_response`, or CLI-answered slash command (e.g. `/model`, `/usage`) -> emits `ERROR` `result` and exits **`2`**.
  - Invalid JSON, missing `event`, or content block with `type != "text"` -> emits `ERROR` `result` and exits **`1`**.

### 3.5 Headless Exit Codes & `AGY_ERROR`
| Exit Code | Meaning |
| --- | --- |
| `0` | Success (including runs with soft-denied tool calls). |
| `1` | Validation/startup/general failure: unknown `--model`, unknown `--agent`, invalid `--json-schema` (`v1.2.14+`), auth required, malformed `stream-json` input, unsupported content block type. |
| `2` | Usage / unsupported feature: combining `-p` + `-i`, `control_request`/`control_response` or CLI-answered slash commands in `stream-json` input, `--remote-control` / `--remote-agent` / `/usage` / `/quota` / `/credits` under `GEMINI_API_KEY` (`v1.5.11`–`v1.5.12`). |
| `3` | Agent or model API failure mid-turn (even after partial streamed output, `v1.2.10+`) or `--print-timeout` expiration (`v1.5.0+`). Prints structured `AGY_ERROR: {...}` JSON on `stderr` (canonical status, HTTP/gRPC code, retryability, error ID, and terminated task names on timeout). |

### 3.6 Terms of Service Compliance (`faq.md`)
`agy -p` is intended for direct scripting and CI. **Wrapping `agy -p` inside third-party coding agents/applications or using consumer Antigravity logins in third-party software is explicitly unsupported and violates consumer Terms of Service.** Compliant third-party wrappers must use ACP with sanctioned credentials (Gemini Enterprise, GCP Pay As You Go / Vertex AI, or Google AI Studio API key) or the official Python SDK.

---

## 4. Complete `settings.json`, `admin_settings.json`, Paths & Environment Variables

### 4.1 CLI `settings.json` Schema (`~/.gemini/antigravity-cli/settings.json`)
- Official JSON Schema: `https://antigravity.google/schemas/v1/settings.json` (unknown keys preserved; sparse persistence removes keys equal to defaults; atomic `0600` writes; UTF-8 BOM supported since `v1.5.8`).
- `hooks` key in `settings.json` is preserved but **never loaded** (hooks live in `hooks.json`).

#### Scalar & Array Keys (All 28 Keys, Including `v1.5.14` `showThinking`)
| Key | Type | Default | Allowed Values / Notes |
| --- | --- | --- | --- |
| `agentMode` | `string` | `"auto"` where available (`v1.5.7+`), else `""` | `""` (`default`), `"accept-edits"`, `"auto"`, `"plan"`. Overridden by `--mode`. |
| `toolPermission` | `string` | `"request-review"` | `"request-review"`, `"proceed-in-sandbox"`, `"always-proceed"` (overrides `auto`), `"strict"` (prompts every tool incl. reads; overrides `auto` & `accept-edits`, `v1.5.7+`). |
| `artifactReviewPolicy` | `string` | `"asks-for-review"` | `"asks-for-review"`, `"agent-decides"`, `"always-proceed"`. |
| `enableTerminalSandbox` | `boolean` | `false` | OS terminal sandbox (`--sandbox` overrides; auto-on in `auto` mode). |
| `allowNonWorkspaceAccess` | `boolean` | `false` | Allows reading/editing outside workspace subject to permission rules. |
| `showThinking` | `boolean` | `false` | *(Added `v1.5.14`, `/thinking [on\|off]`)* Streams full thinking live and keeps thoughts expanded. |
| `colorScheme` | `string` | `"terminal"` | `"terminal"`, `"light"`, `"solarized light"`, `"colorblind-friendly light"`, `"dark"`, `"solarized dark"`, `"colorblind-friendly dark"`, `"tokyo night"`. |
| `altScreenMode` | `string` | `"default"` | `"default"` (inline), `"always"` (full screen altscreen), `"never"` (`settings.json` only). |
| `runningLightSpeed` | `string` | `"medium"` | `"fast"`, `"medium"`, `"slow"`, `"off"`. |
| `verbosity` | `string` | `"medium"` | `"high"`, `"medium"`, `"low"`. |
| `hideActiveItems` | `boolean` | `false` | Hides active subagents/tasks tray below prompt. |
| `copyOnSelect` | `boolean` | `true` | Auto-copies mouse selection in alt-screen mode. |
| `editor` | `string` | `"auto"` | `"auto"` (`$EDITOR` -> `vim` / `edit` / `notepad`), `"vim"`, `"emacs"`, or custom command. |
| `editorMode` | `string` | `"default"` | `"default"` or `"vim"`. |
| `vimInsertFirst` | `boolean` | `false` | Starts prompts in Vim Insert mode with bare Enter submit. |
| `notifications` | `boolean` | `false` | Terminal bell + desktop notification on approval/completion. |
| `queuedMessages` | `string` | `"queue"` | `"queue"` or `"send-immediately"` (`Alt+M` one-shot inverse). |
| `pickerGrouping` | `string` | `"none"` | `"none"` or `"grouped"` (`/resume` workspace grouping). |
| `useG1Credits` | `boolean` | `false` | Enables Google One AI credit overages (personal plans only). |
| `showTips` | `boolean` | `true` | Shows tips while generating. |
| `showFeedbackSurvey` | `boolean` | `true` | Shows periodic feedback surveys. |
| `clearScrollbackOnResize` | `boolean` | `true` | Clears scrollback on inline resize. |
| `disableSlashCommands` | `boolean` | `false` | Disables slash/skill expansion in `-p` (`--disable-slash-commands`). |
| `historySize` | `integer` | `2000` | Prompt history entries persisted (`-1` unlimited, `0` clear). |
| `model` | `string` | `""` | Saved model label from `/model`. |
| `modelProvider` | `string` | `""` | `"gemini"` routes directly to Gemini API using `GEMINI_API_KEY`. |
| `trustedWorkspaces` | `string[]` | `[]` | Auto-populated trusted workspace folders. |
| `useTerminalCursor` | `boolean` | `false` | Uses terminal's native cursor shape. |

#### Object Keys (All 7 Keys, Including `v1.5.14` `orchestrator`)
1. `permissions`: `{ "allow": [...], "ask": [...], "deny": [...] }` (Global CLI scope; see [`platform-security-and-extensibility.md`](./platform-security-and-extensibility.md)).
2. `orchestrator` *(added `v1.5.14`, also valid in `--settings`)*: Stores selected `Planner` + `Executor` models and per-role thinking `Effort` on consumer plans.
3. `statusLine`: `{ "type": "command", "command": "...", "enabled": true, "padding": 0, "stack_with_default": false }`.
4. `title`: `{ "type": "command", "command": "...", "enabled": true }`.
5. `sandboxProxy`: `{ "http": { "address": "host:port", "certFile": "/path/ca.pem", "username"?: "...", "password"?: "..." } }`.
6. `customModelsConfig`: `{ "customModels": [{ "modelName": "gemini-api://<base>/models/<model>", "label"?: "...", "modelFeatures"?: { ... } }] }` (only read when `modelProvider == "gemini"` + `GEMINI_API_KEY`).
7. `gcp`: `{ "project": "<GCP_PROJECT_ID>", "location": "global | us | eu" }`.

*(ACP/IDE `settings.json` also supports Hjson and `"auth": { "type": "oauth-personal" | "oauth-business" | "gemini-api-key" | "agent-platform" }`.)*

### 4.2 Managed Admin Settings (`admin_settings.json`)
Enforces organization policy ceilings that cannot be bypassed by `--dangerously-skip-permissions`, `PreToolUse` `auto_approve`, or `command(*)` wildcards:
- `pluginControls`: `{ "pluginsDisabled": bool, "marketplaceDisabled": bool }` (`v1.5.9+`)
- `featureControls`: `{ "remoteControlEnabled": bool }` (`v1.5.8+`)
- `gateway`: `{ "routeWebFetch": bool }` (`v1.5.8+`)
- `proxy`: `{ "address": string }`, `caCertPath` (`string`)
- `models`: `{ "listSource": ..., "list": [{ ..., "supportsThoughtCirculation": bool, "maxOutputTokens": int }] }` (`v1.5.6+`, default 65,535 output tokens for custom Gemini models)

### 4.3 `statusLine` & `title` Command `stdin` JSON Payload
Piped to custom scripts (max 1 run / 300ms, 5s timeout, auto-disabled after 30 failures):
- Top-level: `cwd`, `session_id`, `conversation_id`, `conversation_title?`, `transcript_path`, `version`, `product` (`"antigravity"`), `plan_tier?`, `email`, `terminal_width`, `exceeds_200k_tokens`, `agent_state` (`idle|working|tool_use|reviewing|authenticating|initializing|error`), `cycle_mode?` (`accept-edits|auto|plan`), `artifact_count?`, `pending_input_count?`, `tool_confirmation_pending?`, `task_count?`
- Nested: `model: { id, display_name, effort } | null`, `workspace: { current_dir, project_dir }`, `context_window: { total_input_tokens, total_output_tokens, context_window_size, used_percentage, remaining_percentage, current_usage: { input_tokens, output_tokens, cache_creation_input_tokens, cache_read_input_tokens } | null }`, `agent?: { name }`, `quota: { "<bucket>": { remaining_fraction|remaining_amount, reset_time, reset_in_seconds, disabled? } }`, `vcs?: { type: "git"|"jj"|"hg" }`, `sandbox: { enabled }`, `subagents: [{ name, role, status: "running"|"completed", model? }]`, `vim?: { mode }`, `cost?: { total_usd, subagent_usd, estimated }`.

### 4.4 Complete On-Disk File Layout
| Purpose | macOS / Linux | Windows |
| --- | --- | --- |
| CLI binary | `~/.local/bin/agy` | `%LOCALAPPDATA%\agy\bin\agy.exe` |
| IDE-managed CLI cache | `~/.gemini/bin/agy`, `agy.version.json`, `.agy-install.lock` | `%USERPROFILE%\.gemini\bin\` |
| CLI settings & keybindings | `~/.gemini/antigravity-cli/{settings,keybindings}.json` | `%USERPROFILE%\.gemini\antigravity-cli\` |
| CLI logs & ACP logs | `~/.gemini/antigravity-cli/cli.log`, `log/acp-*.log` | `%USERPROFILE%\.gemini\antigravity-cli\log\{cli,acp}-*.log` |
| CLI caches & updater | `~/.gemini/antigravity-cli/cache/{last_conversations,projects}.json`, `CHANGELOG.md`; `updater/{update.lock,last_check.timestamp}` | Same under `%USERPROFILE%\.gemini\antigravity-cli\` |
| Shared user config | `~/.gemini/config/config.json`, `projects/`, `mcp_config.json`, `hooks.json`, `plugins/`, `agents/`, `rules/`, `sidecars/` | Same under `%USERPROFILE%\.gemini\config\` |
| Workspace customizations | `<ws>/.agents/{mcp_config.json,hooks.json,skills/,rules/,agents/,plugins/}`, `<repo>/.gemini/config.json` | Same |
| Transcripts & brain dirs | `~/.gemini/{antigravity,antigravity-cli,antigravity-acp,antigravity-ide}/brain/<conv-id>/.system_generated/logs/transcript.jsonl` | Same under `%USERPROFILE%\.gemini\` |
| File-backed OAuth token | `~/.gemini/{antigravity-acp,antigravity-cli}/antigravity-oauth-token` (fallback when keyring unavailable / `AGY_ACP_FORCE_FILE_STORAGE=1`) | Same |
| MCP OAuth tokens | `~/.gemini/antigravity/mcp_oauth_tokens.json` | Same |
| Remote Control daemon logs | Linux: `journalctl --user -u antigravity-cli-daemon`; macOS: `~/Library/Logs/antigravity-cli-daemon.log` | `%USERPROFILE%\.gemini\antigravity-cli\log\cli-*.log` |

### 4.5 Complete Environment Variables Inventory (Docs + `agy 1.5.14` Binary)
| Variable | Scope | Purpose |
| --- | --- | --- |
| `GEMINI_API_KEY` | CLI, ACP, SDK | AI Studio API key (**CLI requires `modelProvider: "gemini"` in `settings.json`**). |
| `GOOGLE_GEMINI_BASE_URL` | CLI | Custom Gemini API base URL. |
| `AGY_ADC_AUTH` | CLI, App, IDE | Set `true`/`1` to enable Google Cloud ADC auth. |
| `GOOGLE_CLOUD_PROJECT`, `GOOGLE_CLOUD_QUOTA_PROJECT`, `GOOGLE_CLOUD_LOCATION` | CLI, ACP, SDK | GCP project & region (`global`, `us`, `eu`, or Vertex region). |
| `GOOGLE_GENAI_USE_VERTEXAI` | SDK | Routes Python SDK to Vertex AI. |
| `GEMINI_HOME` | ACP, CLI | Base `.gemini` directory override. |
| `AGY_ACCOUNT` | Sandboxed Plugin CLIs | Selects a specific connected plugin account name (not user label). |
| `AGY_CLI_DISABLE_AUTO_UPDATE` | CLI | Disables background self-updater (`true`/`1`). |
| `AGY_ACP_FORCE_FILE_STORAGE` | ACP | Forces file-backed OAuth token storage instead of OS keyring. |
| `AGY_CLI_HIDE_LOGO`, `AGY_CLI_HIDE_ACCOUNT_INFO`, `AGY_CLI_DISABLE_LATEX`, `AGY_CLI_DISABLE_ESCAPE_SEQUENCE_OPTIMIZATIONS`, `AGY_CLI_CMD_OUTPUT_PERCENTAGE`, `AGY_CLI_MAC_OPTION_AS_ALT`, `AGY_CLI_OSC7501`, `AGY_CLI_OSC7501_MESSAGES`, `AGY_CLI_THEME`, `AGY_CLI_AUTO_COMPACT_OVERRIDE` | CLI TUI | Display, rendering, terminal status (OSC 7501), and compaction overrides. |
| `AGY_LLM_GATEWAY_*`, `AGY_LLM_GATEWAY_MODELS`, `AGY_LLM_GATEWAY_PROXY_URL` | Enterprise | Custom LLM gateway allowlist and proxy configuration. |
| `ANTIGRAVITY_MIC` | CLI | Loopback `host:port` of SSH-forwarded `agy mic-serve` for `/voice`. |
| `ANTIGRAVITY_DISABLE_EMBEDDED_RIPGREP` | CLI | Skips extracting bundled `ripgrep` (`v1.5.5+`). |
| `ANTIGRAVITY_PYTHON` | App, CLI | Custom Python 3.8+ path for `run_workflow`. |
| `ANTIGRAVITY_SSH_BIN`, `ANTIGRAVITY_RESOLVING_ENVIRONMENT` | App (SSH) | Custom `ssh` binary; set to `1` during login-shell env probe. |
| `ANTIGRAVITY_EXECUTABLE_DATA_DIR`, `ANTIGRAVITY_LS_ADDRESS`, `ANTIGRAVITY_CSRF_TOKEN`, `ANTIGRAVITY_CONVERSATION_ID`, `ANTIGRAVITY_PROJECT_ID`, `ANTIGRAVITY_SIDECAR_UI_TOKEN`, `ANTIGRAVITY_SIDECAR_WEB_PORT` | Sidecars / Runtime | Sidecar runtime data, local LS loopback address & CSRF token, conversation context. |
| `ANTIGRAVITY_HARNESS_PATH` | SDK | Override path to bundled `localharness` binary. |
| `PLUGIN_ROOT`, `PLUGIN_DATA` | Plugin Hooks & MCP | Injected into plugin hooks and MCP servers (`v1.5.3+`). |
| `CLI_GRAPHICS`, `NO_COLOR`, `CLICOLOR`, `CLICOLOR_FORCE`, `COLORTERM`, `LC_TERMINAL`, `EDITOR` | CLI | Terminal graphics (Kitty passthrough), color, and external editor. |

---

## 5. Complete CLI Slash Command Reference

- **Stacking (`v1.1.4+`):** Multiple leading prompt/skill commands chain (e.g. `/plan /grill-me <task>`).
- **Non-slash prefixes:** `! <cmd>` (direct shell, interactive only), `@<path>` (file/dir context), `@<subagent> <msg>` (direct subagent DM), `?` (shortcuts help on empty prompt).

| Command | Aliases | Syntax | Headless (`-p`) / `stream-json` / ACP Support | Summary |
| --- | --- | --- | --- | --- |
| `/add-dir` | — | `/add-dir <path>` | Interactive only (`--add-dir`) | Adds directory to workspace. |
| `/agents` | — | `/agents` | Interactive only (`--agent`, `agy agents`) | Switch custom agents & inspect/approve/kill subagents. |
| `/agy-customizations` | — | `/agy-customizations [req]` | Prompt/skill-expanded in `-p` & ACP | Loads Antigravity Customization System Guide. |
| `/antigravity-guide` | — | `/antigravity-guide [q]` | Prompt/skill-expanded in `-p` & ACP | Loads official AGY docs guide & sitemap. |
| `/artifact` | — | `/artifact` | Interactive only | Artifact picker (`Ctrl+R`) & viewer (approve/reject/comment). |
| `/boost` | — | `/boost <task>` | Prompt/skill-expanded in `-p` (paid) | Multi-agent adversarial deep reasoning loop. |
| `/browser` | — | `/browser <task>` | Prompt/skill-expanded in `-p` | Delegates browser testing/debugging to Chrome DevTools MCP subagent (`.mp4` recordings `v1.5.9+`). |
| `/btw` | — | `/btw <query>` | Interactive only (local only) | Read-only parallel side question without polluting main history. |
| `/changelog` | — | `/changelog` | **CLI-answered read-only in `-p`** (forbidden in `stream-json` input) | Release notes viewer. |
| `/clear` | `/new` | `/clear` | Interactive only (refused in `-p`) | Resets conversation. |
| `/codesearch` | `/cs`, `/search` | `/codesearch [-F] [f:<glob>] <q>` | Interactive only (local only) | Progressive regex/literal code search overlay. |
| `/compact` | — | `/compact [msg]` | **Supported in `-p`** & ACP | Summarizes conversation into compaction checkpoint. |
| `/config` | `/settings` | `/config` | **CLI-answered read-only in `-p`** (forbidden in `stream-json` input) | Interactive settings editor (`TUI` + `Remote Agent` tabs). |
| `/context` | — | `/context` | Interactive only | Visual context window grid & token breakdown. |
| `/copy` | — | `/copy [n\|btw]` | Interactive only | Copies response `n` or latest `/btw` to clipboard. |
| `/credits` | — | `/credits` | **CLI-answered read-only in `-p`** (exits `2` under `GEMINI_API_KEY`) | AI Premium credits balance & usage. |
| `/diff` | — | `/diff` | Interactive only (local only) | 3-tab diff viewer (All Changes, Per Turn, Commit Tree) + line comments. |
| `/effort` | — | `/effort [low\|medium\|high\|xhigh\|max]` | **CLI-answered read-only in `-p` (no args)** | Shows or sets reasoning effort level. |
| `/exit` | `/quit` | `/exit` | Interactive only | Exits CLI & prints resume command. |
| `/feedback` | — | `/feedback` | Interactive only | Opens feedback dialog. |
| `/fork` | `/branch` | `/fork` | Interactive only (local only) | Clones conversation thread into independent session. |
| `/goal` | — | `/goal <objective>` | Prompt/skill-expanded in `-p` | Autonomous goal loop with stop-hook verification. |
| `/grill-me` | — | `/grill-me [task]` | Prompt/skill-expanded in `-p` | One-question-at-a-time scoping interview. |
| `/help` | — | `/help` | **CLI-answered read-only in `-p`** | Help panel & account/log info. |
| `/hooks` | — | `/hooks` | **CLI-answered read-only in `-p`** | Browse/edit/toggle hooks. |
| `/keybindings` | — | `/keybindings` | Interactive only | Opens `keybindings.json` in external editor. |
| `/learn` | — | `/learn [instruction]` | Prompt/skill-expanded in `-p` | Captures corrections into persistent rules/skills via proposal. |
| `/logout` | — | `/logout` | Interactive + **ACP** (`logout`); refused in `-p` | Signs out and purges keyring/file tokens. |
| `/mcp` | — | `/mcp` | Interactive only (`agy mcp ...`) | Interactive MCP server manager & OAuth sign-in. |
| `/model` | — | `/model [model] [prompt]` | **CLI-answered read-only in `-p`** (`--model`) | Switches model (or runs 1-turn override with `<model> <prompt>`). |
| `/open` | — | `/open <path>` | Interactive only | Opens file in external editor. |
| `/permissions` | — | `/permissions` | **CLI-answered read-only in `-p`** | 3-panel permission rule manager (Project/Shared/Global). |
| `/plan` | — | `/plan [task]` | Prompt-expanded in `-p` & **ACP** | Read-only exploration & reviewable implementation plan. |
| `/plugin` | `/plugins` | `/plugin [subcmd]` | Interactive (`agy plugin ...`) | Marketplace discovery, install, MCP config & OAuth. |
| `/projects` | — | `/projects` | Interactive (`--project`, `--new-project`) | Lists/creates/switches local & remote projects (`v1.5.0+`). |
| `/record` | — | `/record` | Interactive only (personal local only) | Starts voice dictation into composer (`F5`). |
| `/remote-control` | — | `/remote-control [on\|off]` | Interactive (`--remote-control`, `agy remote-control`) | Toggles session-scoped Remote Control tunnel. |
| `/rename` | — | `/rename <name>` | Interactive only | Renames current conversation. |
| `/resume` | `/switch`, `/conversation` | `/resume [id-or-prefix]` | Interactive (`-c`, `--conversation`) | Resumes conversation (unique prefix supported `v1.5.9+`; **Other** tab imports App chats). |
| `/rewind` | `/undo` | `/rewind` | Interactive only (local only) | Rolls back history & file snapshots to earlier step. |
| `/rules` | — | `/rules` | Interactive | Inspects loaded rule files. |
| `/run-workflow` | — | `/run-workflow <task>` | Prompt/skill-expanded in `-p` | Python dynamic workflow orchestration (`v1.5.9+`). |
| `/schedule` | — | `/schedule <when> <task>` | Prompt/skill-expanded in `-p` | One-shot timer or recurring 5-field cron waking agent. |
| `/skills` | — | `/skills [reload]` | **CLI-answered read-only in `-p`** | Lists/reloads loaded skills (`json` includes `builtin` flag). |
| `/statusline` | — | `/statusline [on\|off\|reset\|<cmd>]` | Interactive only | Configures custom status line script. |
| `/swarm` | — | `/swarm <task>` \| `/swarm board` | `/swarm <task>` expanded in `-p`; `board` interactive | Multi-agent coordinator/worker/verifier/monitor swarm (`v1.5.5+`). |
| `/tasks` | — | `/tasks` | Interactive only | Monitors background commands, timers, crons & logs. |
| `/teamwork` | — | `/teamwork <task>` | Prompt/skill-expanded in `-p` (paid) | Long-horizon multi-agent project execution. |
| `/thinking` | — | `/thinking [on\|off]` | Interactive (`v1.5.14+`, saves `showThinking`) | Streams full thinking live & keeps thoughts expanded. |
| `/title` | — | `/title [on\|off]` | Interactive only | Toggles custom terminal window title. |
| `/usage` | `/quota` | `/usage` | **CLI-answered read-only in `-p`** & **ACP** (`v1.4.0+`); exits `2` on `GEMINI_API_KEY` | Shows 5-hour & weekly model quotas. |
| `/voice` | — | `/voice [live\|dictate]` | Interactive only (personal local only) | Live two-way voice (`F6`) or dictation (`F5`); SSH via `agy mic-serve`. |

---

## 6. Installation, Self-Updater & Gemini CLI Migration

- **Install commands:**
  - macOS / Linux / Googlebook: `curl -fsSL https://antigravity.google/cli/install.sh | bash` -> `~/.local/bin/agy`
  - Windows PowerShell: `irm https://antigravity.google/cli/install.ps1 | iex` -> `%LOCALAPPDATA%\agy\bin\agy.exe`
  - Windows CMD: `curl -fsSL https://antigravity.google/cli/install.cmd -o install.cmd && install.cmd && del install.cmd`
  - Android Termux: Native prebuilt binaries (`v1.2.15+`).
- **Release manifest & artifact endpoints:**
  - Manifests: `https://antigravity-cli-auto-updater-974169037036.us-central1.run.app/manifests/<target>.json` (`darwin_arm64`, `darwin_amd64`, `linux_amd64`, `linux_arm64`, `linux_amd64_musl`, `linux_arm64_musl`, `windows_amd64`, `windows_arm64`).
  - Binaries: `https://storage.googleapis.com/antigravity-public/antigravity-cli/<ver>-<build>/<folder>/cli_<os>_<arch>[_musl].{tar.gz,exe}` (verified via 128-hex SHA-512).
- **Self-updater mechanics:** Runs in background on launch with 15-min debounce (`~/.gemini/antigravity-cli/updater/last_check.timestamp`) and advisory lock (`update.lock`). Disable via `AGY_CLI_DISABLE_AUTO_UPDATE=true`.
- **Gemini CLI migration:** First launch offers auto-migration; manual import via `agy plugin import gemini` (or `claude`); workspace skills must move from `.gemini/skills/` -> `.agents/skills/`; MCP configs live in `~/.gemini/config/mcp_config.json` & `<ws>/.agents/mcp_config.json` (JSONC supported, `serverUrl` or `url`).
