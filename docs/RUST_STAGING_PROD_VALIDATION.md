# Rust Staging/Prod Validation Matrix

Status: active
Owner: `owner:infra`, `owner:openagents.com`, `owner:runtime`
Issue: OA-RUST-110 (`#1935`)

## Purpose

Define one deterministic matrix that gates Rust cutover readiness for staging and production.

## Canonical Entrypoint

```bash
scripts/release/validate-rust-cutover.sh
```

Validation evidence should be archived in backroom (not committed into `docs/`).

## Matrix Lanes

1. Control service lane
- health/readiness checks
- static host policy checks
- auth/session/sync token checks (when auth token is provided)

2. Runtime lane
- runtime health + authority API smoke
- migration drift check (runtime image matches migrate job image)

3. Khala contract lane

```bash
cargo test --manifest-path apps/runtime/Cargo.toml server::tests::khala_topic_messages -- --nocapture
```

4. Surface parity lane (optional)

```bash
scripts/run-cross-surface-contract-harness.sh
```

5. Observability + rollback readiness
- logging probes
- canary/rollback status checks

## Required Environment

Defaults:

- `STAGING_CONTROL_BASE_URL=https://staging.openagents.com`
- `PROD_CONTROL_BASE_URL=https://openagents.com`
- `GCP_PROJECT=openagentsgemini`
- `GCP_REGION=us-central1`
- `CONTROL_SERVICE=openagents-control-service`
- `RUNTIME_SERVICE=runtime`
- `MIGRATE_JOB=runtime-migrate`

Optional authenticated smoke tokens:

- `STAGING_CONTROL_ACCESS_TOKEN`
- `PROD_CONTROL_ACCESS_TOKEN`

## Go/No-Go Policy

1. All required checks must pass.
2. Required checks cannot be skipped in release gating mode.
3. Any required failure is a no-go until fixed and rerun.
4. Legacy deletion phases require fresh Laravel DB backup evidence in addition to this matrix.

## Related Runbooks

- `apps/openagents.com/service/docs/CANARY_ROLLBACK_RUNBOOK.md`
- `apps/runtime/docs/DEPLOY_CLOUD_RUN.md`
- `docs/SCHEMA_EVOLUTION_PLAYBOOK.md`
