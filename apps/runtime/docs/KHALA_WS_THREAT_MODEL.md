# Khala WebSocket Threat Model and Anti-Replay Policy

Status: Active  
Issue: OA-RUST-092 (`#1927`)

## Scope

This document defines the required threat model and controls for Khala auth/join flows, including browser-origin policy and token anti-replay requirements.

## Threat Classes

1. Token replay (captured token reused during validity window).
2. Session hijack attempts using stolen bearer tokens.
3. Origin misuse from untrusted browser origins.
4. Unauthorized topic joins (scope or ownership mismatch).

## Required Controls

## Token Validation Controls

1. Verify issuer and audience (`iss`, `aud`).
2. Verify expiry and not-before (`exp`, `nbf`).
3. Require `jti` by policy (`RUNTIME_SYNC_TOKEN_REQUIRE_JTI=true`).
4. Reject revoked `jti` values (`RUNTIME_SYNC_REVOKED_JTIS`).
5. Enforce token max age from `iat` (`RUNTIME_SYNC_TOKEN_MAX_AGE_SECONDS`).

Deterministic deny reason codes:

1. `missing_authorization`
2. `invalid_authorization_scheme`
3. `invalid_token`
4. `token_expired`
5. `missing_jti`
6. `missing_iat`
7. `token_too_old`
8. `token_revoked`

## Origin Policy Controls

1. When `Origin` header is present, enforce allowlist check.
2. Origins are normalized (lowercase, trailing slash removed).
3. Deny with deterministic reason code `origin_not_allowed`.
4. Missing `Origin` is allowed for non-browser clients (desktop/mobile/native).

Config:

1. `RUNTIME_KHALA_ENFORCE_ORIGIN` (default `true`)
2. `RUNTIME_KHALA_ALLOWED_ORIGINS` (default includes `https://openagents.com` and `https://www.openagents.com`)

## Join Authorization Controls

1. Scope matrix enforced by topic class (`runtime.run_events`, `runtime.codex_worker_events`, etc).
2. Worker lifecycle topics additionally enforce ownership checks.
3. Denied joins must emit warning logs with reason code and topic context.

## Audit and Observability Requirements

Mandatory deny-path telemetry/logging:

1. Auth denied events (`khala auth denied`) with `reason_code`.
2. Topic denied events (`khala topic denied`) with `reason_code`.
3. Origin denied events (`khala origin denied by policy`) with origin and allowed set.
4. Publish/fanout policy denials (`khala publish rate limit triggered`, payload limit violations).

## Security Test Coverage

Required test scenarios:

1. Revoked token rejection.
2. Missing `jti` rejection (when required).
3. Stale token (`iat` max-age) rejection.
4. Untrusted origin rejection.
5. Allowed origin success.
6. Scope and ownership denied joins.

## Non-Goals

1. Exactly-once anti-replay guarantees.
2. External pen-test execution in this issue.
