# Parity Contract Capture Fixtures

This directory stores machine-readable baseline fixtures used for Laravel -> Rust parity conformance checks.

## Capture Harness

Run from repo root:

```bash
./apps/openagents.com/scripts/archived-laravel/capture-parity-contract-fixtures.sh
```

Equivalent Artisan command:

```bash
cd apps/openagents.com
php artisan ops:capture-parity-contract-fixtures --output=docs/parity-fixtures/baseline
```

Deterministic shared state seeding (Laravel + Rust):

```bash
./apps/openagents.com/scripts/archived-laravel/seed-parity-fixtures.sh
```

Vercel SSE compatibility fixture drift harness:

```bash
./apps/openagents.com/scripts/run-vercel-sse-fixture-harness.sh
```

## Artifacts

- `baseline/http-json-golden.json`
  - OpenAPI-derived request/response fixture catalog for Laravel HTTP routes.
- `baseline/khala-ws-golden.json`
  - Khala WS frame + replay/live transcript fixtures for Codex worker event conformance.
- `baseline/capture-index.json`
  - Summary index with generated-at timestamp and fixture counts.
- `baseline/shared-seed-state.json`
  - Deterministic shared seed state used by:
    - Laravel baseline seed command: `php artisan ops:seed-parity-fixtures --fixture=docs/parity-fixtures/baseline/shared-seed-state.json --replace`
    - Rust store seed command: `apps/openagents.com/service/scripts/seed-parity-fixtures.sh`

These artifacts are inputs to Rust conformance checks and must be refreshed when Laravel contract behavior changes.

## Vercel SSE Compatibility Fixture Corpus

Directory:
- `apps/openagents.com/docs/parity-fixtures/vercel-sse-compat-v1/`

Artifacts:
- `codex-event-scenarios.json`
  - Normalized codex-event scenario inputs for compatibility mapping replay.
- `golden-sse-transcripts.json`
  - Golden Vercel-compatible transcript outputs (event objects + SSE wire payload).
- `golden-error-fixtures.json`
  - Pre-stream JSON and in-stream terminal error fixtures.
- `fixture-index.json`
  - Source lineage references (legacy Laravel streaming shape tests + codex event fixtures + parity manifest baseline).

Drift policy:
- The harness regenerates transcripts from scenario inputs using `apps/openagents.com/scripts/vercel-sse-fixture-map.jq` and fails on any diff versus `golden-sse-transcripts.json`.
