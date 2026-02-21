# Web Shell SW Asset Pinning + Rollback Runbook (OA-RUST-082)

## Purpose

Define the release order and rollback-safe procedure for Rust/WASM web-shell bundles using service-worker pinned assets.

## Build Inputs

`apps/openagents.com/web-shell/build-dist.sh` now emits `manifest.json` v2 with:

- `buildId`
- compatibility window (`minClientBuildId`, `maxClientBuildId`)
- service-worker pinned asset set and cache name

Optional env controls:

- `OA_MIN_CLIENT_BUILD_ID` (default: current build)
- `OA_MAX_CLIENT_BUILD_ID` (default: unset)
- `OA_PROTOCOL_VERSION` (default: `khala.ws.v1`)
- `OA_SYNC_SCHEMA_MIN` / `OA_SYNC_SCHEMA_MAX` (default: `1`)
- `OA_ROLLBACK_BUILD_IDS` (comma list of cache-compatible prior build IDs)

## Release Order (Required)

1. Build and verify new web-shell dist:
   - `apps/openagents.com/web-shell/scripts/sw-policy-verify.sh`
2. Deploy control service static host with new `dist/` assets (`manifest.json`, `sw.js`, `/assets/*`).
3. Keep backend compatibility window permissive enough for existing live clients during rollout.
4. Observe client skew telemetry/status and ensure clients promote to the new service worker.
5. Tighten compatibility window only after rollout saturation.

## Rollback Procedure

1. Deploy previous known-good `dist/` (older `buildId`) and include the newer build ID in `OA_ROLLBACK_BUILD_IDS` if needed during transition.
2. Keep compatibility window broad enough for both rollback build and currently connected clients.
3. Verify `GET /manifest.json` reports rollback `buildId` and correct compatibility window.
4. Verify `GET /sw.js` is `no-store` and service worker installs rollback pinned assets.
5. Once clients converge, remove temporary rollback cache allowances.

## Recovery: Build Skew Detected

Client behavior:

1. Host shim fetches `/manifest.json` with `no-store`.
2. If local `buildId` is out of compatibility window or mismatched, it attempts service-worker promotion.
3. If promotion succeeds, client reloads automatically.
4. If promotion fails, client surfaces deterministic hard-refresh prompt.

## Verification

- Static route checks:
  - `GET /manifest.json` -> `Cache-Control: no-cache, no-store, must-revalidate`
  - `GET /sw.js` -> `Cache-Control: no-cache, no-store, must-revalidate`
- Browser drill:
  - deploy canary build
  - open client on prior build
  - confirm skew detection and promotion path
  - rollback and confirm prior compatible build activation
