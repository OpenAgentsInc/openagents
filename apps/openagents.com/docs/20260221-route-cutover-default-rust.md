# OA-RUST-062 Default Router Cutover: Rust Shell

Status: active
Owner: `owner:openagents.com`
Issue: OA-RUST-062

## Objective

Make Rust shell the default web route target for production cohorts while preserving a fast rollback guard.

## Default cutover posture

Control-service defaults now set:

- `OA_ROUTE_SPLIT_ENABLED=true`
- `OA_ROUTE_SPLIT_MODE=rust`
- `OA_ROUTE_SPLIT_RUST_ROUTES=/`

This means route split evaluates all web paths to Rust shell by default unless explicit override or force-legacy controls are used.

## Rollback guard (must remain enabled)

1. Runtime override endpoint remains authoritative for immediate fallback:

```bash
curl -sS -X POST "https://openagents.com/api/v1/control/route-split/override" \
  -H "authorization: Bearer ${CONTROL_ACCESS_TOKEN}" \
  -H 'content-type: application/json' \
  -d '{"target":"legacy"}'
```

2. Override reset returns to configured default routing:

```bash
curl -sS -X POST "https://openagents.com/api/v1/control/route-split/override" \
  -H "authorization: Bearer ${CONTROL_ACCESS_TOKEN}" \
  -H 'content-type: application/json' \
  -d '{"target":"clear"}'
```

3. Infra-level Cloud Run rollback remains available via:

- `apps/openagents.com/service/deploy/canary-rollout.sh rollback <stable-revision>`

## Verification

From repo root:

```bash
cargo test --manifest-path apps/openagents.com/service/Cargo.toml route_split_serves_auth_entry_routes_in_rust_cohort route_split_serves_management_route_prefixes_in_rust_cohort
cargo test --manifest-path apps/openagents.com/service/Cargo.toml route_split_override_keeps_chat_pilot_on_rust_shell
```

## Canary expectations after default switch

1. `route.split.decision` audit events continue for all checked web paths.
2. Route-level rollback override is exercised and validated in staging before broad production promotion.
3. Auth/session/sync baseline checks remain green while Rust default routing is active.
