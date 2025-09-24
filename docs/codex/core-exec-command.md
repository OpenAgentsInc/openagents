# Core: Exec Command Sessions

Exec sessions expose fine‑grained control beyond a single one‑shot command: you
can open a PTY session, write stdin incrementally, and close it later.

Files:
- `codex-rs/core/src/exec_command.rs`
- `codex-rs/core/src/codex.rs` (dispatch: `EXEC_COMMAND_TOOL_NAME`, `WRITE_STDIN_TOOL_NAME`)

## Tools

- `exec_command` — spawn/manage shell sessions (start, stop, resize, etc.).
- `write_stdin` — write bytes into an existing session’s stdin.
- `unified_exec` — simpler, single tool alternative (see openai_tools docs).

## Flow

1. Tool call is parsed into `ExecCommandParams` / `WriteStdinParams`.
2. The session manager (`ExecSessionManager`) owns state keyed by session id.
3. Output is streamed as `ExecOutputStream` deltas into the event channel.
4. Payloads are converted into `FunctionCallOutputPayload` structures for the
   protocol.

## Safety

- Approval policy is still enforced; most `write_stdin` operations happen within
  a policy previously approved for the session.
- Sandboxing decisions are made when the session is created, not on each
  `write_stdin`.

