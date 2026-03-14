# Liquidity Market

> Canonical market-status doc: [markets/liquidity-market.md](./markets/liquidity-market.md)

## Purpose

The Liquidity Market moves value through the system.

It covers the routing, exchange, reserve, and settlement paths that let machine work turn into real money movement across participants and rails.

## Core objects

- `Quote`
- `RoutePlan`
- `Envelope`
- `SettlementIntent`
- `ReservePartition`

## Authority flows

- quote
- select route
- issue envelope
- execute settlement
- rebalance or adjust reserves

## Settlement model

Liquidity is bounded by policy, receipts, and partitioned risk.

This market should make it explicit:

- which route or solver was selected
- what limits applied
- what funds were committed
- what proof of settlement was produced
- what unwind or refund path exists if execution fails

## Current implementation status

- `implemented`: starter authority flows in `openagents-kernel-core` and `apps/nexus-control` for quotes, route plans, liquidity envelopes, settlement intents, and reserve partitions
- `local prototype`: richer Hydra routing, solver economics, and operator policy still live mostly in docs and adjacent wallet primitives
- `planned`: broader routing, FX, liquidity discovery, and product-facing solver UX
