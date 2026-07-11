# CUT-07 conversation command convergence receipt

- Date: 2026-07-11
- Issue: [#8687](https://github.com/OpenAgentsInc/openagents/issues/8687)
- Parent: [#8677](https://github.com/OpenAgentsInc/openagents/issues/8677)
- Status: accepted; CUT-07 complete, parent remains open for CUT-08/CUT-09

## Result

Desktop and mobile runtime commands now use one command identity and one
authoritative result path: the existing Khala Sync mutation ledger plus the
canonical runtime control-intent projection. There is no recovery-side queue,
adapter-local completion ledger, or Pylon-specific retry protocol.

An intent may carry one immutable `expiresAt` admission deadline. The server
compares it to the transaction clock before changing a turn. A due intent is
written once as `expired`, projected into the owner and exact thread scopes,
and excluded from the runtime-intent dispatch reader. Exact retries reconcile
to that row; no later reconnect can turn it into provider work.

The shared client reader resolves one stable `intentId` to:

- `pending`, with the durable local mutation ID;
- confirmed `accepted`, `settled`, or `expired`, with entity version;
- `failed` or `canceled` when the exact confirmed run has reached that state.

The result survives local-store restart. Runtime Gateway protocol v7 exposes
the exact intent/thread query to Desktop, and the mobile adapter consumes the
same shared reader. Both surfaces show a typed offline-expiry error instead of
waiting forever or fabricating completion.

## Deterministic fault matrix

| Fault | Authoritative transition | Oracle |
| --- | --- | --- |
| ACK lost after apply | Mutation 1 commits once; retry carries Mutation 1 and receives the stored result | `cross-app-compose-turn.test.ts`: Desktop retry, one server entry, matching Desktop/mobile state |
| ACK lost before apply | Mutation stays in the durable client queue; no server call/effect exists; reconnect applies it once | same corpus: offline mobile enqueue/reconnect, one push, matching clients |
| Exact duplicate before commit | Per-client ledger lock serializes concurrent identical pushes | `runtime-mutators.test.ts`: one `applied,applied`, one `duplicate,duplicate`, one intent and turn |
| Exact duplicate after commit | The mutation ledger returns `duplicate` without re-execution | server duplicate replay fixture, one turn |
| Same semantic command under a fresh mutation ID | Byte-equivalent intent identity returns `applied` without reinsertion | conversation admission/semantic retry fixture, one intent |
| Conflicting same ID | Canonical-byte mismatch returns `runtime_intent_conflict` | server conflict fixtures; no state mutation |
| Offline reconnect before deadline | Pending envelope retains the exact command bytes and ID | shared queue/reconnect corpus |
| Offline reconnect after deadline | Server writes `expired`; creates no turn; dispatch reader returns no work | server-clock expiry fixture and migration 0061 |
| Adapter-visible expiry | Exact projected intent/version becomes a bounded terminal error | Desktop renderer and mobile conversation adapter fixtures |
| Restart after terminal result | Confirmed SQLite row reconstructs the same `expired` result/version | shared runtime-command reader restart fixture |

The retry scheduler in the cross-client corpus uses injected microtask yields;
none of these proofs waits on wall-clock sleeps.

## Invariants and migration

Migration `0061_runtime_control_intent_expiry.sql` extends the existing control-
intent status constraint with `expired`. It does not add a table or dispatch
lane. `INVARIANTS.md` now records that:

1. exact retries cannot repeat an effect;
2. conflicting same-ID bytes cannot mutate state; and
3. a server-clock-expired intent remains durable but dispatch-ineligible.

The Fable Pylon streamlining boundary governed the slice. No file under
`apps/pylon/src/orchestration` or `apps/pylon/src/node` changed; the dispatch
exclusion belongs to the existing Khala Sync reader before Pylon sees work.

## Verification

Focused acceptance passed:

```bash
bun test \
  packages/agent-runtime-schema/src/index.test.ts \
  packages/khala-sync-client/src/runtime.test.ts \
  packages/khala-sync-client/src/cross-app-compose-turn.test.ts \
  packages/khala-sync-server/src/runtime-mutators.test.ts \
  packages/khala-sync-server/src/runtime-intents.test.ts \
  apps/openagents-desktop/tests/runtime-gateway.e2e.test.ts \
  apps/openagents-desktop/src/renderer/runtime-conversation.test.ts \
  apps/openagents-mobile/tests/mobile-conversation.test.ts

bun run --filter @openagentsinc/agent-runtime-schema typecheck
bun run --filter @openagentsinc/khala-sync typecheck
bun run --filter @openagentsinc/khala-sync-client typecheck
bun run --filter @openagentsinc/khala-sync-server typecheck
bun run --filter @openagentsinc/openagents-desktop typecheck
bun run --filter @openagentsinc/openagents-mobile typecheck
```

The full `bun run check:deploy` gate passed: architecture/security/contract
guards, Pylon adversarial checks, pending-migration guards, 156 Khala Sync
client tests (three explicitly gated live-smoke skips), 21 web files / 545
tests, and 18 Worker files / 261 tests were green. The drift-guard suite emits
intentional failure diagnostics from its negative fixtures while the suite
itself passes.

## Boundary after acceptance

CUT-07 closes with this receipt. Cursor gaps, event reordering, snapshot
replacement, and store compatibility remain CUT-08 [#8688]. Process/renderer
restart during active streams, stale runtime generations, revocation, and
interrupted finalization remain CUT-09 [#8689]. Parent #8677 stays open until
those leaves and its live network-gap/restart rung complete.
