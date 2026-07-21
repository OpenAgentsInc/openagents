# @openagentsinc/agent-harness-contract

The Effect-native **agent harness contract**. One versioned adapter contract
drives a third-party coding-agent runtime (Codex, Claude Code, an ACP peer, a
managed sandbox) behind a uniform surface with durable, cursor-exact turn
suspend and continue.

It is the OpenAgents port of the Vercel AI SDK `HarnessV1` shape. The ideas are
re-derived. No upstream code is vendored, and there is no runtime dependency on
`@ai-sdk/harness`. The full harvest analysis is in
`docs/fable/2026-07-20-ai-sdk-harness-abstraction-harvest-analysis.md`. This
package is HARN-01 of the HARN epic (#9115).

## What it contains

- **`adapter.ts`** — `AgentHarness`. Fields are `harnessId`, `harnessKind`
  (`AgentDefinitionHarnessKind`), `adapterKind`, `builtinTools`, the two
  built-in tool flags, an optional `lifecycleStateSchema`, an optional
  `getBootstrap`, and one entry method `start`. There is no static capability
  object. Optional behavior is signalled by method presence. A request the
  adapter cannot satisfy fails with `HarnessCapabilityUnsupported` (the
  Box-facade 501 posture, one layer lower).
- **`session.ts`** — `HarnessSession` verbs `promptTurn`, `continueTurn`,
  `suspendTurn`, `compact`, `detach`, `stop`, and `destroy`, plus the
  `HarnessPromptControl` handle (event `Stream`, `done`, tool-result and
  approval and user-message submission, interrupt). Lifecycle is caller-owned
  and explicit, so a session can outlive its process (the durable journal
  persists it). This is a deliberate divergence from an auto-`Scope`-destroyed
  resource.
- **`stream.ts`** — the harness stream event IS the neutral
  `KhalaRuntimeEvent` (`openagents.khala_runtime_event.v1`). There is no new
  event union. The `sequence` field is the durable replay cursor.
- **`lifecycle-state.ts`** — `HarnessResumeState` and
  `HarnessContinuationState`. The continuation state pins the exact suspend
  cursor and records `lossy`.
- **`capability.ts`**, **`permission.ts`**, **`skill.ts`**, **`host-tool.ts`**,
  **`common-tool.ts`** (the `read`/`write`/`edit`/`bash`/`glob`/`grep`/`webSearch`
  vocabulary with `nativeName`/`commonName`/`providerExecuted` normalization),
  and **`bootstrap.ts`**.
- **`reference-adapter.ts`** and **`event-builder.ts`** — an in-memory
  reference `AgentHarness` and event builders for the conformance suite.
- **`sandbox.ts`** and **`local-sandbox-provider.ts`** (HARN-07 core) — the
  harness sandbox-provider contract. `HarnessSandboxProvider` is the stable
  factory for a `HarnessSandboxSession` workspace with file I/O and command
  execution. Optional methods signal capability. A provider that cannot expose
  ports omits `getPortUrl`. A provider that cannot rehydrate a session omits
  `resumeSession`. The provider owns the sandbox lifecycle and the adapter never
  calls `stop`. `makeLocalSandboxProvider` is an in-memory test double for
  hermetic conformance. The managed-sandbox substrate implements this port in the
  desktop cutover. Phase-1 managed sandbox omits `getPortUrl`.
- **`event-log-store.ts`** and **`event-log.ts`** (HARN-02) — the durable
  seq-cursor event log. `HarnessEventLogStore` is the persistence port (the
  in-memory reference ships here, the desktop local-turn journal and the
  managed-sandbox event store implement it later). `HarnessEventLog` is the
  runtime: `appendEvent`, finite `replay` from a cursor (crash recovery), live
  `attach` (replay the persisted tail then follow new events, single-flight per
  `(turn, consumer class)`), `lastCursor`, and `markRerunBoundary` /
  `rerunBoundaries` so a recomputed tail is distinguishable from a lossless
  attach.

- **`slice-runner.ts`** (HARN-06) — the intra-turn slice runner. `runHarnessSlice`
  time-boxes one slice of a turn by event budget and suspends at the exact cursor
  when the budget is spent. `runTurnInSlices` drives a whole turn as a chain of
  slices, re-entering the session from `continueFrom` after each suspension, so a
  long turn survives short-lived process invocations. Every event can be
  persisted to the durable log in order.
- **`readiness.ts`** (HARN-05) — the unified readiness projection.
  `projectHarnessReadiness` turns one set of adapter readiness inputs into the
  router candidate set, the admitted (ready) subset, normalized snapshots aligned
  with `@openagentsinc/harness-conformance`, and Pylon-style counted capacity
  refs. One source feeds the kernel descriptor, FAV routing, the Apple FM
  candidate set, and the heartbeat.

- **`acp-adapter.ts`** (HARN-04) — a generic ACP harness adapter factory.
  `makeAcpHarnessAdapter` turns any admitted Agent Client Protocol peer (Grok,
  Cursor) into an `AgentHarness`. `acpEventToKhalaEvents` projects the ACP
  bridge vocabulary onto the neutral stream, and
  `acpPermissionToRuntimeInteractionPayload` routes an approval through the
  canonical `RuntimeInteraction` model instead of a bespoke path.
- **`opencode-adapter.ts`** (HARN-08) — the opencode harness adapter.
  `makeOpencodeAdapter` presents an opencode session as an `AgentHarness`, and
  `opencodeEventToKhalaEvents` projects opencode's real session stream (text and
  reasoning deltas, tool call and result, the step-ended usage boundary) onto
  the neutral stream. opencode has no explicit turn boundary, so the adapter
  synthesizes `turn.started` and derives `turn.finished` plus usage from the
  step boundary.

## Conformance

`reference-adapter.test.ts` proves the contract semantics. It covers full-turn
streaming, **suspend then continue cursor exactness (attach at `cursor + 1`,
no gap, no duplicate)**, lossy-continuation honesty, fail-closed capability
refusal, and re-importable lifecycle export. `schemas.test.ts` covers the data
schemas. `event-log.test.ts` (HARN-02) proves durable replay after simulated
process death, dup-free rejection of non-increasing sequences, rerun-boundary
visibility, live-attach replay-then-follow, and single-flight supersession.

```sh
pnpm --dir packages/agent-harness-contract test
pnpm --dir packages/agent-harness-contract typecheck
```

## Consumers (later HARN packets)

HARN-02 backs the cursor with a durable event log. HARN-03 and HARN-04 re-home
the desktop provider lanes as adapters. HARN-05 merges readiness under the
router. HARN-06 implements suspend and continue on every adapter. HARN-07 adds
the managed sandbox as a harness sandbox provider.
