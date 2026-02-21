# OA-RUST-059 Route Group Rollout: Account / Settings / Admin

Status: active
Owner: `owner:openagents.com`
Issue: OA-RUST-059

## Route groups migrated to Rust shell

1. `'/account'` and `'/account/*'`
2. `'/settings'` and `'/settings/*'`
3. `'/admin'` and `'/admin/*'`

These route groups are now included in the Rust route-prefix set used by control-service route split.

## Parity checklist

1. Route parsing and path round-trip in shared app-state is deterministic for account/settings/admin sections.
2. Rust control-service route split resolves these paths to Rust target when route split is enabled and cohort allows Rust.
3. Rust web shell renders a management surface for these routes with:
   - authenticated user/session context
   - organization memberships from `GET /api/orgs/memberships`
   - route split status from `GET /api/v1/control/route-split/status`
4. Admin route surface shows explicit guard result:
   - allow only when active-org membership role is `owner` or `admin`
   - deny view when role is not sufficient

## Verification commands

From repo root:

```bash
cargo test -p openagents-app-state
cargo test --manifest-path apps/openagents.com/service/Cargo.toml route_split_serves_management_route_prefixes_in_rust_cohort
cargo check -p openagents-web-shell --target wasm32-unknown-unknown
```

## Staged rollout procedure (route group)

1. Confirm route split prefixes include account/settings/admin:

```bash
curl -sS -H "authorization: Bearer ${CONTROL_ACCESS_TOKEN}" \
  https://openagents.com/api/v1/control/route-split/status | jq
```

2. Evaluate target decisions before traffic shift:

```bash
curl -sS -X POST -H "authorization: Bearer ${CONTROL_ACCESS_TOKEN}" \
  -H 'content-type: application/json' \
  -d '{"path":"/settings/profile","cohortKey":"user:smoke"}' \
  https://openagents.com/api/v1/control/route-split/evaluate | jq

curl -sS -X POST -H "authorization: Bearer ${CONTROL_ACCESS_TOKEN}" \
  -H 'content-type: application/json' \
  -d '{"path":"/admin","cohortKey":"user:smoke"}' \
  https://openagents.com/api/v1/control/route-split/evaluate | jq
```

3. Increase Rust cohort in staged percentages (`5 -> 25 -> 50 -> 100`) while monitoring auth/session and route-split audits.

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
