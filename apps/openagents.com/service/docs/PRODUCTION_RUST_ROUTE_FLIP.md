# Production Rust-Only Route Flip

Tracking: `OA-WEBPARITY-059`

Automation script:
- `apps/openagents.com/service/scripts/run-production-rust-route-flip.sh`

## Purpose

Flip and verify production route split posture so all web/API route probes resolve to Rust authority, with explicit write-path redirect checks.

## Inputs

Required:
- `CONTROL_ACCESS_TOKEN` (admin token for control endpoints)

Optional:
- `BASE_URL` (default: `https://openagents.com`)
- `APPLY` (`0` verify-only, `1` mutate control-plane overrides)
- `COHORT_KEY` (default: `prod-rust-route-flip`)
- `OUTPUT_DIR` (default: `apps/openagents.com/storage/app/production-route-flip/<timestamp>/`)
- `CURL_TIMEOUT_SECONDS` (default: `15`)

## Verify-Only Rehearsal (No Mutations)

```bash
BASE_URL=https://openagents.com \
APPLY=0 \
CONTROL_ACCESS_TOKEN=<admin-token> \
apps/openagents.com/service/scripts/run-production-rust-route-flip.sh
```

## Live Flip + Verification

```bash
BASE_URL=https://openagents.com \
APPLY=1 \
CONTROL_ACCESS_TOKEN=<admin-token> \
apps/openagents.com/service/scripts/run-production-rust-route-flip.sh
```

## What It Enforces

1. Captures pre/post route-split status snapshots.
2. When `APPLY=1`, clears global override, sets each route domain override to `rust`, then sets global target `rust`.
3. Evaluates key web and API paths through `/api/v1/control/route-split/evaluate` and requires `target=rust_shell`.
4. Probes API write paths and fails if any request redirects.

## Artifacts

- `summary.json`
- `SUMMARY.md`
- per-check response artifacts

All artifacts are emitted under:
- `apps/openagents.com/storage/app/production-route-flip/<timestamp>/`
