# R2-A: authenticated personal-scope native Sync hosts

- Issue: #8667
- Parent tracks: #8574 and #8597
- Status: implemented; close after the main receipt is posted
- Authority:
  [`../2026-07-10-r1-r2-identity-sync-contract.md`](../2026-07-10-r1-r2-identity-sync-contract.md)

## Landed boundary

Electron main and the Expo host now compose their already-verified native
session custody into the existing `@openagentsinc/khala-sync-client` session.
Both reuse their persisted installation identity, the production HTTP/
WebSocket transport, shared overlay/state machine, and exactly
`personalScope(serverDerivedOwnerUserId)`.

The transport retains a host-only access-token callback and re-reads it for
requests/reconnects, so rotation does not widen the view contract. Owner refs,
access/refresh tokens, SQLite handles, transport/session objects, and raw rows
remain outside Effect Native state. Status exposes only bounded Sync phase,
freshness, schema, and pending count. Sign-out disconnects network Sync, while
quit/unmount/OTA teardown closes the session before the durable store.

## Verification boundary

The Desktop and mobile host tests run the real shared session engine against a
typed fake transport. They prove the authorized personal scope, dynamic token
lookup, live/freshness transition, and session-before-store close ordering.
Typechecks cover the native lifecycle composition.

## Explicit residual

This is authenticated replication substrate, not the D1 conversation exit. It
does not project or mutate `chat_thread`/`chat_message`, expose a real durable
conversation through either client, claim physical-device acceptance, or
implement the still-draft `device_session` entity.
