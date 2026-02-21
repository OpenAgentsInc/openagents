# Control Auth/Session/Scope Contract Notes (OA-RUST-010)

Canonical wire schema:
- `proto/openagents/control/v1/auth.proto`
- Fixture set: `docs/protocol/fixtures/control-auth-session-v1.json`

## Scope

This contract set defines control-plane boundaries for:
- WorkOS-backed email-code challenge/verify flow.
- OpenAgents session authority (access/refresh/session/device lifecycle).
- Org membership and role/scope claims used for enforcement.
- Sync token request/response semantics for Khala topic access.

## Authority and Ownership

- WorkOS is the identity/authentication provider.
- OpenAgents control-plane remains authoritative for authorization, sessions, device binding, and revocation.
- Sync token issuance is derived from control-plane session/device/org state, not directly from WorkOS records.

## Security Expectations

1. Refresh token rotation is mandatory for `SessionRefreshResponse`; `replaced_refresh_token_id` tracks revoked predecessor token lineage.
2. `device_id` is required on session and sync-token paths for per-device revocation and auditability.
3. Sync-token grants are scope-limited (`granted_scopes` + `granted_topics`) and may not imply mutation authority.
4. Sync-token claims include client-surface attribution (`oa_client_surface`) so runtime policy can enforce per-surface topic allowlists (for example, Onyx restrictions).
5. Reauthentication states are explicit (`SESSION_STATUS_REAUTH_REQUIRED`, `CONTROL_ERROR_CODE_REAUTH_REQUIRED`, `CONTROL_ERROR_CODE_SESSION_REVOKED`) for deterministic client UX.
6. Control-plane errors are machine-readable via `ControlErrorCode` and must preserve the `request_id` correlation boundary.

## Compatibility Rules

1. `openagents.control.v1` is additive-only in-place.
2. Existing field tags are immutable; removals require `reserved` annotations and migration notes.
3. Any breaking auth/session semantics require a versioned package bump (`openagents.control.v2`).
4. Fixture and proto contract checks must pass before merge:
   - `buf lint`
   - `./scripts/verify-proto-generate.sh`
   - `mix runtime.contract.check`
