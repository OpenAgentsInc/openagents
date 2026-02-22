# OA-WEBPARITY-063 Auth/Session Edge-Case Matrix Parity

Date: 2026-02-22  
Status: pass (auth/session edge-case contract tests + harness automation)  
Issue: OA-WEBPARITY-063

## Deliverables

- Auth/session edge-case harness:
  - `apps/openagents.com/scripts/run-auth-session-edge-case-harness.sh`
- Manual workflow dispatch:
  - `.github/workflows/web-auth-session-edge-case-harness.yml`
- Auth/session parity fixes and tests:
  - `apps/openagents.com/service/src/auth.rs`
  - `apps/openagents.com/service/src/lib.rs`

## Covered Contracts

1. Refresh rotation race:
   - parallel refresh requests on same token produce one success + one replay rejection
   - replay rejection revokes the active rotated session
2. Revoke/logout WS impact:
   - logout propagation includes both `session_ids` and `device_ids`
   - revoke-other-device propagation includes `device_ids` and associated `session_ids`
3. Codex-only auth gating for retired guest-session endpoint:
   - unauthenticated calls are rejected
   - authenticated calls return explicit retirement contract (`410 Gone`) and codex canonical route metadata

## Verification Executed

```bash
cargo fmt --manifest-path apps/openagents.com/service/Cargo.toml
bash -n apps/openagents.com/scripts/run-auth-session-edge-case-harness.sh
./apps/openagents.com/scripts/run-auth-session-edge-case-harness.sh
```

Artifact produced:
- `apps/openagents.com/storage/app/auth-session-edge-cases/<timestamp>/summary.json`
