# 2026-02-21 Legacy Infra Decommission Phase A

Issue: OA-RUST-111 (`#1936`)  
Scope: safe staging legacy removal only  
Timestamp (UTC): 2026-02-21T19:35:00Z

## Inventory artifacts

Directory:

- `docs/reports/legacy-infra/20260221T193500Z/`

Files:

- `services.before.json`
- `services.after.json`
- `jobs.before.json`
- `jobs.after.json`
- `artifact-repos.before.json`
- `artifact-repos.after.json`
- `secrets.before.json`
- `secrets.after.json`
- `domain-mappings.before.json`
- `domain-mappings.after.json`
- `smoke.after.json`

## Phase A action executed

Deleted one clearly-unused staging legacy job:

```bash
gcloud run jobs delete openagents-migrate-staging \
  --region=us-central1 \
  --project=openagentsgemini \
  --quiet
```

Result: `Deleted job [openagents-migrate-staging].`

## Before/after deltas

Cloud Run jobs before:

- `openagents-maintenance-down`
- `openagents-migrate`
- `openagents-migrate-staging`
- `openagents-runtime-migrate`

Cloud Run jobs after:

- `openagents-maintenance-down`
- `openagents-migrate`
- `openagents-runtime-migrate`

Other inventories unchanged (services, artifact repos, secrets, domain mappings).

## No-regression checks after deletion

HTTP probes:

- `openagents.com` -> `200`
- `staging.openagents.com` -> `200`
- `openagents-web-ezxz4mgdsq-uc.a.run.app` -> `200`
- `openagents-web-staging-ezxz4mgdsq-uc.a.run.app` -> `200`

See `docs/reports/legacy-infra/20260221T193500Z/smoke.after.json`.

## Active deploy-path verification

Search for the deleted staging job name in active docs/scripts produced no active references outside this report artifact set.

Command:

```bash
rg -n "openagents-migrate-staging" apps docs scripts README.md AGENTS.md
```

## Production deletion policy status

Production Laravel resources were intentionally not deleted in this phase.

Kept for final cutover window:

- `openagents-web` service
- `openagents-migrate` job
- `openagents-web` artifact repo
- `openagents-web-*` secrets
- `openagents.com` and `next.openagents.com` domain mappings

Final production deletion remains gated on OA-RUST-112 maintenance-mode cutover and strict OA-RUST-110 matrix success.
