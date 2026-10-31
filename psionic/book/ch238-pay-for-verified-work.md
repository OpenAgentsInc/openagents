# 238. Pay for verified work

> **Status: unimplemented — design draft.** Nothing here runs yet.

Part XXIII ends with work that someone can pay for. A run pays its
contributors for traces that a validator has replayed exactly. This chapter
plans that run with real payments only, under owner caps, settled in Bitcoin
through the wallet outside Psionic.

## What this chapter covers

- Worker admission. A worker declares its capability only after a
  digest-checked self-test.
- Separate devices. The worker and the validator run on different devices, and
  both are paid.
- Validation by [Chapter 234](ch234-exact-replay-as-a-verification-class.md)'s exact replay. Only an exact replay verdict earns a
  payment.
- Receipts before settlement. Each verified pair produces a receipt, and
  settlement reads receipts.
- Caps. Per-payout and daily caps require owner approval.
- Settlement in Bitcoin through the wallet, outside Psionic.
- Honest counting. Totals include only real, verified payments. A simulated
  payment never counts.

The pre-reset run settled 1,000 sats over Spark, then 5 sats each to worker
and validator per verified pair. It verified only one trivial program. This
chapter keeps its self-test, validator separation, caps and receipt-first
settlement, and requires real programs.

## Dependencies

- [Chapter 226](ch226-a-machine-whose-state-is-a-trace.md) through [Chapter 237](ch237-composable-weight-modules.md): the trace machine, compiled modules and
  replay verification.
- [Chapter 24](ch024-a-pylon-answers-decisions.md), [Chapter 25](ch025-the-api-fans-out.md) and [Chapter 26](ch026-check-the-mesh.md): Pylons, routing and receipts.
- [Chapter 195](ch195-check-a-peers-work.md): checking a peer's work.
- Owner decisions on caps and payments, recorded in `NEEDS_OWNER.md`.

## Done means

- A worker that fails its self-test cannot declare a capability.
- A tampered trace fails replay and earns no payment.
- Every payment has a receipt that names the verified pair, and no payment
  exceeds its caps.
- The reported total counts only settled payments for verified work on
  programs beyond a trivial fixture.

## Sources

- [Chapter plan](PLAN.md), Part XXIII.
- [Exact computation notes](../docs/exact-computation.md).
