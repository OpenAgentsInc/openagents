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

## Conformance

`reference-adapter.test.ts` proves the contract semantics. It covers full-turn
streaming, **suspend then continue cursor exactness (attach at `cursor + 1`,
no gap, no duplicate)**, lossy-continuation honesty, fail-closed capability
refusal, and re-importable lifecycle export. `schemas.test.ts` covers the data
schemas.

```sh
pnpm --dir packages/agent-harness-contract test
pnpm --dir packages/agent-harness-contract typecheck
```

## Consumers (later HARN packets)

HARN-02 backs the cursor with a durable event log. HARN-03 and HARN-04 re-home
the desktop provider lanes as adapters. HARN-05 merges readiness under the
router. HARN-06 implements suspend and continue on every adapter. HARN-07 adds
the managed sandbox as a harness sandbox provider.
