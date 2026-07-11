# CUT-09 lifecycle convergence receipt

- Date: 2026-07-11
- Issue: [#8689](https://github.com/OpenAgentsInc/openagents/issues/8689)
- Parent: [#8677](https://github.com/OpenAgentsInc/openagents/issues/8677)
- State: deterministic matrix landed; physical-device live rung pending
- Scope: restart, stale generation, revocation, interrupted finalization

## Result

The deterministic lifecycle matrix now converges through the canonical Khala
Sync client/server path and both native SQLite adapters:

1. A delayed bootstrap or log response is bound to the scope generation that
   requested it. Unsubscribe, close, or revocation invalidates the response
   before it can replace/advance durable state.
2. A push response arriving after revocation cannot publish a rejection or
   acknowledgement into the closed authority generation.
3. `runtime.recordEvent` accepts only the exact durable next `event_count` and
   a lifecycle-valid transition. Only `turn.started` leaves `queued`; a second
   start cannot mutate `running`; no event can mutate a completed, failed,
   interrupted, or closed turn.
4. Each hosted-runtime tick first finds stale `running` hosted turns and writes
   one `turn.interrupted` at the exact next sequence. It never requeues or
   re-invokes the provider. The unique sequence plus turn lock serializes that
   terminal against a late original worker.
5. If the recovery terminal wins, the original worker stops after its first
   rejected write. It cannot append later text, usage, or a false success.
6. Desktop SQLite and Expo SQLite close/reopen the same partially streamed
   timeline, accept one exact interrupted terminal idempotently, and expose
   matching refs, sequence, cursor, and canceled state. Proven unlink clears
   the hosted personal/thread projections and pending queue in both adapters.

This preserves the Fable streamlining boundary: no file under
`apps/pylon/src/orchestration` or `apps/pylon/src/node` changed. The fixes live
at the shared Sync authority, hosted server consumer, and native adapters.

## Counterexamples converted to regressions

- Generation 1 captures a v1 snapshot, is unsubscribed, and resolves only
  after generation 2 is live at v2. The durable cursor stays v2 and its v2 row
  remains visible.
- A running turn is interrupted, then receives matching-next-sequence stale
  text and a later sequence gap. Both reject before projection; the turn stays
  interrupted with one recorded event.
- A hosted worker's provider completion loses the next-sequence race to
  recovery. The first rejected `text.delta` stops all subsequent writes.
- Desktop/mobile restart with partial output reconstructs the same partial
  history, then the same single interrupted terminal; exact replay adds no
  duplicate output.
- Desktop/mobile unlink after cross-device restart leaves both hosted scopes
  and durable mutation queues empty while the immutable device-local identity
  remains outside the revoked hosted authority.

## Deterministic verification

Focused suites pass with injected scheduling/clock seams and real local
Postgres/SQLite where the authority boundary requires them:

- shared Sync session generation/revocation suite;
- canonical runtime mutator suite against local Postgres;
- hosted-runtime dispatch unit and reply-guard suites;
- real Desktop SQLite + Expo SQLite continuation/restart/revocation suites;
- Desktop/mobile conversation adapter suites;
- package typechecks for Sync client/server, Worker, Desktop, and mobile.

The combined focused run reported 55 Bun cases / 298 expectations plus 26
Worker Vitest cases: 81 focused cases, zero failures.

The built Electron smoke passes Runtime Gateway protocol v7 bootstrap,
operation correlation (`ipc.received` → `gateway.received` → `sync.intent` →
`ipc.returned`), trace navigation/reload restoration, and zero-active-slot
teardown.

The full `bun run check:deploy` gate also passes. Its shared Khala Sync client
corpus reported 163 pass / 3 explicitly gated live-smoke skips / 12,691
expectations with import coverage green. Expected `FAILED` strings emitted by
negative drift-guard fixtures are self-test diagnostics; the enclosing gate
exited successfully.

## Live receipt status

The required built-Desktop plus physical-mobile network-gap/restart receipt is
not yet claimed. On 2026-07-11 the paired physical iPhone was offline in both
Tailnet discovery and Xcode device discovery. The built Desktop rung is green,
but simulator/fixture evidence is intentionally not substituted for the phone.

When the phone is available, run the public-safe procedure in
[`native-streamed-conversation-handoff.md`](./issues/native-streamed-conversation-handoff.md):
reload the renderer during one active run, restart the host after settlement,
continue or interrupt the exact confirmed run on physical mobile, then queue a
harmless offline command and prove unlink/revocation removes it without replay.
Record only build/commit, platform versions, isolated account label, hashed
refs, event-kind counts, terminal/reconciliation verdicts, and restart/revoke
verdicts—never prompt/output text, owner/device identifiers, paths, provider
metadata, raw rows/events, or credentials.

## Close decision

#8689 and #8677 remain open. Deterministic rows 7–9 are implemented, but their
explicit live physical-device acceptance row is still pending.
