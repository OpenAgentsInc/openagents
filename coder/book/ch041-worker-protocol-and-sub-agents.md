# 41. Worker protocol and sub-agents

> **Status: implemented.** Both the terminal UI and the Coder desktop app
> talk to agent workers over one bounded JSON-lines protocol defined in
> `coax/lib/agent/engine/mcworker-wire.coax`. `coder/tui/mcworker.coax`
> runs Microcoder and can delegate up to six parallel sub-agents per step
> across Gemini (`hydrogen`, `carbon`, `neon`) and Claude (`haiku`,
> `sonnet`, `opus`, `fable`) levels; `coder/tui/claudeworker.coax` wraps
> the Claude Code SDK behind the exact same wire frames.

The UI process never holds API keys and never blocks on a model call.
Instead, it spawns child worker processes and exchanges one JSON object per
line over stdin and stdout.

## The wire protocol (`mcworker-wire.coax`)

Every message is parsed and written by
[coax/lib/agent/engine/mcworker-wire.coax](../../coax/lib/agent/engine/mcworker-wire.coax),
which is shared by `mcworker.coax`, `claudeworker.coax`, `coder/tui/` and
`apps/coder-desktop/page.coax`.

### Owner to worker (`mww_request`)

| Constant | JSON shape | Meaning |
| --- | --- | --- |
| `MWW_TASK` (11) | `{"task":T,"session":S,"cwd":D,...}` | Start a turn in working directory `D` (with optional model/effort/history). |
| `MWW_ANSWER` (12) | `{"answer":true/false,"ticket":N[,"why":W]}` | Approve or deny a pending `MWW_ASK` command for ticket `N`. |
| `MWW_STOP` (13) | `{"stop":true}` | Cancel the current run cleanly (`MWW_STOP_LINE`). |
| `MWW_FORGET` (14) | `{"forget":true}` | Clear the worker's in-memory conversation (`MWW_FORGET_LINE`). |
| `MWW_EMBED` (15) | `{"embed":TEXT[,"query":true]}` | Request an embedding vector (input capped at `MWW_EMBED_MOST` = 16,384 bytes). |
| `MWW_RESULTS` (16) | `{"results":[{"exit":N,"output":O}],"ticket":N}` | Return completed sub-agent results for a `MWW_DELEGATE` ticket. |
| `MWW_RECALL` (17) | `{"recall":[{"task":T,"reply":R}]}` | Seed the worker's conversation from a saved chat. |

### Worker to owner (`mww_read`)

| Constant | JSON shape | Meaning |
| --- | --- | --- |
| `MWW_JOURNAL` (1) | `{"kind":"journal","path":P}` | Path to the worker's session journal on disk. |
| `MWW_READY` (2) | `{"kind":"ready"}` | Worker is initialized and ready for a task. |
| `MWW_REPLY` (3) | `{"kind":"reply","text":T}` | Streamed reply text chunk. |
| `MWW_THOUGHT` (18) | `{"kind":"thought","text":T}` | Streamed reasoning or summarized thinking text. |
| `MWW_ATTEMPT` (4) | `{"kind":"attempt","schema":...}` | Observability record for one model request attempt. |
| `MWW_COMMAND` (5) | `{"kind":"command","command":C,"exit":N,"timed_out":B,"output":O}` | Completed shell command and its capped output. |
| `MWW_VIEW` (6) | `{"kind":"view","path":P,"found":B}` | File read by the agent. |
| `MWW_ASK` (7) | `{"kind":"ask","command":C,"ticket":N}` | Request owner approval before running command `C`. |
| `MWW_ENDED` (8) | `{"kind":"ended","ending":E,"text":T}` | Run finished (`finished`, `asked`, `stopped`, `bad_replies`, `no_capacity`, `idle`). |
| `MWW_EMBEDDING` (9) | `{"kind":"embedding","values":[...]}` | Returned embedding vector (or `{"kind":"embedding","error":W}`). |
| `MWW_DELEGATE` (10) | `{"kind":"delegate","tasks":[{"level":L,"task":T}],"ticket":N}` | Request the owner to run 1–6 sub-agents in parallel and return `MWW_RESULTS`. |

