---
description: Build on OpenAgents — run a Pylon, ship a Data Market handler, earn a developer bounty.
---

[Home](../README.md) · **Developer Path**

# For Developers

> _"We've paid more Bitcoin, we've paid more anything to developers than every other AI lab combined."_
>
> — Christopher David, [Bitcoinfi Demo Day](../assets/clips/cdavid-demoday-highlight-90s.mp4)

This pathway is for developers who want to build on OpenAgents — run a Pylon, integrate with the Economy Kernel, ship a Data Market handler, or pick up a paid developer bounty.

{% hint style="warning" %}
**This pathway is under construction.** The outline below is finalized and each section will be filled in ahead of Bitcoin Vegas 2026. The [Investor Path](../investors/README.md) is complete today and includes many of the same technical anchors. Follow along at [openagents.substack.com](https://openagents.substack.com) or [@OpenAgentsInc on YouTube](https://www.youtube.com/@OpenAgentsInc).
{% endhint %}

## What you can do today

Even while this path is being written, three things are already actionable:

1. **Install Pylon.** `npx @openagentsinc/pylon@0.1.13 --help`. Source + docs in [`OpenAgentsInc/openagents`](https://github.com/OpenAgentsInc/openagents).
2. **Publish a NIP-90 Data Market handler** — the current relays are `wss://relay.damus.io` and `wss://relay.primal.net`; the spec lives in [`packages/data-market-mvp/README.md`](https://github.com/OpenAgentsInc/openagents/blob/main/packages/data-market-mvp/README.md). Kinds in use: **5960** (request), **6960** (result), **31990** (handler/capability).
3. **Pick up a developer bounty.** Bounties are announced on [OAPN](https://openagents.substack.com) (see episode #6, _Pay the People_).

## Path outline

### Part 1 — Quickstart

1. **Install Pylon** — npm path, version compatibility, homing, config.
2. **Run a local Nexus** — the hosted Nexus is fine for starter work; running your own is one branch away.
3. **Your first 25 sats** — the operator runbook boiled down to the minimum working example.

### Part 2 — Provider-side integrations

4. **Paid training lanes** — CS336 A1, how workload dispatch works, how to add a new lane.
5. **Psionic runtimes** — what the edge ML framework gives you, how to wire in new inference and training code.
6. **Validator patterns** — the separate-validator-Pylon model, artifact refresh, retry semantics.

### Part 3 — Data Market (NIP-90)

7. **Kinds 5960 / 6960 / 31990 end-to-end** — request, result, handler advertisement.
8. **Handler registration and discovery** — publishing a kind 31990 capability event.
9. **Payment flows** — Lightning escrow, zap-on-delivery, receipts.

### Part 4 — Economy Kernel

10. **The `sv` control loop** — verification, receipts, policy, autonomy throttle.
11. **Kernel Authority API** — how authorized command lanes work.
12. **Receipt reconciliation** — matching kernel payout ids against wallet history (see the [2026-04-23 proof](../investors/09-proof-receipts.md)).

### Part 5 — Labor Market & bounties

13. **How developer bounties work** — reactivated on OAPN #6.
14. **Forge + Probe pattern** — the in-house software factory, as described on OAPN #6.
15. **Contributing upstream** — repo conventions, PR review, what "paid merge" looks like.

### Part 6 — Deployment

16. **Packaging and releasing a Pylon build** — what the release-receipt discipline looks like.
17. **Self-hosted Nexus + relay set** — running OpenAgents infrastructure you fully own.
18. **Monitoring and observability** — Nexus reports in [`docs/reports/`](https://github.com/OpenAgentsInc/openagents/tree/main/docs/reports).

## Reference

- Upstream repo: [`OpenAgentsInc/openagents`](https://github.com/OpenAgentsInc/openagents)
- Architecture decisions: [`docs/adr/`](https://github.com/OpenAgentsInc/openagents/tree/main/docs/adr) — start with [ADR-0001 Authority Boundaries](https://github.com/OpenAgentsInc/openagents/blob/main/docs/adr/0001-authority-boundaries.md)
- Proof receipts: [`docs/reports/`](https://github.com/OpenAgentsInc/openagents/tree/main/docs/reports)
- Glossary: [Compute Market](../shared/glossary/compute-market.md) · [Risk Market](../shared/glossary/risk-market.md) · [TreasuryRouter](../shared/glossary/treasury-router.md)

---

**← Previous:** [Home](../README.md) · **Next:** [User Path](../users/README.md) **→**
