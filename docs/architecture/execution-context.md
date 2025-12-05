# Execution Contexts: Local vs Remote

Durable agent systems need to know which operations can be **safely suspended** and which must **run to completion**. In OpenAgents we distinguish:

- **Local-context operations** – run inside the current process; they cannot be paused and replayed mid-stream. Examples: shell commands, file I/O, verification runs, Healer spells. If the process dies mid-op, we rerun the whole operation (and rely on idempotency/cleanup).
- **Remote-context operations** – delegated to an external service that can continue independently and resume later via an identifier. Example: Claude Code sessions (resume via `sessionId`).

## Why it matters

- **Recovery:** We can resume remote-context ops by reusing their session handles; local-context ops must be retried end-to-end.
- **Idempotency:** Local-context steps (spells, bash, verification) need clear rollback or repeatable side effects.
- **Prompting:** When asking a subagent to resume, include only remote-context handles (e.g., Claude Code session), not partial local actions.

## OpenAgents mapping

- **Local-context (non-suspendable):** `bashTool`, `read/write/edit` tools, `runVerification`, Healer spells (`rewind_uncommitted_changes`, `fix_typecheck_errors`, etc.).
- **Remote-context (suspendable):** Claude Code subagent calls (resume/fork via `sessionId`), future API calls that support idempotency keys.

## Practical guidance

- When orchestrating retries, **restart local-context steps from the top**; don’t assume partial progress is valid.
- For remote-context subagents, **pass through session metadata** so a resumed run can pick up where it left off.
- Design new capabilities with a clear context type: if it’s local, make it idempotent and side-effect scoped; if remote, return a handle that enables resumption.