## Two workers, one protocol

- **`coder/tui/mcworker.coax`** runs Microcoder's loop
  ([Chapter 9](ch009-microcoder.md)) against Vertex, Gemini API and
  OpenRouter gateways. It supports `MWW_DELEGATE`, `MWW_EMBED`, `MWW_RECALL`
  and multi-turn conversations.
- **`coder/tui/claudeworker.coax`** wraps the `claude` CLI through the Coax
  Claude Agent SDK ([Chapter 10](ch010-drive-claude-code.md)) as a
  single-task worker speaking the same `ready`, `thought`, `command`,
  `reply` and `ended` frames. It maps `model` (`haiku`, `sonnet`, `opus`,
  `fable`, or a full model ID; default `sonnet`), enables
  `--thinking-display summarized`, and isolates credentials by translating
  `CODER_CLAUDE_KEY` (`sk-ant-oat...` to `CLAUDE_CODE_OAUTH_TOKEN`, any
  other key to `ANTHROPIC_API_KEY`) while stripping other inherited auth
  variables.

## Multi-level sub-agent delegation

When Microcoder emits a `delegate` array in `next_action`, `mcworker` does
not spawn child processes itself. It emits `MWW_DELEGATE` with a `ticket`
and pauses for `MWW_RESULTS`. The owner (`App.swift` + `page.coax`) owns
the sub-agent lifecycle:

1. `page.coax` registers each delegated task, shows a live sub-agent card
   in the UI (status, model level, streamed thoughts, commands and reply),
   and exposes pending launches through `cd_agent_next` / `cd_agent_task`.
2. `App.swift` spawns the right worker for each level:
   - `hydrogen`, `carbon`, `neon` -> `mcworker` at the requested Gemini
     tier.
   - `haiku`, `sonnet`, `opus`, `fable` -> `claudeworker` on the person's
     Claude login.
3. Up to six sub-agents run concurrently in the same working directory.
   When a sub-agent makes 100 tool calls in a row without changing a file,
   it pauses and reports its handle so the parent can decide whether to
   `resume` it with a note or stop it.
4. Once all delegated tasks for a ticket finish, `page.coax` formats
   `MWW_RESULTS` and sends it back to the parent worker, which resumes its
   loop with every sub-agent's reply as step results.

Keeping sub-agent orchestration in the owner gives the desktop app full
visibility into every child worker—each can be watched, inspected, or
stopped with the parent—without duplicating UI plumbing inside the worker.

## Checks

```sh
coax/bin/coax test coax/lib/agent/engine/mcworker-wire.coax --library
coax/bin/coax test apps/coder-desktop/page.coax --library
```

The wire tests in `mcworker-wire.coax` and `page.coax` verify every owner
and worker frame, embedding bounds, and round-trip `MWW_DELEGATE` /
`MWW_RESULTS` framing.

## Dependencies

- [Chapter 9](ch009-microcoder.md): Microcoder's loop and endings.
- [Chapter 10](ch010-drive-claude-code.md): the Claude Code SDK behind
  `claudeworker`.
- [Chapter 14](ch014-forks-subagents-and-jobs.md): scoped subagents and
  policy inheritance.
- [Chapter 40](ch040-the-desktop-app.md): the desktop host that renders
  sub-agents and routes wire frames.

## Done means

- [x] `mcworker-wire.coax` defines and tests all owner and worker frames,
  including `MWW_THOUGHT`, `MWW_DELEGATE`, `MWW_RESULTS`, `MWW_EMBED` and
  `MWW_RECALL`.
- [x] `claudeworker.coax` speaks the same wire protocol as `mcworker.coax`
  with isolated `CODER_CLAUDE_KEY` handling.
- [x] The desktop app runs delegated Gemini and Claude sub-agents in
  parallel, streams their status, and returns `MWW_RESULTS` by ticket.
