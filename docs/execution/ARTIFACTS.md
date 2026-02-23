# Execution Artifacts (Verified Patch Bundle)

Canonical bundle artifacts:

- `PR_SUMMARY.md` (human-readable)
- `RECEIPT.json` (machine-verifiable)
- `REPLAY.jsonl` (event log; see `docs/execution/REPLAY.md`)

## Bundle Layout

```text
{session_dir}/
  PR_SUMMARY.md
  RECEIPT.json
  REPLAY.jsonl
```

Filenames are stable and must not change.

## `PR_SUMMARY.md`

Recommended sections:

- summary
- key files changed
- verification commands/results
- known risks/rollback notes

## `RECEIPT.json`

Required top-level fields:

- `schema` (`openagents.receipt.v1`)
- `session_id`
- `trajectory_hash`
- `policy_bundle_id`
- `canonical_json_sha256` (sha256 hex over canonical receipt payload; excludes signature fields)

Optional/recommended:

- `created_at`
- `signature` (deterministic signature over `canonical_json_sha256` when a receipt signer is configured)
- repo metadata
- `metrics` (summary values derived from receipts/replay)
- `tool_calls[]`
- `verification[]`
- `payments[]`

### Tool call receipt

Required:

- `tool`
- `params_hash`
- `output_hash`
- `latency_ms`
- `side_effects[]`

Optional:

- `ok`
- `error`

### Verification receipt

Required:

- `command`
- `exit_code`

Optional:

- `cwd`
- `duration_ms`
- `verification_delta`

### Payment receipt

Use protocol-level receipt semantics in `docs/protocol/PROTOCOL_SURFACE.md`.

Runtime-produced payment receipts are embedded in `RECEIPT.json` under `payments[]` and include:

- `schema` (`openagents.treasury.payment_receipt.v1`)
- `canonical_json_sha256` (sha256 hex over canonical payment receipt payload; excludes signature fields)
- `signature` (optional; deterministic signature over `canonical_json_sha256` when a receipt signer is configured)

## Forward Compatibility

Additive evolution is allowed.
Breaking changes require schema/version update and ADR review.
