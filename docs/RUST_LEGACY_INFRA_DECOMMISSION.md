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

## Current disposition map (2026-02-21)

| Resource | Class | Decision | Target phase | Notes |
| --- | --- | --- | --- | --- |
| `openagents-web-staging` (Cloud Run service) | legacy staging | keep temporarily | Phase B/C | Still backs `staging.openagents.com` until Rust staging service is live and mapped. |
| `openagents-migrate-staging` (Cloud Run job) | legacy staging | remove now | Phase A | Deleted in OA-RUST-111 Phase A execution. |
| `openagents-web` (Cloud Run service) | legacy production | keep | Phase C | Serves `openagents.com` and `next.openagents.com`; delete last. |
| `openagents-migrate` (Cloud Run job) | legacy production | keep | Phase C | Required by current production Laravel service until cutover complete. |
| `openagents-maintenance-down` (Cloud Run job) | legacy production | keep temporarily | Phase B/C | Replaced by Rust maintenance mode in OA-RUST-112, then retire. |
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
