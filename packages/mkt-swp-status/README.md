# @openagentsinc/mkt-swp-status

Status and progress against per-signer NIP-MKT Status (openagents#9321,
SWAP-6): the headless projection behind the swap progress view — gaps,
forks, rungs.

This is where the product is structurally more honest than a centralized
exchange. Boltz's stream is one authority reporting one of twenty strings;
its page is a component switch with a spinner. NIP-MKT Status is per-signer
signed evidence: each author has its own `seq`, `previous` references chain
the stream, and the session projection retains both participant lanes.

## What lives here

- `model.ts` — the typed vocabulary: `StatusClaim` (always authored),
  `SwapEvidence` (always attributed to its verifying source), the §11
  authority caps, `CloseRecord` with §15 loss accounting.
- `states.ts` — the §9 tables as data: swp_state → base-state derivation
  (a value matching no row is `swp_status_transition_invalid`;
  `contract_pending`/`contract_bound` are local projections a claim can
  never establish), the per-flow signer map, and the transition edges for
  submarine, reverse, and chain flows.
- `lane.ts` — the per-signer fold. A missing sequence is a visible gap
  (`swp_status_gap`) that only the exact missing record can close; two
  records at one `(session, order, seq)` are a fork (`swp_status_fork`)
  with both retained and no arrival-time winner; the fold is a function of
  the record SET, so out-of-order arrival converges and duplicates are
  idempotent.
- `rungs.ts` — `pledged → reserved → measured → verified → paid → settled`,
  rendering the narrowest rung the exact evidence proves. A status caps at
  `pledged`; a relay observation at `measured`. A claim exceeding its
  evidence is `swp_settlement_overclaim` — a status alone never renders as
  settled or complete.
- `ladder.ts` — the §8 timeout ladders as rungs with a stated user exit at
  each boundary. Heights are the authority; times are flagged estimates.
  Crossing into the user's unilateral window stops trusting counterparty
  claims for progress.
- `terminal.ts` — terminality has ONE definition (`isWatchTerminal`),
  fixing the Boltz two-definitions divergence. Every terminal outcome
  states a user exit; `unresolved` displays as unresolved, never failed or
  complete; loss accounting keeps fees out of principal and renders
  unknown values as unknown, never zero. Conflicting Closes both remain
  visible.
- `session.ts` — the exported session projection: two lanes folded through
  the state machine, signer discipline (`swp_status_signer_invalid`),
  evidence-gated advancement, and the headline display.
- `reconnect.ts` — the socket discipline from the teardown as pure policy:
  exponential jittered backoff, ping-based half-open detection, stability
  reset, degrade-to-polling, change-only emission. Missed events are
  covered by resubscription replay into the idempotent fold.
- `messages.ts` — local `swap.status.*` copy; §17 identifiers render
  through `@openagentsinc/swap-i18n` and never as counterparty prose.

Behaviour contracts (gap rendering, fork rendering, never-infer-a-rung-
upward) are registered in `@openagentsinc/behavior-contracts`
(`market-swap-status`) with oracles in `session.test.ts`, which also
carries the adversarial status-sequence table (gap, fork, duplicate,
out-of-order, expiry-crossing, unresolved, conflicting Closes, reconnect
replay, chain breaks).

Everything here is projection and rendering truth. Signature, grammar, and
verify-before-fund truth belong to the MKT-SWP engine behind the SWAP-0
boundary; wallet and rail authority stay outside per MKT-SWP §11.

## Checks

```
pnpm --dir packages/mkt-swp-status run check
```
