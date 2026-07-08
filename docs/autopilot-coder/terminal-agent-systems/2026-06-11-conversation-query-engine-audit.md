# Conversation And Query Engine Audit

**STATUS: HISTORICAL — point-in-time record (accurate as of its
date). Not current direction; consult MASTER_ROADMAP.**


Date: 2026-06-11

This is system #2 from the Bun/Effect terminal-agent systems list. It describes
the conversation and query engine that should sit above the runtime kernel:
turn admission, message normalization, model-stream consumption, tool-result
round trips, interruption, retries, compaction boundaries, and final result
settlement.

## Target

Build a conversation engine that can accept a user turn, enrich it with local
commands and attachments, run one or more model/tool iterations, and produce a
durable result without confusing transient stream state for durable transcript
state.

The engine should support:

- Interactive terminal sessions.
- Non-interactive scripted runs.
- Remote or web sessions.
- Subagent or delegated loops.
- Resume after process death.
- User interruption during model streaming or tool execution.
- Structured-output retries and recovery paths.

## User-Visible Capability

The user should see a coherent conversation even when the underlying loop is
doing several things at once:

- A submitted prompt is accepted and resumable before the first model token
  arrives.
- Local commands can answer without invoking the model.
- Partial model output can stream while tools are discovered or prepared.
- Tool calls can execute, return results, and trigger a follow-up model turn.
- Interrupted tool calls settle as explicit cancellation or rejection records.
- Compact, summary, or recovery boundaries appear as visible state changes,
  not hidden transcript rewrites.
- The final closeout includes success/error state, elapsed time, usage, turn
  count, stop reason, and permission denials.

## Core Design

The conversation engine should be an Effect service that consumes turns and
emits runtime events. It should not own provider APIs directly. Provider calls
belong behind the model gateway; tool calls belong behind the tool gateway.

Suggested service:

```ts
interface ConversationEngine {
  submitTurn(request: SubmitTurnRequest): Stream.Stream<ConversationEvent, ConversationError>
  interrupt(runId: AgentRunId, reason: InterruptReason): Effect.Effect<void, ConversationError>
  resume(sessionId: SessionId): Effect.Effect<ConversationSnapshot, ConversationError>
}
```

The concrete version should use branded identifiers and Effect Schema rather
than raw strings.

## Durable Versus Ephemeral State

Durable state:

- Accepted user turns.
- Normalized assistant messages after provider block boundaries are known.
- Tool-use requests and tool results.
- Attachment and artifact refs.
- Compaction or summary boundary events.
- Permission decisions and denials.
- Usage/cost snapshots.
- Final result state.

Ephemeral state:

- Provider stream chunks before they form a stable message part.
- UI shimmer/progress state.
- In-flight permission dialog state.
- In-flight classifier or hook promises.
- Active AbortController or process handle state.
- Temporary context projections.
- Speculative fallback attempts that are explicitly tombstoned or discarded.

The durable transcript should be append-only or event-sourced. Recovery should
rebuild the visible conversation from accepted events, not from partially
mutated provider payloads.

## Event Shape

Conversation-level events should refine the runtime kernel events:

- `turn.accepted`
- `turn.local_command_started`
- `turn.local_command_completed`
- `turn.model_requested`
- `turn.model_stream_started`
- `turn.assistant_part_started`
- `turn.assistant_part_delta`
- `turn.assistant_part_completed`
- `turn.tool_use_detected`
- `turn.tool_result_attached`
- `turn.followup_requested`
- `turn.compaction_boundary_recorded`
- `turn.recovery_attempted`
- `turn.tombstone_recorded`
- `turn.interrupted`
- `turn.completed`
- `turn.failed`

Every event should include session/run refs, turn refs, sequence, generatedAt,
visibility, and redaction class.

## Query Loop Phases

1. Accept input and persist the user turn before model I/O begins.
2. Run local command parsing and attachment expansion.
3. Refresh the context snapshot: instructions, memory, tools, skills, file
   read state, diagnostics, and budget.
4. Apply context-shaping steps: replacement, snip, compact, collapse, or
   summary projection.
5. Start a model request through the model gateway.
6. Stream provider output into stable assistant parts.
7. Detect tool-use parts and either execute tools as they stream or after the
   assistant message completes.
8. Attach tool results as user/tool-result messages.
9. Continue the loop if tool results require a follow-up model turn.
10. Run recovery paths for prompt-too-long, max-output, malformed tool input,
   missing tool results, or provider fallback.
11. Settle final result, usage, stop reason, and transcript state.

## Bun/Effect Boundary

Use these primitives:

- `Effect.Service` for the conversation engine.
- `Stream` for model, tool, and conversation events.
- `Queue` for user interrupts, local commands, and pending tool results.
- `Scope` for model request, tool execution, stream watchdog, and transcript
  write lifetimes.
- `Ref` for in-turn state such as current usage and active tool ids.
- `Deferred` for permission and external-control decisions.
- `Schedule` for retries, stream idle timeout, and transcript flush/drain.
- `Layer` for provider, transcript store, context assembler, permission, and
  tool gateway substitution in tests.

## Safety Rules

- Persist the accepted user turn before starting a provider request.
- Never replay or resume from partial provider chunks unless they were
  normalized into durable assistant parts.
- If a provider fallback discards an attempted stream, record tombstones or a
  discarded-attempt event so UI and transcript state agree.
- Tool results must reference the assistant tool-use id that requested them.
- Missing tool results should become explicit synthetic error results, not
  silent context corruption.
- User interruption must close active scopes and produce deterministic
  cancellation events.
- Compact/summarize operations must preserve tool-use/result consistency.
- Public projections must not expose raw prompts, provider payloads, private
  logs, or private paths.

## Failure Taxonomy

- `TurnInputInvalid`
- `LocalCommandFailed`
- `ContextAssemblyFailed`
- `ContextTooLarge`
- `ModelStreamIdleTimeout`
- `ProviderFallbackDiscardedAttempt`
- `ToolUseMissingResult`
- `ToolUseValidationFailed`
- `ToolUsePermissionDenied`
- `ToolUseInterrupted`
- `TranscriptWriteFailed`
- `CompactionFailed`
- `StructuredOutputRetryExhausted`
- `ConversationInterrupted`

Each error should define whether the loop can continue, retry, compact, ask the
user, or terminate.

## Tests

Minimum tests for the query engine:

- Accept-turn persistence happens before a model stream starts.
- Local command turn returns without model invocation.
- Model stream with text only produces one final assistant message.
- Model stream with tool call executes tool, records result, and follows up.
- Tool validation failure becomes a model-visible tool-result error.
- Missing tool result is repaired with a synthetic error result.
- User interrupt during model streaming produces `turn.interrupted`.
- User interrupt during tool execution produces a cancelled tool result.
- Provider fallback tombstones partial assistant output before retrying.
- Compaction boundary preserves the tool-use/result pair.
- Resume rebuilds the same transcript from durable events.

## Decision

The conversation/query engine should be the orchestrator of turns, not the
owner of tools or provider APIs. It should accept turns, create bounded
context, consume streams, mediate continuation, and settle durable transcript
state through typed events. Everything provider-specific or tool-specific
should sit behind Effect service boundaries.
