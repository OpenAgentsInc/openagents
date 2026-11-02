# 10. Drive Claude Code

> **Status: unimplemented — design draft.** The chapter text and its checks aren't written yet. The
> client exists in `coax/lib/agent/claude/`, with its parity table in
> [PARITY.md](../../coax/lib/agent/claude/PARITY.md).

Many people already pay for Claude Code. Coder can run a task through it
instead of through its own loop, and keep the same log, approvals and replies.

## What this chapter covers

- The `claude` CLI's stream-json protocol, not the npm package: frames, typed
  stream messages and the control protocol in both directions.
- The query as a state machine over lines, fed by the caller, with no
  callbacks.
- In-process MCP servers that expose Coder's tools to Claude Code.
- Approvals: Claude Code's permission requests answered by chapter 5's policy.
- The child process through `coax/platform/process-native.c`.

## Dependencies

- [Chapter 2](PLAN.md#i-the-loop): Claude Code's events land in Coder's log.
- [Chapter 5](PLAN.md#i-the-loop): the approval policy.
- [Chapter 16](PLAN.md#iii-many-tasks): MCP.

## Done means

- The crate's protocol and control fixtures pass in Coax.
- `openagents coder run --engine claude "TASK"` runs a task through the
  installed CLI, and its events appear in `coder log`.
- The live smoke runs against a named Claude Code version and never opens a
  browser sign-in.

## Sources

- `coax/lib/agent/claude/` and its [PARITY.md](../../coax/lib/agent/claude/PARITY.md).
