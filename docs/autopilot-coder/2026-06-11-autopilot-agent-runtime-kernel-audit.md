# Autopilot Agent Runtime Kernel Audit

**STATUS: HISTORICAL — point-in-time record (accurate as of its
date). Not current direction; consult MASTER_ROADMAP.**


Date: 2026-06-11

## Scope

Autopilot needs one runtime shape for coding-agent work. That shape must cover
the OpenAgents native agent loop and delegated loops from Claude Code, Codex,
OpenCode, Hermes, hosted containers, SHC lanes, and deterministic fixtures.

The kernel should not make any external agent transcript, provider SDK message,
or UI stream format the source of truth. OpenAgents should own a versioned
Effect Schema event contract and project other shapes at the boundary.

## Current State

OpenAgents already has several pieces of the future runtime, but they are not
yet unified behind one agent-runtime contract.

- `apps/pylon/src/labor.ts` already models local agent kinds including
  `codex`, `opencode`, `claude_code`, and `test_fixture`.
- `apps/pylon/src/opencode-run.ts` already wraps an OpenCode CLI execution
  path.
- `apps/openagents.com/workers/api/src/autopilot-work-adapter-selection.ts`
  maps worker adapter choices into `claude_agent_task` and `codex_agent_task`.
- The current Autopilot roadmap records local Claude and local Codex as peer
  adapters behind the same gate, with `git_checkout` support owned by the
  shared B2 path and consumed by the Codex parity lane.
- Issue #4792 is closed as of 2026-06-11 and records the Codex executor parity
  lane as landed.

Related open issues as of this audit:

- #4786: Autopilot MVP epic.
- #4758: Work list/detail visibility.
- #4759: Web UI request composer.
- #4762: Cloud-Pylon deployment path.
- #4765: Decision queue and notifications.
- #4772: MVP exit review.
- #4779: Writeback symmetry.
- #4782: Spare-capacity provider mode.

I did not find a dedicated open issue for a shared Agent Runtime Kernel event
contract at audit time. This is now filed (2026-06-11, after the original
query — by then #4757, #4758, and #4759 had also closed): epic #4804 tracks
the RK rung lane, with #4805 (RK1, the schema package this audit specifies),
#4806 (RK2, existing loops behind the adapter contract), #4807 (RK3, native
Effect AI loop), #4808 (RK4, worker ingestion + projections), and #4809
(RK5, surfaces + failure smokes). The unified roadmap carries the matching
RK addendum.

## Decision

Use Effect AI as the native OpenAgents model-loop substrate and keep a Vercel
AI SDK compatible projection only at provider or interoperability edges.

That means:

- OpenAgents owns `AgentRuntimeRun`, `AgentRuntimeEvent`, `AgentRuntimePart`,
  `AgentRuntimeToolInvocation`, and `AgentRuntimeExternalInvocation` schemas.
- Effect AI powers the OpenAgents-native loop through `LanguageModel`,
  `Toolkit`, typed tool success/failure schemas, Effects, Streams, Layers, and
  test layers.
- AI SDK `ModelMessage` or `UIMessage` shapes may be useful for provider
  execution and adapter interoperability, but they are not the durable
  OpenAgents storage shape.
- External agents emit normalized OpenAgents runtime events through adapters.
  They do not own OpenAgents run state, projection policy, or acceptance state.

Effect AI is still marked experimental/alpha in the upstream docs, so the
runtime should isolate it behind OpenAgents services. That lets the durable
schema and adapter contract stay stable if the Effect AI package surface moves.

## Proposed Package Boundary

Add a shared runtime package before changing worker or Pylon execution paths.

Candidate package:

- `packages/agent-runtime-schema`

Initial exports:

- `AgentRuntimeRun`
- `AgentRuntimeRunId`
- `AgentRuntimeAdapterKind`
- `AgentRuntimeLoopKind`
- `AgentRuntimeEvent`
- `AgentRuntimeEventId`
- `AgentRuntimePart`
- `AgentRuntimeToolInvocation`
- `AgentRuntimeExternalInvocation`
- `AgentRuntimeArtifactRef`
- `AgentRuntimeUsageRecord`
- `AgentRuntimeVisibility`
- `AgentRuntimeRedactionPolicy`

The package should be schema-only at first. Execution services can live in
Pylon or a later `packages/agent-runtime-effect` package once the event contract
is proven.

## Durable Run Shape

`AgentRuntimeRun` should include:

