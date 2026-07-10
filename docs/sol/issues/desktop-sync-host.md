# D1-B: host-owned Desktop Khala Sync SQLite

- Issue: #8656
- Parent track: #8574
- Depends on: closed #8655
- Status: closed after the main receipt recorded on the live issue
- Authority:
  [`../2026-07-10-r1-r2-identity-sync-contract.md`](../2026-07-10-r1-r2-identity-sync-contract.md)

## Landed boundary

Electron main opens an owner-private `node:sqlite` database under Desktop
`userData`. A thin driver binds Node SQLite to the existing
`@openagentsinc/khala-sync-client` store core; it does not fork schema or store
semantics. One installation identity is written once and reused after restart.
The store closes deterministically during application quit.

The Runtime Gateway reports local persistence ready while the `khala-sync`
network capability remains unavailable until native OpenAgents sign-in lands.
The renderer never receives the database path/handle, identity refs, rows,
pending mutation queue, or credentials.

The enforced behavior contract is
`openagents_desktop.sync.host_owned_sqlite.v1`.

## Explicit residual

This leaf is local persistence only. Bearer custody, server-derived owner scope,
session refresh/revocation, network transport, scope subscription, and typed
conversation projections remain later R1/R2/D1 leaves.
