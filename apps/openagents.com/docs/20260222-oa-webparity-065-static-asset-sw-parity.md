# OA-WEBPARITY-065 Static Asset + Service Worker Delivery Parity

Date: 2026-02-22  
Status: pass (static asset/service-worker parity tests + harness automation)  
Issue: OA-WEBPARITY-065

## Deliverables

- Static asset/service-worker parity harness:
  - `apps/openagents.com/scripts/run-static-asset-sw-parity-harness.sh`
- Manual workflow dispatch:
  - `.github/workflows/web-static-asset-sw-parity-harness.yml`
- Static asset parity fixes and tests:
  - `apps/openagents.com/service/src/lib.rs`

## Covered Contracts

1. Content-hashed asset delivery and cache policy:
   - immutable cache headers for hashed assets
   - short-lived cache policy for non-hashed assets
2. Compression parity:
   - precompressed static variants negotiate correctly (`br` preferred over `gzip`)
   - `Vary: Accept-Encoding` contract preserved for static assets
3. ETag and conditional requests:
   - static assets emit ETag and honor `If-None-Match`
   - `openapi.json` emits ETag and honors `If-None-Match`
4. Service worker update/rollback policy:
   - `manifest.json` + `sw.js` remain no-store/no-cache
   - web-shell service-worker pinning/rollback policy verification passes

## Verification Executed

```bash
cargo fmt --manifest-path apps/openagents.com/service/Cargo.toml
cargo test --manifest-path apps/openagents.com/service/Cargo.toml static_
cargo test --manifest-path apps/openagents.com/service/Cargo.toml openapi_route_
bash -n apps/openagents.com/scripts/run-static-asset-sw-parity-harness.sh
./apps/openagents.com/scripts/run-static-asset-sw-parity-harness.sh
```

Artifact produced:
- `apps/openagents.com/storage/app/static-asset-sw-parity-harness/<timestamp>/summary.json`
