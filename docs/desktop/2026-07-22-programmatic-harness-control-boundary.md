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
2. **The Desktop host layer** (built): drive a turn through the Desktop's
   own main-process services (thread records, Full Auto run records,
   delegation route/attribution) with no Electron and no renderer, via
   `createHeadlessHost`. #9161's acceptance criteria target this layer.
   The dispatch-collapse program (#9167) routes the Desktop lanes onto
   the same SDK adapters, so the two layers share one execution spine.

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

## The Desktop host control surface

`createHeadlessHost({ root })` (`apps/openagents-desktop/src/desktop-headless-host.ts`)
assembles the PRODUCTION host services — the real
`makeProviderLaneDispatcher`, `ThreadStore`, `LocalTurnJournal`, and Full
Auto run registry — over a disposable root, with no Electron and no
renderer. It drives turns through the same dispatch path the Desktop uses:

- `submitOrdinaryTurn({ lane, threadRef, turnRef, message })` — one ordinary
  turn. It returns ordered typed frames, the durable thread, and the Full
  Auto record count. Ordinary turns create ZERO Full Auto records.
- `startFullAutoRun({ title, objective, doneCondition })` — the only path
  that creates a run record. It returns the run with its stable `runRef`,
  durable across a reopen from a fresh host over the same root.
- A delegated turn's `child_started` route/attribution frame precedes the
  promoted answer (`screenDelegatedTurn` enforces the #9159 invariant).

`makeCodexHeadlessLane` plugs a real owner-local Codex turn into this host.
The gated e2e `HEADLESS_HOST_LIVE=1 vitest run tests/headless-host-codex.e2e.test.ts`
runs it through the full production path with no renderer.

The operator command is `pnpm --dir apps/openagents-desktop run headless-host`
(`scripts/headless-host-cli.ts`):

```sh
# a real owner-local Codex turn through the production host (spends capacity):
pnpm --dir apps/openagents-desktop run headless-host -- codex-turn \
  --message "hey who are you" [--model gpt-5.6-terra] [--sandbox read-only] [--private]

# start a Full Auto run (one durable run record, stable ref):
pnpm --dir apps/openagents-desktop run headless-host -- full-auto-start \
  --objective "Implement #NNNN and run the named verification."
```

It prints the public-safe receipt on stdout. Raw answer text and local
paths never appear there. `--private` adds the private receipt on stderr.

`deriveHeadlessReceipts` (`src/desktop-headless-receipt.ts`) turns a host
turn into a public-safe receipt (bounded facts, no raw text or paths) and a
private receipt (full frames + answer), with `screenHeadlessTurn` /
`screenDelegatedTurn` applying the coherence rubric's deterministic
tripwires.

## Convergence with the SDK layer

The dispatch-collapse program (#9167) routes the Desktop's live lanes onto
these same SDK adapters, so a graded headless run and a UI turn share one
execution spine. The codex app-server lane and the claude lane use the
adapter route by default, with renderer parity by construction. Set
`OPENAGENTS_DESKTOP_CODEX_HARNESS_ADAPTER=0` or
`OPENAGENTS_DESKTOP_CLAUDE_HARNESS_ADAPTER=0` to go back to the legacy
drive. The codex exec sub-lane serves fixtures only and stays opt-in
through `OPENAGENTS_DESKTOP_CODEX_HARNESS_ADAPTER=1` when no app-server is
configured.
