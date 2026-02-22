# Rust Legacy Infra Decommission Plan

Status: active (production destructive teardown deferred)
Owner: `owner:infra`
Issue: OA-RUST-111 (`#1936`)

## Goal

Remove legacy Laravel-era infrastructure in controlled phases without breaking production.

## Non-Negotiable Gates

1. Do not delete production `openagents-web` or production domain mappings before final cutover approval.
2. Run the Rust validation matrix before any destructive phase:
   - `scripts/release/validate-rust-cutover.sh`
   - `docs/RUST_STAGING_PROD_VALIDATION.md`
3. Capture before/after inventory artifacts for each action.
4. Capture fresh Laravel DB backups before destructive production deletion:
   - `scripts/release/backup-laravel-db.sh`

## Phase Status

1. Phase A complete (staging-safe removals).
2. Phase B complete (legacy deploy lanes frozen with explicit unfreeze gates).
3. Phase C deferred for production until explicit cutover approval.

Current hold-state:

- `openagents.com` production traffic remains pinned to legacy Laravel revision.
- `staging.openagents.com` is the active Rust validation lane.
- Legacy rollback jobs remain available while production is Laravel-backed.

## Resource Disposition (Current)

| Resource | Class | Decision | Notes |
| --- | --- | --- | --- |
| `openagents-web-staging` | staging lane | keep | Rust staging validation service |
| `openagents-migrate-staging` | legacy staging | removed | completed in Phase A |
| `openagents-web` | production legacy lane | keep | remove only during final approved cutover |
| `openagents-migrate` | production rollback lane | keep | keep while production uses legacy lane |
| `openagents-maintenance-down` | production rollback lane | keep | keep while production uses legacy lane |
| `openagents-runtime-migrate` | active runtime migration | keep | not a legacy teardown target |

## Inventory Commands

```bash
gcloud run services list --platform=managed --region=us-central1 --project=openagentsgemini
gcloud run jobs list --region=us-central1 --project=openagentsgemini
gcloud artifacts repositories list --location=us-central1 --project=openagentsgemini
gcloud secrets list --project=openagentsgemini
gcloud beta run domain-mappings list --platform=managed --region=us-central1 --project=openagentsgemini
```

## Evidence Storage

Store operational evidence under backroom archive batches, not the in-repo docs tree.

Current archive batch:

- `/Users/christopherdavid/code/backroom/openagents-doc-archive/2026-02-21-stale-doc-pass-2/`
