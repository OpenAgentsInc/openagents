# OA-WEBPARITY-067 Mixed-Version Deploy Safety, Rollback, and Backfill Invariants

Date: 2026-02-22  
Status: pass (mixed-version deploy safety harness + runbook)  
Issue: OA-WEBPARITY-067

## Deliverables

- Mixed-version deploy safety harness:
  - `apps/openagents.com/scripts/run-mixed-version-deploy-safety-harness.sh`
- Manual workflow dispatch:
  - `.github/workflows/web-mixed-version-deploy-safety-harness.yml`
- Mixed-version deploy and rollback runbook:
  - `apps/openagents.com/service/docs/MIXED_VERSION_DEPLOY_SAFETY.md`

## What This Locks

1. Expand/migrate/contract ordering is explicit and required.
2. Backfill manifests are mandatory and verified using checksum parity.
3. Count invariants are asserted before cutover for each migrated store.
4. Rollback is validated by restoring pre-migration hashes from backup manifests.
5. Route rollback and store rollback are treated as a single operational gate.

## Verification Executed

```bash
bash -n apps/openagents.com/scripts/run-mixed-version-deploy-safety-harness.sh
./apps/openagents.com/scripts/run-mixed-version-deploy-safety-harness.sh
```

Artifact produced:
- `apps/openagents.com/storage/app/mixed-version-deploy-safety/<timestamp>/summary.json`
