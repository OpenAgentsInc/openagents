# OpenAgents Spacetime Module (`autopilot-sync`)

Status: active canonical module

This directory is the canonical, versioned Spacetime module for retained OpenAgents sync behavior.

## Module Path

- `spacetime/modules/autopilot-sync/spacetimedb`

## Core Tables

- `active_connection`: live connection/presence state and Nostr identity binding fields.
- `nostr_presence_claim`: short-lived challenge records for Nostr ownership proof.
- `stream_head`: stream sequence head for deterministic append behavior.
- `sync_event`: append-only stream event rows keyed by idempotency key.
- `stream_checkpoint`: per client+stream replay checkpoint state.

## Lifecycle Reducers

- `init`
- `client_connected`
- `client_disconnected`

## Domain Reducers

- `heartbeat`
- `request_nostr_presence_challenge`
- `bind_nostr_presence_identity`
- `append_sync_event`
- `ack_stream_checkpoint`

## Nostr Identity Binding

Presence binding requires a challenge-response flow:

1. Client calls `request_nostr_presence_challenge`.
2. Client signs challenge with its Nostr key.
3. Client calls `bind_nostr_presence_identity` with `nostr_pubkey_hex`, `nostr_pubkey_npub`, challenge, and signature.
4. Reducer verifies Schnorr signature over deterministic challenge digest before writing identity fields.

## Publish

From this directory:

```bash
spacetime publish "$OA_SPACETIME_DEV_DATABASE" \
  --server maincloud \
  --module-path ./spacetimedb \
  -y
```

For scripted publish/promote flow, use `scripts/spacetime/publish-promote.sh`.
