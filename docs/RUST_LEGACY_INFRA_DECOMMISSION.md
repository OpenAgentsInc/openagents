# Rust Legacy Infra Decommission Plan

Status: active (Phase C destructive teardown deferred)  
Owner: `owner:infra`  
Issue: OA-RUST-111 (`#1936`)

## Goal

Remove legacy Laravel infrastructure in controlled phases while preserving production availability. Production `openagents.com` Laravel resources are deleted last.

## Non-negotiable gates

1. Never delete production `openagents-web` service or `openagents.com` domain mapping before approved final cutover.
2. Run the shared Rust validation matrix before any Phase C production deletion:
   - `scripts/release/validate-rust-cutover.sh`
   - `docs/RUST_STAGING_PROD_VALIDATION.md`
3. Capture before/after inventory artifacts for each decommission action.
4. Capture fresh Laravel database backups and complete data-port verification before any Phase C deletion:
   - `scripts/release/backup-laravel-db.sh`
   - verify target Rust runtime/control stores can ingest required legacy data before destructive actions.

## Phases

1. Phase A: safe staging removals
- Remove legacy staging jobs/resources that are clearly unused.
- Validate no regressions on current public/staging endpoints.

2. Phase B: freeze legacy lanes
- Disable deploy entrypoints for legacy production resources.
- Keep rollback metadata for final window.

3. Phase C: final cutover + production deletion
- Activate maintenance mode (OA-RUST-112 flow).
- Re-run Rust matrix in strict mode (`FAIL_ON_REQUIRED_FAILURE=1`).
- Re-run Laravel DB backup script and record backup URIs in release evidence.
- Execute/verify Laravel-to-Rust data port checks.
- Remove production Laravel resources last.

## Phase execution status

1. Phase A complete (2026-02-21)
- Report: `docs/reports/2026-02-21-legacy-infra-decommission-phase-a.md`
- Action: deleted `openagents-migrate-staging`.

2. Phase B complete (2026-02-21)
- Report: `docs/reports/2026-02-21-legacy-infra-decommission-phase-b.md`
- Completed in this phase:
  - legacy Laravel deploy entrypoints frozen with explicit unfreeze gates.
  - rollback metadata snapshot captured under `docs/reports/legacy-infra/20260221T195258Z-phase-b/`.
  - keep production traffic on Laravel revision until explicit final cutover approval.

3. Pre-Phase C DB backup evidence captured (2026-02-21)
- Backups created in `gs://openagentsgemini_cloudbuild/backups/laravel/`:
  - `openagents_web-20260221T212513Z.sql.gz`
  - `openagents_web-20260221T214135Z.sql.gz`
  - `openagents_web_staging-20260221T212513Z.sql.gz`
  - `openagents_web_staging-20260221T213651Z.sql.gz`
- Artifact metadata recorded in:
  - `docs/reports/2026-02-21-laravel-db-backup-pre-decom.md`

4. Phase C production-hold update (2026-02-21)
- Report: `docs/reports/2026-02-21-legacy-infra-decommission-phase-c.md`
- Current enforced state:
  - `openagents.com` production traffic pinned to Laravel revision `openagents-web-00097-jr6`.
  - `staging.openagents.com` mapped to Rust staging lane (`openagents-web-staging`).
  - legacy production jobs `openagents-migrate` and `openagents-maintenance-down` restored and retained.
  - destructive legacy teardown remains blocked pending explicit final approval.

## Current disposition map (2026-02-21)

| Resource | Class | Decision | Target phase | Notes |
| --- | --- | --- | --- | --- |
| `openagents-web-staging` (Cloud Run service) | staging lane | keep | Phase C gate | Currently serving Rust staging cutover validation. |
| `openagents-migrate-staging` (Cloud Run job) | legacy staging | remove now | Phase A | Deleted in OA-RUST-111 Phase A execution. |
| `openagents-web` (Cloud Run service) | production lane (legacy service name) | keep | Phase C | `openagents.com` traffic currently pinned to Laravel revision `openagents-web-00097-jr6`; delete/migrate only in approved final window. |
| `openagents-migrate` (Cloud Run job) | legacy production | keep | Phase C | Restored and retained while production remains Laravel-backed. |
| `openagents-maintenance-down` (Cloud Run job) | legacy production | keep | Phase C | Restored and retained for rollback parity while production remains Laravel-backed. |
| `openagents-runtime-migrate` (Cloud Run job) | active runtime migration | keep | ongoing | Active runtime migrate job; not a Laravel teardown target in this phase. |
| `openagents-web` (Artifact Registry repo) | legacy production images | keep | Phase C | Remove after production service deletion and rollback window closes. |
| `openagents-web-*` secrets | legacy production/staging secrets | keep | Phase C | Remove only after service/job deletion and env migration complete. |
| `openagents.com -> openagents-web` domain mapping | production route | keep | Phase C | Keep mapping stable; move traffic only during approved final cutover. |

## Inventory commands

```bash
gcloud run services list --platform=managed --region=us-central1 --project=openagentsgemini
gcloud run jobs list --region=us-central1 --project=openagentsgemini
gcloud artifacts repositories list --location=us-central1 --project=openagentsgemini
gcloud secrets list --project=openagentsgemini
gcloud beta run domain-mappings list --platform=managed --region=us-central1 --project=openagentsgemini
```

Store outputs under timestamped `docs/reports/legacy-infra/<timestamp>/` directories.
