# Liquidity Market

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

- `implemented`: none as a standalone market
- `local prototype`: wallet and routing primitives exist alongside Hydra-oriented protocol notes
- `planned`: quote, route selection, envelope issuance, reserve partitioning, and authoritative liquidity settlement
