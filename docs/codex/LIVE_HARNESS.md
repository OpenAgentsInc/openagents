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
  - `turn/start` (if `--prompt`, optional skill attachment via `--skill`)
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
- Optional live Blink swap probe (real network, no mocks):
  - `skills/blink/scripts/swap_quote.js`
  - `skills/blink/scripts/swap_execute.js` (when `--blink-swap-execute-live`)
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

Run a skill-attached turn (example: `blink`):

First install the skill into Codex user skills (one-time):

```bash
python3 /Users/christopherdavid/.codex/skills/.system/skill-installer/scripts/install-skill-from-github.py \
  --repo OpenAgentsInc/openagents \
  --path skills/blink
```

```bash
cargo run -p autopilot-desktop --bin codex-live-harness -- \
  --cwd /Users/christopherdavid/code/openagents \
  --model gpt-5.2-codex \
  --include-writes \
  --skill blink \
  --prompt "Use the blink skill to summarize available payment operations and first command to check balance."
```

Run live Blink swap quote + execute probe:

```bash
cargo run -p autopilot-desktop --bin codex-live-harness -- \
  --cwd /Users/christopherdavid/code/openagents \
  --skip-experimental \
  --skip-thread-mutations \
  --blink-swap-live \
  --blink-swap-direction btc-to-usd \
  --blink-swap-amount 10 \
  --blink-swap-unit sats \
  --blink-swap-execute-live
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
- `--skill <name>`: resolves and attaches a skill in `turn/start` as `UserInput::Skill`
  - Lookup order: local `skills/list` name/path match, then optional remote match/export if `--include-writes`
  - If found but disabled: harness enables it through `skills/config/write` when `--include-writes` is set
- `--list-limit <n>`: `thread/list` limit (default `20`)
- `--drain-ms <n>`: idle settle window for collecting events (default `700`)
- `--timeout-ms <n>`: max wait window for each event drain (default `4000`)
- `--max-events <n>`: print cap per phase for notifications/requests (default `24`)
- `--include-writes`: run write/mutation probes for config/skills/mcp/export paths
- `--skip-experimental`: skip experimental probe group
- `--skip-thread-mutations`: skip thread mutation probes
- `--allow-echo-replies`: disable harness failure when assistant echoes prompt exactly
- `--blink-swap-live`: run a live Blink quote probe from `skills/blink/scripts/swap_quote.js`
- `--blink-swap-direction <btc-to-usd|usd-to-btc>`: swap direction for live probe
- `--blink-swap-amount <n>`: probe amount (`>0`)
- `--blink-swap-unit <sats|cents>`: probe unit (default derives from direction)
- `--blink-swap-execute-live`: run real execute attempt from `skills/blink/scripts/swap_execute.js`
- `--blink-swap-require-success`: fail unless execute returns `SUCCESS`
- `--blink-swap-memo <text>`: optional memo for execute probe

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
