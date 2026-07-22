# Programmatic harness control boundary

- Date: 2026-07-22
- Status: active reference
- Audience: agents and evaluation authors
- Related issue: [#9161](https://github.com/OpenAgentsInc/openagents/issues/9161)
  (programmatic conversation and Full Auto controls) and
  [#9167](https://github.com/OpenAgentsInc/openagents/issues/9167)
  (dispatch collapse)
- Result authority: reference only

## Purpose

This document names the supported way to drive coding-agent conversations
programmatically for tests and evaluations — without launching the
Electron renderer or driving DOM selectors. It ties together the pieces
built for #9161 and points to what each proves and what it does not.

There are two layers, and they are different control boundaries:

1. **The SDK harness layer** (built, seven lanes live): drive one or more
   coding-agent harnesses directly through the AI SDK adapters, owner-local,
   and grade the result. This is the layer these tools exercise today.
2. **The Desktop host layer** (partly built): drive a turn through the
   Desktop's own main-process services (thread records, Full Auto run
   records, delegation route/attribution). #9161's acceptance criteria
   target this layer. The dispatch-collapse program (#9167) routes the
   Desktop lanes onto the same SDK adapters, so the two layers converge.

## The SDK harness control surface

Every supported coding-agent harness has a live, owner-local execution path
through its production adapter — no fixtures, no renderer. The lanes are
Codex, Claude Code, OpenCode, Grok (ACP), Cursor (ACP), Pi (in-process),
and Goose (ACP). "Owner-local" means the developer's currently-authenticated
account, never an isolated login flow.

### Single-turn smoke

```sh
pnpm run headless:harness -- --grade
# defaults: --harness codex --model gpt-5.6-terra --effort medium
```

Runs one real turn, prints thread id / answer / exact usage, locates the
transcript, and grades it with the coherence screener. Source:
`scripts/run-headless-harness.ts`, `scripts/headless-harness-core.ts`.

### Multi-turn and multi-harness conversations

The AI SDK repo (`OpenAgentsInc/ai`) holds the env-gated live smokes that
drive conversations through the production adapters and the shared
`runHarnessConversation` driver:

- Per-lane multi-turn continuity and correction:
  `CONVO_LIVE_SMOKE=1 vitest run src/harness-conversations.live.test.ts`
- Multi-harness parallel and sequential handoff:
  `MULTI_LIVE_SMOKE=1 vitest run src/multi-harness.live.test.ts`
- Seven-lane orchestrated mini-project:
  `SEVEN_LIVE_SMOKE=1 vitest run src/seven-lane.live.test.ts`

Each writes a combined transcript (user/assistant messages, sub-agent
lifecycle, per-lane KhalaRuntimeEvents with lane and model attribution).

## Grading and evidence

The grader is the evaluation boundary. It reads a transcript and returns a
coherence score plus a complexity tier:

```sh
pnpm run grade:coherence                       # sweep local corpora
pnpm run grade:coherence -- <transcript.jsonl> # grade one
```

- Coherence method: `docs/analysis/conversation-thread-coherence-rubric.md`
  plus the deterministic screener
  (`docs/analysis/deterministic-coherence-screening.md`).
- Complexity method: `docs/analysis/complexity-rubric.md`. Results read as
  `coherence @ complexity`. A high score at a low tier proves little.
- The flywheel process and the sweep ledger are
  `docs/analysis/coherence-flywheel.md` and
  `docs/analysis/coherence-ledger.md`.

## Public evidence

A graded conversation becomes a public trace:

```sh
cd apps/qa-runner
pnpm trace:ingest --file <combined-transcript.jsonl> --visibility public
# or a local conversation id:
pnpm trace:ingest <conversationId> --source claude|codex|openagents
```

The `--file` path converts a combined multi-harness transcript through the
redact → validate → `POST /api/traces` pipeline to a public
`openagents.com/trace/{uuid}`. Needs `OPENAGENTS_AGENT_TOKEN`.

## Honest scope

What these tools prove: every harness adapter executes a real owner-local
turn, multi-turn continuity holds, multi-agent orchestration stays coherent,
and the result grades on a repeatable metric.

What they do NOT prove (the #9161 host-layer residual): running a turn
through the Desktop's own thread and Full Auto run records, exposing
delegation route and attribution before an answer is promoted, and the
isolated-Desktop-host bootstrap. Those are the host control surface #9161's
acceptance criteria describe, and the dispatch-collapse program (#9167) is
the path that makes the Desktop lanes run on these same adapters so a
grader can drive the real host without renderer automation.
