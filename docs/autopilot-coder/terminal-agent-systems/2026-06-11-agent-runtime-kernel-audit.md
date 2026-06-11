# Agent Runtime Kernel Audit

Date: 2026-06-11

This is system #1 from the Bun/Effect terminal-agent systems list. It captures
the runtime contract OpenAgents needs for terminal coding agents: one owned
runtime kernel, one durable event shape, and adapters for native and delegated
loops.

The core boundary is that provider SDK messages, delegated-loop transcripts,
and terminal UI streams are edge formats. The durable OpenAgents domain model
is a versioned, typed event log plus projections derived from that log.

## Target

Build a Bun/Effect terminal coding agent that can run an OpenAgents-native loop
while also consuming delegated loops through the same adapter boundary.

Every loop should enter the system through the same shape:

- A run record describing assignment, workspace, authority, adapter, budget,
  permission policy, and redaction policy.
- A stream of runtime events for context assembly, model output, tool calls,
  approvals, delegated-loop handoffs, artifacts, usage, and closeout.
- Typed tool and artifact records that can be replayed, inspected, redacted,
  projected, and tested without preserving raw provider payloads.

## Base Recommendation

Use Effect services for the native model-loop substrate and keep any
provider-specific message shape at the provider edge.

Effect matches the desired runtime architecture:

- Model and tool services can be provider-agnostic.
- Provider selection can happen through `Layer` composition.
- Tool definitions can use schemas for parameters, success, and failure.
- Streaming, cancellation, retries, timeouts, tracing, and test layers fit
  naturally into Effect programs.

Provider SDK message formats remain useful as interoperability projections.
They should not become durable storage.

## Durable Shape

Define these records with Effect Schema and version every externally persisted
object.

- `AgentRun`: stable run identity, assignment identity, workspace identity,
  adapter kind, loop kind, budget, permissions, visibility, redaction policy,
  and lifecycle state.
- `AgentTurn`: user, system, or scheduler input accepted into a run.
- `AgentStep`: one model or delegated-agent step within a turn.
- `AgentMessage`: durable message with parts that are safe to persist.
- `AgentPart`: text, reasoning summary, tool request, tool result, artifact
  reference, delegated-loop event summary, usage record, or error.
- `AgentRuntimeEvent`: append-only event union with sequence numbers and
  timestamps.
- `ToolInvocation`: typed request, approval state, execution state, result,
  failure, truncation, and artifact refs.
- `DelegatedInvocation`: delegated loop command, session ref, status,
  summarized events, artifact refs, closeout, and failure.

The durable store should keep redacted summaries and references. Raw prompts,
provider payloads, shell logs, private paths, secrets, credentials, and
third-party transcripts belong in local/private traces, not portable records.

## Event Union

The runtime event stream should be explicit enough to power terminal updates,
remote control, logs, closeouts, and regression tests from one source.

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

Every event should include `runId`, `sequence`, `generatedAt`, `visibility`,
and enough stable refs to rehydrate higher-level run state.

## Runtime Services

Keep the kernel small and model everything else as Effect services.

- `RuntimeKernel`: accepts turns, starts runs, consumes event streams, and
  settles lifecycle state.
- `AgentLoopDriver`: executes the native loop and emits runtime events.
- `ModelGateway`: wraps model services and provider projections.
- `ToolGateway`: resolves tools, validates schemas, asks permission, executes,
  truncates, and emits tool events.
- `DelegatedLoopGateway`: starts delegated loops and normalizes their events.
- `ContextAssembler`: builds a bounded context snapshot for a step.
- `WorkspaceService`: owns checkouts, worktrees, file snapshots, and write
  policies.
- `PermissionService`: owns approval prompts, policy decisions, and audit
  records.
- `RuntimeEventLog`: append-only event persistence and replay.
- `ArtifactService`: stores references to patches, files, logs, screenshots,
  closeouts, and receipts.
- `UsageLedger`: records tokens, wall time, tool cost, delegated-loop cost,
  payment state, and budget use.
- `ProjectionService`: produces terminal state, remote-control JSON, API
  records, and public-safe closeouts.

## Adapter Model

Adapters only translate between a loop and the kernel event stream. They do
not own task state, acceptance state, or projection policy.

| Adapter | Input | Output |
| --- | --- | --- |
| Native Effect loop | `AgentRun` plus context snapshot | model/tool events |
| Provider bridge | `AgentMessage` projection | model/tool stream events |
| Delegated coding loop | assignment plus workspace | delegated-loop events and closeout |
| Hosted container | assignment plus workspace | delegated-loop events, artifacts, usage |
| Test fixture | scripted event stream | deterministic runtime events |

The adapter interface should be a typed stream:

```ts
interface AgentRuntimeAdapter {
  readonly kind: string
  canRun(request: AgentRunRequest): Effect.Effect<boolean, AdapterError>
  start(request: AgentRunRequest): Stream.Stream<AgentRuntimeEvent, AdapterError>
  cancel(runId: AgentRunId): Effect.Effect<void, AdapterError>
}
```

The implementation should use Effect services and branded identifiers, not raw
strings.

## Loop Phases

1. Admission: validate assignment, workspace, policy, budget, and adapter.
2. Context snapshot: collect bounded files, prior messages, instructions,
   memory, tool inventory, and issue or ticket state.
3. Adapter selection: choose native loop, provider bridge, delegated loop, or
   fixture through a typed selector.
4. Step execution: stream model or delegated-loop events into the event log.
5. Tool mediation: validate tool input, request permission, execute, truncate,
   and persist references.
6. Settlement: update run, turn, and step state from events.
7. Closeout: record artifacts, patches, tests, failures, usage, and next
   action.
8. Projection: derive terminal, remote-control, API, and public-safe views.

## Provider Edge Shape

If a provider SDK bridge is used, restrict it to this role:

- Convert durable `AgentMessage` records into provider-facing model messages.
- Convert provider stream parts into `AgentRuntimeEvent`.
- Convert provider tool calls into `tool.*` runtime events.
- Keep provider options and metadata at the edge unless explicitly whitelisted
  into the durable schema.

Do not persist provider SDK messages as the canonical record. The canonical
record is the runtime event log plus derived run/message/tool/artifact
projections.

## Native Loop Shape

The native loop should use Effect directly:

- Define toolkits with Effect schemas for inputs, success, and failure.
- Provide model implementations through `Layer`.
- Run streams under scoped resources so cancellation and interruption are
  explicit.
- Use test layers and scripted streams for deterministic regression tests.
- Emit runtime events from every boundary crossing instead of returning opaque
  text.

## Tests

The minimum regression set for the kernel:

- Schema round trips for every event and durable record.
- Deterministic native-loop fixture that emits text, tool call, tool result,
  and completion.
- Delegated-loop fixture that emits start, progress, artifact, failure, and
  closeout events.
- Cancellation test that interrupts model streaming and tool execution.
- Permission test that denies a tool and proves no side effect ran.
- Redaction test that prevents raw prompts, raw logs, secrets, and private
  paths from entering portable projections.
- Replay test that rebuilds terminal state and API state from the same event
  log.
- Budget test that stops the loop before it crosses configured limits.

## Decision

The system should define its own runtime event contract and use Effect as the
native execution base. Provider SDK compatibility is valuable, but it should
stay a provider bridge and optional interoperability layer. Delegated agents
should be treated as adapters that emit the same events as the native loop, so
the rest of the application sees one runtime shape.
