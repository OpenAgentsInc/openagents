# Codex Live Harness

## Purpose

`codex-live-harness` is a programmatic integration probe for the Codex app-server used by `autopilot-desktop`.

It is designed to:

- simulate core chat-pane flows (`refresh threads`, `new chat`, `send`)
- exercise the broader app-server API surface from one command
- capture notifications/requests after each call so regressions are visible without GUI interaction

The harness runs against the user’s real local Codex installation and account/session state.

## Location

- Binary: `apps/autopilot-desktop/src/bin/codex_live_harness.rs`
- Target: `cargo run -p autopilot-desktop --bin codex-live-harness`

## Current Model Policy

The harness does **not** default to legacy hardcoded model IDs.

- Default behavior: resolve model from live `model/list` (`is_default == true`, else first model)
- Override behavior: pass `--model <id>`

This keeps probes aligned with currently available Codex models.

## API Coverage

The harness currently probes these app-server methods:

- Auth/account:
  - `account/read`
  - `account/rateLimits/read`
- Models/features:
  - `model/list`
  - `collaborationMode/list`
  - `experimentalFeature/list`
  - `mock/experimentalMethod`
- Config/external config:
  - `config/read`
  - `configRequirements/read`
  - `externalAgentConfig/detect`
  - optional: `externalAgentConfig/import` (`--include-writes`)
- MCP:
  - `mcpServerStatus/list`
  - optional: `config/mcpServer/reload` (`--include-writes`)
- Apps/skills:
  - `app/list`
  - `skills/list`
  - `skills/remote/list`
  - optional: `skills/remote/export` (`--include-writes`)
  - optional: `skills/config/write` (`--include-writes`)
- Threads/chat:
  - `thread/list`
  - `thread/loaded/list`
  - `thread/read`
  - `thread/start`
  - `turn/start` (if `--prompt`)
- Thread mutation probes (default on, disable with `--skip-thread-mutations`):
  - `thread/name/set`
  - `thread/backgroundTerminals/clean`
  - `thread/compact/start`
  - `thread/fork`
  - `thread/rollback` (if turns exist)
  - `thread/archive`
  - `thread/unarchive`
- Additional execution/review probes:
  - `command/exec`
  - `review/start`
- Experimental probes (default on, disable with `--skip-experimental`):
  - `fuzzyFileSearch/sessionStart`
  - `fuzzyFileSearch/sessionUpdate`
  - `fuzzyFileSearch/sessionStop`
  - `thread/realtime/start`
  - `thread/realtime/appendText`
  - `thread/realtime/stop`
  - `windowsSandbox/setupStart`

## Important Runtime Findings (Observed Live)

From a live run on **February 28, 2026**:

- `thread/read` with `includeTurns=true` on a brand-new thread can fail until first user message:
  - error: `thread ... is not materialized yet; includeTurns is unavailable before first user message`
- Some app-server builds do not expose `thread/realtime/*` methods:
  - error: unknown variant for `thread/realtime/start|appendText|stop`
- `windowsSandbox/setupStart` validates `mode` strictly:
  - accepted values include `elevated` or `unelevated`

These are useful compatibility checks, not harness failures.

## Usage

Minimal run:

```bash
cargo run -p autopilot-desktop --bin codex-live-harness -- \
  --cwd /Users/christopherdavid/code/openagents
```

Run with prompt to test end-to-end thread materialization and turn flow:

```bash
cargo run -p autopilot-desktop --bin codex-live-harness -- \
  --cwd /Users/christopherdavid/code/openagents \
  --prompt "harness ping" \
  --max-events 8
```

Run with write probes enabled:

```bash
cargo run -p autopilot-desktop --bin codex-live-harness -- \
  --cwd /Users/christopherdavid/code/openagents \
  --prompt "harness ping" \
  --include-writes
```

Override model explicitly:

```bash
cargo run -p autopilot-desktop --bin codex-live-harness -- \
  --cwd /Users/christopherdavid/code/openagents \
  --model gpt-5.3-codex
```

## Flags

- `--cwd <path>`: working directory sent to app-server
- `--model <id>`: explicit model override (otherwise use live default model)
- `--prompt <text>`: sends a turn after `thread/start`
- `--list-limit <n>`: `thread/list` limit (default `20`)
- `--drain-ms <n>`: idle settle window for collecting events (default `700`)
- `--timeout-ms <n>`: max wait window for each event drain (default `4000`)
- `--max-events <n>`: print cap per phase for notifications/requests (default `24`)
- `--include-writes`: run write/mutation probes for config/skills/mcp/export paths
- `--skip-experimental`: skip experimental probe group
- `--skip-thread-mutations`: skip thread mutation probes

## Output Format

Each probe prints:

- method name
- `status=ok` or `status=error`
- compact summary (counts, selected ids, etc.)
- `post-<method>` event drain summary:
  - notifications count
  - requests count
  - capped event lines

This allows deterministic diffing of behavior between app-server versions.
