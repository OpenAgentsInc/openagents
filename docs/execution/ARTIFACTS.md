# Execution Artifacts (Verified Patch Bundle)

This document is the **canonical** specification for the **Verified Patch Bundle** artifacts:

- `PR_SUMMARY.md` (human-readable)
- `RECEIPT.json` (machine-verifiable)
- `REPLAY.jsonl` (replay log; see `docs/execution/REPLAY.md`)

See:
- `docs/adr/ADR-0002-verified-patch-bundle.md`
- `docs/adr/ADR-0006-deterministic-hashing.md`
- `docs/protocol/PROTOCOL_SURFACE.md` (receipt/payment proof semantics)

## Bundle Layout

Per `docs/adr/ADR-0008-session-storage-layout.md`, a session directory contains:

```text
{session_dir}/
  PR_SUMMARY.md
  RECEIPT.json
  REPLAY.jsonl
```

Bundle filenames are stable and MUST NOT change.

## PR_SUMMARY.md (Format)

`PR_SUMMARY.md` MUST be valid Markdown.

Recommended sections (order is flexible):
- **Summary**: what changed and why
- **Files**: key files touched (paths)
- **Verification**: commands run + results
- **Risks**: known risks / rollback notes
- **Notes**: follow-ups, TODOs (if any), scope boundaries

## RECEIPT.json (Schema)

`RECEIPT.json` is the **session receipt**: a machine-readable attestation of what was executed, with deterministic hashes for audit and replay.

### Top-Level Fields

Required:
- `schema` (string)
  Canonical id: `openagents.receipt.v1`
- `session_id` (string)
- `trajectory_hash` (string)
- `policy_bundle_id` (string)

Optional (recommended when available):
- `created_at` (string, ISO-8601)
- `repo` (object)
  - `remote` (string)
  - `branch` (string)
  - `commit` (string)
- `tool_calls` (array of `ToolCallReceipt`)
- `verification` (array of `VerificationReceipt`)
- `payments` (array of `PaymentReceipt` as defined in `docs/protocol/PROTOCOL_SURFACE.md`)

### ToolCallReceipt

Each tool execution MUST include deterministic hashes and latency.

Required:
- `tool` (string)
- `params_hash` (string)
- `output_hash` (string)
- `latency_ms` (number)
- `side_effects` (array of strings)

Optional:
- `ok` (boolean)
- `error` (object)
  - `name` (string)
  - `message` (string)
  - `stack` (string, optional)

Normative:
- `params_hash` and `output_hash` hashing rules are defined by `docs/adr/ADR-0006-deterministic-hashing.md`.
- Tool params MUST be schema-validated before execution (see `docs/adr/ADR-0007-tool-execution-contract.md`).

### VerificationReceipt

Verification entries describe objective checks run during the session (lint/test/build/smoke).

Required:
- `command` (string)
- `exit_code` (number)

Optional (recommended):
- `cwd` (string)
- `duration_ms` (number)
- `verification_delta` (number)
  Definition in `docs/GLOSSARY.md`.

### PaymentReceipt

Payment receipt entries MUST use protocol-level fields (rail + asset_id + amount_msats + payment_proof) per:
- `docs/protocol/PROTOCOL_SURFACE.md`
- `docs/adr/ADR-0013-receipt-schema-payment-proofs.md`

## Forward Compatibility

Additive changes are allowed:
- new optional top-level fields
- new optional fields in nested records

Breaking changes require a new schema id and/or a superseding ADR.

