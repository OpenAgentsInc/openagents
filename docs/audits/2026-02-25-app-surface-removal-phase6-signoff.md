# 2026-02-25 App Surface Cleanup Phase 6 Signoff

Status: completed
Date: 2026-02-25
Issue: https://github.com/OpenAgentsInc/openagents/issues/2204
Program epic: https://github.com/OpenAgentsInc/openagents/issues/2205

## Preflight Authority Check

Reviewed before final signoff:

1. `docs/adr/INDEX.md`
2. `docs/plans/rust-migration-invariant-gates.md`

Constraints applied for signoff:

1. Keep Rust-authoritative boundaries and retained surfaces intact.
2. Verify removals with executable gates/tests before closure.
3. Avoid reintroduction of removed web/iOS/Onyx surfaces.

## Required Validation Evidence

### Topology gates

1. `test -d apps` -> `APPS_DIR_PRESENT`
2. `test ! -d apps/autopilot-ios` -> `AUTOPILOT_IOS_REMOVED`
3. `test ! -d apps/onyx` -> `ONYX_REMOVED`

### Workspace and docs gates

1. `cargo check --workspace` -> `CARGO_CHECK_EXIT_0`
   - Result line: `Finished \`dev\` profile [optimized + debuginfo] target(s) in 1.49s`
   - Note: non-blocking warnings remain in existing retained crates.
2. `./scripts/local-ci.sh docs` -> `DOCS_CI_EXIT_0`
   - Output includes: `docs-check: OK`

### Retained deploy/runbook smoke checks

1. `bash -n apps/openagents.com/deploy/deploy-production.sh` -> `CONTROL_DEPLOY_SCRIPT_SYNTAX_OK`
2. `bash -n apps/runtime/deploy/cloudrun/deploy-runtime-and-migrate.sh` -> `RUNTIME_DEPLOY_SCRIPT_SYNTAX_OK`
3. `bash -n apps/lightning-ops/scripts/staging-reconcile.sh` -> `LIGHTNING_OPS_SCRIPT_SYNTAX_OK`

### Web behavior assertion gates

1. `cargo test -p openagents-control-service root_route_serves_landing_page_with_desktop_download_link`
   - `test tests::root_route_serves_landing_page_with_desktop_download_link ... ok`
2. `cargo test -p openagents-control-service download_desktop_route_redirects_to_configured_release_url`
   - `test tests::download_desktop_route_redirects_to_configured_release_url ... ok`
3. `cargo test -p openagents-control-service interactive_web_routes_are_not_mounted`
   - `test tests::interactive_web_routes_are_not_mounted ... ok`

## Deliverables Checklist

1. Final cleanup report with evidence: this file.
2. Backroom archive manifest: `docs/audits/2026-02-25-app-surface-removal-archive-manifest.md`.
3. Explicit signoff decision: provided below.

## Signoff Decision

GO.

Phase 6 gates are green, retained app/service surfaces compile and pass docs checks, and web behavior remains landing/download-only with interactive routes not mounted.
