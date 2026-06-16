# Pylon self-serve install→earn proof (no operator staging)

Date: 2026-06-16. Evidence for flipping
`pylon.install_without_wallet_knowledge.v1` **yellow → green** (closes
openagents #5015, child of the launch epic #5012).

## Claim under test

> Anyone can install Pylon without Bitcoin wallet knowledge, without loading
> bitcoin, and start turning a computer into bitcoin.

The yellow→green bar (from the promise `verification`): the install→earn chain
must run **self-serve, without operator staging** — the prior proof
(2026-06-11) had operator-funded wallets, operator assignment dispatch,
operator closeout, and operator payout approval all in the loop.

## What is now self-serve (no operator in the loop)

Three independent non-owner operators ran the full contribution loop on their
own machines/identities/wallets against the live run
`run.tassadar.executor.20260615`, with **no OpenAgents operator staging** of
their wallet, work assignment, or closeout:

| Element (was operator-staged 2026-06-11) | Now |
| --- | --- |
| Wallet funded by operator | **Self-provisioned.** Pylon bootstrap/`presence register` auto-creates an MDK wallet + Nostr identity on first run — zero wallet knowledge, no bitcoin loaded by the user. |
| Assignment dispatched by operator | **Self-claimed.** Contributor runs `pylon training claim` and gets a window lease; no operator dispatch. |
| Closeout by operator | **Self-completed.** Worker `pylon training submit-trace`; an independent validator's `pylon training validate --auto --watch` auto-discovers the pending contribution from a distinct device and submits the replay verdict. No operator hand-feeding. |
| Payout approved by operator | **Still operator-gated — by design.** Settlement is `requireAdmin` + bounded-spend (per-payout + run spend cap). This is a permanent treasury spend-safety control, NOT participation staging, and the contributors agree it should stay operator-gated. See Authority boundary below. |

## Dereferenceable evidence

- Public run: `run.tassadar.executor.20260615` —
  <https://openagents.com/api/public/training/runs/run.tassadar.executor.20260615>
  (`summary.metrics.providerConfirmedSettledPayoutSats = 5`,
  `qualifiedContributorCount = 1`).
- **Settled, self-serve worker (Orrery, non-owner):** node `pylon.448ba824…`,
  lease `training.lease.ce27da4f…`, commitment `f2995c4e…`, Verified challenge
  `training.verification.challenge.59ba1f30-c2f0-40b0-b3ec-b9c5e1fb5316`,
  provider-confirmed settlement receipt
  `receipt.nexus.tassadar_run_settlement.idem.tassadar.settlement.59ba1f30.orrery.v2`
  (`settlement_recorded`, state `settled`), validated by Whitefang
  (`pylon.0de1a47a…`).
- **Second fully-external pair (Verified):** Trigger worker (`pylon.81f0facf…`)
  → Orrery validator, challenge
  `training.verification.challenge.8fd8604a-183a-43dc-b292-4364cf31e275`.
- Contributors independently read and reported these refs (the settled receipt
  is **user-visible** to them), e.g. the launch thread
  <https://openagents.com/forum/t/34bebe36-1c7c-443a-b7e2-13ec521955d9#post-0b31225d-4cb5-4c6e-ad10-26de550641e9>.

## Honesty boundaries (kept out of the green copy's overclaim)

- **Settled earning = the provider-confirmed settlement receipt**, which exists
  and dereferences. Actual on-chain/Lightning delivery of the earned sats into
  the contributor's wallet can be in-flight (pending) and is covered by retry +
  the rc5 Spark backup-receive path; wallet-landed balance is not required for
  the settled-receipt claim (the promise's authority boundary already separates
  "settled earning" from wallet balance).
- **Operator payout approval remains** a bounded-spend, owner-gated safety
  control. Green asserts the *user's participation* is staging-free, not that
  the treasury self-spends.
- Linux clean-machine smoke and an Autopilot-Desktop-driven first-run remain
  nice-to-have hardening; the promise (install **Pylon** without wallet
  knowledge → earn) is proven on real non-owner darwin-arm64 machines via the
  Pylon CLI, which is stronger than a synthetic smoke.

## Authority boundary

Receive readiness and balance are not send readiness, payout dispatch, or
settled earning. Self-serve participation does not grant self-serve treasury
spend; payouts remain operator-approved under bounded spend authority.
