# 2026-02-21 Legacy Infra Decommission Phase C (Hold / Rollback State)

Issue: OA-RUST-111 (`#1936`)  
Scope: production-last enforcement, staging-first Rust validation, and rollback parity  
Timestamp (UTC): 2026-02-21

## Decision

Phase C destructive teardown is **deferred**.

Current enforced state is:

1. Production `openagents.com` remains on Laravel traffic.
2. Rust cutover work proceeds on `staging.openagents.com`.
3. Legacy production jobs remain available for rollback parity while production is Laravel-backed.

## Inventory artifacts

Snapshot directory:

- `docs/reports/legacy-infra/20260221T232536Z-phase-c-hold/`

Included evidence:

- `services.after.json`
- `jobs.after.json`
- `domain-mappings.after.json`
- `artifact-repos.after.json`
- `secrets.after.json`
- `openagents-web.after.yaml`
- `openagents-web-staging.after.yaml`
- `openagents-migrate.after.yaml`
- `openagents-maintenance-down.after.yaml`
- `openagents.com.headers.txt`
- `openagents.com.body.html`
- `staging.openagents.com.headers.txt`
- `staging.openagents.com.body.html`

## Actions executed for hold state

1. Confirmed production traffic rollback/pin:
   - `openagents-web` traffic -> `openagents-web-00097-jr6` at 100%.
2. Confirmed staging Rust lane:
   - `staging.openagents.com` mapped to `openagents-web-staging` and serving Rust web shell (`<title>OpenAgents</title>`).
3. Restored legacy Laravel production jobs removed in earlier rehearsal:
   - `openagents-migrate`
   - `openagents-maintenance-down`

## Verification

1. Endpoint checks:
   - `https://openagents.com/` -> `200`, Laravel page (`x-powered-by: PHP/8.4.18`, `<title inertia>Laravel</title>`).
   - `https://staging.openagents.com/` -> `200`, Rust shell page (`<title>OpenAgents</title>`).
2. Staging control smoke:
   - `OPENAGENTS_BASE_URL=https://staging.openagents.com apps/openagents.com/service/deploy/smoke-control.sh` -> pass.
3. Job inventory after restoration includes:
   - `openagents-migrate`
   - `openagents-maintenance-down`
   - `openagents-runtime-migrate`
4. Domain mapping status:
   - `openagents.com -> openagents-web`
   - `staging.openagents.com -> openagents-web-staging`
   - all status `True`.

## Remaining work (explicitly deferred)

1. Final Phase C destructive deletion of legacy production resources.
2. Removal of legacy Artifact Registry repo/images (`openagents-web`).
3. Removal of legacy `openagents-web-*` secrets.

These remain blocked until explicit final cutover approval.
