# Khala Code Desktop — Live progress streaming for codex_spawn (and all long tools)

Date: 2026-06-30
Status: design audit / implementation spec. No code changed by this doc.
Scope: make the chat tool cards (especially `codex_spawn`) stream live progress
as work happens, instead of showing an init line, then ~2 minutes of nothing,
then the full result dumped at once.

## 1. Symptom

When the user runs `codex_spawn` in Khala Code Desktop chat, they see:

1. A `RUNNING` tool card with a static "Preparing the Pylon/Codex handoff…" body.
2. ~30s–2min of **nothing** (a real Codex assignment is executing remotely).
3. The full result (delegate trace, lifecycle list, proof, counter) appears all
   at once when the tool finishes.

The compact, auto-scrolling, expandable tool card already shipped
(`tool-card-output` max-height + auto-scroll + click-to-expand,
`clients/khala-code-desktop/src/ui/transcript-render.ts` /
`styles.css`). That card is the *surface* for live output — but nothing streams
into it mid-run. This doc specifies the streaming itself.

## 2. Current event flow (where the gap is)

Chat turn runtime: `clients/khala-code-desktop/src/bun/khala-chat-runtime.ts`.

- Per tool call (sequential `for (const call of toolCalls)`, ~line 399):
  - `emit({ message: toolTranscript, type: "message_start" })` — the running
    card (line ~401).
  - `runToolCall(...)` — **blocks** until the whole tool finishes (line ~417).
  - `emitToolResultEvents(...)` — emits tool events **from the final result**
    (line ~427; `toolEventsFromResult` reads `result.ui.events`, line ~1332).
  - `emit({ message: completedToolTranscript, type: "message_replace" })` — the
    final card (line ~437).

  → Between `message_start` and the final `message_replace`, **no events are
  emitted**. That is the gap.

Tool execution contract: `packages/khala-tools`.

- `KhalaToolDefinition.execute` is effectively `(input) => Effect<KhalaToolResult>`
  (`packages/khala-tools/src/index.ts:183`). **The tool has no emit channel** —
  it can only return a final result.
- The dispatcher *does* have an event mechanism: `emitToolEvent` pushes a
  `KhalaToolEvent` and fires `options.hooks?.onEvent` (`dispatcher.ts:~419`), and
  `tool_progress` is already a defined `KhalaToolEventKind`
  (`index.ts:242`). Today only end-of-run events are pushed.

`codex_spawn`: `clients/khala-code-desktop/src/bun/khala-codex-fleet-tools.ts`.

- `executeCodexSpawnTool` (line 870) → `spawnCodexInstances` (line 630), which is
  a sequence of **blocking** `runPylonCommand` calls (heartbeat, go-online,
  inspect, the delegate program, and the assignment run).
- `runPylonCommand` (line 974) reads the child via `collectStream` (line ~1089),
  which **buffers the full stdout/stderr** and resolves only on process exit.

Pylon (the source of truth): `apps/pylon/src/assignment.ts`.

- The assignment runner emits live lifecycle events during execution:
  `withRuntimeProgress` (line 1713) ticks `assignment_run.runtime_progress`
  events while Codex works; `emitLifecycleEvent` (line 1693) forwards each one to
  an injected `options.onLifecycleEvent`.
- **The CLI already streams these to stderr as NDJSON.** In
  `apps/pylon/src/index.ts`, both `assignment run-no-spend` paths wire:
  ```ts
  onLifecycleEvent: (event) => {
    process.stderr.write(`${JSON.stringify(event)}\n`)   // index.ts:4557 and :5283
  }
  ```
  Each event is `assertPublicProjectionSafe`-checked
  (`assignment.ts:1703`), schema
  `openagents.pylon.assignment_run_lifecycle_event.v0.1`.

### Conclusion

The live data **already exists on the wire** (Pylon → stderr NDJSON, in real
time). The break is entirely on the **consumer + transport** side of the desktop:
`runPylonCommand`/`collectStream` buffers it, and the tool→runtime contract has
no path to emit progress mid-run. Fixing this is a desktop-side streaming change
plus a small, principled contract addition — not a rewrite.

## 3. Proper design (no hacks)

Four pieces, smallest blast radius first. The unifying rule: **Pylon stays the
authority that emits public-safe lifecycle events; the desktop consumes them as a
stream and projects them into the live tool card.** No fabricated progress, no
polling spinners, no counter math from progress frames.

### 3.1 Transport — stream the run, don't buffer it

