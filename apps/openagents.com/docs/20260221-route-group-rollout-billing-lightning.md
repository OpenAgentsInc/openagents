# OA-RUST-060 Route Group Rollout: Billing / Lightning Operator

Status: active
Owner: `owner:openagents.com`
Issue: OA-RUST-060

## Route groups migrated to Rust shell

1. `'/l402'` and `'/l402/*'`
2. `'/billing'` and `'/billing/*'` (alias)

These route groups are now included in the Rust route-prefix set used by control-service route split.

## Parity and safeguard checklist

1. Shared route graph supports deterministic billing route parsing for `/l402/*` and `/billing/*`.
2. Rust route split resolves billing/lightning paths to Rust shell when split mode/cohort targets Rust.
3. Rust shell billing surface renders policy and operator context via Rust HTTP bindings:
   - `POST /api/policy/authorize`
   - `GET /api/orgs/memberships`
   - `GET /api/v1/control/route-split/status`
4. Operator safeguards remain explicit in Rust shell:
   - paywall-operator sections require owner/admin membership on active org
   - denied policy decisions are surfaced with explicit reason text

## Verification commands

From repo root:

```bash
cargo test -p openagents-app-state
cargo test --manifest-path apps/openagents.com/service/Cargo.toml route_split_serves_management_route_prefixes_in_rust_cohort
cargo check -p openagents-web-shell --target wasm32-unknown-unknown
```

## Staged rollout procedure

1. Confirm route split includes billing prefixes:

```bash
curl -sS -H "authorization: Bearer ${CONTROL_ACCESS_TOKEN}" \
  https://openagents.com/api/v1/control/route-split/status | jq
```

2. Evaluate decisions for billing routes before widening cohort:

```bash
curl -sS -X POST -H "authorization: Bearer ${CONTROL_ACCESS_TOKEN}" \
  -H 'content-type: application/json' \
  -d '{"path":"/l402/paywalls","cohortKey":"user:operator-smoke"}' \
  https://openagents.com/api/v1/control/route-split/evaluate | jq
```

3. Progress cohorts (`5 -> 25 -> 50 -> 100`) while checking policy/audit signals.

## Rollback

Immediate route rollback:

```bash
curl -sS -X POST "https://openagents.com/api/v1/control/route-split/override" \
  -H "authorization: Bearer ${CONTROL_ACCESS_TOKEN}" \
  -H 'content-type: application/json' \
  -d '{"target":"legacy"}'
```

Return to configured split mode:

```bash
curl -sS -X POST "https://openagents.com/api/v1/control/route-split/override" \
  -H "authorization: Bearer ${CONTROL_ACCESS_TOKEN}" \
  -H 'content-type: application/json' \
  -d '{"target":"clear"}'
```
