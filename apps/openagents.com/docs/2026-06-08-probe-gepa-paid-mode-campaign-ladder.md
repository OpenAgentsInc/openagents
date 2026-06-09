# Probe GEPA Paid-Mode Campaign Ladder

Date: 2026-06-08

Issue: [#566](https://github.com/OpenAgentsInc/openagents/issues/566)

## Launch Decision

Probe GEPA may claim paid Pylon campaign work only after a campaign-level
ladder has cleared every step:

1. Stage 0 no-spend campaign projection is green.
2. Settlement readiness passes for `unpaid_smoke`.
3. Settlement readiness passes for `payable_pending_settlement`.
4. Settlement readiness passes for `settled_bitcoin`.
5. Payment receipt refs exist for payable work.
6. Settlement receipt refs exist for settled-bitcoin work.
7. Payer wallet send-readiness and outbound liquidity preflight refs exist.
8. A live-small-sats smoke ref exists.
9. Duplicate bridge attempts are replay-safe and do not mint new receipt refs.

The implementation lives in
`workers/api/src/probe-gepa-paid-mode-ladder.ts`.

## Projection

The projection exposes:

- aggregate campaign payment mode;
- per-assignment payment mode;
- payment receipt refs;
- public settlement receipt refs for copy;
- readiness decision refs;
- bridge attempt refs;
- send-readiness state;
- duplicate-replay safety state;
- blocker refs.

`settled_bitcoin_ready` is the only state that allows a settled-bitcoin
campaign claim. `payable_pending_settlement_ready` allows payable-work copy,
but still blocks settled-bitcoin copy.

## Guards

The ladder does not dispatch payments. It is a public-safe claim gate above the
assignment lifecycle and settlement-readiness gate.

It blocks:

- missing Stage 0 no-spend evidence;
- missing unpaid, payable, or settled readiness results;
- payment claims without payment receipt refs;
- settled-bitcoin claims without settlement receipt refs;
- send attempts without wallet send-readiness or outbound liquidity refs;
- duplicate settlement attempts for the same assignment;
- duplicate replay attempts that create fresh receipt refs;
- private payment, wallet, preimage, provider, local filesystem, or secret
  material in public refs.

## Current Gap

OpenAgents product surface now has the repeatable paid-mode ladder model and tests. This still does
not mean a live continuous paid GEPA campaign has run. The next live step is an
operator-approved small-sats campaign that produces the public-safe payment and
settlement receipt refs consumed by this gate.

## Verification

Run:

```sh
bun run --cwd workers/api test -- src/probe-gepa-paid-mode-ladder.test.ts src/probe-gepa-settlement-readiness.test.ts src/probe-gepa-stage0-no-spend-campaign.test.ts src/pylon-gepa-metric-call-assignments.test.ts
```
