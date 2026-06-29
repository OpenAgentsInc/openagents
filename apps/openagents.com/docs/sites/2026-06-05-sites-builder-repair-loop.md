# Sites Builder Repair Loop

Issue #198 adds the first bounded auto-repair contract for OpenAgents Sites
builder sessions.

## Implemented Contract

OpenAgents product surface now has a `site_builder_repair_attempts` ledger for generated Site build,
preview, validation, and runtime failures.

Each repair attempt records:

- builder session
- optional preview ref
- optional phase kind
- attempt number
- retry budget
- status
- failure kind
- redacted failure summary
- optional stop reason
- safe metadata
- created/completed timestamps

The service rejects attempts that exceed the retry budget, so repair loops have
an explicit stop condition and cannot silently run forever.

## Event Timeline

Recording a repair attempt also appends a customer-visible builder event:

- `succeeded` becomes `build_repaired`
- `failed` or `blocked` becomes `build_failed`
- queued/running/skipped repair states become `phase_updated`

That keeps repair progress visible through the existing builder session SSE
stream and phase/event timeline.

## Redaction Rules

Repair summaries and stop reasons are bounded and redacted before storage.
The service rejects private runner payloads, provider account material, access
tokens, wallet/payment secrets, Lightning invoice/preimage-shaped text, bypass
instructions, and similar unsafe material.

Customer-visible events contain only a redacted summary and bounded attempt
metadata. They do not include raw logs, environment variables, provider
payloads, source archives, or secret material.

## Current Limits

This is the repair ledger and contract. It does not yet execute an LLM repair
run or patch files. Later preview/build runners should call this service when
they hit a redacted build/runtime failure and should stop when the retry budget
is exhausted.

## Follow-Up Work

- connect preview/build runner failures to repair attempts
- feed repair attempts into a bounded agent patch loop
- link repaired file snapshots and subsequent preview attempts
- expose repair status in the self-serve builder UI
