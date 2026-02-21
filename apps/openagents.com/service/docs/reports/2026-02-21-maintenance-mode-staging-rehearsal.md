# 2026-02-21 Staging Maintenance Mode Rehearsal

Issue: OA-RUST-112 (`#1937`)  
Timestamp (UTC): 2026-02-21

## Scope

Validate Rust control-service maintenance mode flow in staging before production:

1. maintenance enable
2. public block (`503`)
3. bypass bootstrap + cookie session
4. smoke checks through bypass session
5. maintenance disable and normal traffic restore

## Environment

- Project: `openagentsgemini`
- Region: `us-central1`
- Service: `openagents-control-service-staging`
- Validated base URL: `https://openagents-control-service-staging-ezxz4mgdsq-uc.a.run.app`
- Staging domain state at test time: `staging.openagents.com` remained on legacy Laravel mapping and then entered certificate reprovisioning (`CertificatePending`) during domain remap rehearsal; domain was reverted to `openagents-web-staging` and is left as-is while cert state settles.

## Steps and outcomes

1. Enable maintenance mode on staging service:

```bash
PROJECT=openagentsgemini REGION=us-central1 SERVICE=openagents-control-service-staging MAINTENANCE_BYPASS_TOKEN='<rotated>' apps/openagents.com/service/deploy/maintenance-mode.sh enable
```

Result: success (`openagents-control-service-staging-00002-bcv`).

2. Public request (no bypass):

- `GET /` -> `503`
- body: maintenance HTML page rendered

3. Health/readiness behavior during maintenance:

- `GET /readyz` -> `200` with readiness JSON
- `/healthz` may be unavailable on some ingress paths; smoke now treats it as best-effort and relies on `/readyz` as canonical probe.

4. Bypass bootstrap and cookie:

- `GET /?maintenance_bypass=<token>` -> `307` redirect to `/`
- `Set-Cookie: oa_maintenance_bypass=...; HttpOnly; Secure; SameSite=Lax; Max-Age=900`

5. Bypass session access:

- `GET /` with bypass cookie -> `200`
- `GET /manifest.json` with bypass cookie -> `200`

6. Smoke through bypass:

```bash
OPENAGENTS_BASE_URL='https://openagents-control-service-staging-ezxz4mgdsq-uc.a.run.app' OPENAGENTS_MAINTENANCE_BYPASS_TOKEN='<rotated>' apps/openagents.com/service/deploy/smoke-control.sh
```

Result: pass.

7. Disable maintenance mode:

```bash
PROJECT=openagentsgemini REGION=us-central1 SERVICE=openagents-control-service-staging apps/openagents.com/service/deploy/maintenance-mode.sh disable
```

Result: success (`openagents-control-service-staging-00003-d4c`), `GET /` returned `200`.

## Follow-up gates

1. Before production use, confirm staging custom domain is stable and certificate is `Ready=True`.
2. Keep production decommission actions blocked by Laravel DB backup + data-port gates in `docs/RUST_LEGACY_INFRA_DECOMMISSION.md`.
