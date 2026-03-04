# Security and Privacy Runbook

## Scope

This runbook defines enforceable controls for the email lane:
- Retention windows per data category
- Deletion workflow and receipts
- Export controls with role-based scope gating
- Access auditing for all export/delete operations
- Redaction standards for logs and debug traces

Implementation owner: `crates/email-agent/src/security_privacy.rs`

## Retention Controls

Retention is enforced by `enforce_retention_policy`.

Default windows:
- Inbound messages: 30 days
- Drafts: 30 days
- Send audit records: 180 days
- Follow-up events: 90 days
- Knowledge documents: 365 days

Retention sweeps return deterministic deletion outcomes and remaining counts.

## Deletion Workflow

Deletion requests run through `run_deletion_workflow` with:
- actor identity
- actor role
- reason
- target record IDs

Outputs:
- deterministic deletion receipt ID
- deleted IDs
- missing IDs

Every deletion operation writes an access audit event.

## Export Controls

Exports run through `export_records` and are role-gated:
- `MetadataOnly`: allowed for operator/auditor/automation roles
- `FullContent`: allowed for auditor role only

Non-auditor full-content export requests are denied and auditable.

## Access Auditing

All export/delete requests append to access audit log with:
- actor ID
- role
- action
- timestamp
- allowed/denied outcome details

## Redaction Policy

Redaction applies to metadata keys and debug trace text.

Sensitive metadata key patterns:
- token
- secret
- password
- mnemonic
- authorization
- api_key

Debug trace redaction masks:
- email-like tokens
- token/secret/password/mnemonic markers
- key-like prefixes (e.g. `sk-...`)

## Validation

Current test coverage in `security_privacy` module validates:
- retention enforcement
- deletion receipts + access audit
- export gating and redacted output
- debug trace redaction behavior
