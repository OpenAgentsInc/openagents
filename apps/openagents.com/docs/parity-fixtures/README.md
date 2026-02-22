# Parity Contract Capture Fixtures

This directory stores machine-readable baseline fixtures used for Laravel -> Rust parity conformance checks.

## Capture Harness

Run from repo root:

```bash
./apps/openagents.com/scripts/capture-parity-contract-fixtures.sh
```

Equivalent Artisan command:

```bash
cd apps/openagents.com
php artisan ops:capture-parity-contract-fixtures --output=docs/parity-fixtures/baseline
```

Deterministic shared state seeding (Laravel + Rust):

```bash
./apps/openagents.com/scripts/seed-parity-fixtures.sh
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
