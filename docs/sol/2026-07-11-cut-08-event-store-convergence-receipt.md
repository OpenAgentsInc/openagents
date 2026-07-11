# CUT-08 event, cursor, and store convergence receipt

- Date: 2026-07-11
- Issue: [#8688](https://github.com/OpenAgentsInc/openagents/issues/8688)
- Parent: [#8677](https://github.com/OpenAgentsInc/openagents/issues/8677)
- Status: accepted; CUT-08 complete, parent remains open for CUT-09

## Result

Confirmed conversation timelines now fail closed on a missing scope version
instead of trusting an advancing live-frame cursor. Every advancing catch-up
page or live delta must contain the dense version interval immediately after
the durable cursor through the advertised cursor. Duplicate/stale frames remain
no-ops. A sparse live frame becomes a typed protocol failure, leaves the local
cursor unchanged, and reconnects through authoritative log replay. If that
cursor has fallen outside retention, the existing server error enters
MustRefetch/CVR/full-snapshot replacement; no partial history is promoted.

The local-store compatibility boundary is now explicit and shared. Version 1
stores record `store_schema_version`; the previously shipped unversioned shape
is the supported predecessor and migrates in place without losing confirmed
rows, cursors, identity, or queued mutations. A newer, invalid, or unsupported
version is inspected before additive SQL and refuses as
`incompatible_version` with bounded “update the app or reset its local Sync
cache” guidance. Bun SQLite, Desktop `node:sqlite`, Expo/mobile SQLite, and the
Web worker RPC preserve the same typed result.

## Deterministic matrix

| Fault | Authoritative behavior | Evidence |
| --- | --- | --- |
| Duplicate live delta | Cursor/entity apply is an idempotent no-op | shared session test |
| Stale/out-of-order frame | Frame at or behind durable cursor is ignored | shared session test |
| Missing live version | Sparse batch is rejected; cursor remains; reconnect replays the dense log | new session gap oracle |
| Retained-window cursor gap | Server refuses partial log; client enters MustRefetch/CVR or bootstrap replacement | existing read-service, compaction, session, and CVR oracles |
| Reordered timeline entities | Projection sorts by canonical event sequence while retaining confirmed entity versions | agent-timeline and new native corpus |
| Authoritative snapshot replacement | Old rows absent from the snapshot are atomically retracted | shared store semantics and new native corpus |
| Supported previous store | Unversioned store migrates to v1 in place with rows/cursor intact | SQLite migration fixture |
| Unsupported future store | Refusal occurs before `entities` or other additive tables are created | SQLite future-version fixture |
| Desktop/mobile equivalence | Same duplicate/reordered trace and gap snapshot yield identical snapshots, refs, versions, cursor, and retractions | `native-timeline-fault-convergence.e2e.test.ts` |
| Web/mobile error preservation | Worker RPC and Expo adapter retain `incompatible_version` and recovery guidance | Web/Expo adapter fixtures |

The session fault scheduler uses injected `setImmediate` turns, not elapsed-time
sleeps. The native trace corpus is synchronous over the two real host adapter
implementations.

## Verification

Focused suites cover the shared store core, Bun/Expo/Web adapters, session,
CVR equivalence/fallback/denial, agent timeline, Desktop/mobile hosts, and the
cross-adapter trace corpus: 135 tests and 517 expectations pass. Package and
both app typechecks pass. The closing commit also records the normal
`bun run check:deploy` gate.

The full gate passed: architecture/security/contract and pending-migration
guards, Pylon adversarial checks, 161 Khala Sync client tests (three explicitly
gated live-smoke skips), 21 web files / 545 tests, and 18 Worker files / 261
tests were green. The drift-guard suite's expected negative-fixture diagnostics
appear in its output while the suite itself passes.

After rebasing over the complementary cross-app duplicate/stale-delta oracle
from #8711, the merged Khala Sync client sweep passed 162 tests (three gated
live-smoke skips) and 12,688 expectations; the four native adapter/gateway
files added another 33 passing tests and 130 expectations.

## Invariant and product boundary

`INVARIANTS.md` now names dense scope-version validation and local-store
forward-version refusal. These narrow the existing Khala Sync contract; they
do not create a second cursor, timeline, cache, or migration system.

The Fable protected-core rule held again: no file under
`apps/pylon/src/orchestration` or `apps/pylon/src/node` changed. Process restart,
stale runtime generation, revocation, and interrupted finalization remain
CUT-09 [#8689]. Parent #8677 remains open for that matrix and its live
network-gap/restart receipt.
