# OA-RUST-061 Route Group Rollout: Auth / Onboarding Entry

Status: active
Owner: `owner:openagents.com`
Issue: OA-RUST-061

## Route groups migrated to Rust shell

1. `/login`
2. `/register`
3. `/authenticate`
4. `/onboarding/*`

## Parity checklist

1. Shared app route graph deterministically maps auth entry and onboarding paths.
2. Rust route split resolves auth/onboarding paths to Rust target for active cohorts.
3. Rust shell auth entry surface exposes login/onboarding flows through control-service auth APIs:
   - `POST /api/auth/email`
   - `POST /api/auth/verify`
   - `GET /api/auth/session`
   - `POST /api/auth/logout`
4. Session lifecycle semantics remain intact:
   - restore session path available from route surface
   - reauth-required and signed-out states are explicit in UI state

## Verification commands

From repo root:

```bash
cargo test -p openagents-app-state
cargo test --manifest-path apps/openagents.com/service/Cargo.toml route_split_serves_auth_entry_routes_in_rust_cohort
cargo check -p openagents-web-shell --target wasm32-unknown-unknown
```

## Staged rollout procedure

1. Confirm route split includes auth/onboarding prefixes:

```bash
curl -sS -H "authorization: Bearer ${CONTROL_ACCESS_TOKEN}" \
  https://openagents.com/api/v1/control/route-split/status | jq
```

2. Evaluate routing decisions before cohort expansion:

```bash
curl -sS -X POST -H "authorization: Bearer ${CONTROL_ACCESS_TOKEN}" \
  -H 'content-type: application/json' \
  -d '{"path":"/login","cohortKey":"user:auth-smoke"}' \
  https://openagents.com/api/v1/control/route-split/evaluate | jq

curl -sS -X POST -H "authorization: Bearer ${CONTROL_ACCESS_TOKEN}" \
  -H 'content-type: application/json' \
  -d '{"path":"/onboarding/checklist","cohortKey":"user:auth-smoke"}' \
  https://openagents.com/api/v1/control/route-split/evaluate | jq
```

3. Progress cohort percentages (`5 -> 25 -> 50 -> 100`) with auth failure-rate monitoring.

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
