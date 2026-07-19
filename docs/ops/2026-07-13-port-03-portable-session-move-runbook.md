# PORT-03 portable session move and failback runbook

- Issue: [#8748](https://github.com/OpenAgentsInc/openagents/issues/8748)
- Contract: `openagents.portable_session_move.v1`
- Authority: PORT-01 Cloud SQL/Khala Sync
- Capability boundary: PORT-02 general broker
- Runtime targets: owner-local Pylon and accepted Agent Computer

## Admission

Before a move, read the owner-scoped PORT-01 snapshot and require:

- the command's expected attachment ref/generation exactly equals current.
- migration `0067` has recorded the exact owner/session canonical run,
  repository, and pinned-base refs. Legacy unbound rows cannot move.
- the destination is an authorized, ready target and is not the source.
- the complete canonical graph has one root, every child edge, independent
  thread/transcript refs, and a durable current row for every thread.
- the transfer list names every current attachment lease exactly once and uses
  fresh destination lease/source-grant refs. And
- both target adapters declare the exact durable target refs/classes.

Never derive session identity from a host, path, process, provider session, or
Agent Computer. Never accept a partial descendant or lease set.

## Durable broker claim

Production movement must construct PORT-02 with
`PostgresPortableCapabilityBrokerStore`, not the legacy split evidence/state
test seam. Before the first capability operation, acquire the exact
owner/session move claim with the current broker revision. The claim binds the
move and command refs, source attachment and generation, and destination
target. Every capability operation then commits its complete refs-only broker
state and exact evidence row under one revision-CAS Postgres transaction while
holding and revalidating that claim.

A stale revision, absent claim, or competing claim is terminal for that
coordinator instance. Dispose it and reconcile from PORT-01 plus a fresh store
load. Never continue with its in-memory snapshot. Release the claim only after
the durable move outcome and required cleanup are reconciled. Migration `0069`
owns the aggregate/evidence tables. They contain refs and bounded policy facts
only, never credential bytes, provider payloads, host paths, or repository
content.

## Required order

1. Admit the exact movement command in PORT-01.
2. Ask the source target to quiesce every canonical descendant under one stable
   operation ref.
3. Persist the graph-wide `quiesced` source attachment. From this point the
   source generation is outside the work-accepting attachment index.
4. Create the checkpoint. Require the byte-identical durable execution binding,
   exact durable event head, recomputed
   complete graph digest, repository post-image and diff digest, catalog
   generation, approval/artifact/receipt refs, and explicit
   `secretMaterial:excluded` / `processState:excluded`.
5. Prepare the exact destination resource, transport the private checkpoint
   artifact, and integrity-check the materialized repository at the
   destination. The stage receipt must bind the requested session, target,
   attachment, generation, checkpoint, repository post-image, and graph digest
   and say `acceptingWork:false`. A caller may not nominate an arbitrary or
   pre-existing managed resource.
6. For every source capability, use PORT-02 `reissue`: revoke the source
   grant, wipe the source target, issue a strictly newer
   destination-generation lease, redeem it for the exact verified staged
   resource, and verify the refs-only installation marker. A stage or artifact
   failure therefore occurs before any broker effect. A later broker failure
   aborts the staged resource and releases every attempted destination lease.
7. Reclaim every source agent process and all scratch/port state.
8. Atomically complete the PORT-01 move: store checkpoint/outcome, detach
   source, create the one active destination attachment, and advance every
   canonical node to the new generation.
9. Activate the destination using the stable activation operation ref.

Failback is the same state machine with the current managed attachment as
source and the authorized local Pylon as destination. It is never a special
credential-copy shortcut.

## Reconciliation

| Failure point | Required state/action |
| --- | --- |
| Before command admission | No runtime operation. Reject stale/unauthorized command |
| Source cannot quiesce/checkpoint | Record failed outcome if admitted. Leave source fenced/recovery-required |
| Destination prepare/materialize/verify rejects checkpoint | Compensating-abort the prepared resource. Make no broker change. Retain quiesced source |
| Capability revoke/wipe/reissue/redeem/install | Abort staged state. Release any attempted destination leases. No destination activation |
| Source cleanup incomplete | Do not advance attachment. Abort/release and require recovery |
| PORT-01 completion unknown | Retry identical command/completion bytes. Never construct a second attachment |
| Activation ACK lost after completion | Read completed authority and retry only the identical activation operation |

A completed command replay may reconcile destination activation, but it must
not quiesce, checkpoint, move grants, create an attachment, or accept a parent/
child turn again.

## Deterministic gate

Migration `0067` was applied to both `khala_sync_staging` and
`khala_sync_prod` on 2026-07-13 through the direct Cloud SQL Auth Proxy. Both
post-apply dry runs reported zero pending migrations.

Migration `0069` was applied to staging and production on 2026-07-13 with
SHA-256 prefix `ce9db7cddbb5`. Both post-apply dry runs reported `0 pending, 70
already applied`. Its real-Postgres store oracle is included below.

```sh
bun test packages/khala-sync-server/src/portable-session-move.test.ts \
  packages/khala-sync-server/src/portable-session-authority.test.ts \
  packages/khala-sync-server/src/portable-capability-broker-store.test.ts
bun x tsc -p packages/khala-sync-server/tsconfig.json --noEmit --pretty false
```

## Live acceptance gate

Do not close #8748 from deterministic evidence. #8636 is complete, and the
owner-side coordinator now composes the atomic store, concrete local/managed
targets, exact provider/SCM authority, target-local installers, and private
checkpoint artifact transport. Before the direct live session, require the
remaining production paths to be landed and deployed together:

- retained-guest materialization that independently validates the complete
  manifest, git bundle, post-image, tracked symlinks, and checkpoint digests.
- one fixed authority-bound root/child continuation action with stable
  operation/turn refs and atomic canonical event/node/cursor commits.
- managed-to-local private artifact export plus the concrete local rehydrator.
- restart-safe Pylon control-session binding recovery, prepared-resource
  compensation, and missing-VM teardown reconciliation. And
- deployed exact provider/GitHub grant-movement routes and a freshly baked
  guest/rootfs containing the same controller contract.

Then run one direct session on live infrastructure:

1. start a bounded repository session with a root and at least one active child
   on owner-local Pylon A, recording its canonical run, repository, and pinned
   base through migration `0067` authority.
2. capture canonical refs, per-thread cursors, exact repository/diff digests,
   current capability leases, process/scratch/port inventory, and attachment.
3. move to the accepted #8547 Agent Computer and execute exactly one bounded
   accepted turn for the root and each canonical active child under fresh
   target grants, with stable per-agent turn refs.
4. prove source processes, children, scratch, ports, and grants are gone and a
   stale source command/event is rejected.
5. retry the same move, continuation, and activation acknowledgement and prove
   byte-identical refs/cursors with no duplicate work.
6. export the changed managed repository privately, fail back to local using
   the same state machine, and independently verify the restored post-image.
7. capture final exact post-image, unchanged canonical run/repository/pinned
   base, one live attachment, grant/reclaim evidence, and rollback behavior.

The evidence bundle must be refs/digests only. Exclude raw prompts, private repo
content, tokens, auth homes, host paths, provider payloads, and topology secrets.
