# OpenAgents Kernel Proto

`openagents-kernel-proto` contains generated Rust types for the first thin kernel authority slice.

Current scope:

- `openagents.common.v1`
- `openagents.economy.v1`
- `openagents.compute.v1`
- `openagents.labor.v1`

The compute package tree now includes explicit wire contracts for:

- compute requirements
- compute products
- capacity lots
- capacity instruments
- delivery proofs
- compute indices

This crate remains intentionally scoped to the currently active kernel slices, but the compute layer is no longer only a thin requirement stub. It now covers the compute-market object model and the corresponding mutation/read-model contracts used by the service and reusable client.

Core generated coverage includes:

- `Receipt`
- `ReceiptHints`
- `WorkUnit`
- `Contract`
- `Submission`
- `Verdict`
- `EconomySnapshot`
- `ComputeRequirement`
- `ComputeProduct`
- `CapacityLot`
- `CapacityInstrument`
- `DeliveryProof`
- `ComputeIndex`

Deferred on purpose:

- `Data`
- `Liquidity`
- `Risk`

Those markets are already part of the architecture and docs, but they are not blocked on the first proto slice landing.
