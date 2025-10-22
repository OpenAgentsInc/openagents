# Resuming Conversations with `codex exec --json`

This guide explains how conversation resume works internally and how to resume a session when using JSONL output mode (`--json`).

## TL;DR (CLI)

- Resume the most recent recorded session and send a new prompt:
  ```bash
  codex exec --json resume --last "your next prompt"
  ```
- Resume by explicit session id (UUID shown in rollout filename or meta):
  ```bash
  codex exec --json resume <SESSION_ID> "your next prompt"
  ```
- Read the next prompt from stdin instead:
  ```bash
  echo "your next prompt" | codex exec --json resume --last -
  ```
- You can also place the prompt before the subcommand (parent-level positional):
  ```bash
  codex exec --json "your next prompt" resume --last
  ```

Notes:
- `-C <DIR>` (aka `--cd`) sets the working directory for the session.
- `--skip-git-repo-check` allows running outside a Git repo.
- Any model/sandbox overrides you pass on the command line apply when resuming.

## What “Resume” Does

- Loads a previously recorded session (a rollout file under `~/.codex/sessions/...`).
- Reconstructs conversation state in memory from that rollout.
- Continues the session and appends new items to the same rollout file.
- Streams fresh events to stdout as JSONL (one event per line). The very first event is:
  ```json
  {"type":"thread.started","thread_id":"<SESSION_ID>"}
  ```

JSON exec mode uses the same resume logic as human output mode — only the renderer differs.

## Where Sessions Are Stored

- Default root: `~/.codex` (configurable via `CODEX_HOME`).
- Rollout files: `~/.codex/sessions/YYYY/MM/DD/rollout-YYYY-MM-DDThh-mm-ss-<uuid>.jsonl`
- The `<uuid>` at the end of the filename is the session id. It is also present on the first meta line inside the file as `payload.id`.

## Choosing What to Resume

The `resume` subcommand accepts one of:

- `--last` — picks the newest session across all sources (CLI, VSCode, Exec, MCP).
- `<SESSION_ID>` — looks up a rollout file by UUID (present in the filename and meta).

If no prior session can be found, Codex starts a new session transparently.

Examples:

```bash
# Resume newest and prompt inline
codex exec --json resume --last "continue from where we left off"

# Resume by id and read prompt from stdin
codex exec --json resume 5973b6c0-94b8-487b-a530-2aeb6098ae0e -
```

## What You’ll See in JSONL

On resume, the renderer emits `thread.started` with the existing session id and then only new turn events. A typical sequence for a resumed turn:

```json
{"type":"thread.started","thread_id":"5973b6c0-94b8-487b-a530-2aeb6098ae0e"}
{"type":"turn.started"}
{"type":"item.completed","item":{"id":"item_0","type":"agent_message","text":"…"}}
{"type":"turn.completed","usage":{"input_tokens":123,"cached_input_tokens":0,"output_tokens":45}}
```

History from the rollout is used to seed state but is not re-emitted in the JSONL stream.

## Behavior Details (Code Reference)

- CLI surface:
  - Subcommand and args: `codex-rs/exec/src/cli.rs:67` and `codex-rs/exec/src/cli.rs:96`
  - Parent vs subcommand prompt selection: `codex-rs/exec/src/lib.rs:66`
- Resume wiring (exec):
  - Path resolution (`--last` or by id): `codex-rs/exec/src/lib.rs:484`
  - When a path is found: `ConversationManager::resume_conversation_from_rollout` used to restore and continue; else falls back to a new conversation: `codex-rs/exec/src/lib.rs:230`
- JSON output on resume:
  - `SessionConfigured` → `{"type":"thread.started"}`: `codex-rs/exec/src/event_processor_with_jsonl_output.rs:106`
- Persistence model:
  - Resume opens the same rollout file for append: `codex-rs/core/src/rollout/recorder.rs:86` and `codex-rs/core/src/rollout/recorder.rs:127`
  - Rollout history load and reconstruction: `codex-rs/core/src/rollout/recorder.rs:248` and `codex-rs/core/src/codex.rs:610`
- Session discovery helpers:
  - List newest sessions (used for `--last`): `codex-rs/core/src/rollout/recorder.rs:62`
  - Find by UUID string: `codex-rs/core/src/rollout/list.rs:483`

## Tips

- To programmatically discover the latest session path, inspect `~/.codex/sessions/**/rollout-*.jsonl` and choose the newest by timestamp/UUID order (same logic as the code does).
- To extract the session id, read the first line (SessionMeta) and parse `payload.id`, or grab it from the filename suffix.
- You can change models/sandbox mode on resume; the new settings take effect immediately and are recorded in the configuration summary emitted at the start of the run.

---

If you change the resume behavior or the JSON event mapping, please update this document alongside the relevant code.
