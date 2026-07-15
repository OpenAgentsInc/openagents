---
title: Bitcoin and Lightning
description: Historical Bitcoin, Lightning, L402, wallet, tipping, payout, and settlement experiments.
lastModified: 2026-07-15
sidebar:
  order: 4
---

**Status:** Historical and removed from the accepted product. Bitcoin and Lightning payments are not a live feature of the current MVP.

OpenAgents explored Bitcoin as an open settlement layer for agent work. The work included balance displays, Lightning and L402 payments, paid chat, revenue share, wallet ownership, tipping, provider earnings, and payouts for verified contributions.

## What the experiments taught

- A payment request, payment proof, accepted work receipt, and product entitlement are different records.
- Wallet custody and signing authority must be explicit and recoverable.
- Idempotency, replay protection, reconciliation, and failure handling matter before any money path can be called live.
- A demo, invoice, balance display, or historical receipt does not prove a currently supported rail.

## What was removed

Payments, billing credits, wallets, tips, payouts, markets, and settlement sit outside the accepted Desktop MVP. Their executable paths were decommissioned during the toolchain conversion after outstanding value and historical receipts were reconciled. Former records remain read-only evidence; they are not an invitation to send funds.

Any revival would need fresh owner approval plus custody, ledger, threat-model, invariant, recovery, and live-proof work. This page provides no wallet address, payment endpoint, or operational instructions.

## Historical source trail

The public series traces the idea through [L402](https://github.com/OpenAgentsInc/openagents/blob/main/docs/transcripts/062.md), [Lightning withdrawals](https://github.com/OpenAgentsInc/openagents/blob/main/docs/transcripts/064.md), [agent payments](https://github.com/OpenAgentsInc/openagents/blob/main/docs/transcripts/169.md), [wallet ownership](https://github.com/OpenAgentsInc/openagents/blob/main/docs/transcripts/207.md), and later [agent market](https://github.com/OpenAgentsInc/openagents/blob/main/docs/transcripts/213.md) and [tipping](https://github.com/OpenAgentsInc/openagents/blob/main/docs/transcripts/235.md) demonstrations. They describe past work, not current availability.
