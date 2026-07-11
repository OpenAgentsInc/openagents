# P0 PROOF R4-A: conversation command and event fault convergence

- Issue: #8677
- Parent: #8566
- Consumers: #8574 and #8597
- Depends on: #8676 D1-H live streamed conversation and mobile continuation
- Priority: before broad D3–D6 surface expansion is accepted
- Child leaves: #8687 lost-ACK/idempotency/offline expiry; #8688 event order/
  cursor/refetch/store compatibility; #8689 restart/stale generation/revocation/
  interrupted finalization

## Outcome

Prove that the shared Desktop/mobile conversation seam converges under the
faults that make agent software feel unpredictable: lost acknowledgement,
duplicate or out-of-order delivery, cursor gap, offline enqueue, renderer or
host restart, stale runtime generation, revocation, migration, and rollback.
Every mutation settles once or remains explicitly pending reconciliation; no
client invents completion from transcript text, socket health, or local cache.

## Scope

- Reuse the existing Khala Sync projection, durable mutation ID, cursor,
  `MustRefetch`, snapshot replacement, and Runtime Gateway contracts. Do not
  create a parallel recovery protocol.
- Add a deterministic fault matrix at the canonical request processor and both
  native adapters. Convert every counterexample into a regression fixture and
  a bounded user-visible state.
- Exercise one public-safe live network-gap/restart receipt after deterministic
  fixtures pass.
- Use Effect `TestClock`/deterministic services for lease, retry, expiry,
  reconnect, debounce, and cleanup behavior. Wall-clock sleeps are not proof.
- Verify interruption/finalization separately from typed domain failure.

## Fault matrix

Rows 1–3 and 6 are owned by #8687; rows 4–5 and 10 by #8688; rows 7–9 and the
live network-gap/restart receipt by #8689. #8677 remains the acceptance parent
and closes only after all three children and the complete matrix pass.

1. acknowledgement lost after durable apply;
2. acknowledgement lost before apply;
3. exact duplicate command and conflicting same-ID command;
4. duplicate, delayed, and out-of-order event frames;
5. cursor gap and explicit must-refetch/snapshot replacement;
6. offline enqueue, reconnect, and command expiry;
7. renderer restart during stream and host restart during pending work;
8. stale runtime/worker generation after reconnect;
9. account/device revocation while read and mutation are in flight;
10. persisted-store migration/rollback and incompatible-version refusal.

## Acceptance

- Each matrix row names the authoritative state transition and passes through
  Desktop and mobile adapters with matching refs/versions.
- Exactly one durable effect exists after every exact retry; conflicting reuse
  is rejected without mutating state.
- A gap or incompatible snapshot never silently resumes live delivery; the
  client visibly refetches or refuses.
- Offline and unknown-pending work remains distinguishable from accepted,
  rejected, failed, expired, or canceled.
- Cancellation interrupts owned fibers and runs resource finalizers once.
- Restart and migration do not reopen terminal work, duplicate provider calls,
  or retain revoked authority.
- One real network interruption/restart receipt confirms the deterministic
  model at the live proof rung without exposing private payloads.

## Non-goals

- unrelated Fleet, workroom, payment, or public-web fault breadth;
- generalized chaos infrastructure;
- optimistic UX that hides unresolved authority;
- one issue per individual fault after this matrix is complete.

## Close

Close when the complete matrix, app-visible state assertions, and one live
fault receipt pass. New counterexamples after closure become bounded defects,
not a reopened permanent reliability epic.

## Progress — CUT-07 accepted

Rows 1–3 and 6 are complete under
[`2026-07-11-cut-07-command-convergence-receipt.md`](../2026-07-11-cut-07-command-convergence-receipt.md).
The existing Sync mutation ledger now converges exact retries before and after
commit; conflicting same-ID bytes reject without mutation; a server-clock-due
control intent projects once as `expired` and is excluded from dispatch; and
Desktop/mobile read the same stable intent result after reconnect/restart.
CUT-08 owns rows 4–5 and 10; their subsequent acceptance is recorded below.
This parent remains open.

## Progress — CUT-08 accepted

Rows 4–5 and 10 are complete under
[`2026-07-11-cut-08-event-store-convergence-receipt.md`](../2026-07-11-cut-08-event-store-convergence-receipt.md).
Advancing live/log batches now prove a dense scope-version interval, sparse
live delivery replays from the durable cursor, retained-window loss keeps the
existing MustRefetch/snapshot path, Desktop/mobile converge on one injected
timeline trace, and supported-legacy versus future-incompatible stores have
explicit migration/refusal receipts across Bun, Desktop, Expo, and Web. CUT-09
owns rows 7–9 and the live fault rung next. This parent remains open.
