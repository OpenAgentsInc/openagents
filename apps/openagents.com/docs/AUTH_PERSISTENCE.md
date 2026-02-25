# Auth Persistence (OA-WEBPARITY-008)

Rust auth/session/token domain now supports durable state snapshots.

## Storage model

- Backing store: JSON snapshot on local filesystem.
- Config:
  - `OA_AUTH_STORE_PATH=/absolute/or/relative/path.json`
- Behavior:
  - if `OA_AUTH_STORE_PATH` is not set, auth domain remains in-memory.
  - if set, auth state is loaded at service boot and persisted after mutating auth operations.

## Persisted domains

- challenge state
- users + identity fields
- session state (active/revoked/expired)
- refresh-token replay/revocation records
- personal access token records

## Personal access token domain methods

Implemented in `apps/openagents.com/src/auth.rs`:

- `list_personal_access_tokens(user_id)`
- `issue_personal_access_token(user_id, name, scopes, ttl_seconds)`
- `revoke_personal_access_token(user_id, token_id)`

These methods establish Rust-owned storage semantics for token inventory and revocation state, ready for API surface wiring.

## Verification

- `cargo test --manifest-path apps/openagents.com/Cargo.toml`
- Added tests:
  - persisted session reload from auth store path
  - personal access token issue/list/revoke persisted across service restart
