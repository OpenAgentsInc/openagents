# Compaction And Summarization System Audit

Date: 2026-06-11

This is system #13 from the Bun/Effect terminal-agent systems list. It defines
how long-running agent sessions should preserve continuity when context is too
large: manual compaction, automatic compaction, session-memory compaction,
tool-result trimming, summary boundaries, and recovery from failed compaction.

## Target

Build a compaction system that reduces model context without corrupting the
conversation. Compaction should be visible, replayable, auditable, and safe for
tool-use/result invariants.

The system should support:

- Manual compaction.
- Automatic compaction near context limits.
- Reactive compaction after provider context-limit errors.
- Lightweight trimming of bulky tool results.
- Summary-based replacement of older conversation state.
- Session-memory-based replacement when a durable memory summary exists.
- Post-compaction restoration of still-relevant files, tools, plans, and task
  state.

## User-Visible Capability

The user should see compaction as an explicit state transition:

- The session records that a compaction happened.
- The user can provide compaction instructions.
- The system preserves recent working context.
- The agent continues with plans, tasks, active files, and available tools.
- If compaction fails, the failure is explained without pretending the session
  was compacted.
- If older details were dropped, the closeout should say so honestly when it
  matters.

## Core Design

Define a `CompactionService` that consumes a context snapshot and emits a
compaction result.

Suggested service boundary:

```ts
interface CompactionService {
  compact(request: CompactRequest): Effect.Effect<CompactResult, CompactError>
  microcompact(request: MicrocompactRequest): Effect.Effect<MicrocompactResult, CompactError>
  shouldCompact(request: CompactThresholdRequest): Effect.Effect<CompactDecision, CompactError>
}
```

The conversation engine should decide when to ask this service. The service
should not own the whole query loop.

## Durable Shape

Persist compaction as a boundary plus replacement records:

- Boundary id.
- Trigger: manual, automatic, reactive, session-memory, or recovery.
- Pre-compaction context estimate.
- Post-compaction context estimate.
- Summary source refs.
- Preserved recent-message refs.
- Preserved tool-pair refs.
- Restored file, plan, skill, task, and adapter refs.
- Hook or policy refs that influenced the summary.
- User-visible message.
- Failure or retry metadata.

The boundary is a first-class transcript event. Resume should rebuild the same
post-compaction transcript from that event rather than relying on an in-memory
splice.

## Compaction Strategies

Use multiple strategies with clear scope:

- `microcompact`: clears or summarizes bulky old tool results while keeping the
  surrounding turn structure.
- `summary_compact`: asks a model to summarize older conversation state and
  keeps a recent suffix.
- `session_memory_compact`: uses an already-extracted durable summary and keeps
  only unsummarized recent turns.
- `reactive_compact`: responds to a context-limit failure by trimming and
  retrying with a smaller prompt.
- `partial_compact`: preserves a prefix or suffix when only one segment needs
  replacement.

Each strategy should report what it dropped, what it preserved, and why.

## Invariant Preservation

Compaction must preserve protocol invariants:

- Do not keep a tool result without the matching tool request.
- Do not keep a tool request while dropping its result unless the result is
  represented as an explicit synthetic failure or summary.
- Do not split streaming assistant parts that must be reassembled together.
- Do not drop active approval, task, or plan state.
- Do not re-inject stale tool listings without a freshness marker.
- Do not treat images, documents, or raw logs as required summary input when a
  safe marker or artifact ref is enough.

The summary can be lossy, but the transcript structure cannot be corrupted.

## Trigger Model

Automatic triggers should use context-window estimates with output headroom
reserved. Thresholds should distinguish:

- Warning: user-visible nudge.
- Automatic compaction threshold.
- Blocking threshold.
- Manual compaction buffer.
- Reactive failure path.

Automatic compaction needs a circuit breaker. If repeated compaction attempts
fail in the same session, the system should stop retrying blindly and surface a
recoverable error.

## Bun/Effect Boundary

Use these primitives:

- `Effect.Service` for compaction.
- `Schema` for compact requests, results, boundaries, and strategy metadata.
- `Stream` for summary generation progress.
- `Scope` for model calls, hooks, temporary files, and cancellation.
- `Schedule` for bounded retry after prompt-too-long or transient provider
  errors.
- `Queue` for compaction progress events to the UI.
- `Layer` for summary model, transcript store, memory store, and test fixtures.

Compaction should be cancellable. Cancellation must leave the pre-compaction
transcript unchanged and record a cancelled compaction attempt.

## Post-Compaction Restoration

After successful compaction, restore bounded context that the model needs:

- Active plan and progress state.
- Recent task notifications.
- Active workspace refs.
- Relevant file snapshots or file refs.
- Deferred or discovered tool inventory.
- Active external adapter state.
- Invoked skill or command context.
- Current instruction and memory context.

Restoration should use caps per category. Large restored content should become
artifact refs or summaries.

## Safety Rules

- Compaction is a transcript mutation and must be recorded.
- The original pre-compaction data should remain available in private/local
  storage when policy allows.
- Public projections should show the boundary, not raw dropped content.
- User-provided compaction instructions should be merged with policy and hook
  instructions, not override safety rules.
- Compaction must not introduce new permissions.
- A failed compaction must not mark the session as compacted.
- Summaries should state uncertainty when input was dropped to recover from a
  context-limit error.

## Tests

Minimum regression coverage:

- Manual compaction creates a boundary and summary.
- Automatic compaction triggers at the configured threshold.
- Repeated automatic failures trip the circuit breaker.
- Tool-use/result pairs remain valid after compaction.
- Bulky tool outputs are microcompacted with explicit markers.
- Session-memory compaction keeps unsummarized recent messages.
- Reactive compaction retries after a context-limit fixture.
- Cancellation leaves the original transcript intact.
- Post-compaction restoration includes active plan and task refs.
- Replay from compact boundaries produces the same visible transcript.

## OpenAgents Translation Notes

When promoted, map compaction boundaries to OpenAgents event logs, artifact
refs, public-safe closeouts, and projection freshness. Verify current issue and
roadmap state before claiming any path is live.

## Decision

Compaction should be an explicit event-sourced subsystem. It should reduce
context while preserving conversation invariants, not silently rewrite history
or rely on provider prompts as the durable record.
