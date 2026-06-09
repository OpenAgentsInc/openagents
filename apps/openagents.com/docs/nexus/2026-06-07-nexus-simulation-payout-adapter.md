# Nexus Simulation Payout Adapter

Status: implemented for the OpenAgents product surface/Nexus rebuild.

The simulation payout adapter is the first green gate before OpenAgents product surface moves real
bitcoin through MDK. It runs behind the `TreasuryPaymentAuthority` service, so
the same authority gates apply before any adapter behavior can run:

- accepted work evidence is required;
- payout target approval is required;
- wallet readiness must be fresh;
- spend amount must be within the spend cap;
- authority and adapter pause states block dispatch;
- payout dispatch requires an existing payout intent;
- replayed payout attempt idempotency keys return the existing attempt.

The adapter is deterministic under test and lives at
`workers/api/src/treasury-payment-simulation-adapter.ts`.

## Simulated States

The adapter can simulate:

- dispatch accepted;
- dispatch rejected;
- confirmation pending;
- confirmation succeeded;
- confirmation failed;
- reconciliation found duplicate;
- reconciliation found stale pending attempt.

These states are policy and receipt exercises only. A simulation receipt is not
proof that bitcoin moved. It proves that OpenAgents product surface's payout authority, idempotency,
projection, and reconciliation paths behaved as expected without calling MDK or
any wallet daemon.

## Conformance Suite

The shared conformance harness lives at
`workers/api/src/treasury-payment-adapter-conformance.test-support.ts`.

The simulation adapter currently passes the suite in
`workers/api/src/treasury-payment-simulation-adapter.test.ts`. The future MDK
agent-wallet adapter must pass the same suite with mocked MDK command output
before any live-wallet smoke is attempted.

The suite covers:

- preview through `TreasuryPaymentAuthority`;
- intent creation before dispatch;
- accepted dispatch;
- rejected dispatch;
- idempotent dispatch replay;
- pending, succeeded, failed, duplicate, and stale-pending reconciliation;
- spend cap, target approval, wallet readiness, pause, and missing-intent
  policy gates.

## Receipt Projection

The simulation adapter can build public-safe receipt records for:

- dispatch recorded;
- confirmation recorded;
- verification recorded;
- settlement recorded.

Every generated public projection includes:

```json
{
  "adapter": "simulation",
  "moneyMovement": "none",
  "policyProofOnly": true,
  "simulation": true
}
```

This distinction must remain visible anywhere simulation receipts are displayed
or exported. Production copy must not imply that a simulation receipt proves
real bitcoin settlement.

## Verification

Current verification commands:

```bash
bun run --cwd workers/api test -- src/nexus-treasury-payout-ledger.test.ts src/treasury-payment-authority.test.ts src/treasury-payment-simulation-adapter.test.ts
bun run --cwd workers/api typecheck
```
