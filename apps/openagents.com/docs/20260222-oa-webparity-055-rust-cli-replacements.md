# OA-WEBPARITY-055 Rust CLI Replacements for Operator Commands

Date: 2026-02-22
Status: pass
Issue: OA-WEBPARITY-055

## Scope

Provide Rust-native command replacements for the operator Artisan command set:
- `demo:l402`
- `khala:import-chat`
- `ops:test-login-link`
- `runtime:tools:invoke-api`
- `ops:create-api-token`

## Delivered Binary

New Rust binary:
- `openagents-control-ops`
- Source: `apps/openagents.com/service/src/bin/ops_cli.rs`

Run from repo root:

```bash
cargo run --manifest-path apps/openagents.com/service/Cargo.toml --bin openagents-control-ops -- <command>
```

## Command Mapping

1. `demo:l402`
- Replacement command: `openagents-control-ops demo:l402`
- Behavior: calls Rust control-service L402 endpoints (`/api/l402/wallet`, `/api/l402/transactions`) and prints deterministic JSON summary.

2. `khala:import-chat`
- Replacement command: `openagents-control-ops khala:import-chat`
- Behavior: imports Khala export tables (`users`, `threads`, `messages`) from directory or zip JSONL source into Rust auth/codex stores, supports `--dry-run` and `--replace`, reports import stats.
- Input format: `<table>/documents.jsonl` (directory or zip entry).

3. `ops:test-login-link`
- Replacement command: `openagents-control-ops ops:test-login-link`
- Behavior: generates signed `/internal/test-login` URLs using Rust HMAC signing compatible with service verification semantics.

4. `runtime:tools:invoke-api`
- Replacement command: `openagents-control-ops runtime:tools:invoke-api`
- Behavior: posts coding manifest/request payload to Rust `/api/runtime/tools/execute`, prints status and parsed response.

5. `ops:create-api-token`
- Replacement command: `openagents-control-ops ops:create-api-token`
- Behavior: resolves existing user by email from Rust auth store and issues PAT via `AuthService`, including abilities/expiry handling.

## Files Changed

- `apps/openagents.com/service/Cargo.toml`
- `apps/openagents.com/service/src/bin/ops_cli.rs`
- `apps/openagents.com/service/src/auth.rs`
- `apps/openagents.com/docs/20260222-web-parity-charter-checklist.md`

## Verification

Executed:

```bash
cargo fmt --manifest-path apps/openagents.com/service/Cargo.toml
cargo check --manifest-path apps/openagents.com/service/Cargo.toml --bin openagents-control-ops
cargo test --manifest-path apps/openagents.com/service/Cargo.toml
```

## Notes

- Replacement command names intentionally preserve legacy command identifiers (`demo:l402`, etc.) under a Rust binary umbrella to maintain operator familiarity.
- The Rust import path is store-authoritative (`OA_AUTH_STORE_PATH`, `OA_CODEX_THREAD_STORE_PATH`) and does not route through Laravel command infrastructure.
