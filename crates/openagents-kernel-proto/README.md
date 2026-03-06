# OpenAgents Kernel Proto

`openagents-kernel-proto` contains generated Rust types for the first thin kernel authority slice.

Current scope:

- `openagents.common.v1`
- `openagents.economy.v1`
- `openagents.compute.v1`
- `openagents.labor.v1`

This crate is intentionally thin. It covers the minimum wire surface needed to start backending the compute-provider earn flow:

- `Receipt`
- `ReceiptHints`
- `WorkUnit`
- `Contract`
- `Submission`
- `Verdict`
- `EconomySnapshot`
- minimal compute requirements

Deferred on purpose:

- `Data`
- `Liquidity`
- `Risk`

Those markets are already part of the architecture and docs, but they are not blocked on the first proto slice landing.
