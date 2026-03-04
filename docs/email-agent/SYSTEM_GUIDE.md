# Email Agent System Guide

## Welcome

If you are new to this system, here is the simple version:

The Email Agent is a safety-first automation lane that helps process inbound email, generate grounded draft replies, require auditable approval, send with idempotent delivery rules, and schedule follow-ups under explicit policy. It is designed to be deterministic, replay-safe, tenant-isolated, and operationally transparent.

This guide is the single "what this system is and how it works" document.

## Who This Is For

- Product and operations teams who need an end-to-end mental model.
- Engineers implementing or extending the lane.
- On-call operators handling incidents.
- Auditors reviewing traceability, privacy, and control coverage.

## System At A Glance

The lane is implemented primarily in `crates/email-agent` and surfaced in desktop through dedicated panes in `apps/autopilot-desktop`.

Core lifecycle:
1. Connect mailbox and validate OAuth lifecycle.
2. Backfill mailbox state and maintain incremental sync cursor.
3. Normalize inbound content into deterministic conversation records.
4. Retrieve historical context + knowledge grounding.
5. Generate draft response artifact.
6. Run approval workflow (manual or policy path).
7. Send through idempotent execution path with retries.
8. Schedule and execute policy-driven follow-ups.
9. Emit trace/audit records for full accountability.

## Goals And Guarantees

### Primary goals

- Deterministic behavior and replay-safe state transitions.
- No outbound send without auditable decision path.
- Exact-once-or-deterministic-failure send semantics.
- Tenant hard isolation for runtime/config/storage/secret scope.
- Traceability from sent message back to source context and decisions.

### Non-goals in current scope

- Multi-provider mailbox support beyond Gmail.
- Enterprise policy surface beyond current explicit controls.
- Autonomous send without clear policy/approval controls.

## Architecture

### Ownership boundaries

- `apps/autopilot-desktop`
  - Owns operator UX, panes, and interaction workflows.
- `crates/email-agent`
  - Owns reusable communication-domain logic and policy primitives.

### Runtime lanes

1. Connector lane
- OAuth lifecycle state, mailbox backfill, incremental sync.

2. Domain lane
- Normalization, retrieval, style profile, grounding, draft generation.

3. Operator lane
- Approval controls, queue controls, send execution, follow-up policy.

4. Governance lane
- Observability/audit trails, quality gates, security/privacy controls, tenant isolation, release gates.

## Module Map (Implemented)

`crates/email-agent/src/lib.rs` exports the full lane API. Main modules:

- `gmail_connector`: backfill import checkpoints and mailbox provider contract.
- `gmail_sync`: incremental cursor sync, delta dedupe, rebootstrap signaling.
- `normalization`: deterministic normalization of message content sections.
- `retrieval`: lexical/semantic context retrieval with stable ordering.
- `style_profile`: derive tone/format tendencies from historical samples.
- `knowledge_base`: ingest/chunk/search grounding knowledge sources.
- `draft_pipeline`: draft artifact generation + policy validation.
- `approval_workflow`: approve/reject/edit decisions, pause/resume, kill switch.
- `send_execution`: idempotent send path, retry classing, final delivery state.
- `follow_up_scheduler`: policy-driven follow-up scheduling and execution events.
- `observability`: lifecycle event tracing with correlation IDs and redaction.
- `quality_scoring`: rubric-based regression scoring and threshold gate.
- `security_privacy`: retention, deletion/export control, access audit, redaction.
- `tenant_isolation`: per-tenant provisioning, scope rotation, teardown, isolation verification.
- `e2e_harness`: deterministic integration harness with failure injection.

## End-To-End Lifecycle Detail

### 1) Mailbox connect and preflight

- Validate OAuth token lifecycle before connector operations.
- Reject expired token state before attempting sync.

### 2) Backfill

- `run_gmail_backfill` imports messages page-by-page.
- Persisted checkpoint includes `next_page_token` and `imported_count`.

### 3) Incremental sync

- `apply_gmail_incremental_sync` advances cursor using deltas.
- Duplicate delta keys are dropped deterministically.
- Backward cursor movement triggers explicit rebootstrap requirement.

### 4) Normalization and indexing

- Normalize headers/body into `NormalizedConversationItem`.
- Preserve deterministic IDs and extracted sections (summary/quoted/signature).
- Upsert into retrieval index for replay-safe search behavior.

### 5) Grounding and drafting

- Search historical context + knowledge chunks.
- Generate draft artifact with:
  - source message linkage
  - context/grounding pointers
  - confidence score
  - rationale metadata

### 6) Approval gate

- Draft enters approval queue.
- Allowed actions: approve, reject, request edits.
- Each decision records actor, timestamp, reason, and policy path.
- Queue pause/resume and kill switch controls are audited.

### 7) Send execution

