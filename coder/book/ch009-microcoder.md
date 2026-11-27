# 9. Microcoder

> **Status: implemented.** Microcoder (`coax/lib/microcoder/`,
> `coder/tui/mcworker.coax`) still runs for `coder run "TASK" --engine micro`
> and deterministic recorded-stream replay, but was formally deprecated for
> primary desktop sessions in `#13983`: first succeeded by **Inky**
> (`coax/lib/inky/`, `coder/tui/inky.coax`, `#13979`–`#13982`) for native
> Inkling 2.0 streaming/tool calls, and then by the unified 24-layer pure-Coax
> **`openagents/core/`** agent runtime (`coder/tui/oa-worker.coax`,
> `#14050`–`#14057`), which now owns primary turns across local Psionic
> Metal/CUDA, OpenAgents Cloud, and Vertex/Gemini.

## Evolution: Microcoder → Inky → Unified `openagents/core/` (`oa-worker`)

Microcoder proved that a small, zero-dependency Coax loop can drive frontier
and local models without wrapping a third-party agent CLI. Production desktop
and benchmark usage exposed three limits of Microcoder's single-JSON-object
`next_action` protocol: no native SSE tool-call streaming, rigid 6-turn memory
truncation, and lack of a unified permission/hook/subagent/compaction core.
OpenAgents replaced it in two steps while preserving the shared
`mcworker-wire.coax` host boundary:

1. **Inky (`coax/lib/inky/`, `coder/tui/inky.coax`, `#13979`–`#13982`):** an
   I/O-free pure-Coax loop (`IkLoop`) tailored to local Inkling 2.0 on Psionic
   Metal 4 and CUDA with native `<|content_text|>` / `<|content_thinking|>`
   streaming, parallel `GwToolStep` calls, 2-stage compaction, and post-decode
   KV reuse. Phase 5 (`#13983`) deprecated `mcworker` as the default desktop
   worker.
2. **Unified `openagents/core/` (`coder/tui/oa-worker.coax`, `#14050`–`#14057`):**
   a complete 24-layer pure-Coax autonomous agent harness (`openagents/core/core.coax`)
   with `#authority` zero-survivor mutation boundaries across atoms/scheduler
   (`ids.coax`, `scheduler.coax`), tool contracts (`tool.coax`, `tool_exec.coax`,
   `tool_spill.coax`), prompt-cache boundaries (`prompt_cache.coax`), SSE
   streaming (`sse_stream.coax`), query loop (`query_loop.coax`), transcript
   DAG (`session.coax`, `transcript_dag.coax`), five-layer compaction
   (`compaction.coax`), memory (`memory.coax`), permissions & hooks
   (`permissions.coax`, `perm_match.coax`, `hooks.coax`), member packs with
   bypass-immune `WalletPay` (`members.coax`, `#14057`), background tasks
   (`tasks.coax`), CC-parity subagents (`subagent.coax`, `agents.coax`,
   `coder/tui/oa-subagent.coax`, `#14096`), and swarm/remote-bridge protocols.
   `oa-worker` runs primary turns across OpenAgents Cloud (`#14070`), local
   Psionic Inkling 2 Mini (`#14055`), and Vertex/Gemini while speaking the same
   `mcworker-wire.coax` stdin/stdout protocol (`#14053`).

The sections below retain Microcoder's original loop, replay fixtures, and
checks, which continue to back `coder run --engine micro`.

