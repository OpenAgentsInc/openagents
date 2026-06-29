# Sites Builder Phase Timeline

Issue #197 adds the first durable phase timeline for OpenAgents Sites builder
sessions.

## Implemented Contract

Builder sessions can now record phase runs for:

- planning
- foundation
- core
- styling
- integration
- optimization
- preview
- save
- deploy

Each phase run stores:

- phase kind
- status: queued, running, succeeded, failed, blocked, or skipped
- sequence
- title
- summary
- optional started/completed timestamps
- safe metadata

Recording a phase run also appends a matching customer-visible event into the
existing `site_builder_events` ledger:

- `running` becomes `phase_started`
- `succeeded` and `skipped` become `phase_completed`
- queued, failed, blocked, and other updates become `phase_updated`

That means the existing builder session SSE endpoint can replay phase progress
without adding a separate streaming surface.

## Projection Rules

Customer projections expose:

- `currentPhase`
- `phases`

Those projections intentionally omit raw timestamps. They contain only phase
kind, status, sequence, title, and summary so UI can render friendly display
copy without leaking runner details.

Operator projections expose:

- `phaseCount`
- `phaseCurrent`

Raw phase row timestamps remain in D1 storage for audit and later operator
views.

## Current Limits

This slice records durable phase runs and links them to the event stream. It
does not yet add a public UI timeline component or a phase-specific write API.
Those should build on the repository helper and existing event stream.

## Follow-Up Work

- surface the phase timeline in the self-serve builder UI
- add operator-safe phase details where needed
- link preview, save, deploy, and repair services to phase runs directly
- convert raw timestamps to friendly display text in UI components
