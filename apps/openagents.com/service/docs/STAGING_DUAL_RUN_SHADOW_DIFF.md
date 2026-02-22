# Staging Dual-Run Shadow Diff Harness

This harness compares Rust and legacy web/API responses for the same request set and emits a structured diff report.

Script:
- `apps/openagents.com/service/scripts/run-staging-dual-run-shadow-diff.sh`

Request manifest:
- `apps/openagents.com/docs/parity-manifests/staging-dual-run-requests.json`

## Required Inputs

- `RUST_BASE_URL` (for example Rust staging service URL)
- `LEGACY_BASE_URL` (for example legacy Laravel staging URL)

Optional:
- `AUTH_TOKEN` (Bearer token for auth-required request rows)
- `REQUESTS_FILE` (override request manifest path)
- `OUTPUT_DIR` (override output directory)

## Run

```bash
RUST_BASE_URL='https://rust-staging.example.com' \
LEGACY_BASE_URL='https://legacy-staging.example.com' \
AUTH_TOKEN='<token>' \
apps/openagents.com/service/scripts/run-staging-dual-run-shadow-diff.sh
```

## Output

The harness writes:
- `summary.json`
- `SUMMARY.md`
- request-level response captures and normalized payloads
- unified diff files for mismatches

Default output directory:
- `apps/openagents.com/storage/app/staging-dual-run/<timestamp>/`

## Status Semantics

- `pass`: status code and normalized response payload match.
- `fail`: mismatch detected (`status_mismatch` or `body_mismatch`).
- `skipped`: request required auth and `AUTH_TOKEN` was not supplied.

The script exits non-zero only for critical mismatches.
