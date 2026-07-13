# PORT-03 first graph-wide move implementation receipt

- Issue: [#8748](https://github.com/OpenAgentsInc/openagents/issues/8748)
- Packet: PORT-03 of the remote-first portable coding-session pathway
- Status: implementation landed; live acceptance intentionally open
- Depends on: PORT-01 #8746, PORT-02 #8747, completed FC-4 #8636
- Contract: `openagents.portable_session_move.v1`

## What this receipt proves

`packages/khala-sync-server/src/portable-session-move.ts` composes the actual
PORT-01 Cloud SQL authority and PORT-02 capability broker. The deterministic
oracle runs a session containing one root and one child from an `owner_local`
target to an `openagents_managed` Agent Computer target and back.

The coordinator preserves the same canonical session, root/child agent,
parent edge, thread, transcript, per-thread cursor, run, repository, and pinned
base refs. Migration `0067` stores the additive owner/session execution binding;
new registrations require it, and legacy unbound rows cannot move. It requires a
byte-stable checkpoint with an exact durable-head cursor plus repository
post-image, diff, graph, catalog, approval, artifact, and receipt digests. The
PORT-01 completion transaction now independently recomputes the stored complete
graph digest and rejects a checkpoint whose event cursor is either ahead of or
behind durable head.

Every source attachment capability participates exactly once. PORT-02 revokes
and wipes the source installation before issuing and redeeming fresh leases
for the next attachment generation and explicit target. The target stages the
checkpoint with `acceptingWork:false`; source cleanup must report every graph
node plus released process, scratch, and port state; only then may PORT-01
detach the source and advance the one live attachment. Target activation uses
one stable operation ref after durable commit.

The production-durability follow-up adds migration `0069` and
`PostgresPortableCapabilityBrokerStore`. The exact active move claim, complete
refs-only broker state, operation evidence, and revision advance now share one
Postgres transaction. A stale CAS, lost claim, conflicting move, duplicate
evidence identity, or forbidden private material leaves state and evidence
unchanged. A fresh broker restores the committed operation bytes and returns a
replay without repeating the capability operation.

## Fault and replay evidence

The real-Postgres suite proves:

- complete local → managed → local move/failback with one live attachment;
- exact session/thread/agent/parent/transcript/per-thread cursor preservation;
- exact canonical run/repository/pinned-base binding preservation;
- exact checkpoint/repository/diff/graph digest verification;
- source generation fencing and rejection of a late source-child event;
- source capability revoke/wipe and destination reissue/redeem on both legs;
- complete source process/scratch/port cleanup before authority transfer;
- exact command replay with one checkpoint, attachment, and accepted root/child
  work identity;
- lost activation acknowledgement followed by idempotent reconciliation;
- destination rejection and source cleanup failure leaving a quiesced source,
  `recovery_required`, released destination grants, and no accepted work;
- stale generation after a fresh SQL handle performing no runtime mutation;
- checkpoint digest, graph digest, cursor, and execution-binding tamper refusal
  before transfer, plus refusal of legacy unbound rows;
  and
- serialized result/authority/broker state contains neither the canary raw
  credential nor a destination source-grant field.

## Verification

Migration `0067_portable_session_execution_binding.sql` was applied on
2026-07-13 through the direct Cloud SQL Auth Proxy as `khala_migrate` to both
`khala_sync_staging` and `khala_sync_prod`. Each ledger recorded SHA-256 prefix
`4fa862b732d0`; a subsequent dry run in each database reported `0 pending, 68
already applied`.

```sh
bun test packages/khala-sync-server/src/portable-session-move.test.ts \
  packages/khala-sync-server/src/portable-session-authority.test.ts \
  packages/khala-sync-server/src/portable-capability-broker-store.test.ts
bun x tsc -p packages/khala-sync-server/tsconfig.json --noEmit --pretty false
bun test --cwd packages/portable-session-contract
bun x tsc -p packages/portable-session-contract/tsconfig.json --noEmit --pretty false
```

## Honest remaining gate

This is production-path implementation and real database/broker boundary
evidence, not the real-host acceptance demanded by #8748. Migration `0069`
was applied to staging and production on 2026-07-13 (SHA-256 prefix
`ce9db7cddbb5`; both post-apply dry runs: `0 pending, 70 already applied`). The
atomic store still needs owner-side composition with the real local and managed
adapters. The issue must
remain open until #8636 is completed and one direct live journey moves the same bounded
child-bearing repository session from the owner's local Pylon to the accepted
#8547 Agent Computer and back. That receipt must show actual target/runtime
refs, exact repository/diff post-image, fresh grants, source reclaim, zero
duplicate accepted parent/child work, and failure/rollback behavior. No fixture
or composition of separate receipts may substitute for that journey.
