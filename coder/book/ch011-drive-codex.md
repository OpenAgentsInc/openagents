# 11. Drive Codex

> **Status: unimplemented — design draft.** The chapter text and its checks aren't written yet. The
> client exists in `coax/lib/agent/codex/`, with its parity table in
> [PARITY.md](../../coax/lib/agent/codex/PARITY.md).

Codex is the other engine many people have. Coder drives it through
`codex app-server`, the Codex CLI's JSON-RPC 2.0 server over stdio.

## What this chapter covers

- A pull-based client in caller storage: requests, answers to server
  requests and `cx_feed` over lines.
- Approvals that Codex asks for, answered by chapter 5's policy.
- Two recorded sessions, scrubbed, replayed byte for byte.
- `codex app-server` as a child process.

## Dependencies

- [Chapter 2](PLAN.md#i-the-loop) and [Chapter 5](PLAN.md#i-the-loop).
- [Chapter 10](ch010-drive-claude-code.md): the same transport pattern.

## Done means

- The request, replay, approval and refusal tests pass.
- `openagents coder run --engine codex "TASK"` runs a task through the
  installed CLI, and its events appear in `coder log`.
- The live smoke runs against a named Codex version.

## Sources

- `coax/lib/agent/codex/` and its [PARITY.md](../../coax/lib/agent/codex/PARITY.md).
