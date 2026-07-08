# Error Taxonomy And Recovery Audit

**STATUS: HISTORICAL — point-in-time record (accurate as of its
date). Not current direction; consult MASTER_ROADMAP.**


Date: 2026-06-11

This is system #11 from the Bun/Effect terminal-agent systems list. It defines
how the terminal agent should classify failures, decide whether recovery is
possible, report user-safe errors, and preserve enough evidence for debugging.

## Target

Build a typed error system where failures are explicit values at service
boundaries. The user should see clear, actionable failure states. The runtime
should know whether to retry, compact, ask, deny, continue, or stop.

## User-Visible Capability

The user should get:

- A short explanation of what failed.
- Whether the agent retried or can retry.
- Whether user approval or input is needed.
- Whether work can continue in another lane.
- Which artifacts or logs can be inspected.
- A final state that distinguishes failure, cancellation, denial, timeout, and
  partial success.

The agent should not collapse all failures into generic exceptions.

## Error Shape

Define a versioned `AgentError` union:

- Error id.
- Category.
- Message for user.
- Private diagnostic ref.
- Retryability.
- Recovery strategy.
- Redaction class.
- Origin service.
- Related run, turn, task, tool, or artifact refs.
- Cause chain ref when available.
- Timestamp.

The public message should be safe to show in transcripts. Private stack traces,
raw provider payloads, secrets, file paths, and logs should be referenced, not
inlined.

## Taxonomy

Recommended categories:

- `InputInvalid`
- `WorkspaceBoundaryViolation`
- `PermissionDenied`
- `ApprovalUnavailable`
- `ToolValidationFailed`
- `ToolExecutionFailed`
- `ProcessTimeout`
- `ProcessKilled`
- `ContextTooLarge`
- `ContextAssemblyFailed`
- `ModelRequestFailed`
- `ModelStreamTimeout`
- `ModelOutputInvalid`
- `ProviderRateLimited`
- `ProviderOverloaded`
- `ProviderAuthFailed`
- `NetworkTransient`
- `NetworkPermanent`
- `StorageReadFailed`
- `StorageWriteFailed`
- `StorageCorrupt`
- `ResumeConflict`
- `TaskFailed`
- `ExternalAdapterFailed`
- `ArtifactWriteFailed`
- `InvariantViolation`
- `InternalBug`

Every category should declare whether it is usually retryable, recoverable by
compaction, recoverable by user input, recoverable by alternate adapter, or
terminal.

## Recovery Matrix

| Category | Default Recovery |
| --- | --- |
| Input invalid | Ask for corrected input or reject the turn. |
| Workspace boundary violation | Deny, explain boundary, optionally ask to add a root. |
| Permission denied | Record denial, continue without the action if possible. |
| Approval unavailable | Deny in headless mode or route to remote approval. |
| Tool validation failed | Return structured tool error to the model. |
| Tool execution failed | Preserve output, summarize, and let the model recover. |
| Process timeout | Cancel process, preserve partial output, ask or retry if safe. |
| Context too large | Compact, trim, or ask to narrow scope. |
| Model stream timeout | Retry with bounded policy or fail the turn. |
| Provider rate limit | Back off, use budget policy, or stop with retry time. |
| Provider auth failed | Stop and ask for credential repair. |
| Network transient | Retry with schedule and jitter. |
| Storage corrupt | Stop mutation, preserve evidence, require repair path. |
| Resume conflict | Fork, reconcile, or ask the user to choose. |
| External adapter failed | Capture adapter closeout and decide alternate lane. |
| Invariant violation | Stop affected run and record a high-severity event. |
| Internal bug | Fail closed and preserve diagnostic ref. |

Recovery policy should be data-driven so tests can assert it.

## Bun/Effect Boundary

Use Effect's typed failure channel for expected failures and defects only for
programmer bugs.

Recommended primitives:

- `Schema` for error records and recovery decisions.
- `Effect.catchTag` for local recovery.
- `Effect.retry` with `Schedule` for retryable external failures.
- `Cause` capture for private diagnostics.
- `Layer` substitution for provider and storage failure fixtures.
- `Stream` error mapping for model, tool, and task streams.
- `Queue` for user-visible error events and notifications.

Each service should convert unknown thrown values into a typed `AgentError`
before crossing the boundary.

## Recovery Events

Record recovery as events:

- `error.recorded`
- `recovery.started`
- `recovery.retry_scheduled`
- `recovery.compaction_attempted`
- `recovery.alternate_adapter_selected`
- `recovery.user_input_requested`
- `recovery.permission_denial_recorded`
- `recovery.partial_result_preserved`
- `recovery.succeeded`
- `recovery.failed`
- `run.failed_closed`

This keeps the transcript honest when the runtime discards a partial stream,
switches providers, truncates output, or asks the user to intervene.

## User-Safe Reporting

Every error should have two projections:

- Private diagnostic projection for local logs and operator debugging.
- Public-safe projection for transcript, issue comments, receipts, and remote
  views.

Public-safe errors should avoid raw paths, stack traces, credentials, provider
payloads, and long tool output. They should include stable refs to artifacts
when the user has authority to inspect them.

## Safety Rules

- Fail closed on unknown permission, boundary, or storage-integrity errors.
- Never retry a mutation automatically unless it is idempotent or guarded by an
  idempotency key.
- Do not compact away tool-use/result consistency to recover context.
- Do not hide repeated provider failures behind endless retries.
- Denials are normal outcomes, not exceptional crashes.
- Cancellation should be distinct from failure.
- A recovery attempt should not overwrite the original error.
- Public closeouts must headline any shortfall or unverified state.

## Tests

Minimum regression coverage:

- Convert unknown thrown values into typed errors.
- Preserve category, retryability, and redaction metadata.
- Retry a transient network fixture with bounded schedule.
- Do not retry non-idempotent mutation after timeout.
- Compact on context-too-large and record the recovery event.
- Return structured tool validation error to the model.
- Stop on storage corruption and fail closed.
- Distinguish cancellation, denial, timeout, and failure.
- Redact private diagnostic details in public projection.
- Replay error and recovery events into the same final run state.

## OpenAgents Translation Notes

When promoted, map this taxonomy to OpenAgents policy refs, receipt language,
operator diagnostics, projection freshness, and issue-linked failure receipts.
Verify live issue state before making implementation-status claims.

## Decision

Failures should be typed domain events with recovery policy, not opaque thrown
exceptions. The runtime should preserve the original failure, record every
recovery attempt, and expose only public-safe summaries outside local/private
diagnostics.
