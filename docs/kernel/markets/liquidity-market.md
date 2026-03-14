# Liquidity Market

This is the canonical status doc for the `Liquidity Market`.

## Purpose

The Liquidity Market moves value through the system.

Kernel-facing objects:

- `Quote`
- `RoutePlan`
- `Envelope`
- `SettlementIntent`
- `ReservePartition`

This market covers how value is quoted, routed, bounded, reserved, and settled
across participants and rails.

## Current repo verdict

| Dimension | Status | Notes |
| --- | --- | --- |
| Product surface | not productized | there is no dedicated liquidity-market lane in the desktop app today |
| Kernel authority | `implemented` starter slice | quotes, routes, envelopes, settlements, and reserve partitions are live in kernel authority |
| Wire/proto | not yet dedicated | there is no checked-in liquidity proto package today |
| Local prototype | `implemented` | richer routing, solver economics, and adjacent wallet or credit concepts still live mostly in docs and desktop-local state |
| Planned | yes | broader routing, FX, liquidity discovery, and solver UX remain planned |

## Implemented now

- create a liquidity `Quote`
- select a `RoutePlan`
- issue an `Envelope`
- execute a `SettlementIntent`
- register a `ReservePartition`
- adjust an existing reserve partition

These flows are implemented in:

- `crates/openagents-kernel-core/src/liquidity.rs`
- `crates/openagents-kernel-core/src/authority.rs`
- `apps/nexus-control/src/lib.rs`
- `apps/nexus-control/src/kernel.rs`

Authenticated HTTP mutation routes are live under:

- `POST /v1/kernel/liquidity/quotes`
- `POST /v1/kernel/liquidity/routes`
- `POST /v1/kernel/liquidity/envelopes`
- `POST /v1/kernel/liquidity/settlements`
- `POST /v1/kernel/liquidity/reserve_partitions`
- `POST /v1/kernel/liquidity/reserve_partitions/{partition_id}/adjust`

## Local prototype or partial only

- richer Hydra routing and solver-market semantics
- desktop-adjacent wallet, swap, and credit concepts that are not yet one
  generalized liquidity-market product
- broader operator policy and FX logic beyond the starter authority objects

## Not implemented yet

- a dedicated user-facing liquidity market in Autopilot
- generalized solver discovery and solver competition UX
- richer FX, routing, and reserve management product surfaces
- a dedicated checked-in liquidity proto package
- fuller read-model or discovery APIs for quotes, routes, and liquidity supply

## Current repo truth lives in

- `crates/openagents-kernel-core/src/liquidity.rs`
- `crates/openagents-kernel-core/src/authority.rs`
- `apps/nexus-control/src/lib.rs`
- `apps/nexus-control/src/kernel.rs`
- [../economy-kernel.md](../economy-kernel.md)
- [../economy-kernel-proto.md](../economy-kernel-proto.md)

## Boundary notes

- liquidity moves value; it does not define the work being bought
- compute and labor can trigger settlement, but liquidity owns route, envelope,
  and reserve semantics
- risk may price a route or reserve posture, but that does not make it part of
  the risk market
