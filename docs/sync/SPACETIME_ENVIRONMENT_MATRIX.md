# Spacetime Environment Matrix (Dev / Staging / Prod)

Date: 2026-02-25
Status: Active
Owner lanes: Infra, Runtime, Control

## Purpose

Define required environment-level Spacetime configuration and health checks for deployment readiness.

## Required Environment Variables

For each environment (`DEV`, `STAGING`, `PROD`) the following keys are required:

1. `OA_SPACETIME_<ENV>_HTTP_BASE_URL`
2. `OA_SPACETIME_<ENV>_DATABASE`
3. `OA_SPACETIME_<ENV>_TOKEN_ISSUER`
4. `OA_SPACETIME_<ENV>_TOKEN_AUDIENCE`
5. `OA_SPACETIME_<ENV>_JWT_SIGNING_KEY`

Optional:

1. `OA_SPACETIME_<ENV>_WEBSOCKET_PATH` (default: `/v1/database/{database}/subscribe`)
2. `OA_SPACETIME_<ENV>_HEALTH_PATH` (default: `/health`)
3. `RUNTIME_SYNC_TOKEN_FALLBACK_SIGNING_KEYS` (comma-separated previous HS256 keys accepted during key rotation windows)
4. `RUNTIME_SYNC_TOKEN_CLOCK_SKEW_SECONDS` (JWT leeway for `nbf`/`exp`; default `30`)

Control token lease expectations:

1. Spacetime token mint responses include `refresh_after_in` and `refresh_after`.
2. Desktop/runtime clients should remint before `refresh_after` and retry with a fresh token on auth failures.

## Environment Targets

### Dev

1. Fast iteration and broad compatibility windows.
2. Secrets can use short rotation intervals and local-dev overrides.
3. Health probes required before enabling local dual-lane harness.

### Staging

1. Production-like config and claims policy.
2. Mandatory canary gates and rollback rehearsal before prod promotion.
3. No manual secret injection outside runbook process.

### Prod

1. Tight compatibility/support windows and audited secret rotation.
2. Rollout by cohort only with alerting gates active.
3. Khala retirement changes blocked unless Spacetime SLO evidence is green.

## Provisioning Validation Command

Use:

```bash
scripts/spacetime/provision-check.sh dev
scripts/spacetime/provision-check.sh staging
scripts/spacetime/provision-check.sh prod
```

This command validates:

1. required env var presence,
2. base URL format,
3. health endpoint reachability.
