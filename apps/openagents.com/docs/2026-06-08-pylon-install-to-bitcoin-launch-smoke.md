# Pylon Install-To-Bitcoin Launch Smoke

Date: 2026-06-08
Related issue: #572

## Purpose

This is the single launch-gate contract for the promised Pylon path:

```text
fresh install
-> registration
-> heartbeat
-> MDK wallet readiness
-> assignment lease
-> accepted-work closeout
-> payment evidence
-> public settlement receipt
-> public projection
```

The smoke is implemented in
`workers/api/src/pylon-install-to-bitcoin-smoke.ts` and covered by
`workers/api/src/pylon-install-to-bitcoin-smoke.test.ts`.

## Modes

`ci_no_spend`

- deterministic CI gate;
- requires install, register, heartbeat, wallet, assignment, closeout, payout
  readiness, and public projection refs;
- keeps payment and settlement as `planned_no_spend`;
- never allows wallet spend or settled-bitcoin claims.

`sandbox_fake_payment`

- accepts fake payment and settlement refs for dry-run evidence retention;
- may prove bundle wiring;
- cannot claim settled bitcoin or real public earning.

`live_small_sats`

- requires an explicit operator approval ref;
- requires `amountSats <= spendCapSats`;
- requires original funded MDK wallet-home mode, not mnemonic-only restore;
- requires payout readiness, payment receipt refs, settlement receipt refs,
  and public projection refs;
- is the only mode that may produce `live_settled_bitcoin_ready` and
  `settledBitcoinClaimAllowed: true`.

## Guards

The smoke blocks on:

- missing fresh install ref;
- missing Pylon registration or public Pylon ref;
- missing heartbeat;
- missing MDK wallet readiness;
- missing assignment ref;
- stale assignment lease;
- missing accepted-work closeout;
- missing payout readiness;
- live spend without operator approval;
- live spend above the explicit spend cap;
- live spend from mnemonic-restore or unknown wallet-home mode;
- missing payment receipt;
- missing settlement receipt;
- missing public projection.

The MDK command boundary is the existing local
`@moneydevkit/agent-wallet@latest` bridge. The smoke delegates wallet-command
planning to `workers/api/src/mdk-agent-wallet-smoke-fixture.ts`, so MDK remains
the payment primitive and its send-readiness guard remains shared with Treasury
adapter coverage.

## Script

The operator-facing checklist script is:

```bash
bun run smoke:pylon:install-to-bitcoin -- --mode ci_no_spend
```

Sandbox wiring:

```bash
bun run smoke:pylon:install-to-bitcoin -- \
  --mode sandbox_fake_payment \
  --payment-ref payment_receipt.public.fake_sandbox.redacted \
  --settlement-ref settlement.public.fake_sandbox.redacted
```

Live small-sats checklist:

```bash
bun run smoke:pylon:install-to-bitcoin -- \
  --mode live_small_sats \
  --amount-sats 1 \
  --spend-cap-sats 10 \
  --operator-approved \
  --wallet-home-mode original_funded_wallet_home \
  --payment-ref payment_receipt.public.live_small_sats.redacted \
  --settlement-ref settlement.public.live_small_sats.receipt_recorded
```

The script does not spend bitcoin or call OpenAgents. It emits the checklist
and retained ref shape. The actual wallet execution remains an operator action
using MDK agent-wallet, with raw invoices, payment hashes, preimages,
mnemonics, wallet paths, and payout targets excluded from the retained bundle.

## Verification

Run:

```bash
bun run --cwd workers/api test -- src/pylon-install-to-bitcoin-smoke.test.ts src/mdk-agent-wallet-smoke-fixture.test.ts src/treasury-payment-mdk-agent-wallet-adapter.test.ts
bun run smoke:pylon:install-to-bitcoin -- --mode ci_no_spend
bun run typecheck:api
git diff --check
```

The full API suite currently has unrelated Forum fixture failures documented in
the issue closeout comments for the June 8 launch-gate batch.
