# Structured Event Log Audit

**STATUS: HISTORICAL — point-in-time record (accurate as of its
date). Not current direction; consult MASTER_ROADMAP.**


Date: 2026-06-11

This is system #46 from the Bun/Effect terminal-agent systems list. It defines
the append-only event log that should underwrite terminal-agent replay,
workroom projection, receipts, debugging, and audit trails.

## Target

Build a structured event log where every meaningful runtime transition is
captured as a typed event with sequence, subject refs, visibility,
redaction class, and replay semantics.

The event log is the raw operational spine. User-facing projections are derived
views, not separate truth.

## User-Visible Capability

Users should be able to:

- Resume sessions from durable history.
- Inspect what happened in a run.
- See public-safe progress and private detail at the right scope.
- Trust that cancellation, approval, tool calls, artifacts, and closeouts are
  not invented in final prose.
- Export a redacted support bundle.
- Recover from corrupted or partial projections by replaying events.

## Event Model

Each event should include:

- Event id.
- Run id.
- Sequence.
- Event kind.
- Subject refs.
- Actor or service ref.
- Timestamp.
- Visibility.
- Redaction class.
- Payload schema version.
- Idempotency key where applicable.
- Parent or correlation refs.

Event kinds should cover model streaming, tool proposals, tool results,
approval prompts, approvals, denials, file edits, shell execution, artifact
creation, receipt creation, status transitions, errors, cancellation, and
compaction.

## Bun/Effect Boundary

Use Effect services for:

- `RuntimeEventLogService`: append, read, and tail event streams.
- `RuntimeReplayService`: rebuild projections from events.
- `EventSchemaRegistry`: decodes versioned event payloads.
- `EventProjectionService`: derives terminal, companion, public, team, and
  operator views.
- `EventExportService`: emits redacted support bundles.

Use Stream for event tails, Queue for write buffering, Schema for payload
versions, and Scope for run-bound event subscriptions.

## Safety Rules

- Append events before deriving projections.
- Enforce monotonic sequence per run.
- Store private payloads only at private visibility.
- Never include secrets, provider payloads, wallet material, raw private repo
  content, or raw prompts in public-safe events.
- Projection failures do not mutate the event log.
- Event replay must be deterministic for a fixed event stream.
- Event deletion requires retention policy, not ad hoc cleanup.

## OpenAgents Translation Notes

As of 2026-06-11, OpenAgents has an agent runtime schema package and Pylon
adapter tests that prove event-to-projection behavior for selected runtime
lanes. The terminal-agent README does not yet include a structured event log
audit.

Related open issue anchors:

- #4778 mission/work-order unification should put every lane under one event
  and record layer.
- #4768 overnight unattended proof smoke needs replayable event evidence.
- #4779 writeback symmetry depends on artifact and authority events.
- #4785 settlement visibility law depends on dereferenceable receipt events.

No terminal-wide event-log claim should be green until all native and adapter
events share one schema registry and replay path.

## Tests

Minimum coverage:

- Append ordered events and reject sequence gaps.
- Decode every supported event kind.
- Replay events into run and workroom projections.
- Reject public events with private material.
- Recover projection state after deleting derived views.
- Preserve idempotency for duplicate writes.
- Export redacted support bundles.
- Migrate an older event schema fixture.

## Decision

The structured event log should be the runtime's durable truth. UI status,
model context, companion views, and closeout summaries should all be
derivable, checkable projections over typed events.