Add a streaming variant of the Pylon subprocess call used for the assignment-run
step (only that step needs it). Build it on **Effect `Stream`**:

- Spawn the child in a `Scope` (Effect-managed, so it's torn down on
  interrupt/timeout/finalization).
- `Stream.fromReadableStream` (or `fromAsyncIterable`) over `child.stderr` →
  decode → split into lines → `Schema.decode` each line as
  `AssignmentRunLifecycleEvent` (drop non-JSON lines, which are human logs) →
  `Stream.runForEach(onEvent)`.
- Keep stdout collected as today for the final JSON result (or also stream it).
- Honor the existing timeout/kill + `maxOutputBytes` bounds.

Consult `effect-solutions` (`basics` for `Effect.gen`/`Effect.fn`,
`services-and-layers`, `error-handling`) and the corpus at
`~/.local/share/effect-solutions/effect` for current `Stream` primitives
(`Stream.fromReadableStream`, `Stream.decodeText`, `Stream.splitLines`,
`Stream.runForEach`, `Stream.scoped`, `Schema.decodeUnknown`). Do not hand-roll a
reader where a `Stream` combinator exists.

Note: a prior ad-hoc reader already exists for the device-auth flow
(`beginCodexConnect`, `khala-codex-fleet-tools.ts:~2900`). That is intentionally
a one-shot capture; the streaming run path should use proper `Stream` combinators
rather than copying that loop.

Alternative/again-server-authoritative transport (document, don't default to it):
the dispatch returns `durableRequestId` / `durableStreamUrl`
(`khala-codex-fleet-tools.ts:210`; `apps/pylon/src/khala-spawn.ts:125`), and
`khala resume <durableRequestId> --offset 0` replays the durable frames. Consuming
the durable stream is the most authoritative source if/when the run executes off
the local subprocess (e.g. a standing Pylon). For the current
local-`run-no-spend` path, stderr NDJSON is already live and is the lower-risk
default. **Implementer must confirm where the run actually executes for the
desktop path** (local subprocess vs standing Pylon vs server) and pick the
transport accordingly; the two are not mutually exclusive.

### 3.2 Progress channel — give the tool a per-invocation sink

The tool needs to emit `tool_progress` while it runs. Two acceptable designs;
prefer (A).

(A) **Proper contract extension (recommended).** Extend `packages/khala-tools`
so a tool's `execute` receives an event sink (e.g. an `emitProgress(payload)` or
a scoped `Queue`/`PubSub`) alongside `input`. The dispatcher already owns
`emitToolEvent` + `hooks.onEvent`; thread a per-invocation emitter into `execute`
that pushes `kind: "tool_progress"` events (the kind already exists,
`index.ts:242`). This is the clean, reusable path — every long tool benefits, and
the runtime's `onEvent` hook is the single forwarding point.

(B) **Desktop-scoped sink (smaller, still principled).** Because the desktop tool
loop is strictly **sequential** (`for…of`, one tool at a time), the runtime may
set a per-invocation progress callback in `KhalaCodexFleetToolOptions` before
`runToolCall` and clear it after. No shared-package change. The only constraint —
no concurrent tool execution — is true today and must be asserted/guarded if that
ever changes. Acceptable as an interim, but (A) is the correct end state.

In both cases, `spawnCodexInstances` (and `runDelegatedBatchSpawn`) thread the
sink down to the streaming run step (3.1) and call it once per lifecycle event,
rendering each event with the **existing** `renderLifecycleSummaryLines` /
`batchLifecycleSummaryLines` helpers
(`khala-codex-fleet-tools.ts:~2178`, `~2239`) so the live body matches the final
body.

### 3.3 Runtime — emit the live tool card

In `khala-chat-runtime.ts`, the per-call progress emitter renders the running
tool card with accumulated progress and emits `message_replace` on the **same
`toolTranscript.id`** created at `message_start` (line ~401). Concretely:

- Wire the dispatcher `hooks.onEvent` (design A) — or the per-call sink (design
  B) — to: append the progress line, rebuild the tool transcript body
  (running header + streamed lifecycle lines), and
  `emit({ message: { ...toolTranscript, body }, type: "message_replace" })`.
- Coalesce/throttle (e.g. rAF or ~150–250ms) so a burst of `runtime_progress`
  ticks doesn't thrash the transcript. The final `message_replace` (line ~437)
  still lands the complete result and supersedes the streamed body.

### 3.4 UI — already done

