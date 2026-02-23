# HTMX Route-Group Rollout and Rollback

Issue: OA-HTMX-062 (`#2073`)

## Purpose

Roll out HTMX behavior by route group, with independent rollback controls and explicit staging canary validation before production changes.

## Route Groups

- `auth_entry`: `/login`, `/register`, `/authenticate`, `/onboarding/*`
- `account_settings_admin`: `/account/*`, `/settings/*`, `/admin/*`
- `billing_l402`: `/billing/*`, `/l402/*`
- `chat_pilot`: `/`, `/chat/*`, `/feed`

## Control API Targets

All controls use `POST /api/v1/control/route-split/override`.

Route-target controls (existing):

- `legacy`
- `rust`
- `rollback`
- `clear`

HTMX mode controls (domain required):

- `htmx_fragment`
- `htmx_full_page`
- `htmx_rollback`
- `htmx_clear`

Examples:

```bash
curl -sS -X POST "${BASE_URL}/api/v1/control/route-split/override" \
  -H "authorization: Bearer ${CONTROL_ACCESS_TOKEN}" \
  -H "content-type: application/json" \
  -d '{"target":"htmx_full_page","domain":"chat_pilot"}'
```

```bash
curl -sS -X POST "${BASE_URL}/api/v1/control/route-split/override" \
  -H "authorization: Bearer ${CONTROL_ACCESS_TOKEN}" \
  -H "content-type: application/json" \
  -d '{"target":"htmx_rollback","domain":"chat_pilot"}'
```

## Staging Canary (Required Before Production)

Run the staging canary script:

```bash
BASE_URL=https://staging.openagents.com \
CONTROL_ACCESS_TOKEN=<admin-token> \
apps/openagents.com/service/scripts/htmx-route-group-canary.sh
```

What it validates:

1. Each route group can flip to `htmx_full_page` independently.
2. Each route group can apply `htmx_rollback` to its rollback mode.
3. Each route group can clear HTMX override (`htmx_clear`).
4. Route-split status reflects expected `htmx_domain_overrides` transitions.

Artifacts are written under:

- `apps/openagents.com/storage/app/htmx-route-group-canary/<timestamp>/`

## Production Rollout Sequence

1. Confirm staging canary passed for all route groups.
2. Apply production HTMX mode updates one route group at a time.
3. After each group change, run:

```bash
BASE_URL=https://openagents.com \
OA_ACCESS_TOKEN=<admin-token> \
REQUIRE_AUTH_FLOWS=1 \
apps/openagents.com/service/scripts/htmx_perf_check.sh
```

4. Run browser smoke:

```bash
BASE_URL=https://openagents.com \
OA_BROWSER_SMOKE_ACCESS_TOKEN=<admin-token> \
OA_BROWSER_SMOKE_REQUIRE_LOGIN_FLOW=0 \
apps/openagents.com/service/scripts/htmx_browser_smoke.sh
```

## Rollback

Immediate route-group rollback:

```bash
curl -sS -X POST "https://openagents.com/api/v1/control/route-split/override" \
  -H "authorization: Bearer ${CONTROL_ACCESS_TOKEN}" \
  -H "content-type: application/json" \
  -d '{"target":"htmx_rollback","domain":"chat_pilot"}'
```

Full-page fallback without changing route target:

```bash
curl -sS -X POST "https://openagents.com/api/v1/control/route-split/override" \
  -H "authorization: Bearer ${CONTROL_ACCESS_TOKEN}" \
  -H "content-type: application/json" \
  -d '{"target":"htmx_full_page","domain":"chat_pilot"}'
```

## Telemetry and Audit Signals

Expected control-plane events:

- `route.split.htmx.override.updated`
- `route.htmx.mode.decision`

The mode-decision event includes:

- `path`, `route_domain`
- `mode`, `reason`, `rollback_mode`
- HTMX request metadata (`is_hx_request`, `hx_boosted`, `hx_history_restore_request`, optional target/trigger/current_url)
