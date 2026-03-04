# Autopilot Earn Guide (Work In Progress)

This guide explains how earning works in OpenAgents today, what to expect, and what is still under active development.

Status: **Work in progress (WIP)**.  
Behavior, UI, and operator defaults may change as the MVP hardens.

## Revenue Lanes (Canonical Model)

Autopilot Earn is a provider marketplace with multiple revenue lanes, not a single job type.

1. **Compute Provider (MVP now)**  
   Execute paid NIP-90 jobs and earn sats on wallet-confirmed settlement.
2. **Liquidity Solver (future lane)**  
   Fill Hydra liquidity intents with capital + execution and earn routing fees/spreads.

Future lanes (for example data/storage providers) are possible, but not part of current MVP delivery.

## Current MVP Lane: Compute Provider

When you turn provider mode online in the current MVP, your desktop can accept paid NIP-90 jobs, execute supported tasks locally, and receive sats through the Spark wallet lane.

The MVP success condition is:

1. Go online
2. Receive paid job
3. Complete job
4. See wallet-confirmed sats
5. Withdraw over Lightning

### Current MVP Flow

1. Open desktop app.
2. Verify wallet is connected.
3. Click the Mission Control `GO ONLINE` action.
4. Wait for a job to appear in recent jobs / active job lanes.
5. Confirm the job reaches terminal state and payout is wallet-confirmed.
6. Use pay-invoice flow to verify sats can be moved out.

## Future Lane: Liquidity Solver

Planned Hydra lane example:

1. Hydra publishes a liquidity intent.
2. A solver accepts and executes the route/swap.
3. Solver earns fee/spread when settlement completes.

Important constraint: solver mode is **never auto-enabled**.  
It requires explicit user opt-in because it introduces capital commitment and route-risk policy decisions.

## What `GO ONLINE` Means

- **MVP behavior:** available to earn via compute jobs.
- **Future behavior:** available to earn via any enabled provider modules (compute first, liquidity solver only when explicitly enabled).

## What Is Authoritative

For payout and earnings numbers, authoritative sources are:

- wallet receive evidence in Spark payment history,
- settled job history receipts correlated to wallet payment pointers,
- reconciliation gates that reject synthetic/unconfirmed pointers.

If a job looks completed but has no wallet-confirmed payout evidence, it is not treated as successful earnings.

## Known MVP Constraints

- The earn surface is still evolving rapidly.
- Relay/network conditions may affect time-to-first-job.
- Seed-demand lanes and operator controls are still being tuned.
- Public stats and in-app stats may lag while aggregation paths harden.
- Liquidity solver lane is future scope and remains disabled in MVP.

## Safety / Operational Notes

- Do not assume all incoming jobs are safe by default; run with expected operator policy.
- Keep wallet credentials and identity material private.
- If payout and wallet history diverge, treat it as an incident and follow operator runbooks.

## Related Docs

- Product authority: [MVP.md](MVP.md)
- Mission/control implementation spec: [AUTOPILOT_EARN_MVP.md](AUTOPILOT_EARN_MVP.md)
- Programmatic harness coverage: [AUTOPILOT_EARN_MVP_TEST_HARNESS.md](AUTOPILOT_EARN_MVP_TEST_HARNESS.md)
- Operator runbook: [AUTOPILOT_EARNINGS_OPERATOR_RUNBOOK.md](AUTOPILOT_EARNINGS_OPERATOR_RUNBOOK.md)
- Rollout plan: [AUTOPILOT_EARNINGS_ROLLOUT_PLAN.md](AUTOPILOT_EARNINGS_ROLLOUT_PLAN.md)
