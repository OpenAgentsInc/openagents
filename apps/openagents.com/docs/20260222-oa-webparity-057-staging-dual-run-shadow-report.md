# OA-WEBPARITY-057 Staging Dual-Run and Shadow Diff Report

Date: 2026-02-22
Status: pass (harness + local rehearsal; live staging run deferred by environment availability)
Issue: OA-WEBPARITY-057

## Deliverables

Implemented staging dual-run/shadow diff harness:
- `apps/openagents.com/service/scripts/run-staging-dual-run-shadow-diff.sh`

Request manifest:
- `apps/openagents.com/docs/parity-manifests/staging-dual-run-requests.json`

Runbook:
- `apps/openagents.com/service/docs/STAGING_DUAL_RUN_SHADOW_DIFF.md`

## Local Rehearsal (Executed)

Rehearsal run used Rust service on both sides to validate harness behavior and report output shape.

Command:

```bash
AUTH_TOKEN='<mock-token>' \
RUST_BASE_URL='http://127.0.0.1:8788' \
LEGACY_BASE_URL='http://127.0.0.1:8788' \
apps/openagents.com/service/scripts/run-staging-dual-run-shadow-diff.sh
```

Result artifact:
- `apps/openagents.com/storage/app/staging-dual-run/20260222T155446Z/summary.json`
- `apps/openagents.com/storage/app/staging-dual-run/20260222T155446Z/SUMMARY.md`

Rehearsal summary:
- overall_status: `passed`
- request_count: `7`
- passed: `7`
- failed: `0`
- skipped: `0`
- critical_failed: `0`

## Staging Environment Check (Current)

Observed on 2026-02-22:
- `https://openagents-control-service-staging-ezxz4mgdsq-uc.a.run.app/healthz` returned `404`
- `https://staging.openagents.com/healthz` returned `404`

Because both staging entrypoints currently return 404, live staging dual-run diff execution is blocked until staging URLs are restored/confirmed.

## Staging Execution Command (Ready)

```bash
RUST_BASE_URL='https://<rust-staging-host>' \
LEGACY_BASE_URL='https://<legacy-staging-host>' \
AUTH_TOKEN='<staging-bearer-token>' \
apps/openagents.com/service/scripts/run-staging-dual-run-shadow-diff.sh
```

## Mismatch Resolution Policy

- `critical` request mismatches fail the run (non-zero exit).
- non-critical mismatches are reported as warnings with unified diffs.
- auth-required requests are marked `skipped` when no token is provided.
