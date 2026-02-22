# OA-WEBPARITY-062 Non-HTTP Behavior Parity Harness

Date: 2026-02-22
Status: pass (contract tests + harness automation)
Issue: OA-WEBPARITY-062

## Deliverables

- Non-HTTP parity harness:
  - `apps/openagents.com/scripts/run-non-http-parity-harness.sh`
- Manual workflow dispatch:
  - `.github/workflows/web-non-http-parity-harness.yml`
- Contract and behavior updates in service:
  - `apps/openagents.com/service/src/lib.rs`

## Covered Contracts

1. Cookie attributes:
   - `HttpOnly`
   - `Secure`
   - `SameSite=Lax`
   - host-scoped cookie behavior (no `Domain=` attribute)
2. CORS/preflight:
   - `OPTIONS /api/*` returns `204` with CORS contract headers
3. Cache headers:
   - `openapi.json` pinned to manifest cache policy
   - API list endpoints default to `Cache-Control: no-store`
4. Rate limiting:
   - throttle behavior remains keyed/scoped by request identity
5. WS handshake/auth compatibility:
   - sync token handshake and Khala WS stream contract tests

## Verification Executed

```bash
cargo fmt --manifest-path apps/openagents.com/service/Cargo.toml
bash -n apps/openagents.com/scripts/run-non-http-parity-harness.sh
./apps/openagents.com/scripts/run-non-http-parity-harness.sh
```

Artifact produced:
- `apps/openagents.com/storage/app/non-http-parity-harness/<timestamp>/summary.json`