The [repository pilot](../../docs/coder/repository-pilot.md) exercises this
engine on actual edits and external acceptance checks. Gateway playback
below still executes returned commands; it is different from
[`coder replay` input verification](CONTRACT.md#what-replay-means).

Microcoder is Coder's own engine: a small loop that runs on any model we can
reach and needs no other agent installed. It is the engine for plain
questions, small jobs and machines without Claude Code or Codex.

## The loop

Each step makes one structured call. The request is the state, rendered as
one JSON object ([state.coax](../../coax/lib/microcoder/state.coax)):

- the environment facts (the operating system, the app and the working
  directory) and, in Coder, the earlier turns of the conversation;
- the task and the routing decision;
- the last 12 actions, each command with its exit status and the first and
  last 6,000 bytes of its output;
- the files the model asked to view, and the loop's own notes.

The model returns `next_action`
([action.coax](../../coax/lib/microcoder/action.coax)): a rationale for
the log, commands to run in order, files to view, `finished`, the reply and
whether it asks a question. A gateway that can't hold a strict JSON schema
answers in prose, and
[text_action.coax](../../coax/lib/microcoder/text_action.coax) reads the
action out of the text: a fenced object after prose, a leaked reasoning
block, a command written as a string or an object.

Commands run in the working directory with a timeout and capped output, and
`view` reads capped files under it
([tools.coax](../../coax/lib/microcoder/tools.coax)). Writes go through
commands, so every change shows in the transcript.

## Endings

A run ends one way, and the run reports which
([loop.coax](../../coax/lib/microcoder/loop.coax)):

| Ending | When |
| --- | --- |
| `finished` | the model set `finished` and gave a reply |
| `asked` | the model asked the person a question |
| `bad_replies` | three replies in a row failed the schema or its meaning |
| `no_capacity` | no gateway in the plan answered |
| `stopped` | the person pressed Esc or sent STOP |
| `idle` | the model gave nothing to do and no reply |

The person sets limits; the loop adds none beyond the output caps. A good
reply between bad ones resets the count, and each refusal is written into
the next step's notes so the model can correct itself.

## In Coder

[run.coax](../../coax/lib/microcoder/run.coax) makes the loop pollable:
`mc_run_start`, then `mc_run_poll` returns events (an attempt, more reply
text, a command and its output, a file viewed, the ending), and
`mc_run_stop` ends it. The reply streams out of the `reply` member of a
partial action as the bytes arrive.

Coder runs the loop in a worker process of its own,
[coder/tui/mcworker.coax](../../openagents/workers/legacy/mcworker.coax), and talks to it in
JSON lines the way it talks to the Claude Code and Codex CLIs. The worker
keeps the conversation (the last six turns, each cut to 1,200 bytes, passed
as `conversation` in the facts) and owns the credentials.
[microcoder_keys.coax](../../coax/lib/microcoder/keys.coax) reads them as
the desktop Lab does: files under `~/work/.secrets` go into curl's child
environment only, and Vertex's token is minted by gcloud from a
service-account key file with a gcloud config of its own. The plan is every
gateway with a credential, Vertex first; when Vertex is skipped, the worker
says why once on stderr.

Each completed request attempt also reaches the session log under
[`coder.observability.v2`](../../docs/protocol/coder-observability-v2.md):
configured gateway/model, requested effort, outcome, available token counts,
and separate provider-reported cost and rate-card estimate. Missing details
are null. This records internal attempts without reconstructing every internal
request, and a configured model is not proof of a resolved upstream identity.

```sh
coder/tui/coder.sh run "What is 2+2? Reply with only the number." --engine micro
```

```text
› What is 2+2? Reply with only the number.
  session 20261102-065728-986
● 4
  └ finished · 6.4s
```

## Record and replay

The gateway transport ([coax/lib/gateway/transport.coax](../../coax/lib/gateway/transport.coax))
can keep what a server sent and play it back:

- `GW_RECORD=DIR` saves each response as `DIR/1.http`, `DIR/2.http` and so
  on: the status line, the headers and the body, as `curl -i` prints them.
  Nothing of the request is kept, so no key. While recording, a response
  arrives whole rather than streamed.
- `GW_REPLAY=DIR` sends nothing. Each request reads the next saved response
  instead of starting curl, in order, so no credential or server is needed.
  The plan holds one Vertex attempt, and the psionic engine skips its
  server check.

Both keep their place in `DIR/.next`. The commands a step asks for still run
for real, so a recording should ask for ones that give the same output on
any machine.

[coder/tui/fixtures/micro-replay](../../coder/tui/fixtures/micro-replay)
holds two responses recorded from Vertex Gemini Flash for "Run `echo
replay-check` and then tell me what it printed". Replayed with no
credentials, the session log matched the live one line for line, and two
replays matched each other byte for byte apart from the session's ID,
times and folder: the same request digest, the same command and result,
and the reply "It printed replay-check."

The smoke also reaches the endings through `coder run`, each from a few
replayed responses written in the check: a question (`asked`, which Coder
shows as the reply and ends as finished), three replies that aren't JSON
(`bad_replies`), a 503 on every try (`no_capacity`) and three empty steps
(`idle`). Each failed ending says what happened in plain words, such as "The
model gave nothing to do and no reply three times in a row." `stopped` is
the unit check's and the smoke's `coder stop` checks'.

## Checks

```sh
coax test coax/lib/microcoder/run.coax --library   # the loop, state, actions, tools and gateways: 83 checks
coax/lib/microcoder/tools_smoke.sh           # commands and views in a real directory
coax/lib/microcoder/live.sh                  # one live run on the plan
python3 coder/tui/smoke.py --live microcoder # two turns in the terminal UI; the second needs the first
python3 coder/tui/smoke.py                   # the Vertex recording replayed twice, with no credentials;
                                             # asked, bad_replies, no_capacity and idle through coder run
```

## Dependencies

- [Chapter 1](PLAN.md#i-the-loop): the simplest loop, which Microcoder is an
  instance of.
- [Chapter 5](PLAN.md#i-the-loop): confinement for its commands.

## Done means

- [x] `coder run --engine micro "TASK"` runs the loop in the current
  directory and prints each step and the ending.
- [x] Recorded gateway streams replay through the loop with byte-identical
  logs, apart from the session's ID, times and folder.
- [x] Each ending is reached by a scripted check
  (`loop.coax`: "asked, no_capacity, idle and stopped", "three replies in
  a row that fail the schema"), and all but `stopped` also through `coder
  run` with replayed responses.

## Exact-core edit and schema guards (W6.1)

Microcoder's edit and tool loop now wires directly into the four W2.2 Class-E
compiled weight modules (`oa-exec-diff-verify`, `oa-exec-json-schema`,
`oa-exec-dep-sched`, `oa-exec-arith-budget`) via
[`coax/lib/microcoder/executor-guard.coax`](../../coax/lib/microcoder/executor-guard.coax)
and [`coax/lib/microcoder/executor-bench.coax`](../../coax/lib/microcoder/executor-bench.coax).
Before a patch or tool payload takes effect, the exact compiled core verifies
hunk/line offsets, JSON schemas, DAG ordering, and satoshi/token budgets with
zero silent hallucinations ([W6.1 verification record](../../docs/verification/executor-w61-coding-agent.md)).

## Sources

- `coax/lib/microcoder/` (`loop.coax`, `state.coax`, `action.coax`,
  `text_action.coax`, `tools.coax`, `run.coax`, `executor-guard.coax`,
  `executor-bench.coax`) — deprecated legacy engine (`#13983`).
- `coax/lib/inky/` (`loop.coax`, `run.coax`, `compact.coax`, `tools.coax`) and
  `coder/tui/inky.coax` — native Inkling 2.0 loop (`#13979`–`#13982`).
- `openagents/core/` (`core.coax`, `query_loop.coax`, `compaction.coax`,
  `permissions.coax`, `members.coax`, `agents.coax`) and
  `coder/tui/oa-worker.coax` — unified 24-layer pure-Coax runtime (`#14050`–`#14057`).
- `coder/tui/mcworker.coax`, `coder/tui/microcoder_keys.coax`,
  `coder/tui/microcoder.coax`.
- [Microcoder in Coax, and the inference gateways](../../docs/coder/microcoder.md).
