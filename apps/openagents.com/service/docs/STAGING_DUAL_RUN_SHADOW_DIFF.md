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
- `MAX_CRITICAL_MISMATCHES` (default `0`)
- `MAX_TOTAL_MISMATCHES` (default `0`)
- `MAX_STREAM_MISMATCHES` (default `0`)
- `MAX_P95_LATENCY_DELTA_MS` (default `250`)

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
- normalized SSE semantic projections for stream requests:
  - event order
  - tool event sequence
  - finish/error event sets

Default output directory:
- `apps/openagents.com/storage/app/staging-dual-run/<timestamp>/`

## Status Semantics

- `pass`: status code and normalized response payload match.
- `fail`: mismatch detected (`status_mismatch` or `body_mismatch`).
- `skipped`: request required auth and `AUTH_TOKEN` was not supplied.

Go/no-go gates:
- critical mismatches must be `<= MAX_CRITICAL_MISMATCHES`
- total mismatches must be `<= MAX_TOTAL_MISMATCHES`
- SSE stream mismatches must be `<= MAX_STREAM_MISMATCHES`
- Rust-vs-legacy p95 latency delta must be `<= MAX_P95_LATENCY_DELTA_MS`

The script exits non-zero when any gate threshold is exceeded.
