# Core: Exec Streaming and Truncation

This document focuses on how exec output is streamed to the UI and truncated to
avoid unbounded memory or event floods.

Files:
- `codex-rs/core/src/exec.rs`
- Protocol types: `ExecCommandOutputDeltaEvent`, `ExecOutputStream`

## Streaming

- When a process is spawned under a sandbox, its stdout is read incrementally
  and forwarded as deltas (`ExecOutputStream`) on the event channel.
- The sender wraps the stream id (`sub_id`) and tool call id (`call_id`) so the
  UI can multiplex multiple concurrent tool streams and preserve ordering.

## Truncation limits

- `MAX_EXEC_OUTPUT_DELTAS_PER_CALL` caps the number of delta events per exec
  call to limit UI updates and IPC overhead on very chatty commands.
- Aggregated output is still collected fully (up to memory and timeout limits)
  and returned in the final function call output payload.

## Timeouts

- Each call uses a resolved `timeout_duration()`; if the child fails to exit in
  time, we treat it as a sandbox timeout and surface a structured error with any
  captured output.

