[Home](../README.md) · [Investor Path](README.md) · **06. Data Market MVP**

# 6. Data Market — A Second Way to Get Paid

> _"The current Data Market is a real secondary MVP slice, not just a spec."_
>
> — [`OpenAgentsInc/openagents` README](https://github.com/OpenAgentsInc/openagents/blob/main/README.md)

**You will learn:**

- Why a Data Market matters
- What a single operator can sell once compute and data both pay
- What we ship today vs. what's roadmap

## The second live market

Compute is market #1. Data is market #2. Both already work. Both already settle in Bitcoin. Both ship in the same desktop app.

**Compute** sells your machine's time. **Data** sells access to packaged, permissioned context — datasets you've built, conversations you own, artifacts your work has produced. The buyer requests, you confirm, payment is made, access is granted, and you keep the right to revoke.

That last part matters. Every grant is paired with a `RevocationReceipt`. If a buyer misuses what they bought, you turn it off. The kernel keeps the audit trail.

## Why a Data Market at all

The five-market story only works if data can be priced and settled like compute can. Without it, agents silently scrape, and every layer above — labor, liquidity, risk — is built on un-permissioned context.

The Data Market is how OpenAgents drags the data layer into the same _verifiable outcomes under uncertainty_ primitive that the rest of the kernel runs on. It's the difference between scraping and selling.

## What ships today

The Data Market is published live on **two public Nostr relays** — `wss://relay.damus.io` and `wss://relay.primal.net`. Independent infrastructure. No OpenAgents-controlled relay in the loop. The full buyer-to-seller flow has been verified end-to-end, on the open network, against relays we don't run.

Inside the desktop app:

- A **seller** lane for drafting an asset, previewing exactly what will be shared, confirming, publishing, granting access, and revoking.
- A **market** lane that shows the live snapshot of what's been published.
- A **buyer** lane that picks an asset, sees the bundle being purchased, and publishes a targeted request.

Every lane is also accessible from the shell via `autopilotctl data-market`, and via a no-window headless runtime for daemons, agents, and skill flows. Same state machine, same acceptance semantics, no shadow truth between surfaces.

## Two ways to earn from one operator

Here's the architectural payoff: a single Pylon operator can turn on _both_ revenue surfaces.

The same machine that earns 25 sats per accepted CS336 contribution can also list a Data Market handler — a packaged dataset, a research artifact, a project context bundle — and earn from buyers requesting that data. Two streams, one identity, one wallet.

That's why Compute and Data went first: they're the two supply-side lanes a single operator can turn on, in any order, without rearchitecting anything above them. Labor (market #3) consumes both. Liquidity (#4) and Risk (#5) follow.

## Honest scope

The Data Market is **not** a catalog discovery experience yet. It's a narrow, targeted-request lane that proves the kernel primitives work under live relay conditions. From [`docs/MVP.md`](https://github.com/OpenAgentsInc/openagents/blob/main/docs/MVP.md):

> _"We are intentionally not shipping… broad public Data Market discovery, catalog search, or rich buyer procurement UX beyond the current narrow targeted-request flow… a broad end-user finetuning product, raw chat-log upload flow, or multi-family finetuning platform claim."_

What ships today is the rails — request, payment, delivery, revocation, signed receipts on every step — verified on independent infrastructure. The catalog UX, multi-family finetuning, and broad discovery are explicit post-MVP lanes.

## How it compounds compute

If Compute is the first revenue surface a Pylon operator turns on, Data is the second. A machine that already earns Bitcoin for training work can next sell packaged local data — stored conversations, curated artifacts, project context — at a price the kernel settles, under grants the operator can revoke.

That compounding is why this is one company instead of five. Every market makes every other market more valuable to the same operator.

---

{% hint style="info" %}
**Under the hood.** Developers building handlers should go to the [Developer Path → Build a Data Market handler](../developers/data-market-handler.md). It walks through the full NIP-90 data-vending profile (request kind `5960`, result kind `6960`, handler advertisement kind `31990`), the live relay set, the kernel object model (`DataAsset`, `AccessGrant`, `DeliveryBundle`, `RevocationReceipt`), and a code skeleton for publishing your own handler. The implementation spec is [`packages/data-market-mvp/README.md`](https://github.com/OpenAgentsInc/openagents/blob/main/packages/data-market-mvp/README.md). Note: kind `31990` is NIP-89-shaped today; full NIP-89 conformance across `crates/nostr/core` is in progress per the [2026-02-27 gap analysis](https://github.com/OpenAgentsInc/openagents/blob/main/docs/audits/2026-02-27-nostr-full-vision-nip-gap-analysis.md).
{% endhint %}

---

**← Previous:** [05. Pylon, the Provider](05-pylon-provider.md) · **Next:** [07. Economy Kernel](07-economy-kernel.md) **→**
