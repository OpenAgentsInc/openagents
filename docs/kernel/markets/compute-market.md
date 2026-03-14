# Compute Market

This is the canonical status doc for the `Compute Market`.

Use this file for the definitive answer to:

- what the compute market is,
- what is implemented now,
- what is only partially landed or local-prototype quality,
- and what is still planned.

## Purpose

The Compute Market allocates machine capacity as a receipted economic object.

Kernel-facing objects:

- `ComputeProduct`
- `CapacityLot`
- `CapacityInstrument`
- `StructuredCapacityInstrument`
- `DeliveryProof`
- `ComputeIndex`

It is the deepest current market because the MVP earn loop is compute-provider
first.

## Current repo verdict

| Dimension | Status | Notes |
| --- | --- | --- |
| Product surface | `implemented` | Autopilot ships a real compute-provider earn loop today. |
| Kernel authority | `implemented` | `openagents-kernel-core` and `apps/nexus-control` manage the broadest starter market slice here. |
| Wire/proto | `implemented`, thin | `proto/openagents/compute/v1/*` and generated Rust types exist, but the broader commodity-market package surface is still incomplete. |
| Local prototype | `implemented` | richer commodity semantics, broader compute families, and some market framing still live in spec/docs beyond the active MVP lane |
| Planned | yes | fuller commodity instruments, broader product families, richer buyer/seller UX, and broader compute-market operations remain planned |

## Implemented now

### Productized in the desktop app

- the compute-provider earn loop in `apps/autopilot-desktop`
- provider go-online flow, NIP-90 work intake, execution, result publishing,
  wallet-confirmed payout, and withdraw flow
- compute-provider supply is the only visibly productized market lane today

### Implemented in kernel authority

- `ComputeProduct` creation and listing
- `CapacityLot` creation, listing, lookup, and cancel
- `CapacityInstrument` creation, listing, lookup, close, and cash settlement
- `StructuredCapacityInstrument` creation, listing, lookup, and close
- `DeliveryProof` recording, listing, and lookup
- `ComputeIndex` publish, list, lookup, and correction

### Implemented in compute-adjacent kernel extensions

- environment package registry
- evaluation-run lifecycle
- synthetic-data lifecycle
- benchmark-adapter import into canonical eval runs

These are already documented in:

- [../compute-environment-packages.md](../compute-environment-packages.md)
- [../compute-evaluation-runs.md](../compute-evaluation-runs.md)
- [../compute-synthetic-data.md](../compute-synthetic-data.md)
- [../compute-benchmark-adapters.md](../compute-benchmark-adapters.md)

### Implemented wire and authority surfaces

- checked-in proto under `proto/openagents/compute/v1/*`
- generated Rust contracts in `crates/openagents-kernel-proto`
- authenticated HTTP routes in `apps/nexus-control`

## Local prototype or partial only

- compute is framed as the umbrella market, but the retained live
  implementation is still inference-led today
- embeddings are in launch positioning and product IDs, but not yet as complete
  or visibly productized as the inference-led lane
- richer compute-market receipts, snapshots, and commodity-market semantics
  still extend beyond the thin authoritative surface
- broader compute-market lifecycle ideas in `docs/kernel/economy-kernel.md`
  Section 8 remain more detailed than the live product path

## Not implemented yet

- a full user-facing spot, forward, futures, options, and hedging market
- exchange-like books or deterministic auction productization as a live user
  surface
- generalized buyer-facing compute procurement UX beyond the current retained
  slices
- broader compute family productization such as training, evaluation as a
  first-class sold family, adapter hosting, and cluster families as live market
  products
- full index governance, external index integration, and broader market-depth
  operations
- fully symmetric wire coverage for all compute-market extensions described in
  the normative kernel spec

## Current repo truth lives in

- `apps/autopilot-desktop`
- `apps/nexus-control/src/lib.rs`
- `apps/nexus-control/src/kernel.rs`
- `crates/openagents-kernel-core/src/compute.rs`
- `crates/openagents-kernel-core/src/compute_contracts.rs`
- `crates/openagents-kernel-core/src/authority.rs`
- `proto/openagents/compute/v1/*`
- `crates/openagents-kernel-proto`
- [../economy-kernel.md](../economy-kernel.md)
- [../economy-kernel-proto.md](../economy-kernel-proto.md)

## Boundary notes

- compute sells declared machine capacity and machine-execution contracts
- if the request is open-ended agent work rather than bounded execution, it
  belongs in the Labor Market
- if the request is mainly about permissioned access to context, it belongs in
  the Data Market
- if the question is routing money or reserves, it belongs in the Liquidity
  Market
- if the question is pricing uncertainty or liability, it belongs in the Risk
  Market
