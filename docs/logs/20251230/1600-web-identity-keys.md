# Web identity + credential encryption

Date: 2025-12-30

## Summary
- Generated a Nostr/Bitcoin identity on GitHub OAuth using secp256k1 keypairs (pure Rust) and stored encrypted key material in D1.
- Encrypted GitHub tokens and API keys with a key derived from the user's identity; `SESSION_SECRET` protects the private key material at rest.
- Exposed `nostr_npub` via `/api/auth/me` and account settings, and rendered it under the GitHub username in the web UI.

## Implementation details
- Added a web worker identity helper for secp256k1 key generation, credential key derivation, and ChaCha20-Poly1305 encryption.
- Added a D1 migration for identity columns on `users`.
- Added `get_github_access_token` for decrypting (and migrating) stored GitHub tokens before API calls.
- Updated web client Repo Selector view to show `npub`.
- Updated web docs for API responses, config secrets, schema, and UI layout.

## Files touched
- `crates/web/worker/src/identity.rs`
- `crates/web/worker/src/db/users.rs`
- `crates/web/worker/src/lib.rs`
- `crates/web/worker/src/routes/auth.rs`
- `crates/web/worker/src/routes/account.rs`
- `crates/web/worker/Cargo.toml`
- `crates/web/client/src/lib.rs`
- `crates/web/migrations/0002_identity_keys.sql`
- `crates/web/docs/README.md`
- `crates/web/docs/architecture.md`
- `crates/web/docs/client-ui.md`

## Builds
- `cargo build --target wasm32-unknown-unknown` in `crates/web/worker`
- `cargo build --target wasm32-unknown-unknown` in `crates/web/client`
