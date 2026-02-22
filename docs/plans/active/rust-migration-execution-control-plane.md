# Rust Migration Execution Control Plane

Status: active
Last updated: 2026-02-21

This plan tracks control-plane closure work needed for Rust-only endstate.

## Objectives

1. Keep WorkOS as identity/authentication provider.
2. Keep OpenAgents control service authoritative for authorization, sessions, devices, and revocation.
3. Keep command APIs on authenticated HTTP.
4. Keep sync-token minting and topic scope derivation centralized in control service.

## Current Required Lanes

1. Auth/session stability and token rotation.
2. Sync token issuance + revocation propagation.
3. Staging/prod deploy validation with rollback drills.
4. Protocol compatibility policy enforcement.

## Verification

```bash
./scripts/local-ci.sh changed
./scripts/local-ci.sh workspace-compile
scripts/release/validate-rust-cutover.sh
```