- `runId`
- `assignmentId` or `workOrderId`
- `workspaceRef`
- `adapterKind`: `openagents_native`, `claude_code`, `codex`, `opencode`,
  `hermes`, `hosted_container`, `shc`, or `test_fixture`
- `loopKind`: `native_model_loop`, `external_agent_loop`, `hosted_loop`, or
  `fixture_loop`
- `sourceIssue`, `sourceRequest`, or `sourceForumThread` refs where relevant
- `budgetRef` and `usagePolicy`
- `permissionPolicy`
- `redactionPolicy`
- `visibility`
- `publicProjectionAllowed`
- `state`
- `createdAt`, `startedAt`, `completedAt`

The run owns lifecycle state. Adapter-specific session IDs are subordinate
fields, not primary identities.

## Event Contract

`AgentRuntimeEvent` should be an append-only tagged union. Every event should
carry:

- `eventId`
- `runId`
- `sequence`
- `generatedAt`
- `visibility`
- `redactionClass`
- stable refs to the run, turn, step, tool invocation, external invocation, or
  artifact that the event updates

Initial event tags:

- `run.started`
- `run.input_accepted`
- `context.snapshot_created`
- `step.started`
- `model.stream_started`
- `model.text_delta`
- `model.text_completed`
- `model.reasoning_delta`
- `model.reasoning_completed`
- `tool.call_proposed`
- `tool.input_delta`
- `tool.input_completed`
- `tool.approval_requested`
- `tool.approved`
- `tool.denied`
- `tool.started`
- `tool.completed`
- `tool.failed`
- `external_agent.started`
- `external_agent.event`
- `external_agent.artifact_recorded`
- `external_agent.completed`
- `external_agent.failed`
- `artifact.recorded`
- `usage.recorded`
- `step.completed`
- `step.failed`
- `run.paused`
- `run.interrupted`
- `run.cancelled`
- `run.completed`
- `run.failed`

This event stream should power worker ingestion, Pylon supervision, terminal
status, web workroom status, public-safe receipts, and regression tests.

## Adapter Contract

Each loop should implement the same conceptual service:

```ts
interface AgentRuntimeAdapter {
  readonly kind: AgentRuntimeAdapterKind
  canRun(request: AgentRuntimeRunRequest): Effect.Effect<boolean, AdapterError>
  start(request: AgentRuntimeRunRequest): Stream.Stream<AgentRuntimeEvent, AdapterError>
  cancel(runId: AgentRuntimeRunId): Effect.Effect<void, AdapterError>
}
```

Concrete adapter responsibilities:

- Native OpenAgents: run Effect AI model/tool loops and emit model/tool events.
- Claude Code: run the local adapter and emit external-agent events plus
  artifact refs.
- Codex: run the local adapter and emit the same external-agent event shape.
- OpenCode: normalize its CLI or AI-SDK-shaped output into the same event
  stream.
- Hermes: implement the same adapter boundary once its invocation contract is
  selected.
- Test fixture: emit scripted deterministic events for unit and smoke coverage.

Adapters may know how to invoke a loop. They should not decide whether work is
accepted, public, stale, paid, redacted, or operator-approved.

## Provider Shape

The native OpenAgents model loop should be Effect-first:

- Use Effect AI `LanguageModel` services for provider-agnostic generation.
- Provide concrete providers through `Layer`.
- Define tools through Effect AI `Tool` and `Toolkit` with Effect Schema
  parameters, success schemas, and failure schemas.
- Consume model output as Effect `Stream`s under scoped resources so
  interruption and cancellation are explicit.
- Use test `Layer`s for deterministic model and tool coverage.

The AI SDK shape should be a bridge, not storage:

- `AgentRuntimePart` and `AgentRuntimeMessage` can project into provider-facing
  model messages when needed.
- AI SDK stream parts can project back into `AgentRuntimeEvent`.
- Provider-specific options stay at the edge unless explicitly admitted into
  OpenAgents schemas.

This gives OpenAgents one runtime log while preserving compatibility with SDKs
and agents that already speak AI-SDK-like model, tool, or UI message shapes.

## Redaction And Authority

The runtime kernel must preserve existing OpenAgents projection discipline:

- Do not store raw prompts, private transcripts, provider payloads, raw shell
  logs, secrets, private repo contents, wallet material, or credential-bearing
  paths in public or portable projections.
- Store artifact references and redacted summaries, not unbounded logs.
- Keep work acceptance, payout, public claim, and authority decisions outside
  adapter code.
- Include `generatedAt`, source refs, and staleness metadata on any public
  projection generated from runtime events.
