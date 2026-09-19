# OpenAgents

Rust workspace for OpenAgents tooling. Everything here is public; see
`AGENTS.md` for the repository contract.

## Crates

- `crates/jev` — SDK for TypeSafe Jev judgments (classification).
- `crates/nostr` — pure Nostr protocol primitives: events, tags, filters,
  canonical IDs, replacement, NIP-19, NIP-42, NIP-44, NIP-98, and the pinned
  Block-lane kind validators. No third-party Nostr crate.
- `crates/nostr-relay` — the `nostr-relay` binary: a hardened Nostr relay with
  a Postgres store, WebSocket gateway, NIP-42 auth, NIP-29 groups, NIP-45
  COUNT, NIP-50 search, NIP-86 management, NIP-98, and Blossom media.
  Deployed at `wss://relay.openagents.com`.
- `crates/coder` — the Coder agent: two-stage classify-then-generate
  conversation engine.
- `crates/coder-terminal` — the amber terminal composer used by Coder.

## NIPs

`nips/` holds the pinned specification lanes (`official/` from
nostr-protocol/nips, `block/` from block/buzz) plus `manifest.json` and
`scripts/sync-nips.sh` for parity checks. Protocol changes require fixtures
under `tests/fixtures/`.

## Verify

```bash
cargo fmt --check
cargo clippy --workspace --all-targets
cargo test --workspace
./scripts/test-postgres.sh   # full live gate: store, gateway, deploy, load
```

Local development: `docs/deployment/runbook-local-dev.md`.
Deployment: `docs/deployment/runbook-debian-vps.md` and `deploy/`.
