# M1-A: host-owned mobile Expo SQLite Sync store

- Issue: #8657
- Parent track: #8597
- Status: closed after the main receipt recorded on the live issue
- Authority:
  [`../2026-07-10-r1-r2-identity-sync-contract.md`](../2026-07-10-r1-r2-identity-sync-contract.md)

## Landed boundary

The OpenAgents mobile host opens one private Expo SQLite database through a
thin adapter over the existing `@openagentsinc/khala-sync-client` store core.
It does not fork the schema, mutation queue, cursor, reset, or confirmed-state
semantics. One cryptographically generated installation identity is written
once and reused after restart.

The native database handle and identity remain outside the Effect Native view
program. The UI receives only `Local Sync ready` or an unavailable state and
continues to require an OpenAgents session before claiming network Sync. The
host closes idempotently on unmount and after an owned OTA is fetched but
before the JS runtime reloads.

The enforced behavior contract is
`openagents_mobile.sync.host_owned_expo_sqlite.v1`.

## Explicit residual

This leaf is local persistence only. SecureStore session custody,
`device_session`, token refresh/revocation, network transport, authorized scope
subscription, conversation projections, and physical-device recovery proof
remain later R1/R2/F1/F2 leaves.
