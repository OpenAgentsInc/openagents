# Probe GEPA Settlement Readiness Gate

Date: 2026-06-08

Status: implemented for `OpenAgentsInc/openagents#517`.

OpenAgents product surface now has a typed settlement-readiness gate for Probe GEPA Pylon benchmark
work. The gate is intentionally downstream of the unpaid smoke lifecycle: a
batch can complete as `unpaid_smoke` with no payment or settlement refs, and no
public payout claim. Paid states are blocked until accepted assignment evidence
is also represented in operator accounting records.

The implementation lives in
`workers/api/src/probe-gepa-settlement-readiness.ts`.

## Contract

The gate evaluates:

- accepted Pylon GEPA metric-call assignment records;
- operator accounting records keyed by assignment ref;
- batch-level operator accounting refs;
- the requested payment mode;
- the requested public claim state.

For `operator_credit` and `payable_pending_settlement`, every accepted
assignment must have matching accounting coverage for closeout refs, proof
bundle refs, resource usage refs, and verifier result refs. The accounting
record or assignment must also carry payment or credit receipt refs.

For `settled_bitcoin`, the same accounting and payment receipt requirements
apply, and settlement receipt refs are additionally required before the result
allows any settled-bitcoin claim.

## Public Claim Boundary

The public claim state cannot exceed the requested payment mode:

- `unpaid_smoke` may only claim no-spend completion;
- `operator_credit` may claim recorded operator credit, not payable or settled
  bitcoin;
- `payable_pending_settlement` may claim payable pending settlement, not settled
  bitcoin;
- `settled_bitcoin` may claim settlement only when settlement receipt refs are
  present.

This keeps Artanis and OpenAgents product surface summaries honest while allowing the network to
move from no-spend smoke to paid work once receipts and accounting are stable.

## Verification

Run:

```sh
bun run --cwd workers/api test -- probe-gepa-settlement-readiness.test.ts pylon-gepa-metric-call-assignments.test.ts
```
