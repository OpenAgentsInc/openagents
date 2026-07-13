# PORT-01 durable portable-session authority receipt

- Issue: [#8746](https://github.com/OpenAgentsInc/openagents/issues/8746)
- Packet: PORT-01 of the remote-first portable coding-session pathway
- Authority: Cloud SQL Postgres through Khala Sync

## Landed authority

Migration `0066_portable_session_authority.sql` and
`@openagentsinc/khala-sync-server/portable-session-authority` persist one
owner-minted host-independent session identity, its complete parent/child graph
and per-thread cursors, authorized targets, exclusive attachment generations,
secret-free checkpoint metadata, append-only events, repairable current rows,
and durable commands/outcomes. `portable.registerSession` and
`portable.requestCommand` are registered in the production Worker request
processor. Runtime writers share the same transaction boundary for event
append, repair, move completion, generic outcome recording, reads, and purge.

No raw credential, host path, process/socket handle, provider-native session
ID, or transcript body has a column or allowed post-image field. Movement
requires the complete canonical descendant set and advances every nonterminal
node only after the old attachment is detached. Exact lost-ACK retries return
the recorded result; conflicting command or completion bytes fail closed.

## Verification

- `bun run --cwd packages/khala-sync-server typecheck`
- `bun test packages/khala-sync-server/src/portable-session-authority.test.ts`
  — 5 tests, 25 expectations against a real disposable Postgres database
- `bun run --cwd apps/openagents.com/workers/api typecheck`
- focused Worker Khala Sync registry/push tests
- migration dry-run/apply and relation/index inspection on staging then
  production, recorded in the issue close comment

The Postgres oracle deletes the derived current projection and reconstructs it
through a fresh SQL handle; rejects a cursor gap and late source generation;
reconciles lost command and move-completion acknowledgements without duplicate
rows; proves one detached source plus one active destination covering both root
and child; records a non-movement outcome idempotently; and verifies retention
purge cascades durable events and emits projection tombstones.

## Boundary

PORT-01 establishes durable control-plane authority. It does not redeem target
credentials or claim that a provider process moved. Those proofs remain
PORT-02 and PORT-03.
