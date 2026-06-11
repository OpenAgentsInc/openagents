# Tool Registry And Tool Contracts Audit

Date: 2026-06-11

This is system #3 from the Bun/Effect terminal-agent systems list. It captures
the tool contract needed for a terminal coding agent: typed registration,
schema validation, permission mediation, progress streaming, result mapping,
hooks, cancellation, output budgeting, and public-safe summaries.

## Target

The tool system should let the native agent loop, provider adapters, MCP
adapters, plugins, and external-agent bridges all call tools through one
contract. A tool should be a typed capability, not a loose function in a global
map.

## User-Visible Capability

The user should experience tools as consistent actions:

- Every tool has a name, description, input schema, and visible summary.
- Invalid tool input fails with a useful model-visible error.
- Dangerous actions ask for approval or deny deterministically.
- Read-only/search tools can run concurrently.
- Mutating tools run serially unless they explicitly prove isolation.
- Long-running tools stream progress.
- Large outputs become artifact refs with previews.
- Tool rejections and failures are rendered distinctly.
- Tool results can be searched in transcripts without indexing invisible
  model-only wrappers.

## Core Design

Define tools with Effect Schema, not ad hoc JSON objects.

Suggested service boundary:

```ts
interface ToolRegistry {
  list(request: ToolListRequest): Effect.Effect<ReadonlyArray<ToolDescriptor>, ToolRegistryError>
  resolve(name: ToolName): Effect.Effect<ToolDefinition, ToolRegistryError>
  execute(request: ToolExecutionRequest): Stream.Stream<ToolExecutionEvent, ToolExecutionError>
}
```

Tool definitions should be data plus behavior:

- Name and stable aliases.
- Search/discovery hint.
- Input schema.
- Output schema.
- Visibility and redaction metadata.
- Read-only/destructive/concurrency metadata.
- Permission matcher.
- Permission check.
- Input validator.
- Executor.
- Progress event renderer/projection.
- Model-facing result mapper.
- User-facing result summary.
- Output persistence policy.

## Durable Shape

Persist tool execution as events and records, not raw UI messages.

- `ToolDescriptor`: stable name, version, schema refs, capability tags.
- `ToolInvocation`: tool name, input hash/ref, caller step, permission state,
  execution state, and output refs.
- `ToolProgressEvent`: bounded progress payload.
- `ToolResult`: typed success output, typed failure output, artifact refs,
  truncation metadata, and redaction class.
- `ToolDecision`: allow, deny, ask, forced, classifier, rule, hook, mode, or
  external controller.

## Execution Pipeline

1. Resolve the tool by primary name or alias.
2. Parse input through schema.
3. Run tool-specific validation.
4. Run pre-tool hooks.
5. Run permission decision.
6. Ask or deny if policy requires it.
7. Start execution in a scoped Effect.
8. Stream progress events.
9. Map output to a typed result and a model-facing result block.
10. Persist oversized output as an artifact ref.
11. Run post-tool hooks.
12. Emit final success, failure, denied, or cancelled event.

## Concurrency Rules

Default to serial execution. A tool may opt into concurrency only when all of
these are true:

- It is read-only or otherwise proves no shared mutation.
- Its input can be validated before execution.
- Its output does not mutate context in a conflicting order.
- Its cancellation cannot orphan child work.

The scheduler may batch adjacent concurrency-safe tools. Non-concurrent tools
should form exclusive barriers. If a concurrent sibling fails in a way that
invalidates the batch, remaining sibling executions should be cancelled and
recorded as synthetic failures.

## Tool Discovery

Tools can be present, deferred, or hidden:

- Present tools are sent to the model immediately.
- Deferred tools require a search/discovery step before their schema is sent.
- Hidden/internal tools are callable only by runtime code.

When a deferred tool is called without its schema having been loaded, the error
should tell the model how to discover the tool and retry.

## Bun/Effect Boundary

Use:

- `Schema` for input, output, progress, and failure.
- `Effect.Service` for registry and execution.
- `Layer` for base tools, plugin tools, MCP tools, and test tools.
- `Stream` for progress and result events.
- `Scope` for child process, file handle, network, and cancellation lifetime.
- `Queue` for queued tool calls and progress fanout.
- `Cause` for preserving typed failure detail without leaking private payloads.

## Permission Contract

Tool permission checks should return a typed decision:

- `allow`
- `deny`
- `ask`
- `passthrough`

The decision should include:

- Decision reason.
- Optional updated input.
- Suggested remembered rules.
- Redaction metadata.
- Whether the decision can be persisted.

Tool-specific permission checks may add context such as path, command prefix,
remote endpoint, or destructive flag, but they should not own the global
approval UI.

## Output Budgeting

Each tool should declare an output policy:

- Inline maximum.
- Preview size.
- Artifact persistence allowed/forbidden.
- Public-safe summary mapper.
- Search-index text mapper.
- Model-facing truncation message.

Some tools should never persist output because persisting it creates circular
read paths or private-data risk. Other tools should persist large output and
return a preview plus artifact ref.

## Safety Rules

- Schema parse runs before validation and permission.
- Validation failure is not a side effect.
- Permission denial must not execute the tool.
- Hooks cannot silently broaden permission.
- Tool input copied to observers should be redacted/backfilled separately from
  provider-bound input so prompt-cache stability is preserved.
- Tool names and aliases should be stable; renamed tools need compatibility
  aliases but should not create duplicate authority paths.
- Security-relevant tools must implement classifier/permission summaries.
- Unknown tool calls produce explicit model-visible errors.

## Tests

Minimum tests:

- Unknown tool yields a typed tool-result error.
- Invalid schema yields a validation error with no execution.
- Tool-specific validation failure yields an error with no permission prompt.
- Denied permission yields no side effect.
- Ask permission waits for an external decision.
- Read-only tools batch concurrently.
- Mutating tool serializes after read-only batch.
- Progress streams before final result.
- Oversized output persists to artifact ref with preview.
- Cancelled tool closes its scope and emits a cancelled result.
- Deferred tool called too early tells the model how to discover it.

## Decision

The tool registry should be an Effect service with schemas, permission
mediation, progress streams, and result policies. Tools should be composable
capabilities with typed execution records. The model-facing tool result is only
one projection of the tool execution; it should not be the canonical record.