- Send request includes idempotency key + payload fingerprint.
- Same key + same fingerprint reuses existing send record.
- Same key + different fingerprint is rejected as conflict.
- Failure classes:
  - transient -> retry schedule with deterministic backoff
  - permanent -> terminal deterministic failure
- Success stores provider message ID and final state.

### 8) Follow-up scheduling

Rules include:
- no reply after N days
- unanswered critical thread after N hours
- reminder cadence with cap

Constraints include:
- business-hour window
- quiet-hour deferral
- per-recipient daily limit

Outputs include explicit upcoming/executed/deferred/skipped events.

### 9) Traceability

- Correlation IDs tie ingest -> retrieve -> draft -> approve -> send -> follow-up.
- Diagnostics report required stage completeness for sent outcomes.
- Sensitive metadata keys are redacted in diagnostic outputs.

## Data And ID Strategy

Determinism relies on stable identity contracts:

- Normalized IDs: stable by source message ID.
- Draft IDs: derived from source message and style profile.
- Decision IDs / queue control IDs: monotonic lane-local sequence IDs.
- Send IDs / audit event IDs: deterministic sequence IDs per execution state.
- Correlation IDs: deterministic seed-based derivation for trace continuity.

## Desktop UX Surfaces

Email lane panes in desktop:

- Email Inbox
- Email Draft Queue
- Email Approval Queue
- Email Send Log
- Email Follow-up Queue

These panes expose operator-visible state machines and failure reasons. Pane docs:
- `docs/PANES.md`

## Security, Privacy, And Compliance Controls

Implemented controls:

- Retention policy enforcement by data category.
- Structured deletion workflow with receipts.
- Role-gated export scopes:
  - metadata-only (broad role access)
  - full-content (auditor only)
- Access audit log for export/delete actions (allowed and denied).
- Redaction policy for sensitive keys/tokens/emails in diagnostics.

Operational runbook:
- `docs/email-agent/SECURITY_PRIVACY_RUNBOOK.md`

## Tenant Isolation Model

Per-tenant separation covers:

- Config paths
- State DB and attachment storage
- Audit log paths
- Runtime identity refs
- Secret scope + credential namespace
- Network and relay boundaries

Lifecycle operations:
- Provision tenant lane
- Rotate tenant secret scope version
- Produce explicit teardown plan (revoke + wipe)
- Verify hard isolation constraints

Reference:
- `docs/email-agent/TENANT_ISOLATION.md`

## Quality And Release Gates

### Quality rubric dimensions

- tone match
- factual grounding
- clarity
- actionability
- safety

### Gates

- `scripts/lint/email-agent-quality-gate.sh`
  - Fails if golden-set quality thresholds regress.
- `scripts/lint/email-agent-release-gate.sh`
  - Runs quality gate + deterministic e2e harness.

## Testing Strategy

### Unit-level deterministic tests

Each module includes focused tests for invariants and edge cases (dedupe, cursor stale, policy violations, redaction, etc.).

### Integration-level harness

`e2e_harness` validates the full loop and failure injection scenarios:

- token expiry
- Gmail API rate limit during sync
- stale sync cursor
- permanent send failure

### Common commands

- `cargo test -p openagents-email-agent`
- `cargo test -p openagents-email-agent e2e_harness`
- `scripts/lint/email-agent-quality-gate.sh`
- `scripts/lint/email-agent-release-gate.sh`

## Operations And Runbooks

Primary docs:

- Onboarding: `docs/email-agent/ONBOARDING.md`
- Day-2 operations: `docs/email-agent/OPERATIONS_PLAYBOOK.md`

These include:
- setup checklist
- credential setup and sync verification
- quality/release checks
- troubleshooting matrix
- SLA/SLO targets and escalation path

## Failure Model Summary

Key failure classes and handling:

- OAuth token expired: fail-fast at connect stage.
- Sync rate limit: explicit provider error path and retry/recovery workflow.
- Stale sync cursor: rebootstrap-required signal.
- Draft policy failure: deterministic policy violation error.
- Approval missing/blocked: send authorization denied.
- Send permanent failure: terminal failure with audit record.
- Follow-up constraint violations: deferred or skipped with explicit reason.

## Extensibility Guidance

When extending this lane:

1. Keep all state transitions explicit and auditable.
2. Preserve stable IDs and replay-safe ordering.
3. Add failure-mode tests before adding happy-path behavior.
4. Keep secrets out of logs by default, not by convention.
5. Update quality and e2e gates for every new stage.
6. Update this guide and linked runbooks when behavior changes.

## Canonical Reference Set

- `docs/email-agent/ARCHITECTURE.md`
- `docs/email-agent/TENANT_ISOLATION.md`
- `docs/email-agent/SECURITY_PRIVACY_RUNBOOK.md`
- `docs/email-agent/ONBOARDING.md`
- `docs/email-agent/OPERATIONS_PLAYBOOK.md`
- `docs/PANES.md`
- `crates/email-agent/src/lib.rs`
