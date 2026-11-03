# 11. Drive Codex

> **Status: partly implemented.** Coder runs tasks through `codex
> app-server` on the person's own ChatGPT login (checked with codex-cli
> 0.163.0): `coder run "TASK" --engine codex` and the terminal UI's second
> agent. One thread holds the conversation; commands, their output and file
> changes as unified diffs show in the transcript and the session log.
> With `--allow`, `--ceiling` or `--root`, chapter 5's policy answers
> approvals. Still to come: a live run against a current Codex release
> (chapter 26 found `main` well past 0.163.0).

The [2026-11-03 repository pilot](../../docs/verification/coder-repository-2026-11-03/README.md)
completed ten actual CLI repair tasks on codex-cli 0.163.0, including a
diagnostic turn and repair fork. The earlier quota failure below is a dated
observation, not a claim that this later run remained blocked. Qualification
against a newer release and the interactive UI smoke remain separate work.

Codex is the other engine many people have. Coder drives it through
`codex app-server`, the Codex CLI's JSON-RPC 2.0 server over stdio.

## The client

[coax/lib/agent/codex/](../../coax/lib/agent/codex/) is a pull-based
client in caller storage. The caller starts the process, then calls
`cx_initialize`, `cx_thread_start` and `cx_turn_start`, and polls with
`cx_poll`, which feeds lines through `cx_feed` and returns typed
notifications:

| Notification | What Coder does with it |
| --- | --- |
| `item/agentMessage/delta` | adds the text to the streaming reply |
| `item/started` (`commandExecution`) | shows the command row |
| `item/completed` (`commandExecution`) | shows the output and exit status |
| `item/completed` (`fileChange`) | shows each change's unified diff |
| `turn/completed` | ends the turn: `completed`, `interrupted` or `failed` |
| `error` | shows the error, such as a usage limit |

Server requests for approval are answered with `cx_approve`. Two recorded
sessions, scrubbed, replay byte for byte in the client's checks
([fixture.coax](../../coax/lib/agent/codex/fixture.coax)).

`CxSettings.dynamic_tools` supplies `thread/start`'s experimental
`dynamicTools`. `item/tool/call` becomes `CxEventKind.tool_call`, answered
with `cx_tool_answer` before the next poll; an unanswered request fails.
Coder registers `coder_session_read` in every fresh Codex thread through
this route, sharing the MCP schema and typed reader. It reads Claude
conversation turns and separately paginated inert compaction context
([chapter 14](ch014-forks-subagents-and-jobs.md)).

## In Coder

[codex.coax](../../coder/tui/codex.coax) starts the server on the first
message and keeps one thread; later messages are turns on it. By default
the thread runs with approval policy `never` and sandbox
`danger-full-access`, the same trust as the claude engine. With `--ask`,
the policy is `untrusted` in a `workspace-write` sandbox and each approval
goes to the y/a/n prompt. Esc sends `turn/interrupt`; if Codex doesn't end
the turn within 5 seconds, Coder ends the process.

`df_unified` in [diff.coax](../../coder/tui/diff.coax) draws Codex's
unified diffs with the same line numbers, colors and syntax highlighting as
Claude Code's edits, and writes each one to the log as a `diff` line with
its path and counts of added and removed lines.

## Approvals by policy

`--allow`, `--ceiling` and `--root` work as in
[chapter 10](ch010-drive-claude-code.md#approvals-by-policy). With a policy,
Coder starts the thread with approval policy `untrusted`, the
`workspace-write` sandbox and you as the reviewer, then answers each
approval itself:

- A command is `run`, in its `cwd`.
- A file change is `write`, in its `grantRoot` when Codex names one.
- A request for more permissions is `run`.

A verdict of ask comes to the y/a/n prompt in the terminal UI and is
declined in a job.

## Checks

```sh
coax test coax/lib/agent/codex/tests.coax --library   # 29 checks: requests, replay, approvals, refusals
coax/lib/agent/codex/smoke.sh                 # one live turn
python3 coder/tui/smoke.py --live codex       # two turns in the terminal UI
```

On 2026-11-02 the live smoke reached Codex and showed its answer, a usage
limit on the account until 2026-11-08. The check needs an account with
quota left to pass.

## Dependencies

- [Chapter 2](PLAN.md#i-the-loop) and [Chapter 5](PLAN.md#i-the-loop).
- [Chapter 10](ch010-drive-claude-code.md): the same transport pattern.

## Done means

- [x] The request, replay, approval and refusal tests pass.
- [x] `coder run --engine codex "TASK"` runs a task through the installed
  CLI, and its events appear in `coder log`.
- [ ] The live smoke passes against a named Codex version (blocked by the
  account's usage limit until 2026-11-08).
- [x] Approvals are answered by chapter 5's policy. The decision is the
  same code as chapter 10's, checked there; the live Codex check waits for
  the usage limit above.

## Sources

- `coax/lib/agent/codex/` and its [PARITY.md](../../coax/lib/agent/codex/PARITY.md).
- `coder/tui/codex.coax`, `coder/tui/diff.coax`.
