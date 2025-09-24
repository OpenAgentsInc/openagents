# Core: Orchestration (`codex.rs`)

`codex-rs/core/src/codex.rs` is the central orchestrator for a session. It
consumes `ResponseEvent`s from the model, renders text/reasoning deltas,
dispatches tool calls, applies patches, and records rollouts.

## Responsibilities

- Drive turn lifecycle: start, stream, tools, finish.
- Translate model tool calls to internal actions.
- Enforce approval and sandbox policies for exec.
- Track diffs across a turn (`TurnDiffTracker`).
- Persist rollout lines to disk (`RolloutRecorder`).

## Tool call handling

- Function calls are dispatched by `match name.as_str()` inside the handler for
  input items:
  - `unified_exec` — multiplexes spawn/interactive stdin with a single tool.
  - `view_image` — attaches a local image path to context.
  - `apply_patch` — forwards to the CLI/grammar and wires result into diffs.
  - `exec_command` and `write_stdin` — low-level exec session management.
  - Unknown tools are surfaced as structured failures so models can adapt.

- Custom tools (Responses `type: "custom"`) are converted to function-call
  output when needed so the rest of the pipeline remains uniform.

## Exec path

- `handle_container_exec_with_params` prepares an `ExecParams`, optionally
  translates for PowerShell/profile scenarios, and calls
  `process_exec_tool_call` with a platform `SandboxType`.
- Safety checks and approval policy are applied before any escalated request.
- Truncation of live output is enforced by `MAX_EXEC_OUTPUT_DELTAS_PER_CALL`.

## Apply Patch integration

- Before executing, the code runs `maybe_parse_apply_patch_verified`. If the
  patch is verified, we may apply it internally (no process spawn). Otherwise
  we delegate to the external `apply_patch` command.
- `TurnDiffTracker` hooks (`on_patch_begin`) snapshot baselines and
  `get_unified_diff` produces an aggregated unified diff at the end of the
  turn.

## Rollouts

- Rollout persistence is handled via `RolloutRecorder` (see
  `core-rollout.md`). The orchestrator pushes `RolloutItem`s for meta,
  response items, compaction, and events.

## Reasoning visibility

- `Config.hide_agent_reasoning` and `Config.show_raw_agent_reasoning` control
  how reasoning deltas are shown/suppressed by the UI.