`tool-card-output` is a compact, auto-scrolling, expandable box
(`transcript-render.ts` + `styles.css`, shipped 2026-06-30). Streamed
`message_replace` updates flow straight into it; the box keeps the latest line in
view and expands on click. No further UI work required beyond confirming the
streamed body renders identically to the final body.

### 3.5 Pylon — make the lifecycle stream a first-class contract

Pylon already emits the events; the "do it right" work is to make it a
**documented, consistent contract** rather than an incidental stderr write:

- Confirm/standardize the NDJSON lifecycle channel for every desktop-reachable
  run path (the two `run-no-spend` sites, `index.ts:4555` and `:5280`, plus the
  batch spawn path in `apps/pylon/src/khala-spawn.ts:677`). All should emit the
  same `assignment_run_lifecycle_event.v0.1` NDJSON when progress streaming is
  requested.
- Keep `assertPublicProjectionSafe` on every emitted event (already in place,
  `assignment.ts:1703`) — no raw prompts, paths, or secrets in lifecycle frames.
- Prefer a stable, opt-in switch over relying on `--json` side effects (e.g. an
  explicit `--lifecycle-ndjson` flag or a dedicated fd), so the desktop's intent
  ("I want the live stream") is explicit and stdout stays the clean
  result-only channel. Document the schema + channel in the Pylon CLI help and a
  short runbook.

## 4. Files to touch (precise)

- `apps/pylon/src/index.ts` — formalize/confirm the lifecycle NDJSON channel on
  the `run-no-spend` paths (`:4555`, `:5280`); optional explicit flag.
- `apps/pylon/src/khala-spawn.ts` — ensure the batch spawn run path
  (`:677` onLifecycleEvent) emits the same NDJSON when streaming is requested.
- `packages/khala-tools/src/index.ts` + `dispatcher.ts` — (design A) add the
  per-invocation progress emitter to the `execute` contract using the existing
  `tool_progress` kind + `hooks.onEvent`.
- `clients/khala-code-desktop/src/bun/khala-codex-fleet-tools.ts` — Effect
  `Stream`-based streaming run command; thread the progress sink through
  `spawnCodexInstances` / `runDelegatedBatchSpawn` / `executeCodexSpawnTool`;
  reuse `renderLifecycleSummaryLines` for live lines.
- `clients/khala-code-desktop/src/bun/khala-chat-runtime.ts` — wire the
  progress sink → `message_replace` on the live tool transcript; throttle.
- `clients/khala-code-desktop/src/shared/rpc.ts` — only if any new event field is
  surfaced to the webview (the existing `chatTurnEvent` `message_replace` path
  already carries it, so likely no change).
- Tests — see §5.

## 5. Verification (must pass before "done")

- **Unit:** feed a fixture NDJSON stream of `assignment_run_lifecycle_event.v0.1`
  lines into the streaming parser and assert one decoded event per line, malformed
  lines dropped, bounds honored, `Scope` finalization kills the child.
- **Runtime unit:** assert that, given N progress events, the runtime emits ≥N
  incremental `message_replace` events on the same `toolTranscript.id` before the
  final result `message_replace`.
- **Fast e2e:** `codex_spawn --fixture` (the fixture path is quick) — assert the
  tool card body grows across multiple `message_replace` frames, not one.
- **Real e2e:** a live `codex_spawn` against the connected fleet — confirm
  `queued → requesting → runtime_progress (repeated) → closeout_submitted →
  completed → accepted` appear in the card **as they happen**, and the final card
  equals today's full output. Reconcile the public counter only from the exact
  `token_usage_events` rows (never from progress frames).
- `bun run --cwd clients/khala-code-desktop typecheck` + `test` + `build:ui`
  green; `apps/pylon` tests green; effect code follows `effect-solutions`.

## 6. Risks / non-negotiables

- **No fabricated progress.** Only render events Pylon actually emitted. No
  elapsed-time spinners standing in for real lifecycle frames (a timer is OK
  *in addition to* real events, but not as a substitute).
- **Public-safety holds end-to-end.** Lifecycle frames are already
  `assertPublicProjectionSafe`; do not widen them. Raw Codex output, prompts,
  paths, and tokens never enter the streamed card.
- **Counter integrity.** The served-token counter is reconciled from exact
  `token_usage_events`; progress frames must not move it.
- **Sequential-loop assumption (design B only) must be guarded** if concurrent
  tool execution is ever introduced. Design A removes this constraint.
- **Throttle** `message_replace` to avoid transcript thrash on `runtime_progress`
  bursts.
