# Email Agent Architecture (Initial)

Date: 2026-03-04
Status: Implemented baseline contract

## Purpose

Define the retained architecture and contracts for the communication-agent lane so implementation can proceed with deterministic behavior, replay safety, and clear ownership.

## Ownership Boundaries

- `apps/autopilot-desktop`
  - Owns user workflows, pane orchestration, approvals, queue controls, and operator UX.
- `crates/email-agent`
  - Owns reusable communication-domain primitives and deterministic processing pipelines.
- `crates/wgpui*`
  - Owns product-agnostic rendering primitives only.

## Runtime Topology

1. Connector lane
- Gmail OAuth credentials and token lifecycle.
- Mailbox import + incremental sync.

2. Domain processing lane
- Raw message normalization.
- Retrieval indexing.
- Style profile derivation.
- Knowledge source grounding.
- Draft generation.

3. Operator lane
- Inbox/Draft/Approval/Send/Follow-up queue state.
- Actionable failures and explicit recovery paths.

4. Audit lane
- Correlation IDs across ingest -> draft -> approval -> send.
- Immutable event records for replay and diagnostics.

## Contract Types

## `MailboxMessage`
- Stable id, thread id, participant metadata, timestamps.
- Canonicalized body fields.

## `MailboxDelta`
- Cursor + changed ids + operation type (`create`, `update`, `delete`).

## `NormalizedConversationItem`
- Deterministic normalized key.
- Structured sections (summary, quoted text, signature).

## `RetrievedContextChunk`
- Source id, source type, snippet text, score, trace metadata.

## `StyleProfile`
- Versioned profile id.
- Tone and format preferences derived from historical sent messages.

## `DraftArtifact`
- Draft id and lifecycle state.
- Input pointers (message id + context ids + profile id).
- Rendered draft text and confidence metadata.

## `ApprovalDecision`
- Decision id, actor id, timestamp, action (`approve`, `reject`, `edit`).
- Optional reason and edit notes.

## `SendAttempt`
- Idempotency key, provider message id, attempt count.
- Final outcome classification and retry metadata.

## `FollowUpJob`
- Scheduled timestamp, policy id, trigger reason.
- State (`queued`, `running`, `sent`, `failed`, `canceled`).

## Determinism and Replay Rules

- Every transform emits stable IDs from canonical inputs.
- Every external side effect uses explicit idempotency keys.
- Cursors are monotonic and persisted.
- Rebootstrap path must recompute equivalent state from source records.

## Non-goals (Current Phase)

- Multi-provider support beyond Gmail.
- Full enterprise policy surface.
- Advanced adaptive fine-tuning workflow.

## Implementation Sequence

1. Credentials + connector primitives.
2. Backfill + incremental sync.
3. Normalization + retrieval.
4. Style/knowledge + draft generation.
5. Approval/send/follow-up workflows.
6. Observability, security controls, and release gates.
