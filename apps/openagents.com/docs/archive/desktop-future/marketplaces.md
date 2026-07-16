---
title: Marketplaces
description: The historical OpenAgents thesis for agent, plugin, data, compute, and outcome markets.
lastModified: 2026-07-15
sidebar:
  order: 2
---

**Status:** Historical and removed from the accepted product. Marketplaces are not a live feature or an active roadmap lane.

The broad OpenAgents thesis imagined a market where agents and people could publish capabilities, contribute data or compute, complete bounded work, and receive verifiable outcomes. Different eras described agent stores, WASM plugin registries, data vending machines, compute providers, developer bounties, and markets for accepted work.

## What carried across the experiments

- Capabilities should be inspectable before an agent receives authority to use them.
- Work should have a bounded request, durable outcome, and evidence trail.
- Identity, reputation, admission, verification, and settlement are separate authorities.
- A listing or demo does not prove supply, demand, delivery, or payment.

The most durable concept was the **accepted outcome**: scoped work that is executed, evaluated, receipted, and only then eligible for any later economic action. That idea still informs evidence design, but there is no current market or settlement surface behind it.

## What was removed

The current Desktop MVP does not include agent, plugin, data, compute, labor, or bounty markets. The Node/pnpm/Vite Plus cutover explicitly decommissioned market, payment, wallet, payout, and settlement paths rather than porting them.

Historical code and receipts remain evidence only. They do not authorize a listing, purchase, payout, or product claim.

## Historical source trail

The public transcript index groups the arc around the [agent store](https://github.com/OpenAgentsInc/openagents/blob/main/docs/transcripts/092.md), [One Market](https://github.com/OpenAgentsInc/openagents/blob/main/docs/transcripts/141.md), [data marketplace planning](https://github.com/OpenAgentsInc/openagents/blob/main/docs/transcripts/147.md), [agent markets](https://github.com/OpenAgentsInc/openagents/blob/main/docs/transcripts/213.md), and later [compute](https://github.com/OpenAgentsInc/openagents/blob/main/docs/transcripts/214.md) and [data](https://github.com/OpenAgentsInc/openagents/blob/main/docs/transcripts/215.md) market experiments.
