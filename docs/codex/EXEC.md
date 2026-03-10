# Autopilot Codex Exec

`autopilot-codex-exec` is the app-owned one-shot Codex surface for scripts,
hooks, and local automation.

It talks to the same Codex app-server integration used by Autopilot Desktop,
but it is shaped for non-interactive runs instead of the desktop UI.

## Usage

Basic run:

```bash
cargo run -p autopilot-desktop --bin autopilot-codex-exec -- \
  "summarize the uncommitted Rust changes in this repo"
```

JSONL event stream:

```bash
cargo run -p autopilot-desktop --bin autopilot-codex-exec -- \
  --json \
  "inspect the repo and report the likely failing test"
```

Ephemeral run:

```bash
cargo run -p autopilot-desktop --bin autopilot-codex-exec -- \
  --ephemeral \
  "draft a migration plan without persisting the session"
```

Resume an existing thread:

```bash
cargo run -p autopilot-desktop --bin autopilot-codex-exec -- \
  --thread-id <thread-id> \
  "continue and finish the refactor"
```

Structured final output:

```bash
cargo run -p autopilot-desktop --bin autopilot-codex-exec -- \
  --output-schema schema.json \
  --output-last-message result.json \
  "return the answer as JSON matching the provided schema"
```

## Supported flags

- `--json` writes machine-readable JSONL events to stdout.
- `--ephemeral` starts an in-memory thread that is not materialized on disk.
- `--thread-id <id>` resumes a prior persistent thread.
- `--image <path>` attaches local images to the initial prompt.
- `--model <id>` selects the Codex model.
- `--cd <dir>` sets the working directory sent to app-server.
- `--sandbox <mode>` sets `read-only`, `workspace-write`, or
  `danger-full-access`.
- `--approval-policy <mode>` sets `never`, `on-request`, `on-failure`, or
  `unless-trusted`.
- `--full-auto` is a convenience alias for `--sandbox workspace-write`.
- `--dangerously-bypass-approvals-and-sandbox` / `--yolo` disables sandboxing.
- `--output-schema <file>` reads a JSON Schema file and sends it in
  `turn/start`.
- `--output-last-message <file>` writes the final assistant message after the
  run finishes.

## JSONL contract

The JSONL stream uses Codex-style event names so existing automation patterns
stay familiar:

- `thread.started`
- `turn.started`
- `item.started`
- `item.updated`
- `item.completed`
- `turn.completed`
- `turn.failed`
- `error`

Autopilot adds explicit `thread_id` and `turn_id` fields on turn and item
events, and each item includes:

- normalized snake_case `type`
- stable convenience fields such as `text`, `command`, `status`, or
  `aggregated_output` when available
- a `raw` object preserving the underlying app-server item payload

This makes the stream usable from `jq`, shell scripts, or local regression
tests without depending on the desktop UI state.

## Non-interactive rules

- The surface is intentionally non-interactive.
- Approval prompts are auto-declined.
- Tool user-input prompts fail fast.
- Auth refresh requests fail fast.
- If you need interactive approvals, use the desktop chat surface instead.
