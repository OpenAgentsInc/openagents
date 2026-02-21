# Rust Legacy Infra Decommission Plan

Status: active  
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
- Remove production Laravel resources last.

## Phase execution status

1. Phase A complete (2026-02-21)
- Report: `docs/reports/2026-02-21-legacy-infra-decommission-phase-a.md`
- Action: deleted `openagents-migrate-staging`.

2. Phase B in progress (2026-02-21)
- Report: `docs/reports/2026-02-21-legacy-infra-decommission-phase-b.md`
- Completed in this phase:
  - legacy Laravel deploy entrypoints frozen with explicit unfreeze gates.
  - rollback metadata snapshot captured under `docs/reports/legacy-infra/20260221T195258Z-phase-b/`.
- Remaining before Phase C:
  - keep production traffic on `openagents-web` until OA-RUST-112 maintenance-mode cutover window opens.

## Current disposition map (2026-02-21)

| Resource | Class | Decision | Target phase | Notes |
| --- | --- | --- | --- | --- |
| `openagents-web-staging` (Cloud Run service) | legacy staging | keep temporarily | Phase B/C | Still backs `staging.openagents.com` until Rust staging service is live and mapped. |
| `openagents-migrate-staging` (Cloud Run job) | legacy staging | remove now | Phase A | Deleted in OA-RUST-111 Phase A execution. |
| `openagents-web` (Cloud Run service) | legacy production | keep | Phase C | Serves `openagents.com` and `next.openagents.com`; delete last. Phase B rollback pointer captured. |
| `openagents-migrate` (Cloud Run job) | legacy production | keep | Phase C | Required by current production Laravel service until cutover complete. |
| `openagents-maintenance-down` (Cloud Run job) | legacy production | keep temporarily | Phase B/C | Replaced by Rust maintenance mode in OA-RUST-112, then retire. Phase B rollback pointer captured. |
| `openagents-runtime-migrate` (Cloud Run job, legacy command shape) | legacy runtime migration | keep temporarily | Phase B/C | Replace with Rust runtime migrate job, then retire legacy job. |
| `openagents-web` (Artifact Registry repo) | legacy production images | keep | Phase C | Remove after production service deletion and rollback window closes. |
| `openagents-web-*` secrets | legacy production/staging secrets | keep | Phase C | Remove only after service/job deletion and env migration complete. |
| `openagents.com -> openagents-web` domain mapping | legacy production route | keep | Phase C | Switch to Rust service during final cutover only. |

## Inventory commands

```bash
gcloud run services list --platform=managed --region=us-central1 --project=openagentsgemini
gcloud run jobs list --region=us-central1 --project=openagentsgemini
gcloud artifacts repositories list --location=us-central1 --project=openagentsgemini
gcloud secrets list --project=openagentsgemini
gcloud beta run domain-mappings list --platform=managed --region=us-central1 --project=openagentsgemini
```

Store outputs under timestamped `docs/reports/legacy-infra/<timestamp>/` directories.
