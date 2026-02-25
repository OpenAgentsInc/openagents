# Spacetime Token Scope and Rotation Contract

Date: 2026-02-25  
Status: Active  
Owner lanes: Control, Runtime, Desktop

## Purpose

Define runtime-enforced token scope/stream-grant behavior and key-rotation semantics for Spacetime sync sessions.

## Claims and Scope Contract

Control-issued sync token responses include:

1. `transport=spacetime_ws`
2. `protocol_version=spacetime.sync.v1`
3. `refresh_after_in` and `refresh_after`
4. `granted_streams` plus legacy `granted_topics` compatibility fields

JWT claims include:

1. `oa_sync_scopes`
2. `oa_sync_streams`
3. legacy compatibility `oa_sync_topics`
4. `jti`, `iat`, `nbf`, `exp`

Runtime auth enforcement:

1. Scope checks remain mandatory per stream class.
2. If `oa_sync_streams`/`oa_sync_topics` are present, requested stream must be explicitly granted.
3. Missing explicit stream grants is treated as unbounded compatibility mode (temporary).

## Rotation and Time Validation

Runtime supports:

1. Primary signing key: `RUNTIME_SYNC_TOKEN_SIGNING_KEY`
2. Fallback signing keys: `RUNTIME_SYNC_TOKEN_FALLBACK_SIGNING_KEYS` (comma-separated)
3. Clock skew leeway: `RUNTIME_SYNC_TOKEN_CLOCK_SKEW_SECONDS` (default `30`)

Validation behavior:

1. Reject expired tokens (`exp`) outside leeway.
2. Reject not-yet-valid tokens (`nbf`) outside leeway.
3. Enforce max token age with `iat` and `RUNTIME_SYNC_TOKEN_MAX_AGE_SECONDS`.
4. Enforce revocation denylist with `RUNTIME_SYNC_REVOKED_JTIS`.

## Desktop Client Behavior

Desktop sync token minting behavior:

1. Prefer `POST /api/spacetime/token`.
2. Fallback to `POST /api/sync/token` for compatibility.
3. Treat missing/invalid token payload as hard error.
4. Use `refresh_after_in` as the proactive remint boundary for long-lived sessions.

