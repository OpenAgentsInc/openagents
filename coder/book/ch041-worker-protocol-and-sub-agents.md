# 41. Worker protocol and sub-agents

> **Status: implemented.** Both the terminal UI and OpenAgents Desktop
> (`apps/openagents-desktop/`) talk to agent workers over one bounded
> JSON-lines protocol defined in `coax/lib/agent/engine/mcworker-wire.coax`.
> **`coder/tui/oa-worker.coax`** (powered by `openagents/core/` and
> `coder/tui/oa-subagent.coax`, `#14053`, `#14096`) owns primary turns and
> CC-parity parallel/background/fork subagents (up to 12 per batch, personas,
> worktree isolation), while `coder/tui/inky.coax`, `coder/tui/agyworker.coax`,
> `coder/tui/claudeworker.coax` (`#14104`), and deprecated
> `coder/tui/mcworker.coax` speak the exact same wire frames.

The UI process never holds API keys and never blocks on a model call.
Instead, it spawns child worker processes (or attaches via `coder-host`) and
exchanges one JSON object per line over stdin and stdout.

## The wire protocol (`mcworker-wire.coax`)

Every message is parsed and written by
[coax/lib/agent/engine/mcworker-wire.coax](../../coax/lib/agent/engine/mcworker-wire.coax),
which is shared by `oa-worker.coax`, `oa-subagent.coax`, `inky.coax`,
`agyworker.coax`, `claudeworker.coax`, `mcworker.coax`, `coder/tui/`, and
`apps/openagents-desktop/page.coax`.

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

## Workers on one protocol

- **`coder/tui/oa-worker.coax` + `coder/tui/oa-subagent.coax` (primary, `#14053`, `#14096`):**
  runs the 24-layer pure-Coax `openagents/core/` harness against OpenAgents
  Cloud (`FLAWED_BE_OURS`, `#14070`), local Psionic Inkling 2 Mini (`$0` tier,
  `#14055`), and Vertex/Gemini with SSE streaming, 8+ built-in tools plus
  member packs (`Wallet`, `OpenAgentsCli`, `GameBuilder`, `#14057`), interactive
  approvals, five-layer compaction, and CC-parity subagent delegation
  (`general`, `explore`, `plan`, `verify`, `fork`, custom `.openagents/agents/*.md`
  / `.claude/agents/*.md` personas, background `async_launched` + `SendMessage`,
  and per-subagent Git worktree isolation).
- **`coder/tui/inky.coax` (`#13979`–`#13982`):** native Inkling 2.0 loop for
  local Psionic Metal/CUDA sessions speaking the identical wire.
- **`coder/tui/agyworker.coax` (`#13802`, `#14024`, `#14031`):** drives
  Antigravity (`agy acp`) as an opt-in delegated subagent with mid-turn steer
  and prompt-cache expiry/idle compaction.
- **`coder/tui/claudeworker.coax` (`#14104`):** wraps the `claude` CLI through
  the Coax Claude Agent SDK ([Chapter 10](ch010-drive-claude-code.md)) as an
  opt-in delegated worker with `--thinking-display summarized` and isolated
  `CODER_CLAUDE_KEY` handling.
- **`coder/tui/mcworker.coax` (deprecated `#13983`):** legacy Microcoder worker
  retained for CLI `--engine micro` and recorded-stream replay compatibility.

## Multi-level and CC-parity sub-agent delegation

When `oa-worker` (or legacy `mcworker`) emits `MWW_DELEGATE` with a `ticket`,
the desktop host (`Worker.swift` + `apps/openagents-desktop/page.coax`) owns
the live sub-agent lifecycle:

1. `page.coax` registers each delegated task, renders a live sub-agent card in
   the UI (persona/model level, streamed thoughts, tool calls, and reply), and
   exposes launches through `cd_agent_next` / `cd_agent_task`.
2. `Worker.swift` spawns the right worker for the requested persona or opt-in
   external harness (`oa-worker` for built-in/custom personas and cache-exact
   `fork` children; `claudeworker` or `agyworker` when external harness
   delegation is enabled `#14071`).
3. Up to 12 sub-agents per batch (`FWW_MAX_DELEGATE_BATCH`) run concurrently
   in the workspace or in isolated Git worktrees, with sidechain transcripts
   persisted under `~/.openagents/subagents/` and background subagents
   reachable via `SendMessage`, `TaskOutput`, and `TaskStop`.
4. Once delegated tasks complete, `page.coax` returns `MWW_RESULTS` by ticket
   so the parent resumes cleanly with every child's reply.

Keeping sub-agent orchestration in the owner gives OpenAgents Desktop full
visibility into every child worker—each can be watched, inspected, or stopped
with the parent—without duplicating UI plumbing inside the worker.

## Checks

```sh
coax/bin/coax test coax/lib/agent/engine/mcworker-wire.coax --library
coax/bin/coax test openagents/core/agents.coax --library
coax/bin/coax test apps/openagents-desktop/page.coax --library
```

The wire and subagent checks verify every owner and worker frame, embedding
bounds, persona precedence, read-only Bash guards, compaction-resistant fork
guards, and round-trip `MWW_DELEGATE` / `MWW_RESULTS` framing.

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