- Keep model routing and tool selection typed. Do not add prompt-keyword
  inference for adapter selection.

## Implementation Plan

1. File a new #4786 sub-issue for the shared event contract. Done: filed as
   #4805 (RK1) under epic #4804.
2. Add `packages/agent-runtime-schema` with only Effect Schema definitions and
   fixtures. Done in RK1 (#4805): the package exports the run/event/part,
   invocation, artifact, usage, visibility, redaction, lifecycle, and fixture
   log contract.
3. Add schema tests for every event tag, redaction class, adapter kind, and
   run lifecycle transition. Done in RK1 (#4805): the filed tag list contains
   32 tags, and the package tests cover all 32 rather than silently dropping
   one to match the earlier prose count.
4. Wrap existing Pylon `claude_code`, `codex`, `opencode`, and `test_fixture`
   paths behind the adapter contract without changing their execution behavior.
   Done in RK2 (#4806): Pylon now exposes `AgentRuntimeAdapter` wrappers for
   Claude, Codex, OpenCode, test fixtures, and a reserved Hermes adapter. The
   wrappers project existing executor closeout records into kernel events.
5. Add an event-log replay test that rebuilds current workroom/projection state
   from runtime events. Done in RK2 (#4806): the replay reducer rebuilds
   terminal state, external status, artifact refs, blocker refs, freshness
   timestamp, and event count from events alone.
6. Add the native OpenAgents Effect AI loop behind the same adapter contract.
   Done in RK3 (#4807): `openagents_native` now runs behind
   `AgentRuntimeAdapter` with Effect service/layer boundaries for the language
   model and toolkit, Schema-typed tool input/output, deterministic test
   layers, typed tool denial, budget-stop interruption, and cancellation.
7. Add worker ingestion for runtime events and public-safe projections. Done in
   RK4 (#4808): the Worker has a schema-decoded append-only ingestion module,
   an explicit visibility split, public-safe projection rebuild from the log,
   generatedAt plus staleness metadata, and authority-disabled projection
   fields. No HTTP route was added in this slice, so there was no OpenAPI
   surface to register.
8. Add workroom/TUI status views that read projections rather than raw adapter
   logs. Done in RK5 (#4809): `packages/agent-runtime-schema` now exports a
   shared surface-status presenter, the Worker workroom helper and Pylon TUI
   store both derive rows from kernel projections, and neither surface needs
   adapter transcripts.
9. Add cancellation, permission-denial, budget-stop, and adapter-failure smokes.
   Done in RK5 (#4809): the smoke test drives the real adapter event streams,
   rebuilds the Worker public projection, feeds the exact projection into the
   TUI store, and asserts equal public-safe rows for all four failure paths.

## Acceptance Criteria

The first complete slice is done when:

- A fixture run, a Codex run, and an OpenCode or Claude Code run all emit the
  same runtime event contract.
- The worker can ingest that event stream without adapter-specific parsing.
- A public-safe projection can be rebuilt from the event log and includes
  freshness metadata.
- A redaction test proves raw prompts, raw logs, provider payloads, secrets, and
  private paths are excluded from portable/public records.
- A denied tool request produces `tool.denied` and no side effect.
- A cancelled run produces `run.cancelled` and closes any scoped process or
  model stream.
- The native Effect AI loop can run the same fixture contract under a test
  provider layer.

As of RK5 (#4809), the first complete slice above is implemented and tested.

## Open Questions

- Should `packages/agent-runtime-schema` be schema-only permanently, or should
  execution helpers move there after the contract stabilizes?
- The worker now stores events with an explicit visibility split and rebuilds
  public projections only from public-visible events. A durable private-event
  storage backend remains a later product/storage decision.
- Should AI SDK compatibility be implemented in the first slice or deferred
  until a provider/agent needs it?
- Should Hermes enter as a first-class adapter kind now, or remain reserved
  until the invocation protocol is fixed?

## Recommendation

Do the schema package first and keep it boring. The current adapter work is
close enough to converge, but OpenAgents should not add another one-off loop
path until `claude_code`, `codex`, `opencode`, the native Effect AI loop, and
test fixtures all map into the same runtime event contract.

## References Checked

- Effect AI introduction: https://effect.website/docs/ai/introduction/
- Effect AI tool use: https://effect.website/docs/ai/tool-use/
- Vercel AI SDK `streamText`: https://ai-sdk.dev/docs/reference/ai-sdk-core/stream-text
- Vercel AI SDK `ModelMessage`: https://ai-sdk.dev/docs/reference/ai-sdk-core/model-message
