# Resuming Conversations with `codex exec --json`

This document explains how resume works with the non‑interactive Exec CLI and how to use it reliably in JSONL mode. It also calls out version differences you may encounter.

## Supported Syntax (newer Codex builds)

Recent Codex builds include a dedicated subcommand under Exec:

- Inspect support: `codex exec --help` should list a `resume` command. Or run `codex exec resume --help`.

Common patterns when supported:

- Start fresh and read prompt from stdin:
  ```bash
  echo "first prompt" | codex exec --json -
  ```
- Resume most recent and read the next prompt from stdin:
  ```bash
  echo "next prompt" | codex exec --json resume --last -
  ```
- Resume by id and read from stdin:
  ```bash
  echo "next prompt" | codex exec --json resume <SESSION_ID> -
  ```
- Positional prompt is also accepted:
  ```bash
  codex exec --json resume --last "next prompt"
  ```

Notes:
- `-C <DIR>` (aka `--cd`) sets the working directory for the session.
- `--skip-git-repo-check` allows running outside a Git repo.
- Model/sandbox flags you pass on the command line apply when resuming (they override the stored config).

## Older Codex builds (no `exec resume`)

Some installed versions of Codex do not include the `resume` subcommand under `exec`. In those versions, `codex exec --json resume --last "…"` fails with an argument error (because `resume` is parsed as the prompt and `--last` is unexpected). You can detect this case because `codex exec --help` shows no `resume` command.

Behavior in that case:

- `codex exec` always starts a new session; there is no non‑interactive resume.
- For interactive flows you can still run `codex resume` (interactive CLI), but that is not JSONL/exec mode.
- The OpenAgents bridge detects the absence of `exec resume` and falls back to starting a fresh session on each message.

If you need `exec resume`, update Codex to a build that includes it (the Rust workspace target `codex-exec` shows `resume` in its `--help`).

## What “Resume” Does

- Loads a previous rollout from `~/.codex/sessions/**/rollout-*.jsonl`.
- Reconstructs the conversation state and continues in the same file (append).
- Emits JSONL to stdout; the first line on a run is:
  ```json
  {"type":"thread.started","thread_id":"<SESSION_ID>"}
  ```
- History is used to seed state but is not re‑emitted; you only see new turn/items.

## Choosing What to Resume

When `exec resume` is available, you can choose:

- `--last` — newest recorded session across sources.
- `<SESSION_ID>` — a specific UUID (present in filename suffix and in the first meta line `payload.id`).

Examples:

```bash
# Resume newest and prompt inline
codex exec --json resume --last "continue from where we left off"

# Resume by id and read prompt from stdin
codex exec --json resume 5973b6c0-94b8-487b-a530-2aeb6098ae0e -
```

## Code References (current implementation)

- Exec CLI surface and `resume` subcommand: `codex-rs/exec/src/cli.rs`
- Prompt selection (parent vs subcommand): `codex-rs/exec/src/lib.rs`
- Resume path resolution and conversation restore: `codex-rs/exec/src/lib.rs`
- JSONL renderer `thread.started`: `codex-rs/exec/src/event_processor_with_jsonl_output.rs`
- Rollout persistence and discovery helpers: `codex-rs/core/src/rollout/*`

## Bridge Integration (OpenAgents app)

- First message (fresh session):
  ```
  codex exec --json -
  ```
- Subsequent messages (when supported):
  ```
  codex exec --json resume --last -
  # or
  codex exec --json resume <SESSION_ID> -
  ```
- The bridge captures `thread.started.thread_id` from JSONL and reuses it when respawning.
- If `exec resume` isn’t supported, the bridge logs a note and starts a fresh session.

Tip: To target a known‑good binary during development, point the bridge at a locally built Codex:
```
cargo run -p codex-bridge -- --codex-bin ~/code/codex-openai/codex-rs/target/release/codex
```
