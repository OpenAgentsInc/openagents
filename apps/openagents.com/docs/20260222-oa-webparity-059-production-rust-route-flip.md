# OA-WEBPARITY-059 Production Rust-Only Route Target Flip

Date: 2026-02-22
Status: pass (automation + authority lock checks; live apply requires operator credentials)
Issue: OA-WEBPARITY-059

## Deliverables

- Route flip automation: `apps/openagents.com/service/scripts/run-production-rust-route-flip.sh`
- Runbook: `apps/openagents.com/service/docs/PRODUCTION_RUST_ROUTE_FLIP.md`
- Workflow automation removed (invariant: no `.github/workflows`).

## Rust-Authority Lock Added

Route-split evaluation now hard-pins all `/api/*` paths to Rust authority, even when legacy overrides are configured.

Code/tests:
- `apps/openagents.com/service/src/route_split.rs`
- `apps/openagents.com/service/src/lib.rs`

## Verification Executed

```bash
cargo test --manifest-path apps/openagents.com/service/Cargo.toml api_paths_are_rust_authority_even_under_legacy_overrides
cargo test --manifest-path apps/openagents.com/service/Cargo.toml route_split_evaluate_pins_api_paths_to_rust_authority
bash -n apps/openagents.com/service/scripts/run-production-rust-route-flip.sh
```

## Production Apply Command (Ready)

```bash
BASE_URL=https://openagents.com \
APPLY=1 \
CONTROL_ACCESS_TOKEN=<admin-token> \
apps/openagents.com/service/scripts/run-production-rust-route-flip.sh
```

The script emits a structured summary under:
- `apps/openagents.com/storage/app/production-route-flip/<timestamp>/summary.json`
