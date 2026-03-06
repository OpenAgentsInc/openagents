# Autopilot Earn Guide (Work In Progress)

This guide explains how earning works in OpenAgents today, what to expect, and what is still under active development.

Status: **Work in progress (WIP)**.  
Behavior, UI, and operator defaults may change as the MVP hardens.

Current implementation status authority: `AUTOPILOT_EARN_MVP_EPIC_TRACKER.md`.

## Docs In This Directory

Use this directory as the consolidated entry point for Autopilot Earn docs.

Canonical docs:

- `README.md`: user-facing guide and current behavior summary.
- `AUTOPILOT_EARN_MVP.md`: compute-lane MVP product/spec cut.
- `AUTOPILOT_EARN_MVP_EPIC_TRACKER.md`: canonical current implementation status.

Operational and verification docs:

- `AUTOPILOT_EARN_MVP_TEST_HARNESS.md`: programmatic coverage and harness expectations.
- `AUTOPILOT_EARNINGS_OPERATOR_RUNBOOK.md`: operator incident and payout-correctness procedures.
- `AUTOPILOT_EARNINGS_ROLLOUT_PLAN.md`: staged rollout and rollback gates.
- `AUTOPILOT_EARNINGS_AUTOMATION.md`: goal automation and scheduler flow.
- `AUTOPILOT_EARN_RECIPROCAL_LOOP_RUNBOOK.md`: bilateral/local loop verification.
- `AUTOPILOT_EARN_RUNTIME_PLACEMENT_DECISION.md`: runtime ownership and extraction posture.

Historical appendices:

- `AUTOPILOT_EARN_MVP_IMPLEMENTATION_LOG.md`
- `AUTOPILOT_EARN_BACKROOM_HARVEST_AUDIT.md`
- `AUTOPILOT_EARN_BACKROOM_PROVENANCE.md`

## Revenue Lanes (Canonical Model)

Autopilot Earn is a provider marketplace with multiple revenue lanes, not a single job type.

1. **Compute Provider (MVP now)**  
   Execute paid NIP-90 jobs and earn sats on wallet-confirmed settlement.
2. **Liquidity Solver (future lane)**  
   Fill Hydra liquidity intents with capital + execution and earn routing fees/spreads.

Future lanes (for example data/storage providers) are possible, but not part of current MVP delivery.

## Current MVP Lane: Compute Provider

When you turn provider mode online in the current MVP, your desktop can accept paid NIP-90 jobs, execute supported tasks locally, and receive sats through the Spark wallet lane.

All provider earnings land in the built-in Spark wallet first. MVP does not include a way to configure an external receive invoice for provider payouts; moving funds to another wallet happens through withdrawal.

By default, the desktop should use the OpenAgents-hosted Nexus as its primary Nostr relay path, with additional relays configured as backup for resilience. Advanced users and organizations should be able to point the app at their own Nexus deployment instead.

At first, the starter-jobs / seed-demand bootstrap comes only from the OpenAgents-hosted Nexus. A self-hosted Nexus should still operate on the open marketplace path by default, but should not be assumed to include OpenAgents-funded starter demand. Closed/private Nexus modes are later roadmap work. The OpenAgents-hosted Nexus remains anon/open for general participation, but OpenAgents starter jobs target Autopilot users only and are available only when the provider is connected to the OpenAgents-hosted Nexus itself.

That targeting should be enforced from OpenAgents-hosted-Nexus session/auth evidence where possible, not from optional Nostr `client` tags alone. Stronger anti-spoofing attestation can be added later as hardening work after MVP.

Recommended first-time product sequence:

1. Hand the user into Mission Control with a clear `GO ONLINE` prompt immediately.
2. Get the first paid job and celebrate the first wallet-confirmed sats.
3. Use task success as reinforcement later, not as a prerequisite for earning.

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

Liquidity solver jobs are an OpenAgents-native market. We do not rely on third-party solver networks.

Important constraint: solver mode is **never auto-enabled**.  
It requires explicit user opt-in because it introduces capital commitment and route-risk policy decisions.

## What `GO ONLINE` Means

- **MVP behavior:** available to earn via compute jobs.
- **Future behavior:** available to earn via any enabled provider modules (compute first, liquidity solver only when explicitly enabled).

## Runtime vs Kernel

- `OpenAgents Runtime`: the provider-side execution environment in the desktop app where jobs run, local state advances, and provenance is produced.
- `OpenAgents Kernel`: the authority layer that verifies outcomes, settles value, and emits canonical receipts.

In the current MVP, the desktop embeds the runtime. Kernel authority is represented in the product by receipt/reconciliation semantics today, while the full server-side kernel remains planned infrastructure.

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

- Product authority: [../MVP.md](../MVP.md)
- Canonical implementation status: [AUTOPILOT_EARN_MVP_EPIC_TRACKER.md](AUTOPILOT_EARN_MVP_EPIC_TRACKER.md)
- Mission/control implementation spec: [AUTOPILOT_EARN_MVP.md](AUTOPILOT_EARN_MVP.md)
- Programmatic harness coverage: [AUTOPILOT_EARN_MVP_TEST_HARNESS.md](AUTOPILOT_EARN_MVP_TEST_HARNESS.md)
- Operator runbook: [AUTOPILOT_EARNINGS_OPERATOR_RUNBOOK.md](AUTOPILOT_EARNINGS_OPERATOR_RUNBOOK.md)
- Rollout plan: [AUTOPILOT_EARNINGS_ROLLOUT_PLAN.md](AUTOPILOT_EARNINGS_ROLLOUT_PLAN.md)
